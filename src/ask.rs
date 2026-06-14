//! RAG "Ask" mode — answer a natural-language question over the indexed notes.
//!
//! This is the one feature that is *not* on an instant query path: it embeds the
//! question, retrieves the most semantically similar notes from the stored
//! vectors, and asks the local model to answer from those excerpts. Like
//! `bridge`, it is an explicit, user-invoked generative action — the INV-1
//! carve-out (D-015), not the daily-driver `search` / `near` / `show` paths.
//! Compiled only under the `enrich` feature. Returns `Err` on any failure (no
//! embeddings, server down, malformed reply) so the caller degrades gracefully
//! (INV-4).

use anyhow::{Context, Result};

use crate::store::Store;
use crate::{embed, enrich, query};

/// A note that fed an answer (for citation / click-through).
pub struct Source {
    pub id: String,
    pub title: String,
    pub similarity: f32,
}

/// The result of an [`ask`]: the model's answer plus the notes it was grounded in.
pub struct Answer {
    pub text: String,
    pub sources: Vec<Source>,
}

const ASK_SYSTEM: &str = "You answer questions about a personal collection of \
    project-idea notes, using ONLY the excerpts provided. Cite the notes you draw \
    on by their title in square brackets, e.g. [Spatial Canvas]. If the excerpts \
    do not contain the answer, say so plainly rather than inventing one. Be \
    concise and concrete.";

/// Answer `question` over the indexed notes (retrieval-augmented generation):
/// embed the question, take the `k` most cosine-similar notes from the stored
/// vectors, and have the local model answer from those excerpts with citations.
/// **User-invoked / on-demand** — the INV-1 carve-out (D-015). `Err` on any
/// failure so the caller can report it without crashing.
pub fn ask(store: &Store, question: &str, k: usize) -> Result<Answer> {
    let qvec = embed::embed(question).context("failed to embed the question")?;
    let embeddings = store.all_embeddings()?;
    let ranked = rank(&embeddings, &qvec, k);
    if ranked.is_empty() {
        anyhow::bail!("no embedded notes to search — run `index --embed` first");
    }

    // Hydrate the ranked ids into sources (title) and context blocks (body).
    let mut sources = Vec::new();
    let mut blocks = Vec::new();
    for (id, sim) in ranked {
        let Some(idea) = query::get(store, &id)? else {
            continue;
        };
        blocks.push(format!("## {}\n{}", idea.title, truncate(&idea.body, 1500)));
        sources.push(Source { id, title: idea.title, similarity: sim });
    }

    let user = format!(
        "Notes:\n\n{}\n\nQuestion: {question}\n\nAnswer using only the notes above; cite titles in [brackets].",
        blocks.join("\n\n")
    );
    let text = enrich::chat(ASK_SYSTEM, &user, None, 600)?;
    Ok(Answer { text: text.trim().to_string(), sources })
}

/// Rank stored embeddings by cosine similarity to the query vector, most similar
/// first, keeping the top `k` (default 5). Pure, so it's testable offline.
fn rank(embeddings: &[(String, Vec<f32>)], qvec: &[f32], k: usize) -> Vec<(String, f32)> {
    let mut scored: Vec<(String, f32)> = embeddings
        .iter()
        .map(|(id, v)| (id.clone(), query::cosine(qvec, v)))
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(if k == 0 { 5 } else { k });
    scored
}

/// Truncate to at most `max` bytes on a char boundary.
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_orders_by_similarity_and_caps_k() {
        let e = vec![
            ("a".to_string(), vec![1.0, 0.0]),
            ("b".to_string(), vec![0.0, 1.0]),
            ("c".to_string(), vec![0.9, 0.1]),
        ];
        let q = vec![1.0, 0.0];
        let r = rank(&e, &q, 2);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].0, "a"); // identical direction ranks first
        assert_eq!(r[1].0, "c"); // next closest, beating the orthogonal "b"
    }

    #[test]
    fn rank_empty_input_is_empty() {
        assert!(rank(&[], &[1.0, 0.0], 5).is_empty());
    }

    #[test]
    fn truncate_is_char_boundary_safe() {
        let s = "é".repeat(1000); // 2000 bytes, multibyte
        let t = truncate(&s, 1501);
        assert!(s.is_char_boundary(t.len())); // didn't split a char
        assert!(t.len() <= 1501);
    }
}

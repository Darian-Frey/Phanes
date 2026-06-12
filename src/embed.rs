//! Embeddings via a local OpenAI-compatible server (the `/v1/embeddings`
//! endpoint — LM Studio, Ollama, llama.cpp `--api`). Compiled only under the
//! `enrich` feature.
//!
//! Used at index time to compute one vector per note for semantic "near this"
//! (F-012). The similarity itself is plain cosine math in [`crate::query::near`],
//! so no model ever runs on a query (INV-1). A failed embed is non-fatal — the
//! note is simply left without a vector (INV-4).
//!
//! Config: `PHANES_EMBED_URL` (default `http://127.0.0.1:1234/v1/embeddings`),
//! `PHANES_EMBED_MODEL` (default `text-embedding-nomic-embed-text-v1.5`).

use anyhow::{Context, Result};
use serde_json::json;

const DEFAULT_URL: &str = "http://127.0.0.1:1234/v1/embeddings";
const DEFAULT_MODEL: &str = "text-embedding-nomic-embed-text-v1.5";

fn endpoint() -> String {
    std::env::var("PHANES_EMBED_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn model_id() -> String {
    std::env::var("PHANES_EMBED_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
}

/// Embed one note's text into a vector. Returns `Err` on any transport or parse
/// failure so the caller keeps the note vector-less rather than failing the pass.
pub fn embed(text: &str) -> Result<Vec<f32>> {
    let payload = json!({ "model": model_id(), "input": truncate(text, 8000) });
    // Shares the retry-on-cold-load POST helper with the chat client (IMP-001).
    let raw = crate::enrich::post_json(&endpoint(), &payload)?;
    parse_embedding(&raw)
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

/// Pull the float vector out of an OpenAI embeddings response
/// (`data[0].embedding`). Split out so it can be tested without a live server.
fn parse_embedding(raw: &serde_json::Value) -> Result<Vec<f32>> {
    let arr = raw
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("embedding"))
        .and_then(|e| e.as_array())
        .context("no data[0].embedding in the embedding server response")?;
    let vector: Vec<f32> = arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
    if vector.is_empty() {
        anyhow::bail!("embedding vector was empty");
    }
    Ok(vector)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_embedding_response() {
        let raw = json!({ "data": [ { "embedding": [0.1, 0.2, -0.3] } ] });
        assert_eq!(parse_embedding(&raw).unwrap(), vec![0.1_f32, 0.2, -0.3]);
    }

    #[test]
    fn rejects_missing_or_empty_embedding() {
        assert!(parse_embedding(&json!({ "data": [] })).is_err());
        assert!(parse_embedding(&json!({ "data": [ { "embedding": [] } ] })).is_err());
    }

    #[test]
    fn truncate_keeps_char_boundaries() {
        let s = "é".repeat(5000); // 10k bytes
        let t = truncate(&s, 8000);
        assert!(s.is_char_boundary(t.len()));
    }
}

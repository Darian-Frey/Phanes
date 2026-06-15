//! Index-time orchestration.
//!
//! # Load-bearing rule (do not erode)
//! The enrichment model runs *here and only here*, and only when a file is new
//! or its content hash has changed. Everything downstream (search, related,
//! stale) reads cached, resolved records. No query ever waits on a model.

use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::model::{Idea, Provenance, Sourced, Status};
use crate::parser;
use crate::store::Store;

/// Options for a single index pass.
pub struct IndexOptions {
    /// Run the enrichment model on changed files. Requires the `enrich`
    /// feature *and* a reachable model server. Off by default.
    pub enrich: bool,
    /// Compute and store an embedding vector for changed files (semantic
    /// "near this", F-012). Requires the `enrich` feature + a server. Off by
    /// default. Independent of `enrich` — either, both, or neither.
    pub embed: bool,
    /// Re-process every file regardless of hash (e.g. after a prompt change).
    pub force: bool,
}

#[derive(Debug, Default)]
pub struct IndexReport {
    pub scanned: usize,
    pub changed: usize,
    pub enriched: usize,
    pub embedded: usize,
    pub skipped: usize,
    pub pruned: usize,
}

/// Walk `root`, (re)indexing every `.md` file. Unchanged files are skipped
/// cheaply on a hash match — this is what keeps a daily `phanes index` fast
/// and what stops the model re-running on a corpus that hasn't moved.
pub fn run(store: &mut Store, root: &Path, opts: &IndexOptions) -> Result<IndexReport> {
    let mut report = IndexReport::default();
    let mut seen = Vec::new();

    // The established tag vocabulary, fed to the model so proposed tags reuse it
    // rather than inventing synonyms (taxonomy-aware tags). Snapshotted once;
    // new tags converge over runs (and a `--force` pass re-enriches with the
    // full vocabulary). Empty on a never-indexed corpus.
    #[cfg(feature = "enrich")]
    let vocabulary: Vec<String> = if opts.enrich {
        crate::query::tag_vocabulary(store, 80).unwrap_or_default()
    } else {
        Vec::new()
    };

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
    {
        report.scanned += 1;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(path);
        let id = parser::id_from_path(rel);
        seen.push(id.clone());

        let raw = std::fs::read(path)?;
        let hash = parser::content_hash(&raw);

        // The cache gate. Unchanged + not forced => no parse, no model, no write.
        if !opts.force && store.hash_for_path(&path.to_string_lossy())?.as_deref() == Some(&hash) {
            report.skipped += 1;
            continue;
        }
        report.changed += 1;

        let text = String::from_utf8_lossy(&raw);
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let parsed = parser::parse(&stem, &text);

        let mtime: DateTime<Utc> = entry.metadata()?.modified()?.into();

        // Start from asserted facts only. `mut` is used only when enrichment is
        // compiled in (merge_proposed mutates it); silence the warning otherwise.
        #[cfg_attr(not(feature = "enrich"), allow(unused_mut))]
        let mut idea = Idea {
            id: id.clone(),
            path: path.to_path_buf(),
            title: parsed.title,
            status: parsed
                .status
                .map(Sourced::asserted)
                .unwrap_or_else(|| Sourced::asserted(Status::Unknown)),
            summary: None,
            tags: parsed.tags.into_iter().map(Sourced::asserted).collect(),
            topics: Vec::new(),
            last_reviewed: parsed.last_reviewed,
            mtime,
            content_hash: hash,
            body: parsed.body,
            // Resolve raw link targets to ids so they can join to ideas at query
            // time (a dangling target simply won't join).
            links: parsed
                .link_targets
                .iter()
                .map(|t| parser::link_target_to_id(t, rel))
                .collect(),
        };

        // Enrich a *changed* file before its single upsert. Proposed values
        // never clobber asserted ones — see merge rules below.
        #[cfg_attr(not(feature = "enrich"), allow(unused_mut))]
        let mut enriched = false;
        if opts.enrich {
            #[cfg(feature = "enrich")]
            match crate::enrich::enrich(&idea.title, &idea.body, &vocabulary) {
                Ok(e) => {
                    merge_proposed(&mut idea, e);
                    report.enriched += 1;
                    enriched = true;
                }
                // Graceful degradation: a missing/slow model must not fail the
                // index. We keep the asserted-only record.
                Err(err) => eprintln!("enrich skipped for {id}: {err}"),
            }
        }

        // A deterministic re-index (no enrichment this pass) must NOT destroy the
        // note's model-proposed data. A plain edit — changing the status, fixing a
        // typo — would otherwise wipe the proposed summary/tags/topics (and, via
        // the cleared embedding, disconnect the note from the graph). So carry the
        // existing proposed values forward; they stay until a `--enrich`/`--force`
        // pass refreshes them. The embedding is likewise preserved (no clear).
        if !enriched {
            preserve_proposed(store, &mut idea)?;
        }
        store.upsert(&idea)?;
    }

    // --- AI gap-fill passes ---
    // These run over EVERY current note, not just hash-changed ones, so a note
    // indexed earlier (e.g. by a plain scan, or whose vector was cascade-dropped
    // when it was last re-indexed) still gets its enrichment/embedding. Each
    // no-ops for notes that already have the layer, and degrades gracefully.
    #[cfg(feature = "enrich")]
    if opts.enrich {
        for id in &seen {
            if store.has_summary(id)? {
                continue;
            }
            let Some(mut idea) = crate::query::get(store, id)? else {
                continue;
            };
            match crate::enrich::enrich(&idea.title, &idea.body, &vocabulary) {
                Ok(e) => {
                    merge_proposed(&mut idea, e);
                    store.upsert(&idea)?;
                    report.enriched += 1;
                }
                Err(err) => eprintln!("enrich skipped for {id}: {err}"),
            }
        }
    }

    // Embedding is filled here, not in the loop: an upsert above cascade-drops a
    // note's vector, and gate-skipped notes may never have had one — so ensure
    // every current note ends up with an embedding.
    #[cfg(feature = "enrich")]
    if opts.embed {
        for id in &seen {
            if store.has_embedding(id)? {
                continue;
            }
            let Some(idea) = crate::query::get(store, id)? else {
                continue;
            };
            match crate::embed::embed(&format!("{}\n{}", idea.title, idea.body)) {
                Ok(vector) => {
                    store.set_embedding(id, &vector)?;
                    report.embedded += 1;
                }
                Err(err) => eprintln!("embed skipped for {id}: {err}"),
            }
        }
    }

    #[cfg(not(feature = "enrich"))]
    if opts.enrich || opts.embed {
        eprintln!("--enrich/--embed requested but the binary was built without the `enrich` feature");
    }

    report.pruned = store.prune_missing(&seen)?;
    Ok(report)
}

/// Carry a note's existing **model-proposed** data forward into a freshly-parsed
/// (asserted-only) `idea`, so a deterministic re-index doesn't destroy it. Fills
/// the proposed summary, re-adds proposed tags the author hasn't asserted, and
/// keeps the topics — all gap-fill only, so asserted facts still win (INV-2). The
/// embedding is preserved separately (the indexer no longer clears it). No-op for
/// a never-indexed note. Deterministic — no model.
fn preserve_proposed(store: &Store, idea: &mut Idea) -> Result<()> {
    let Some(existing) = crate::query::get(store, &idea.id)? else {
        return Ok(());
    };
    if idea.summary.is_none() {
        if let Some(s) = existing.summary {
            if s.source == Provenance::Proposed {
                idea.summary = Some(s);
            }
        }
    }
    for t in existing.tags {
        if t.source == Provenance::Proposed && !idea.tags.iter().any(|x| x.value == t.value) {
            idea.tags.push(t);
        }
    }
    if idea.topics.is_empty() {
        idea.topics = existing.topics;
    }
    Ok(())
}

/// Merge model output into an idea. Asserted always wins.
#[cfg(feature = "enrich")]
fn merge_proposed(idea: &mut Idea, e: crate::model::Enrichment) {
    if idea.summary.is_none() {
        idea.summary = Some(Sourced::proposed(e.summary));
    }
    if idea.status.source == Provenance::Asserted && matches!(idea.status.value, Status::Unknown) {
        idea.status = Sourced::proposed(e.status);
    }
    // Add proposed tags the author didn't already assert.
    for t in e.tags {
        if !idea.tags.iter().any(|existing| existing.value == t) {
            idea.tags.push(Sourced::proposed(t));
        }
    }
    idea.topics = e.topics;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Store, SCHEMA};
    use chrono::Utc;
    use rusqlite::Connection;
    use std::path::PathBuf;

    fn mem_store() -> Store {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        Store { conn }
    }

    fn note(id: &str, status: Status, summary: Option<&str>, tags: Vec<Sourced<String>>, topics: &[&str], body: &str) -> Idea {
        Idea {
            id: id.into(),
            path: PathBuf::from(format!("/{id}.md")),
            title: id.into(),
            status: Sourced::asserted(status),
            summary: summary.map(|s| Sourced::proposed(s.into())),
            tags,
            topics: topics.iter().map(|s| s.to_string()).collect(),
            last_reviewed: None,
            mtime: Utc::now(),
            content_hash: format!("h-{body}"),
            body: body.into(),
            links: Vec::new(),
        }
    }

    /// A deterministic re-index of an edited note (e.g. a status change) must keep
    /// its model-proposed summary/tags/topics — the BUG-003 fix.
    #[test]
    fn preserve_proposed_keeps_model_data_across_a_deterministic_edit() {
        let mut store = mem_store();
        // Enriched record: proposed summary, an asserted + a proposed tag, topics.
        let existing = note(
            "n",
            Status::Active,
            Some("auto summary"),
            vec![Sourced::asserted("ui".into()), Sourced::proposed("ml".into())],
            &["viz"],
            "old body",
        );
        store.upsert(&existing).unwrap();

        // A freshly-parsed, asserted-only idea, as a plain re-index builds — with a
        // changed status and no model data.
        let mut fresh = note("n", Status::Draft, None, vec![Sourced::asserted("ui".into())], &[], "new body");
        preserve_proposed(&store, &mut fresh).unwrap();

        assert_eq!(fresh.status.value, Status::Draft); // the asserted edit is kept
        assert_eq!(fresh.summary.as_ref().unwrap().value, "auto summary"); // proposed preserved
        assert!(fresh.tags.iter().any(|t| t.value == "ml" && t.source == Provenance::Proposed));
        assert!(fresh.tags.iter().any(|t| t.value == "ui" && t.source == Provenance::Asserted));
        assert_eq!(fresh.topics, vec!["viz"]);
    }
}

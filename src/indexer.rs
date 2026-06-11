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

use crate::model::{Idea, Sourced, Status};
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

        // Optional enrichment fills *gaps only*. Proposed values never clobber
        // asserted ones — see merge rules below.
        if opts.enrich {
            #[cfg(feature = "enrich")]
            {
                match crate::enrich::enrich(&idea.title, &idea.body) {
                    Ok(e) => {
                        merge_proposed(&mut idea, e);
                        report.enriched += 1;
                    }
                    // Graceful degradation: a missing/slow model must not fail
                    // the index. We keep the asserted-only record.
                    Err(err) => eprintln!("enrich skipped for {id}: {err}"),
                }
            }
            #[cfg(not(feature = "enrich"))]
            eprintln!("--enrich requested but binary built without the `enrich` feature");
        }

        store.upsert(&idea)?;

        // Embedding runs after upsert: the INSERT OR REPLACE on `ideas` cascades
        // away any stale vector, so we write the fresh one here. Index-time only.
        if opts.embed {
            #[cfg(feature = "enrich")]
            {
                let text = format!("{}\n{}", idea.title, idea.body);
                match crate::embed::embed(&text) {
                    Ok(vector) => {
                        store.set_embedding(&idea.id, &vector)?;
                        report.embedded += 1;
                    }
                    // Graceful degradation: a failed embed leaves the note without
                    // a vector; it just won't appear in `near` until re-embedded.
                    Err(err) => eprintln!("embed skipped for {}: {err}", idea.id),
                }
            }
            #[cfg(not(feature = "enrich"))]
            eprintln!("--embed requested but binary built without the `enrich` feature");
        }
    }

    report.pruned = store.prune_missing(&seen)?;
    Ok(report)
}

/// Merge model output into an idea. Asserted always wins.
#[cfg(feature = "enrich")]
fn merge_proposed(idea: &mut Idea, e: crate::model::Enrichment) {
    use crate::model::Provenance;

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

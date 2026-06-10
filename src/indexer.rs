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
    /// feature *and* a reachable llama-server. Off by default.
    pub enrich: bool,
    /// Re-enrich every file regardless of hash (e.g. after a prompt change).
    pub force: bool,
}

#[derive(Debug, Default)]
pub struct IndexReport {
    pub scanned: usize,
    pub changed: usize,
    pub enriched: usize,
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

        // Start from asserted facts only.
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
            links: parsed.link_targets,
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

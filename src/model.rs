//! Core data types for Phanes.
//!
//! # Load-bearing rule (do not erode)
//! Every field that *can* originate from the enrichment model carries its
//! [`Provenance`]. Deterministic facts read straight from the file
//! (title, explicit links, dates) are [`Provenance::Asserted`]. Anything the
//! model inferred is [`Provenance::Proposed`] and is advisory only — it must
//! never silently overwrite an asserted value. This is the same
//! proposed-vs-canonical boundary used in story_mindmap, expressed in the type
//! system so the implementation can't drift away from it.

use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Where a field's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    /// Read directly from the file (frontmatter, body) or the filesystem.
    Asserted,
    /// Inferred by the enrichment model. Advisory.
    Proposed,
}

impl Provenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Provenance::Asserted => "asserted",
            Provenance::Proposed => "proposed",
        }
    }

    /// Parse a `*_source` column value. Anything other than `proposed` is treated
    /// as asserted, since asserted is the authoritative default.
    pub fn from_db(s: &str) -> Self {
        match s {
            "proposed" => Provenance::Proposed,
            _ => Provenance::Asserted,
        }
    }
}

/// A value paired with the source that produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sourced<T> {
    pub value: T,
    pub source: Provenance,
}

impl<T> Sourced<T> {
    pub fn asserted(value: T) -> Self {
        Self { value, source: Provenance::Asserted }
    }
    pub fn proposed(value: T) -> Self {
        Self { value, source: Provenance::Proposed }
    }
}

/// Project lifecycle status. Mirrors the project-scaffold status vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Early-stage idea, not yet committed to. The corpus's most common status.
    Concept,
    /// Being written up but not yet active work.
    Draft,
    Active,
    Dormant,
    Complete,
    Archived,
    Superseded,
    /// No status found in the file and none inferred.
    Unknown,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Concept => "concept",
            Status::Draft => "draft",
            Status::Active => "active",
            Status::Dormant => "dormant",
            Status::Complete => "complete",
            Status::Archived => "archived",
            Status::Superseded => "superseded",
            Status::Unknown => "unknown",
        }
    }
}

impl FromStr for Status {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "concept" => Status::Concept,
            "draft" => Status::Draft,
            "active" => Status::Active,
            "dormant" => Status::Dormant,
            "complete" => Status::Complete,
            "archived" => Status::Archived,
            "superseded" => Status::Superseded,
            _ => Status::Unknown,
        })
    }
}

/// A single idea file, fully resolved and ready to store.
#[derive(Debug, Clone)]
pub struct Idea {
    /// Stable slug derived from the path relative to the ideas root.
    pub id: String,
    pub path: PathBuf,
    /// Asserted: first H1 in the document, else the filename stem.
    pub title: String,
    pub status: Sourced<Status>,
    /// Usually proposed by the model; absent if enrichment didn't run.
    pub summary: Option<Sourced<String>>,
    /// A coarse, proposed classification of the note's kind (e.g. `developer-tool`,
    /// `research`, `creative`, `spec`); model output, absent without enrichment.
    pub category: Option<Sourced<String>>,
    pub tags: Vec<Sourced<String>>,
    /// Proposed concept labels. Kept distinct from asserted tags.
    pub topics: Vec<String>,
    /// Asserted from frontmatter when present.
    pub last_reviewed: Option<NaiveDate>,
    /// Filesystem modified time — the staleness fallback when no date is given.
    pub mtime: DateTime<Utc>,
    /// blake3 hex of the raw file bytes. The enrichment cache key.
    pub content_hash: String,
    /// Plain text body, fed to FTS.
    pub body: String,
    /// Explicit out-links, resolved to target ids.
    pub links: Vec<String>,
}

/// The exact JSON shape the enrichment model is constrained to return.
/// Must stay in lockstep with `grammars/idea_extract.gbnf`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enrichment {
    pub summary: String,
    pub status: Status,
    /// A coarse classification of the note's kind. `#[serde(default)]` so older
    /// replies without the field still parse (it's required by the json_schema).
    #[serde(default)]
    pub category: String,
    pub tags: Vec<String>,
    pub topics: Vec<String>,
}

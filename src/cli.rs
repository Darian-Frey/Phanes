//! Command-line surface.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "phanes", version, about = "Index, search, and relate project-idea notes.")]
pub struct Cli {
    /// Root folder holding the idea `.md` files. DB lives at <root>/.phanes/index.db.
    #[arg(long, global = true, default_value = ".")]
    pub root: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// (Re)build the index from the ideas folder.
    Index {
        /// Run the enrichment model on changed files (needs the `enrich` build + a model server).
        #[arg(long)]
        enrich: bool,
        /// Compute embedding vectors for changed files, for `near` (same build + server).
        #[arg(long)]
        embed: bool,
        /// Re-process every file regardless of hash.
        #[arg(long)]
        force: bool,
    },
    /// Full-text search with optional filters.
    Search {
        query: String,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        stale_days: Option<i64>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List ideas not reviewed in a while.
    Stale {
        #[arg(long, default_value_t = 180)]
        days: i64,
    },
    /// Show ideas related to one (explicit links + shared tags).
    Related { id_or_title: String },
    /// Show semantically similar ideas (needs a prior `index --embed`).
    Near { id_or_title: String },
    /// Surface structural gaps: orphan ideas and candidate bridges (needs `--embed`).
    Gaps,
    /// Propose an idea bridging two notes (needs the `enrich` build + a server).
    Bridge { a: String, b: String },
    /// Show one idea's metadata and relationships.
    Show { id_or_title: String },
    /// Create a new idea note with the frontmatter pre-filled.
    New {
        title: String,
        #[arg(long)]
        tag: Vec<String>,
    },
}

use anyhow::Result;
use clap::Parser;

use phanes::cli::{Cli, Command};
use phanes::indexer::{self, IndexOptions};
use phanes::query::{self, SearchFilter};
use phanes::store::Store;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.root.join(".phanes").join("index.db");
    let mut store = Store::open(&db_path)?;

    match cli.command {
        Command::Index { enrich, force } => {
            let report = indexer::run(&mut store, &cli.root, &IndexOptions { enrich, force })?;
            println!(
                "scanned {} · changed {} · enriched {} · skipped {} · pruned {}",
                report.scanned, report.changed, report.enriched, report.skipped, report.pruned
            );
        }
        Command::Search { query: q, status, tag, stale_days, limit } => {
            let filter = SearchFilter {
                status: status.as_deref().map(|s| s.parse().unwrap_or(phanes::model::Status::Unknown)),
                tag,
                stale_days,
                limit,
            };
            let hits = query::search(&store, &q, &filter)?;
            print_hits(&hits);
        }
        Command::Stale { days } => print_hits(&query::stale(&store, days)?),
        Command::Related { id_or_title } => print_hits(&query::related(&store, &id_or_title)?),
        Command::Show { id_or_title } => {
            // TODO(claude-code): resolve + render metadata, provenance flags, related.
            let _ = query::resolve(&store, &id_or_title)?;
            todo!("render a single idea")
        }
        Command::New { title, tag } => {
            // TODO(claude-code): write <root>/<slug>.md with frontmatter
            //   (title, status: active, tags, last_reviewed: today), then index it.
            let _ = (title, tag);
            todo!("scaffold a new idea note")
        }
    }
    Ok(())
}

fn print_hits(hits: &[query::Hit]) {
    // TODO(claude-code): swap for a tabled::Table with owo-colors status tints.
    for h in hits {
        println!("[{}] {} — {}", h.status.as_str(), h.id, h.title);
    }
}

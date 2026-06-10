use anyhow::Result;
use clap::Parser;
use owo_colors::{OwoColorize, Stream};
use tabled::builder::Builder;
use tabled::settings::Style;

use phanes::cli::{Cli, Command};
use phanes::indexer::{self, IndexOptions};
use phanes::model::{Idea, Provenance, Status};
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
        Command::Show { id_or_title } => match query::resolve(&store, &id_or_title)? {
            Some(id) => {
                let idea = query::get(&store, &id)?.expect("a resolved id always exists");
                let related = query::related(&store, &id)?;
                print_idea(&idea, &related);
            }
            None => println!("no idea matches '{id_or_title}'."),
        },
        Command::New { title, tag } => {
            // TODO(claude-code): write <root>/<slug>.md with frontmatter
            //   (title, status: active, tags, last_reviewed: today), then index it.
            let _ = (title, tag);
            todo!("scaffold a new idea note")
        }
    }
    Ok(())
}

/// Render hits as a bordered table with colour-tinted statuses. The "match"
/// column is only shown when at least one hit carries an FTS snippet (i.e. for
/// `search`, not `stale`).
fn print_hits(hits: &[query::Hit]) {
    if hits.is_empty() {
        println!("no matches.");
        return;
    }

    let show_snippet = hits.iter().any(|h| h.snippet.is_some());

    let mut builder = Builder::new();
    let mut header = vec!["status".to_string(), "id".to_string(), "title".to_string()];
    if show_snippet {
        header.push("match".to_string());
    }
    builder.push_record(header);

    for h in hits {
        let mut row = vec![tint_status(h.status), h.id.clone(), h.title.clone()];
        if show_snippet {
            // Snippets can span lines; keep each row on one line.
            row.push(h.snippet.clone().unwrap_or_default().replace('\n', " "));
        }
        builder.push_record(row);
    }

    let mut table = builder.build();
    table.with(Style::rounded());
    println!("{table}");
    println!("{} idea(s).", hits.len());
}

/// Status label tinted by lifecycle stage. Colour is emitted only when stdout is
/// a terminal (via `if_supports_color`), so piped/redirected output stays clean;
/// the `ansi` feature on `tabled` keeps the coloured cells aligned.
fn tint_status(status: Status) -> String {
    let label = status.as_str();
    match status {
        Status::Concept => label.if_supports_color(Stream::Stdout, |t| t.cyan()).to_string(),
        Status::Draft => label.if_supports_color(Stream::Stdout, |t| t.blue()).to_string(),
        Status::Active => label.if_supports_color(Stream::Stdout, |t| t.green()).to_string(),
        Status::Dormant => label.if_supports_color(Stream::Stdout, |t| t.yellow()).to_string(),
        Status::Complete => label.if_supports_color(Stream::Stdout, |t| t.bright_green()).to_string(),
        Status::Superseded => label.if_supports_color(Stream::Stdout, |t| t.magenta()).to_string(),
        Status::Archived | Status::Unknown => {
            label.if_supports_color(Stream::Stdout, |t| t.bright_black()).to_string()
        }
    }
}

/// Render one idea: metadata with provenance flags, then related ideas. This is
/// the CLI surface of INV-2 — asserted vs proposed is visible on every field
/// that can carry it.
fn print_idea(idea: &Idea, related: &[query::Hit]) {
    println!();
    println!(
        "{}",
        idea.title.if_supports_color(Stream::Stdout, |t| t.bold())
    );
    println!("  id:       {}", idea.id);
    println!("  path:     {}", idea.path.display());
    println!(
        "  status:   {} {}",
        tint_status(idea.status.value),
        prov_tag(idea.status.source)
    );
    if let Some(date) = idea.last_reviewed {
        println!("  reviewed: {date}");
    }
    println!("  modified: {}", idea.mtime.format("%Y-%m-%d"));

    if let Some(summary) = &idea.summary {
        println!("  summary:  {} {}", summary.value, prov_tag(summary.source));
    }
    if !idea.tags.is_empty() {
        let tags = idea
            .tags
            .iter()
            .map(|t| format!("{}{}", prov_mark(t.source), t.value))
            .collect::<Vec<_>>()
            .join(", ");
        println!("  tags:     {tags}");
    }
    if !idea.topics.is_empty() {
        println!("  topics:   {}", idea.topics.join(", "));
    }

    if related.is_empty() {
        println!("\n  (no related ideas)");
    } else {
        println!("\n  related:");
        for h in related {
            let how = h.snippet.as_deref().unwrap_or("");
            println!("    {}  {} — {} ({})", tint_status(h.status), h.id, h.title, how);
        }
    }
    println!();
}

/// A parenthetical provenance flag for a whole field (status, summary).
fn prov_tag(p: Provenance) -> String {
    match p {
        Provenance::Asserted => "(asserted)"
            .if_supports_color(Stream::Stdout, |t| t.dimmed())
            .to_string(),
        Provenance::Proposed => "(proposed)"
            .if_supports_color(Stream::Stdout, |t| t.yellow())
            .to_string(),
    }
}

/// A compact inline marker for list items (tags): proposed values get a `~`
/// prefix so they read as distinct from asserted ones.
fn prov_mark(p: Provenance) -> String {
    match p {
        Provenance::Asserted => String::new(),
        Provenance::Proposed => "~"
            .if_supports_color(Stream::Stdout, |t| t.yellow())
            .to_string(),
    }
}

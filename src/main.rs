use anyhow::Result;
use clap::Parser;
use owo_colors::{OwoColorize, Stream};
use tabled::builder::Builder;
use tabled::settings::Style;

use phanes::cli::{Cli, Command};
use phanes::indexer::{self, IndexOptions};
use phanes::model::{Idea, Provenance, Status};
use phanes::query::{self, SearchFilter};
use phanes::scaffold;
use phanes::store::Store;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.root.join(".phanes").join("index.db");
    let mut store = Store::open(&db_path)?;

    match cli.command {
        Command::Index { enrich, embed, force } => {
            let report = indexer::run(&mut store, &cli.root, &IndexOptions { enrich, embed, force })?;
            println!(
                "scanned {} · changed {} · enriched {} · embedded {} · skipped {} · pruned {}",
                report.scanned, report.changed, report.enriched, report.embedded, report.skipped, report.pruned
            );
        }
        Command::Search { query: q, status, tag, stale_days, limit, semantic } => {
            let filter = SearchFilter {
                status: status.as_deref().map(|s| s.parse().unwrap_or(phanes::model::Status::Unknown)),
                tag,
                stale_days,
                limit,
            };
            let hits = if semantic {
                query::hybrid(&store, &q, &filter)?
            } else {
                query::search(&store, &q, &filter)?
            };
            print_hits(&hits);
        }
        Command::Stale { days } => print_hits(&query::stale(&store, days)?),
        Command::Related { id_or_title } => print_hits(&query::related(&store, &id_or_title)?),
        Command::Near { id_or_title } => print_hits(&query::near(&store, &id_or_title, 10)?),
        Command::Gaps => {
            let g = phanes::graph::build(&store, &phanes::graph::GraphOptions::default())?;
            print_gaps(&g);
        }
        Command::Bridge { a, b } => {
            match (query::resolve(&store, &a)?, query::resolve(&store, &b)?) {
                (Some(ida), Some(idb)) => {
                    let na = query::get(&store, &ida)?.expect("resolved id exists");
                    let nb = query::get(&store, &idb)?.expect("resolved id exists");
                    propose_bridge_cli(&na, &nb);
                }
                _ => println!("could not resolve both notes (need two existing ids/titles)"),
            }
        }
        Command::Tags => print_tags(&query::tag_index(&store)?),
        Command::Ask { question } => ask_cli(&store, &question),
        Command::Show { id_or_title } => match query::resolve(&store, &id_or_title)? {
            Some(id) => {
                let idea = query::get(&store, &id)?.expect("a resolved id always exists");
                let related = query::related(&store, &id)?;
                let backlinks = query::backlinks(&store, &id)?;
                let mentions = query::unlinked_mentions(&store, &id)?;
                print_idea(&idea, &related, &backlinks, &mentions);
            }
            None => println!("no idea matches '{id_or_title}'."),
        },
        Command::New { title, tag } => {
            let path = cli.root.join(scaffold::filename(&title));
            if path.exists() {
                anyhow::bail!(
                    "a note already exists at {} — pick a different title",
                    path.display()
                );
            }
            let today = chrono::Utc::now().date_naive();
            std::fs::write(&path, scaffold::note_body(&title, &tag, today))?;
            println!("created {}", path.display());

            // Index the new note so it's immediately searchable (no enrichment —
            // that's opt-in and index-time only). Unchanged files hash-skip.
            indexer::run(&mut store, &cli.root, &IndexOptions { enrich: false, embed: false, force: false })?;

            if let Some(id) = query::resolve(&store, &title)? {
                let idea = query::get(&store, &id)?.expect("the note was just indexed");
                let related = query::related(&store, &id)?;
                let backlinks = query::backlinks(&store, &id)?;
                let mentions = query::unlinked_mentions(&store, &id)?;
                print_idea(&idea, &related, &backlinks, &mentions);
            }
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
fn print_idea(idea: &Idea, related: &[query::Hit], backlinks: &[query::Hit], mentions: &[query::Hit]) {
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

    if !backlinks.is_empty() {
        println!("\n  backlinks (notes linking here):");
        for h in backlinks {
            println!("    {}  {} — {}", tint_status(h.status), h.id, h.title);
        }
    }
    if !mentions.is_empty() {
        println!("\n  unlinked mentions (mention the title but don't link it):");
        for h in mentions {
            println!("    {}  {} — {}", tint_status(h.status), h.id, h.title);
        }
    }
    println!();
}

/// Resolve + render a model-proposed bridge between two notes. The model call is
/// gated on the `enrich` feature (D-015); the analysis around it is deterministic.
fn propose_bridge_cli(a: &Idea, b: &Idea) {
    println!("\n{}\n  ↕\n{}\n", a.title, b.title);
    #[cfg(feature = "enrich")]
    match phanes::enrich::propose_bridge(&a.title, &a.body, &b.title, &b.body) {
        Ok(idea) => println!("Proposed bridge:\n  {idea}\n"),
        Err(e) => println!("bridge failed: {e}\n"),
    }
    #[cfg(not(feature = "enrich"))]
    {
        let _ = (a, b);
        println!("(build with --features enrich, and run a model server, to propose a bridge)\n");
    }
}

/// Answer a question from the indexed notes (RAG). The model call is gated on the
/// `enrich` feature and the carve-out from INV-1 for user-invoked generative
/// actions (D-015); retrieval is deterministic over the stored embeddings.
fn ask_cli(store: &Store, question: &str) {
    #[cfg(feature = "enrich")]
    match phanes::ask::ask(store, question, 5) {
        Ok(answer) => {
            println!("\n{}\n", answer.text);
            if !answer.sources.is_empty() {
                println!("Sources:");
                for s in &answer.sources {
                    println!("  {} — {} ({:.0}% similar)", s.id, s.title, s.similarity * 100.0);
                }
            }
            println!();
        }
        Err(e) => println!("ask failed: {e}\n"),
    }
    #[cfg(not(feature = "enrich"))]
    {
        let _ = (store, question);
        println!("(build with --features enrich, run a model server, and `index --embed` to ask)\n");
    }
}

/// Print the tag vocabulary with per-tag counts (asserted, plus proposed if any).
fn print_tags(groups: &[phanes::query::TagGroup]) {
    if groups.is_empty() {
        println!("\n(no tags)\n");
        return;
    }
    println!("\nTags ({}):", groups.len());
    for g in groups {
        let total = g.asserted + g.proposed;
        if g.proposed > 0 {
            println!("  {:>3}  {}  ({} asserted, {} proposed)", total, g.tag, g.asserted, g.proposed);
        } else {
            println!("  {:>3}  {}", total, g.tag);
        }
    }
    println!();
}

/// Print the structural gap analysis: orphan ideas and candidate bridges.
fn print_gaps(g: &phanes::graph::RelGraph) {
    let orphans = g.orphans();
    println!("\nOrphans ({}) — connected to nothing:", orphans.len());
    if orphans.is_empty() {
        println!("  (none)");
    }
    for &i in &orphans {
        println!("  {} — {}", g.nodes[i].id, g.nodes[i].title);
    }

    let bridges = g.bridges(10);
    println!("\nCandidate bridges — semantically near but not linked:");
    if bridges.is_empty() {
        println!("  (none — run `index --embed` first?)");
    }
    for e in bridges {
        println!(
            "  {} ↔ {}  ({:.0}% similar)",
            g.nodes[e.a].title,
            g.nodes[e.b].title,
            e.weight * 100.0
        );
    }

    // Hubs: the most central "bridge" notes (F-020).
    let hubs: Vec<_> = g.hubs(5).into_iter().filter(|&(_, c)| c > 0.0).collect();
    if !hubs.is_empty() {
        println!("\nHubs — most central (paths run through these):");
        for (i, c) in hubs {
            println!("  {:>3.0}%  {} — {}", c * 100.0, g.nodes[i].id, g.nodes[i].title);
        }
    }

    // Clusters: topical communities (F-020).
    let com = g.communities();
    if let Some(&max) = com.iter().max() {
        let count = max + 1;
        // size of each community, largest first
        let mut sizes = vec![0usize; count];
        for &c in &com {
            sizes[c] += 1;
        }
        let mut ranked: Vec<(usize, usize)> = sizes.into_iter().enumerate().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        let multi = ranked.iter().filter(|&&(_, s)| s > 1).count();
        println!("\nClusters — {count} topical group(s) ({multi} with >1 note):");
        for (_, size) in ranked.into_iter().filter(|&(_, s)| s > 1).take(5) {
            println!("  {size} notes");
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

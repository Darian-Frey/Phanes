//! Phanes desktop UI — a three-panel app (explorer / editor / info) over the
//! deterministic core. Built only with `--features ui`.
//!
//! Usage: `phanes-ui [root]` — `root` is the ideas folder (default `ideas`),
//! whose index lives at `<root>/.phanes/index.db`.
//!
//! Panels:
//!   - Left (explorer): folder tree of indexed notes + filter box. (Done.)
//!   - Centre (editor): View (rendered markdown) / Edit (raw) toggle; explicit
//!     Save (button or Ctrl+S) writes the file and runs a one-file index pass —
//!     enrichment never fires here (it's opt-in and index-time only). (Done.)
//!   - Right (info): selected id. (Provenance/relationships next.)

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use phanes::graph::{self, EdgeKind, RelGraph};
use phanes::indexer::{self, IndexOptions, IndexReport};
use phanes::model::{Idea, Provenance, Status};
use phanes::parser;
use phanes::query::{self, ListItem, SearchFilter};
use phanes::scaffold;
use phanes::store::Store;
use walkdir::WalkDir;

/// The statuses offered by the info-panel dropdown (every real status; `Unknown`
/// is the "no status" sentinel and isn't something you set).
const STATUS_CHOICES: [Status; 7] = [
    Status::Concept,
    Status::Draft,
    Status::Active,
    Status::Dormant,
    Status::Complete,
    Status::Archived,
    Status::Superseded,
];

fn main() -> eframe::Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ideas"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Phanes",
        options,
        Box::new(move |_cc| Ok(Box::new(PhanesApp::new(root)))),
    )
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    View,
    Edit,
    Graph,
    Ask,
}

/// What a click on the graph canvas resolved to.
enum GraphAction {
    Select(String),
    Bridge(String, String),
}

/// State of an on-demand bridge proposal (the model call runs on a background
/// thread so the window never freezes).
enum BridgeState {
    None,
    Proposing { a: String, b: String },
    Done { a: String, b: String, text: String },
    Error(String),
}

/// State of an on-demand RAG "Ask" query (model call on a background thread).
enum AskState {
    Idle,
    Asking,
    Answered { question: String, text: String, sources: Vec<(String, String)> },
    Error(String),
}

/// What the background Ask worker sends back: the answer plus its cited sources
/// as `(id, "Title (NN%)")` pairs (plain types, so this compiles without enrich).
/// Only constructed under `enrich`; read by `poll_ask` regardless.
#[cfg_attr(not(feature = "enrich"), allow(dead_code))]
struct AnswerData {
    question: String,
    text: String,
    sources: Vec<(String, String)>,
}

/// Which tree the left panel shows: the indexed-notes view (semantic, status-
/// tinted) or the raw filesystem view (everything under the root, like an IDE
/// explorer). F-025.
#[derive(PartialEq, Clone, Copy)]
enum ExplorerMode {
    Ideas,
    Files,
}

/// A folder in the explorer: subdirectories plus the notes directly in it.
#[derive(Default)]
struct Tree {
    dirs: BTreeMap<String, Tree>,
    files: Vec<FileEntry>,
}

struct FileEntry {
    id: String,
    title: String,
    status: Status,
}

/// A folder in the raw **Files** view: subdirectories plus the files in it.
#[derive(Default)]
struct FileTree {
    dirs: BTreeMap<String, FileTree>,
    files: Vec<FsEntry>,
}

/// One file in the Files view. `id` is `Some` for `.md` files (the index id
/// computed from the path, so a click can open the note); `None` otherwise.
struct FsEntry {
    name: String,
    path: PathBuf,
    id: Option<String>,
}

/// Build the raw filesystem tree under `root`: every file and subfolder, like an
/// IDE explorer. Hidden entries (dotfiles, `.phanes/`, `.git/`) are skipped so
/// the index DB and VCS internals stay out of view. Deterministic — no DB, no
/// model.
fn build_file_tree(root: &Path) -> FileTree {
    let mut tree = FileTree::default();
    let normal = |rel: &Path| -> Vec<String> {
        rel.components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect()
    };

    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden entries (and thus their subtrees): .phanes, .git, etc.
            e.depth() == 0
                || e.file_name().to_str().is_none_or(|n| !n.starts_with('.'))
        })
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path);
        let comps = normal(rel);
        if comps.is_empty() {
            continue;
        }

        // Descend to the parent node, creating directory nodes as needed (so
        // empty folders still appear).
        let mut node = &mut tree;
        for dir in &comps[..comps.len() - 1] {
            node = node.dirs.entry(dir.clone()).or_default();
        }
        let leaf = comps.last().unwrap().clone();
        if entry.file_type().is_dir() {
            node.dirs.entry(leaf).or_default();
        } else {
            let is_md = path.extension().is_some_and(|x| x == "md");
            node.files.push(FsEntry {
                name: leaf,
                path: path.to_path_buf(),
                id: is_md.then(|| parser::id_from_path(rel)),
            });
        }
    }
    tree
}

/// Build a folder tree from indexed notes, grouping by each note's path
/// components below `root`.
fn build_tree(items: &[ListItem], root: &Path) -> Tree {
    let mut tree = Tree::default();
    for item in items {
        let full = Path::new(&item.path);
        let rel = full.strip_prefix(root).unwrap_or(full);
        let comps: Vec<String> = rel
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();

        let mut node = &mut tree;
        if comps.len() > 1 {
            for dir in &comps[..comps.len() - 1] {
                node = node.dirs.entry(dir.clone()).or_default();
            }
        }
        node.files.push(FileEntry {
            id: item.id.clone(),
            title: item.title.clone(),
            status: item.status,
        });
    }
    tree
}

struct PhanesApp {
    root: PathBuf,
    store: Option<Store>,
    error: Option<String>,
    tree: Tree,
    filter: String,
    results: Vec<query::Hit>,
    selected: Option<String>,
    selected_idea: Option<Idea>,
    related: Vec<query::Hit>,
    near: Vec<query::Hit>,
    backlinks: Vec<query::Hit>, // notes linking to the selected one (F-016)
    mentions: Vec<query::Hit>,  // notes mentioning the title but not linking (F-016)
    // centre editor
    mode: Mode,
    buffer: String,       // raw file content of the selected note (editable)
    saved: String,        // last-saved content, for the dirty check
    md_cache: CommonMarkCache,
    status_msg: Option<String>,
    // graph view (built lazily; force-directed layout)
    graph: Option<RelGraph>,
    layout: Vec<egui::Pos2>,
    pinned: Option<usize>, // node currently being dragged (held to the cursor)
    graph_pan: egui::Pos2, // world point shown at the canvas centre
    graph_zoom: f32,
    graph_alpha: f32, // sim "temperature": cools to settle, reheats on drag
    show_gaps: bool,  // overlay orphans + candidate bridges
    // model-proposed bridge (background thread + channel)
    bridge: BridgeState,
    bridge_rx: Option<std::sync::mpsc::Receiver<anyhow::Result<String>>>,
    // background "Scan + AI" worker (enrich + embed on its own DB connection)
    ai_rx: Option<std::sync::mpsc::Receiver<anyhow::Result<IndexReport>>>,
    tag_input: String, // the "add tag" field in the info panel
    // RAG "Ask" mode (background thread + channel)
    ask_input: String,
    ask: AskState,
    ask_rx: Option<std::sync::mpsc::Receiver<anyhow::Result<AnswerData>>>,
    // left-panel view: indexed Ideas vs raw Files (F-025)
    explorer_mode: ExplorerMode,
    file_tree: Option<FileTree>, // raw filesystem tree, built lazily / invalidated on reindex
    reveal_selected: bool,       // one-shot: expand+scroll the explorer to the selection
    // quick switcher (Ctrl+P) — fuzzy jump to any note (F-017)
    switcher_open: bool,
    switcher_query: String,
    switcher_index: usize,
    switcher_items: Vec<ListItem>, // snapshot of all notes, taken on open
    switcher_focus: bool,          // request text-field focus on the next frame
}

/// A note's asserted tag values (the editable, file-backed set).
fn asserted_tags(idea: &Idea) -> Vec<String> {
    idea.tags
        .iter()
        .filter(|t| t.source == Provenance::Asserted)
        .map(|t| t.value.clone())
        .collect()
}

impl PhanesApp {
    fn new(root: PathBuf) -> Self {
        let (store, error, tree) = match Store::open(&root.join(".phanes").join("index.db")) {
            Ok(mut store) => {
                // Index the folder on startup so the UI reflects the current
                // notes even on a never-indexed folder. Cheap: hash-gated, and
                // no enrichment (that stays opt-in, CLI-only).
                let opts = IndexOptions { enrich: false, embed: false, force: false };
                let _ = indexer::run(&mut store, &root, &opts);
                let items = query::list(&store).unwrap_or_default();
                let tree = build_tree(&items, &root);
                (Some(store), None, tree)
            }
            Err(e) => (None, Some(e.to_string()), Tree::default()),
        };
        Self {
            root,
            store,
            error,
            tree,
            filter: String::new(),
            results: Vec::new(),
            selected: None,
            selected_idea: None,
            related: Vec::new(),
            near: Vec::new(),
            backlinks: Vec::new(),
            mentions: Vec::new(),
            mode: Mode::View,
            buffer: String::new(),
            saved: String::new(),
            md_cache: CommonMarkCache::default(),
            status_msg: None,
            graph: None,
            layout: Vec::new(),
            pinned: None,
            graph_pan: egui::Pos2::ZERO,
            graph_zoom: 1.0,
            graph_alpha: 1.0,
            show_gaps: false,
            bridge: BridgeState::None,
            bridge_rx: None,
            ai_rx: None,
            tag_input: String::new(),
            ask_input: String::new(),
            ask: AskState::Idle,
            ask_rx: None,
            explorer_mode: ExplorerMode::Ideas,
            file_tree: None,
            reveal_selected: false,
            switcher_open: false,
            switcher_query: String::new(),
            switcher_index: 0,
            switcher_items: Vec::new(),
            switcher_focus: false,
        }
    }

    /// Start a background "Scan + AI" pass: a worker thread opens its own DB
    /// connection and re-indexes with enrichment + embeddings, so the new/changed
    /// notes gain their proposed and semantic layers without freezing the UI (the
    /// model calls are slow). Needs the `enrich` build + a model server; without
    /// them it's just a background re-index.
    fn start_ai_scan(&mut self) {
        if self.ai_rx.is_some() {
            return; // already running
        }
        let db = self.root.join(".phanes").join("index.db");
        let root = self.root.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.ai_rx = Some(rx);
        self.status_msg = Some("scanning + enriching… (this can take a while)".into());
        std::thread::spawn(move || {
            let result = (|| {
                let mut store = Store::open(&db)?;
                let opts = IndexOptions { enrich: true, embed: true, force: false };
                indexer::run(&mut store, &root, &opts)
            })();
            let _ = tx.send(result);
        });
    }

    /// Poll the background AI-scan worker; refresh everything when it finishes.
    fn poll_ai_scan(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.ai_rx else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.ai_rx = None;
                match result {
                    Ok(r) => {
                        self.status_msg = Some(format!(
                            "AI scan done · enriched {} · embedded {}",
                            r.enriched, r.embedded
                        ));
                        self.reload_after_index();
                    }
                    Err(e) => self.status_msg = Some(format!("AI scan failed: {e}")),
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint(),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.ai_rx = None;
                self.status_msg = Some("AI scan worker stopped unexpectedly".into());
            }
        }
    }

    /// Kick off a bridge proposal between two notes on a background thread. The
    /// model call is gated on the `enrich` feature (D-015); without it, the UI
    /// just reports how to enable it.
    fn start_bridge(&mut self, a_id: &str, b_id: &str) {
        let notes = self.store.as_ref().and_then(|s| {
            let a = query::get(s, a_id).ok().flatten()?;
            let b = query::get(s, b_id).ok().flatten()?;
            Some((a, b))
        });
        let Some((a, b)) = notes else { return };
        self.bridge = BridgeState::Proposing { a: a.title.clone(), b: b.title.clone() };
        self.bridge_rx = None;

        #[cfg(feature = "enrich")]
        {
            let (tx, rx) = std::sync::mpsc::channel();
            self.bridge_rx = Some(rx);
            let (at, ab, bt, bb) = (a.title, a.body, b.title, b.body);
            std::thread::spawn(move || {
                let _ = tx.send(phanes::enrich::propose_bridge(&at, &ab, &bt, &bb));
            });
        }
        #[cfg(not(feature = "enrich"))]
        {
            let _ = (a, b);
            self.bridge =
                BridgeState::Error("rebuild with `--features ui,enrich` to propose bridges".into());
        }
    }

    /// Poll the background bridge worker; transition the state when it finishes.
    fn poll_bridge(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.bridge_rx else { return };
        match rx.try_recv() {
            Ok(result) => {
                let (a, b) = match &self.bridge {
                    BridgeState::Proposing { a, b } => (a.clone(), b.clone()),
                    _ => (String::new(), String::new()),
                };
                self.bridge = match result {
                    Ok(text) => BridgeState::Done { a, b, text },
                    Err(e) => BridgeState::Error(format!("bridge failed: {e}")),
                };
                self.bridge_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint(),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.bridge = BridgeState::Error("bridge worker stopped unexpectedly".into());
                self.bridge_rx = None;
            }
        }
    }

    /// Kick off a RAG "Ask" query on a background thread: the worker opens its own
    /// (read-only) DB connection, embeds the question, retrieves the nearest notes,
    /// and asks the local model — so the slow model calls never freeze the UI. A
    /// query-time generative action, the INV-1 carve-out (D-015). Gated on `enrich`.
    fn start_ask(&mut self) {
        let question = self.ask_input.trim().to_string();
        if question.is_empty() || self.ask_rx.is_some() {
            return;
        }
        self.ask = AskState::Asking;

        #[cfg(feature = "enrich")]
        {
            let db = self.root.join(".phanes").join("index.db");
            let (tx, rx) = std::sync::mpsc::channel();
            self.ask_rx = Some(rx);
            std::thread::spawn(move || {
                let result = (|| {
                    let store = Store::open(&db)?;
                    let answer = phanes::ask::ask(&store, &question, 5)?;
                    Ok(AnswerData {
                        question,
                        text: answer.text,
                        sources: answer
                            .sources
                            .into_iter()
                            .map(|s| (s.id, format!("{} ({:.0}%)", s.title, s.similarity * 100.0)))
                            .collect(),
                    })
                })();
                let _ = tx.send(result);
            });
        }
        #[cfg(not(feature = "enrich"))]
        {
            self.ask = AskState::Error("rebuild with `--features ui,enrich` to use Ask".into());
        }
    }

    /// Poll the background Ask worker; store the answer when it arrives.
    fn poll_ask(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.ask_rx else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.ask_rx = None;
                self.ask = match result {
                    Ok(a) => AskState::Answered {
                        question: a.question,
                        text: a.text,
                        sources: a.sources,
                    },
                    Err(e) => AskState::Error(format!("ask failed: {e}")),
                };
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint(),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.ask_rx = None;
                self.ask = AskState::Error("ask worker stopped unexpectedly".into());
            }
        }
    }

    /// The centre-pane "Ask" panel: a question field + the latest answer with its
    /// cited sources (clickable). Returns a note id if a source was clicked.
    fn ask_ui(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let mut clicked = None;
        let busy = self.ask_rx.is_some();

        ui.horizontal(|ui| {
            let resp = ui.add_enabled(
                !busy,
                egui::TextEdit::singleline(&mut self.ask_input)
                    .hint_text("Ask a question about your notes…")
                    .desired_width(ui.available_width() - 70.0),
            );
            let submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.add_enabled(!busy, egui::Button::new("Ask")).clicked() || submit)
                && !self.ask_input.trim().is_empty()
            {
                self.start_ask();
            }
        });
        ui.weak("Answers are grounded in your most relevant notes (needs `index --embed`).");
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| match &self.ask {
            AskState::Idle => {
                ui.weak("Ask a question to search and summarise across your notes.");
            }
            AskState::Asking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Retrieving notes and asking the model…");
                });
            }
            AskState::Answered { question, text, sources } => {
                ui.strong(question.as_str());
                ui.add_space(6.0);
                ui.label(text.as_str());
                if !sources.is_empty() {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.strong("Sources");
                    for (id, label) in sources {
                        if ui.selectable_label(false, label.as_str()).clicked() {
                            clicked = Some(id.clone());
                        }
                    }
                }
            }
            AskState::Error(msg) => {
                ui.colored_label(egui::Color32::from_rgb(235, 140, 90), msg.as_str());
            }
        });
        clicked
    }

    /// Quick switcher (F-017): Ctrl+P opens a fuzzy "jump to a note" overlay,
    /// usable from any view. ↑/↓ move, Enter opens, Esc closes. Deterministic —
    /// just a fuzzy filter over the indexed-note list.
    fn quick_switcher(&mut self, ctx: &egui::Context) {
        // Toggle on Ctrl/Cmd+P.
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::P)) {
            self.switcher_open = !self.switcher_open;
            if self.switcher_open {
                self.switcher_query.clear();
                self.switcher_index = 0;
                self.switcher_items = self
                    .store
                    .as_ref()
                    .map(|s| query::list(s).unwrap_or_default())
                    .unwrap_or_default();
                self.switcher_focus = true;
            }
        }
        if !self.switcher_open {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.switcher_open = false;
            return;
        }

        // Rank the notes against the query (owned, so the window closure borrows
        // no note state). Empty query → first notes in order.
        let q = self.switcher_query.to_lowercase();
        let mut matches: Vec<(String, String, Status)> = if q.is_empty() {
            self.switcher_items
                .iter()
                .map(|it| (it.id.clone(), it.title.clone(), it.status))
                .collect()
        } else {
            let mut scored: Vec<(i32, &ListItem)> = self
                .switcher_items
                .iter()
                .filter_map(|it| {
                    let by_title = fuzzy_score(&q, &it.title.to_lowercase());
                    let by_id = fuzzy_score(&q, &it.id);
                    by_title.max(by_id).map(|s| (s, it))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.title.len().cmp(&b.1.title.len())));
            scored.into_iter().map(|(_, it)| (it.id.clone(), it.title.clone(), it.status)).collect()
        };
        matches.truncate(50);

        if self.switcher_index >= matches.len() {
            self.switcher_index = matches.len().saturating_sub(1);
        }

        // Keyboard nav — read before the window so the text field doesn't eat it.
        let n = matches.len();
        let mut navigated = false;
        if n > 0 {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                self.switcher_index = (self.switcher_index + 1) % n;
                navigated = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                self.switcher_index = (self.switcher_index + n - 1) % n;
                navigated = true;
            }
        }
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));

        let mut chosen: Option<String> = None;
        egui::Window::new("Quick switcher")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .default_width(440.0)
            .show(ctx, |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.switcher_query)
                        .hint_text("Jump to a note…")
                        .desired_width(f32::INFINITY),
                );
                if self.switcher_focus {
                    resp.request_focus();
                    self.switcher_focus = false;
                }
                ui.weak("↑/↓ move · Enter open · Esc close");
                ui.separator();
                if matches.is_empty() {
                    ui.weak("no matches");
                }
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (i, (id, title, status)) in matches.iter().enumerate() {
                        let is_sel = i == self.switcher_index;
                        let text = egui::RichText::new(title).color(status_color(*status));
                        let r = ui.selectable_label(is_sel, text);
                        if is_sel && navigated {
                            r.scroll_to_me(Some(egui::Align::Center));
                        }
                        if r.clicked() {
                            chosen = Some(id.clone());
                        }
                    }
                });
            });

        if enter {
            if let Some((id, _, _)) = matches.get(self.switcher_index) {
                chosen = Some(id.clone());
            }
        }
        if let Some(id) = chosen {
            self.switcher_open = false;
            self.select(id);
        }
    }

    /// Floating window showing the in-progress / finished bridge proposal.
    fn bridge_window(&mut self, ctx: &egui::Context) {
        if matches!(self.bridge, BridgeState::None) {
            return;
        }
        let mut close = false;
        egui::Window::new("Proposed bridge")
            .collapsible(false)
            .resizable(true)
            .default_width(380.0)
            .show(ctx, |ui| {
                match &self.bridge {
                    BridgeState::Proposing { a, b } => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Proposing a bridge…");
                        });
                        ui.add_space(4.0);
                        ui.weak(format!("{a}  ↕  {b}"));
                    }
                    BridgeState::Done { a, b, text } => {
                        ui.strong(a.as_str());
                        ui.weak("↕");
                        ui.strong(b.as_str());
                        ui.separator();
                        ui.label(text.as_str());
                    }
                    BridgeState::Error(msg) => {
                        ui.colored_label(egui::Color32::from_rgb(235, 140, 90), msg.as_str());
                    }
                    BridgeState::None => {}
                }
                ui.separator();
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if close {
            self.bridge = BridgeState::None;
        }
    }

    /// Build the relationship graph and seed a deterministic spiral layout, once.
    fn ensure_graph(&mut self) {
        if self.graph.is_some() {
            return;
        }
        let Some(store) = &self.store else { return };
        // Tighter than the `gaps` default: keep only each note's strongest few
        // semantic links so clusters separate instead of forming a hairball.
        let opts = graph::GraphOptions { semantic_threshold: 0.70, semantic_per_node: 3 };
        let g = match graph::build(store, &opts) {
            Ok(g) => g,
            Err(_) => return,
        };
        let golden = 2.399963_f32;
        self.layout = (0..g.nodes.len())
            .map(|i| {
                let r = 55.0 * (i as f32 + 1.0).sqrt();
                let a = i as f32 * golden;
                egui::pos2(r * a.cos(), r * a.sin())
            })
            .collect();
        self.pinned = None;
        self.graph_alpha = 1.0;
        self.graph = Some(g);
    }

    /// One Fruchterman-Reingold step: `k²/d` repulsion between all pairs, `d²/k`
    /// attraction along edges, a weak pull to centre, capped displacement.
    /// `1/d` repulsion (vs `1/d²`) reaches far enough to actually untangle.
    /// Runs each frame while the Graph tab is open; the pinned (dragged) node is
    /// held fixed so its neighbours spring toward it.
    fn simulate(&mut self) {
        let Some(g) = &self.graph else { return };
        let n = g.nodes.len();
        if n < 2 {
            return;
        }
        // Settled and not being dragged: nothing to do (saves CPU, stops jitter).
        if self.graph_alpha < 0.008 && self.pinned.is_none() {
            return;
        }
        let k = 80.0_f32; // ideal edge length
        let mut force = vec![egui::Vec2::ZERO; n];

        let min_sep = 28.0_f32; // node diameter + breathing room (collision radius)
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.layout[i] - self.layout[j];
                let dist = d.length().max(1.0);
                let dir = d / dist;
                // Long-range Fruchterman-Reingold repulsion.
                let mut push = dir * (k * k / dist);
                // Short-range collision shove (d3's forceCollide): the main fix
                // for a lumpy layout — keeps nodes from clumping into blobs.
                if dist < min_sep {
                    push += dir * ((min_sep - dist) * 3.0);
                }
                force[i] += push;
                force[j] -= push;
            }
        }
        for e in &g.edges {
            let d = self.layout[e.b] - self.layout[e.a];
            let dist = d.length().max(0.01);
            let pull = d / dist * (dist * dist / k) * (0.4 + 0.6 * e.weight);
            force[e.a] += pull;
            force[e.b] -= pull;
        }
        for i in 0..n {
            force[i] -= self.layout[i].to_vec2() * 0.01; // gentle centring
        }

        let max_disp = 10.0;
        let alpha = self.graph_alpha;
        for i in 0..n {
            if self.pinned == Some(i) {
                continue; // dragged node stays under the cursor
            }
            let f = force[i];
            let len = f.length();
            if len > 1e-3 {
                self.layout[i] += f / len * len.min(max_disp) * alpha;
            }
        }
        self.graph_alpha *= 0.975; // cool toward a settled layout
    }

    /// Draw the graph canvas; returns what a click resolved to (a node, or a
    /// candidate-bridge edge when the Gaps overlay is on).
    fn graph_ui(&mut self, ui: &mut egui::Ui) -> Option<GraphAction> {
        self.ensure_graph();
        self.simulate();

        let Some(g) = &self.graph else {
            ui.weak("no graph");
            return None;
        };
        if g.nodes.is_empty() {
            ui.weak("no notes to graph");
            return None;
        }

        let (resp, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let center = resp.rect.center();

        // zoom around the cursor
        if let Some(hover) = resp.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let before = self.graph_pan + (hover - center) / self.graph_zoom;
                self.graph_zoom = (self.graph_zoom * (1.0 + scroll * 0.0015)).clamp(0.1, 6.0);
                self.graph_pan = before - (hover - center) / self.graph_zoom;
            }
        }

        let (zoom, pan) = (self.graph_zoom, self.graph_pan);
        let to_screen = |w: egui::Pos2| center + (w - pan) * zoom;
        let from_screen = |s: egui::Pos2| pan + (s - center) / zoom;

        // drag a node (it follows the cursor; neighbours spring along) — or, on
        // empty space, pan the view.
        if resp.drag_started() {
            self.pinned = resp
                .interact_pointer_pos()
                .and_then(|p| node_at(g, &self.layout, to_screen, p));
        }
        if resp.dragged() {
            match self.pinned {
                Some(i) => {
                    if let Some(p) = resp.interact_pointer_pos() {
                        self.layout[i] = from_screen(p);
                    }
                    self.graph_alpha = self.graph_alpha.max(0.35); // reheat so neighbours follow
                }
                None => self.graph_pan -= resp.drag_delta() / zoom,
            }
        }
        if resp.drag_stopped() {
            self.pinned = None;
        }

        for e in &g.edges {
            painter.line_segment(
                [to_screen(self.layout[e.a]), to_screen(self.layout[e.b])],
                egui::Stroke::new(1.0, edge_color(e.kind, e.weight)),
            );
        }

        let hovered = resp.hover_pos().and_then(|hp| node_at(g, &self.layout, to_screen, hp));

        for (i, node) in g.nodes.iter().enumerate() {
            let p = to_screen(self.layout[i]);
            let sel = self.selected.as_deref() == Some(node.id.as_str());
            let r = if sel || hovered == Some(i) { 8.0 } else { 6.0 };
            painter.circle_filled(p, r, status_color(node.status));
            if sel {
                painter.circle_stroke(p, r + 2.5, egui::Stroke::new(2.0, egui::Color32::WHITE));
            }
        }

        // labels for the selected and hovered nodes only (keeps it legible)
        if let Some(sel) = &self.selected {
            if let Some(i) = g.nodes.iter().position(|n| &n.id == sel) {
                draw_label(&painter, to_screen(self.layout[i]), &g.nodes[i].title);
            }
        }
        if let Some(i) = hovered {
            draw_label(&painter, to_screen(self.layout[i]), &g.nodes[i].title);
        }

        // Gap overlay: the strongest candidate bridges (dashed, with %), and any
        // orphan ideas (ringed + always labelled).
        if self.show_gaps {
            for e in g.bridges(8) {
                let a = to_screen(self.layout[e.a]);
                let b = to_screen(self.layout[e.b]);
                painter.extend(egui::Shape::dashed_line(
                    &[a, b],
                    egui::Stroke::new(1.8, PROPOSED),
                    6.0,
                    4.0,
                ));
                painter.text(
                    a + (b - a) * 0.5,
                    egui::Align2::CENTER_CENTER,
                    format!("{:.0}%", e.weight * 100.0),
                    egui::FontId::proportional(10.0),
                    PROPOSED,
                );
            }
            let orphan_ring = egui::Color32::from_rgb(235, 140, 90);
            for i in g.orphans() {
                let p = to_screen(self.layout[i]);
                painter.circle_stroke(p, 11.0, egui::Stroke::new(2.0, orphan_ring));
                draw_label(&painter, p, &g.nodes[i].title);
            }
        }

        // Stats overlay (top-left of the canvas).
        let mut stats = format!("{} notes · {} links", g.nodes.len(), g.edges.len());
        if self.show_gaps {
            let mut comps = g.components();
            comps.sort_unstable();
            comps.dedup();
            stats += &format!(" · {} clusters · {} orphans", comps.len(), g.orphans().len());
        }
        painter.text(
            resp.rect.min + egui::vec2(8.0, 6.0),
            egui::Align2::LEFT_TOP,
            stats,
            egui::FontId::proportional(12.0),
            egui::Color32::from_gray(150),
        );

        // Keep animating only while the layout is still warm (or being dragged);
        // a settled graph stays static until the next interaction.
        if self.graph_alpha > 0.008 || self.pinned.is_some() {
            ui.ctx().request_repaint();
        }

        if resp.clicked() {
            if let Some(i) = hovered {
                return Some(GraphAction::Select(g.nodes[i].id.clone()));
            }
            // Not on a node — if the gap overlay is on, a click near a dashed
            // bridge proposes an idea connecting its two notes.
            if self.show_gaps {
                if let Some(pos) = resp.interact_pointer_pos().or_else(|| resp.hover_pos()) {
                    for e in g.bridges(8) {
                        let a = to_screen(self.layout[e.a]);
                        let b = to_screen(self.layout[e.b]);
                        if dist_to_segment(pos, a, b) < 6.0 {
                            return Some(GraphAction::Bridge(
                                g.nodes[e.a].id.clone(),
                                g.nodes[e.b].id.clone(),
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    fn dirty(&self) -> bool {
        self.buffer != self.saved
    }

    /// Re-run the filter search (only when the filter text changes).
    fn run_filter(&mut self) {
        let Some(store) = &self.store else { return };
        let q = self.filter.trim();
        self.results = if q.is_empty() {
            Vec::new()
        } else {
            let filter = SearchFilter { limit: 200, ..Default::default() };
            query::search(store, q, &filter).unwrap_or_default()
        };
    }

    /// Select a note by id: load its record and its raw file into the buffer.
    /// (Switching notes discards unsaved edits — the dirty marker warns first.)
    fn select(&mut self, id: String) {
        let (idea, related, near, backlinks, mentions) = match self.store.as_ref() {
            Some(s) => (
                query::get(s, &id).ok().flatten(),
                query::related(s, &id).unwrap_or_default(),
                query::near(s, &id, 8).unwrap_or_default(),
                query::backlinks(s, &id).unwrap_or_default(),
                query::unlinked_mentions(s, &id).unwrap_or_default(),
            ),
            None => (None, Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };
        self.buffer = match &idea {
            Some(i) => std::fs::read_to_string(&i.path)
                .unwrap_or_else(|e| format!("<could not read {}: {e}>", i.path.display())),
            None => String::new(),
        };
        self.saved = self.buffer.clone();
        self.selected_idea = idea;
        self.related = related;
        self.near = near;
        self.backlinks = backlinks;
        self.mentions = mentions;
        self.selected = Some(id);
        self.mode = Mode::View;
        self.status_msg = None;
        self.reveal_selected = true; // expand+scroll the explorer to it next render
    }

    /// Open a file picked from the **Files** view. If it's an indexed note, this is
    /// just `select`; otherwise (a not-yet-indexed `.md`, or any other file) the
    /// raw content is loaded for viewing, with no DB-backed info — a hint suggests
    /// a Scan to index it.
    fn open_file(&mut self, path: PathBuf, id: Option<String>) {
        let indexed = id.as_deref().and_then(|i| {
            self.store.as_ref().and_then(|s| query::get(s, i).ok().flatten())
        });
        if let (Some(id), Some(_)) = (id, indexed) {
            self.select(id);
            return;
        }
        self.buffer = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| format!("<could not read {}: {e}>", path.display()));
        self.saved = self.buffer.clone();
        self.selected_idea = None;
        self.related.clear();
        self.near.clear();
        self.backlinks.clear();
        self.mentions.clear();
        self.selected = None;
        self.mode = Mode::View;
        let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        self.status_msg = Some(format!("{name} — not indexed (Scan to add it)"));
    }

    /// Accept an unlinked mention (F-016): write a resolvable markdown link to the
    /// selected note into the *mentioning* note's file, then re-index so the new
    /// link is picked up. Deterministic; no model.
    fn accept_mention(&mut self, mention: query::Hit) {
        let Some(target) = &self.selected_idea else { return };
        let target_title = target.title.clone();
        let target_path = target.path.clone();

        let Some(mention_path) = self
            .store
            .as_ref()
            .and_then(|s| query::get(s, &mention.id).ok().flatten())
            .map(|i| i.path)
        else {
            return;
        };

        let rel = relative_md_link(&mention_path, &target_path);
        // A markdown destination with spaces must be wrapped in <…> or the space
        // truncates it (verified in parser tests).
        let target = if rel.contains(' ') { format!("<{rel}>") } else { rel };
        let Ok(raw) = std::fs::read_to_string(&mention_path) else {
            self.status_msg = Some(format!("could not read {}", mention_path.display()));
            return;
        };
        let Some(updated) = scaffold::link_mention(&raw, &target_title, &target) else {
            self.status_msg = Some("no clean mention found to link".into());
            return;
        };
        if std::fs::write(&mention_path, &updated).is_ok() {
            self.status_msg = Some(format!("linked {} → {}", mention.title, target_title));
            if let Some(store) = &mut self.store {
                let opts = IndexOptions { enrich: false, embed: false, force: false };
                let _ = indexer::run(store, &self.root, &opts);
            }
            self.reload_after_index();
        }
    }

    /// Write the buffer to disk and run a one-file index pass. This is the only
    /// place an edit re-enters the index; enrichment stays off (INV-1).
    fn save(&mut self) {
        let Some(idea) = &self.selected_idea else { return };
        let path = idea.path.clone();
        if let Err(e) = std::fs::write(&path, &self.buffer) {
            self.status_msg = Some(format!("save failed: {e}"));
            return;
        }
        self.saved = self.buffer.clone();
        if let Some(store) = &mut self.store {
            let opts = IndexOptions { enrich: false, embed: false, force: false };
            if let Err(e) = indexer::run(store, &self.root, &opts) {
                self.status_msg = Some(format!("saved, but index failed: {e}"));
                return;
            }
        }
        self.reload_after_index();
        self.status_msg = Some("saved".into());
    }

    /// Refresh the tree and the selected record after an index pass (title or
    /// status may have changed). The buffer is left alone — it matches the file.
    fn reload_after_index(&mut self) {
        let Some(store) = &self.store else { return };
        let items = query::list(store).unwrap_or_default();
        let tree = build_tree(&items, &self.root);
        let (idea, related, near, backlinks, mentions) = match self.selected.as_ref() {
            Some(id) => (
                query::get(store, id).ok().flatten(),
                query::related(store, id).unwrap_or_default(),
                query::near(store, id, 8).unwrap_or_default(),
                query::backlinks(store, id).unwrap_or_default(),
                query::unlinked_mentions(store, id).unwrap_or_default(),
            ),
            None => (None, Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };
        self.tree = tree;
        self.selected_idea = idea;
        self.related = related;
        self.near = near;
        self.backlinks = backlinks;
        self.mentions = mentions;
        self.graph = None; // rebuilt next time the Graph tab is opened
        self.file_tree = None; // rebuilt next time the Files view is shown
    }

    /// Write a new asserted status into the selected note's file (deterministic,
    /// via `scaffold::set_status`), then re-index and refresh. Applied to the
    /// in-memory buffer so any open edits persist too rather than being clobbered.
    fn set_selected_status(&mut self, status: Status) {
        let Some(idea) = &self.selected_idea else { return };
        if idea.status.value == status {
            return;
        }
        let path = idea.path.clone();
        let updated = scaffold::set_status(&self.buffer, status);
        if std::fs::write(&path, &updated).is_ok() {
            self.buffer = updated.clone();
            self.saved = updated;
            self.status_msg = Some(format!("status → {}", status.as_str()));
            if let Some(store) = &mut self.store {
                let opts = IndexOptions { enrich: false, embed: false, force: false };
                let _ = indexer::run(store, &self.root, &opts);
            }
            self.reload_after_index();
        }
    }

    /// Replace the selected note's asserted tags (add / remove / accept-proposed
    /// from the info panel). Writes the new set into the file's frontmatter via
    /// `scaffold::set_tags` (applied to the live buffer so open edits persist),
    /// then updates just the DB tag rows via `store::set_asserted_tags` — which
    /// also promotes an accepted proposed tag in place. No full re-index, so the
    /// note's other proposed tags (model output, file-absent) survive (INV-2).
    fn apply_tags(&mut self, new_asserted: Vec<String>) {
        let Some(idea) = &self.selected_idea else { return };
        let id = idea.id.clone();
        let path = idea.path.clone();
        let updated = scaffold::set_tags(&self.buffer, &new_asserted);
        if std::fs::write(&path, &updated).is_ok() {
            self.buffer = updated.clone();
            self.saved = updated;
            self.status_msg = Some("tags updated".to_string());
            if let Some(store) = &mut self.store {
                let _ = store.set_asserted_tags(&id, &new_asserted);
            }
            self.reload_after_index();
        }
    }

    /// Re-index the folder in place (deterministic, no model) and refresh the
    /// explorer, graph, and current view — so new or edited notes appear without
    /// restarting. The semantic/proposed layers still need a CLI `--embed` /
    /// `--enrich` pass (those are slow model calls).
    fn rescan(&mut self) {
        if let Some(store) = &mut self.store {
            let opts = IndexOptions { enrich: false, embed: false, force: false };
            let _ = indexer::run(store, &self.root, &opts);
        }
        self.reload_after_index();
        self.run_filter();
    }
}

impl eframe::App for PhanesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // --- left: explorer ---
        let left = egui::Panel::left("explorer")
            .resizable(true)
            .default_size(260.0)
            .show_inside(ui, |ui| {
                let mut rescan = false;
                let mut ai_scan = false;
                let mut file_click: Option<(PathBuf, Option<String>)> = None;

                // View toggle: indexed Ideas vs the raw Files tree (F-025).
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.explorer_mode == ExplorerMode::Ideas, "Ideas")
                        .on_hover_text("Indexed notes — status-tinted, semantic")
                        .clicked()
                    {
                        self.explorer_mode = ExplorerMode::Ideas;
                    }
                    if ui
                        .selectable_label(self.explorer_mode == ExplorerMode::Files, "Files")
                        .on_hover_text("Raw folder tree — every file under the root")
                        .clicked()
                    {
                        self.explorer_mode = ExplorerMode::Files;
                    }
                });
                ui.horizontal(|ui| {
                    let busy = self.ai_rx.is_some();
                    if ui
                        .add_enabled(!busy, egui::Button::new("⟳ Scan"))
                        .on_hover_text("Re-index the folder — picks up new/edited notes (no model)")
                        .clicked()
                    {
                        rescan = true;
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new("✨ Scan + AI"))
                        .on_hover_text(
                            "Re-index + run enrichment & embeddings on changed notes \
                             (needs the enrich build + a model server)",
                        )
                        .clicked()
                    {
                        ai_scan = true;
                    }
                    if busy {
                        ui.spinner();
                        ui.weak("enriching…");
                    }
                });
                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::RED, format!("index error: {err}"));
                    return (false, None, rescan, ai_scan, file_click);
                }

                // Build the filesystem tree lazily when the Files view is shown.
                if self.explorer_mode == ExplorerMode::Files && self.file_tree.is_none() {
                    self.file_tree = Some(build_file_tree(&self.root));
                }

                // The filter box drives the Ideas view only.
                let mut filter_changed = false;
                if self.explorer_mode == ExplorerMode::Ideas {
                    filter_changed = ui
                        .add(egui::TextEdit::singleline(&mut self.filter).hint_text("filter…"))
                        .changed();
                }
                ui.separator();

                let mut clicked = None;
                let reveal = self.reveal_selected; // consumed below; one-shot
                egui::ScrollArea::vertical().show(ui, |ui| match self.explorer_mode {
                    ExplorerMode::Files => match &self.file_tree {
                        Some(tree) if !tree.dirs.is_empty() || !tree.files.is_empty() => {
                            render_file_tree(ui, tree, &self.selected, &mut file_click, reveal);
                        }
                        _ => {
                            ui.weak(format!("{} is empty", self.root.display()));
                        }
                    },
                    ExplorerMode::Ideas => {
                        if self.filter.trim().is_empty() {
                            if self.tree.dirs.is_empty() && self.tree.files.is_empty() {
                                ui.weak(format!("no notes in {}", self.root.display()));
                            } else {
                                render_tree(ui, &self.tree, &self.selected, &mut clicked, reveal);
                            }
                        } else if self.results.is_empty() {
                            ui.weak("no matches");
                        } else {
                            for hit in &self.results {
                                let selected = self.selected.as_deref() == Some(hit.id.as_str());
                                let text = egui::RichText::new(&hit.title)
                                    .color(status_color(hit.status));
                                if ui.selectable_label(selected, text).clicked() {
                                    clicked = Some(hit.id.clone());
                                }
                            }
                        }
                    }
                });
                self.reveal_selected = false; // pulse consumed for this render
                (filter_changed, clicked, rescan, ai_scan, file_click)
            });
        let (filter_changed, clicked, rescan, ai_scan, file_click) = left.inner;
        if rescan {
            self.rescan();
        }
        if ai_scan {
            self.start_ai_scan();
        }
        if filter_changed {
            self.run_filter();
        }
        if let Some(id) = clicked {
            self.select(id);
        }
        if let Some((path, id)) = file_click {
            self.open_file(path, id);
        }

        // --- right: info (the GUI counterpart of `show`) ---
        let right = egui::Panel::right("info")
            .resizable(true)
            .default_size(300.0)
            .show_inside(ui, |ui| {
                ui.heading("Info");
                ui.separator();
                let mut clicked = None;
                let mut new_status = None;
                let mut new_tags: Option<Vec<String>> = None;
                let mut mention_accept: Option<query::Hit> = None;
                match &self.selected_idea {
                    None => {
                        ui.weak("(select a note)");
                    }
                    Some(idea) => {
                        ui.horizontal(|ui| {
                            ui.strong("status");
                            egui::ComboBox::from_id_salt("info_status")
                                .selected_text(
                                    egui::RichText::new(idea.status.value.as_str())
                                        .color(status_color(idea.status.value)),
                                )
                                .show_ui(ui, |ui| {
                                    for s in STATUS_CHOICES {
                                        if ui
                                            .selectable_label(idea.status.value == s, s.as_str())
                                            .clicked()
                                        {
                                            new_status = Some(s);
                                        }
                                    }
                                });
                            prov_badge(ui, idea.status.source);
                        });
                        if let Some(date) = idea.last_reviewed {
                            ui.label(format!("reviewed:  {date}"));
                        }
                        ui.label(format!("modified:  {}", idea.mtime.format("%Y-%m-%d")));

                        if let Some(summary) = &idea.summary {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.strong("summary");
                                prov_badge(ui, summary.source);
                            });
                            ui.label(&summary.value);
                        }

                        ui.add_space(6.0);
                        ui.strong("tags");
                        let current_asserted = asserted_tags(idea);
                        ui.horizontal_wrapped(|ui| {
                            for t in &idea.tags {
                                match t.source {
                                    Provenance::Asserted => {
                                        ui.label(&t.value);
                                        if ui
                                            .small_button("×")
                                            .on_hover_text("remove tag")
                                            .clicked()
                                        {
                                            new_tags = Some(
                                                current_asserted
                                                    .iter()
                                                    .filter(|v| *v != &t.value)
                                                    .cloned()
                                                    .collect(),
                                            );
                                        }
                                    }
                                    Provenance::Proposed => {
                                        ui.colored_label(PROPOSED, format!("~{}", t.value));
                                        if ui
                                            .small_button("✓")
                                            .on_hover_text("accept (make asserted)")
                                            .clicked()
                                        {
                                            let mut v = current_asserted.clone();
                                            v.push(t.value.clone());
                                            new_tags = Some(v);
                                        }
                                    }
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.tag_input)
                                    .hint_text("add tag")
                                    .desired_width(110.0),
                            );
                            let submit = resp.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if (ui.small_button("+").clicked() || submit)
                                && !self.tag_input.trim().is_empty()
                            {
                                let mut v = current_asserted.clone();
                                let tag = self.tag_input.trim().to_string();
                                if !v.contains(&tag) {
                                    v.push(tag);
                                }
                                new_tags = Some(v);
                                self.tag_input.clear();
                            }
                        });
                        if !idea.topics.is_empty() {
                            ui.add_space(6.0);
                            ui.strong("topics");
                            ui.horizontal_wrapped(|ui| {
                                for topic in &idea.topics {
                                    ui.weak(topic);
                                }
                            });
                        }

                        ui.add_space(10.0);
                        ui.separator();
                        ui.strong("Related");
                        if self.related.is_empty() {
                            ui.weak("none");
                        } else {
                            for h in &self.related {
                                let how = h.snippet.as_deref().unwrap_or("");
                                let text = egui::RichText::new(format!("{}  ({how})", h.title))
                                    .color(status_color(h.status));
                                if ui.selectable_label(false, text).clicked() {
                                    clicked = Some(h.id.clone());
                                }
                            }
                        }

                        if !self.backlinks.is_empty() {
                            ui.add_space(10.0);
                            ui.separator();
                            ui.strong("Backlinks");
                            for h in &self.backlinks {
                                let text = egui::RichText::new(format!("{}  (links here)", h.title))
                                    .color(status_color(h.status));
                                if ui.selectable_label(false, text).clicked() {
                                    clicked = Some(h.id.clone());
                                }
                            }
                        }

                        if !self.mentions.is_empty() {
                            ui.add_space(10.0);
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.strong("Unlinked mentions");
                                ui.weak("(accept → link)");
                            });
                            for h in &self.mentions {
                                ui.horizontal(|ui| {
                                    if ui
                                        .small_button("🔗")
                                        .on_hover_text("Accept: write a link to this note into that one")
                                        .clicked()
                                    {
                                        mention_accept = Some(h.clone());
                                    }
                                    let text = egui::RichText::new(&h.title)
                                        .color(status_color(h.status));
                                    if ui.selectable_label(false, text).clicked() {
                                        clicked = Some(h.id.clone());
                                    }
                                });
                            }
                        }

                        ui.add_space(10.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.strong("Near");
                            ui.weak("(semantic)");
                        });
                        if self.near.is_empty() {
                            ui.weak("none — run `index --embed`");
                        } else {
                            for h in &self.near {
                                let how = h.snippet.as_deref().unwrap_or("");
                                let text = egui::RichText::new(format!("{}  · {how}", h.title))
                                    .color(status_color(h.status));
                                if ui.selectable_label(false, text).clicked() {
                                    clicked = Some(h.id.clone());
                                }
                            }
                        }
                    }
                }
                (clicked, new_status, new_tags, mention_accept)
            });
        let (clicked, new_status, new_tags, mention_accept) = right.inner;
        if let Some(s) = new_status {
            self.set_selected_status(s);
        }
        if let Some(tags) = new_tags {
            self.apply_tags(tags);
        }
        if let Some(hit) = mention_accept {
            self.accept_mention(hit);
        }
        if let Some(id) = clicked {
            self.select(id);
        }

        // --- centre: editor / graph ---
        let central = egui::CentralPanel::default().show_inside(ui, |ui| {
            let mut save_requested = false;
            let mut action: Option<GraphAction> = None;
            let mut ask_select: Option<String> = None;

            ui.horizontal(|ui| {
                if ui.selectable_label(self.mode == Mode::View, "View").clicked() {
                    self.mode = Mode::View;
                }
                if ui.selectable_label(self.mode == Mode::Edit, "Edit").clicked() {
                    self.mode = Mode::Edit;
                }
                if ui.selectable_label(self.mode == Mode::Graph, "Graph").clicked() {
                    self.mode = Mode::Graph;
                }
                if ui.selectable_label(self.mode == Mode::Ask, "Ask").clicked() {
                    self.mode = Mode::Ask;
                }
                ui.separator();
                match self.mode {
                    Mode::Graph => {
                        ui.checkbox(&mut self.show_gaps, "Gaps")
                            .on_hover_text("Highlight orphans and candidate bridges");
                        ui.separator();
                        if self.show_gaps {
                            ui.weak("drag a node · click a dashed bridge to propose an idea");
                        } else {
                            ui.weak("scroll = zoom · drag = pan · click a node");
                        }
                    }
                    Mode::Ask => {
                        ui.weak("ask a question answered from your notes (RAG)");
                    }
                    _ => {
                        let dirty = self.dirty();
                        if ui
                            .add_enabled(dirty, egui::Button::new("Save"))
                            .on_hover_text("Write the file and re-index (Ctrl+S)")
                            .clicked()
                        {
                            save_requested = true;
                        }
                        if dirty {
                            ui.colored_label(egui::Color32::from_rgb(225, 200, 110), "● unsaved");
                        }
                        if let Some(msg) = &self.status_msg {
                            ui.weak(msg);
                        }
                    }
                }
            });
            ui.separator();

            if self.mode == Mode::Graph {
                action = self.graph_ui(ui);
            } else if self.mode == Mode::Ask {
                ask_select = self.ask_ui(ui);
            } else if self.selected_idea.is_none() {
                ui.weak("Select a note from the left.");
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| match self.mode {
                    Mode::Edit => {
                        ui.add_sized(
                            egui::vec2(ui.available_width(), ui.available_height().max(400.0)),
                            egui::TextEdit::multiline(&mut self.buffer)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    }
                    // View (and the unreachable Graph/Ask arms)
                    _ => {
                        CommonMarkViewer::new().show(ui, &mut self.md_cache, &self.buffer);
                    }
                });
                if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                    save_requested = true;
                }
            }
            (save_requested, action, ask_select)
        });
        let (save_requested, action, ask_select) = central.inner;
        if save_requested && self.dirty() {
            self.save();
        }
        match action {
            Some(GraphAction::Select(id)) => self.select(id),
            Some(GraphAction::Bridge(a, b)) => self.start_bridge(&a, &b),
            None => {}
        }
        if let Some(id) = ask_select {
            self.select(id);
        }

        // Background workers: AI scan + bridge proposal + ask.
        self.poll_ai_scan(ui.ctx());
        self.poll_bridge(ui.ctx());
        self.poll_ask(ui.ctx());
        self.bridge_window(ui.ctx());
        self.quick_switcher(ui.ctx());
    }
}

/// Distance from point `p` to the segment `a`–`b`.
fn dist_to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 <= f32::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Edge colour by relationship kind, faded by weight.
fn edge_color(kind: EdgeKind, weight: f32) -> egui::Color32 {
    let a = (60.0 + weight * 150.0).clamp(40.0, 230.0) as u8;
    match kind {
        EdgeKind::Link => egui::Color32::from_rgba_unmultiplied(205, 205, 215, 210),
        EdgeKind::Tag => egui::Color32::from_rgba_unmultiplied(120, 200, 140, a),
        EdgeKind::Semantic => egui::Color32::from_rgba_unmultiplied(120, 150, 215, a),
    }
}

/// Index of the node whose screen position is nearest `p`, within a small
/// pixel radius.
fn node_at(
    g: &RelGraph,
    layout: &[egui::Pos2],
    to_screen: impl Fn(egui::Pos2) -> egui::Pos2,
    p: egui::Pos2,
) -> Option<usize> {
    let mut best = 18.0;
    let mut found = None;
    for i in 0..g.nodes.len() {
        let d = to_screen(layout[i]).distance(p);
        if d < best {
            best = d;
            found = Some(i);
        }
    }
    found
}

/// Draw a node label just to the right of its position.
fn draw_label(painter: &egui::Painter, pos: egui::Pos2, text: &str) {
    painter.text(
        pos + egui::vec2(9.0, 0.0),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(12.0),
        egui::Color32::from_gray(220),
    );
}

/// Render the folder tree: collapsing headers for directories, status-tinted
/// selectable labels for notes. Sets `clicked` to a note id when one is clicked.
fn render_tree(
    ui: &mut egui::Ui,
    tree: &Tree,
    selected: &Option<String>,
    clicked: &mut Option<String>,
    reveal: bool,
) {
    let sel = selected.as_deref();
    for (name, sub) in &tree.dirs {
        // On a reveal pulse, force-open the folders on the path to the selection
        // so a node picked elsewhere (e.g. the graph) becomes visible here.
        let mut header = egui::CollapsingHeader::new(name).default_open(false);
        if reveal && sel.is_some_and(|id| tree_contains(sub, id)) {
            header = header.open(Some(true));
        }
        header.show(ui, |ui| render_tree(ui, sub, selected, clicked, reveal));
    }
    for f in &tree.files {
        let is_selected = sel == Some(f.id.as_str());
        let text = egui::RichText::new(&f.title).color(status_color(f.status));
        let resp = ui.selectable_label(is_selected, text);
        if is_selected && reveal {
            resp.scroll_to_me(Some(egui::Align::Center));
        }
        if resp.clicked() {
            *clicked = Some(f.id.clone());
        }
    }
}

/// Subsequence fuzzy score for the quick switcher (F-017): `Some(score)` if every
/// char of `q` (already lowercased) appears in order in `text` (already
/// lowercased), higher being a better match; `None` if it doesn't match.
/// Rewards earlier and contiguous matches.
fn fuzzy_score(q: &str, text: &str) -> Option<i32> {
    if q.is_empty() {
        return Some(0);
    }
    let mut score = 0i32;
    let mut last: Option<usize> = None;
    let mut chars = text.char_indices();
    for qc in q.chars() {
        loop {
            let (idx, tc) = chars.next()?;
            if tc == qc {
                if last.is_some_and(|p| idx == p + 1) {
                    score += 5; // contiguous run bonus
                }
                score += 50 - idx.min(50) as i32; // earlier-is-better
                last = Some(idx);
                break;
            }
        }
    }
    Some(score)
}

/// Whether the indexed-note subtree contains the note with this id.
fn tree_contains(tree: &Tree, id: &str) -> bool {
    tree.files.iter().any(|f| f.id == id) || tree.dirs.values().any(|s| tree_contains(s, id))
}

/// Render the raw **Files** tree (F-025). `.md` files are clickable (open the
/// note); other files are shown dimmed and inert. Sets `click` to the picked
/// file's `(path, id)`.
fn render_file_tree(
    ui: &mut egui::Ui,
    tree: &FileTree,
    selected: &Option<String>,
    click: &mut Option<(PathBuf, Option<String>)>,
    reveal: bool,
) {
    let sel = selected.as_deref();
    for (name, sub) in &tree.dirs {
        let mut header = egui::CollapsingHeader::new(format!("🗀 {name}")).default_open(false);
        if reveal && sel.is_some_and(|id| file_tree_contains(sub, id)) {
            header = header.open(Some(true));
        }
        header.show(ui, |ui| render_file_tree(ui, sub, selected, click, reveal));
    }
    for f in &tree.files {
        match &f.id {
            Some(id) => {
                let is_selected = sel == Some(id.as_str());
                let resp = ui.selectable_label(is_selected, &f.name);
                if is_selected && reveal {
                    resp.scroll_to_me(Some(egui::Align::Center));
                }
                if resp.clicked() {
                    *click = Some((f.path.clone(), Some(id.clone())));
                }
            }
            // Non-note files: visible but inert (a click still opens them raw).
            None => {
                if ui
                    .add(egui::Label::new(egui::RichText::new(&f.name).weak()).sense(egui::Sense::click()))
                    .clicked()
                {
                    *click = Some((f.path.clone(), None));
                }
            }
        }
    }
}

/// Whether the filesystem subtree contains an indexed `.md` with this id.
fn file_tree_contains(tree: &FileTree, id: &str) -> bool {
    tree.files.iter().any(|f| f.id.as_deref() == Some(id))
        || tree.dirs.values().any(|s| file_tree_contains(s, id))
}

/// A relative markdown-link target from `from_file`'s directory to `to_file`
/// (forward slashes, with `../` as needed) — what `link_target_to_id` resolves
/// back to the target note's id. Used when accepting an unlinked mention (F-016).
fn relative_md_link(from_file: &Path, to_file: &Path) -> String {
    let from_dir = from_file.parent().unwrap_or_else(|| Path::new(""));
    let f: Vec<_> = from_dir.components().collect();
    let t: Vec<_> = to_file.components().collect();
    let mut i = 0;
    while i < f.len() && i < t.len() && f[i] == t[i] {
        i += 1;
    }
    let mut rel = PathBuf::new();
    for _ in i..f.len() {
        rel.push("..");
    }
    for c in &t[i..] {
        rel.push(c.as_os_str());
    }
    let s = rel.to_string_lossy().replace('\\', "/");
    if s.is_empty() {
        to_file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        s
    }
}

/// Colour for proposed (model-inferred) values, kept visually distinct from
/// asserted ones — the GUI half of INV-2.
const PROPOSED: egui::Color32 = egui::Color32::from_rgb(225, 200, 110);

/// A small provenance flag shown next to a field (status, summary).
fn prov_badge(ui: &mut egui::Ui, source: Provenance) {
    match source {
        Provenance::Asserted => {
            ui.weak("(asserted)");
        }
        Provenance::Proposed => {
            ui.colored_label(PROPOSED, "(proposed)");
        }
    }
}

/// Per-status colour, mirroring the CLI's `owo-colors` tints.
fn status_color(status: Status) -> egui::Color32 {
    match status {
        Status::Concept => egui::Color32::from_rgb(110, 200, 220),
        Status::Draft => egui::Color32::from_rgb(120, 160, 235),
        Status::Active => egui::Color32::from_rgb(120, 210, 120),
        Status::Dormant => egui::Color32::from_rgb(225, 200, 110),
        Status::Complete => egui::Color32::from_rgb(150, 230, 150),
        Status::Superseded => egui::Color32::from_rgb(215, 140, 220),
        Status::Archived | Status::Unknown => egui::Color32::GRAY,
    }
}

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
use phanes::indexer::{self, IndexOptions};
use phanes::model::{Idea, Provenance, Status};
use phanes::query::{self, ListItem, SearchFilter};
use phanes::store::Store;

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
        let (idea, related, near) = match self.store.as_ref() {
            Some(s) => (
                query::get(s, &id).ok().flatten(),
                query::related(s, &id).unwrap_or_default(),
                query::near(s, &id, 8).unwrap_or_default(),
            ),
            None => (None, Vec::new(), Vec::new()),
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
        self.selected = Some(id);
        self.mode = Mode::View;
        self.status_msg = None;
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
        let (idea, related, near) = match self.selected.as_ref() {
            Some(id) => (
                query::get(store, id).ok().flatten(),
                query::related(store, id).unwrap_or_default(),
                query::near(store, id, 8).unwrap_or_default(),
            ),
            None => (None, Vec::new(), Vec::new()),
        };
        self.tree = tree;
        self.selected_idea = idea;
        self.related = related;
        self.near = near;
        self.graph = None; // rebuilt next time the Graph tab is opened
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
                ui.horizontal(|ui| {
                    ui.heading("Ideas");
                    if ui
                        .button("⟳ Scan")
                        .on_hover_text("Re-index the folder — picks up new or edited notes")
                        .clicked()
                    {
                        rescan = true;
                    }
                });
                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::RED, format!("index error: {err}"));
                    return (false, None, rescan);
                }

                let filter_changed = ui
                    .add(egui::TextEdit::singleline(&mut self.filter).hint_text("filter…"))
                    .changed();
                ui.separator();

                let mut clicked = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.filter.trim().is_empty() {
                        if self.tree.dirs.is_empty() && self.tree.files.is_empty() {
                            ui.weak(format!("no notes in {}", self.root.display()));
                        } else {
                            render_tree(ui, &self.tree, &self.selected, &mut clicked);
                        }
                    } else if self.results.is_empty() {
                        ui.weak("no matches");
                    } else {
                        for hit in &self.results {
                            let selected = self.selected.as_deref() == Some(hit.id.as_str());
                            let text =
                                egui::RichText::new(&hit.title).color(status_color(hit.status));
                            if ui.selectable_label(selected, text).clicked() {
                                clicked = Some(hit.id.clone());
                            }
                        }
                    }
                });
                (filter_changed, clicked, rescan)
            });
        let (filter_changed, clicked, rescan) = left.inner;
        if rescan {
            self.rescan();
        }
        if filter_changed {
            self.run_filter();
        }
        if let Some(id) = clicked {
            self.select(id);
        }

        // --- right: info (the GUI counterpart of `show`) ---
        let right = egui::Panel::right("info")
            .resizable(true)
            .default_size(300.0)
            .show_inside(ui, |ui| {
                ui.heading("Info");
                ui.separator();
                let mut clicked = None;
                match &self.selected_idea {
                    None => {
                        ui.weak("(select a note)");
                    }
                    Some(idea) => {
                        ui.horizontal(|ui| {
                            ui.strong("status");
                            ui.colored_label(
                                status_color(idea.status.value),
                                idea.status.value.as_str(),
                            );
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

                        if !idea.tags.is_empty() {
                            ui.add_space(6.0);
                            ui.strong("tags");
                            ui.horizontal_wrapped(|ui| {
                                for t in &idea.tags {
                                    match t.source {
                                        Provenance::Asserted => {
                                            ui.label(&t.value);
                                        }
                                        Provenance::Proposed => {
                                            ui.colored_label(PROPOSED, format!("~{}", t.value));
                                        }
                                    }
                                }
                            });
                        }
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
                clicked
            });
        if let Some(id) = right.inner {
            self.select(id);
        }

        // --- centre: editor / graph ---
        let central = egui::CentralPanel::default().show_inside(ui, |ui| {
            let mut save_requested = false;
            let mut action: Option<GraphAction> = None;

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
                ui.separator();
                if self.mode == Mode::Graph {
                    ui.checkbox(&mut self.show_gaps, "Gaps")
                        .on_hover_text("Highlight orphans and candidate bridges");
                    ui.separator();
                    if self.show_gaps {
                        ui.weak("drag a node · click a dashed bridge to propose an idea");
                    } else {
                        ui.weak("scroll = zoom · drag = pan · click a node");
                    }
                } else {
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
            });
            ui.separator();

            if self.mode == Mode::Graph {
                action = self.graph_ui(ui);
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
                    // View (and the unreachable Graph arm)
                    _ => {
                        CommonMarkViewer::new().show(ui, &mut self.md_cache, &self.buffer);
                    }
                });
                if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                    save_requested = true;
                }
            }
            (save_requested, action)
        });
        let (save_requested, action) = central.inner;
        if save_requested && self.dirty() {
            self.save();
        }
        match action {
            Some(GraphAction::Select(id)) => self.select(id),
            Some(GraphAction::Bridge(a, b)) => self.start_bridge(&a, &b),
            None => {}
        }

        // Background bridge worker + its floating result window.
        self.poll_bridge(ui.ctx());
        self.bridge_window(ui.ctx());
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
fn render_tree(ui: &mut egui::Ui, tree: &Tree, selected: &Option<String>, clicked: &mut Option<String>) {
    for (name, sub) in &tree.dirs {
        egui::CollapsingHeader::new(name)
            .default_open(false)
            .show(ui, |ui| render_tree(ui, sub, selected, clicked));
    }
    for f in &tree.files {
        let is_selected = selected.as_deref() == Some(f.id.as_str());
        let text = egui::RichText::new(&f.title).color(status_color(f.status));
        if ui.selectable_label(is_selected, text).clicked() {
            *clicked = Some(f.id.clone());
        }
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

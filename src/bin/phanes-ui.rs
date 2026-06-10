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
    // centre editor
    mode: Mode,
    buffer: String,       // raw file content of the selected note (editable)
    saved: String,        // last-saved content, for the dirty check
    md_cache: CommonMarkCache,
    status_msg: Option<String>,
}

impl PhanesApp {
    fn new(root: PathBuf) -> Self {
        let (store, error, tree) = match Store::open(&root.join(".phanes").join("index.db")) {
            Ok(mut store) => {
                // Index the folder on startup so the UI reflects the current
                // notes even on a never-indexed folder. Cheap: hash-gated, and
                // no enrichment (that stays opt-in, CLI-only).
                let opts = IndexOptions { enrich: false, force: false };
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
            mode: Mode::View,
            buffer: String::new(),
            saved: String::new(),
            md_cache: CommonMarkCache::default(),
            status_msg: None,
        }
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
        let (idea, related) = match self.store.as_ref() {
            Some(s) => (
                query::get(s, &id).ok().flatten(),
                query::related(s, &id).unwrap_or_default(),
            ),
            None => (None, Vec::new()),
        };
        self.buffer = match &idea {
            Some(i) => std::fs::read_to_string(&i.path)
                .unwrap_or_else(|e| format!("<could not read {}: {e}>", i.path.display())),
            None => String::new(),
        };
        self.saved = self.buffer.clone();
        self.selected_idea = idea;
        self.related = related;
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
            let opts = IndexOptions { enrich: false, force: false };
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
        let (idea, related) = match self.selected.as_ref() {
            Some(id) => (
                query::get(store, id).ok().flatten(),
                query::related(store, id).unwrap_or_default(),
            ),
            None => (None, Vec::new()),
        };
        self.tree = tree;
        self.selected_idea = idea;
        self.related = related;
    }
}

impl eframe::App for PhanesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // --- left: explorer ---
        let left = egui::Panel::left("explorer")
            .resizable(true)
            .default_size(260.0)
            .show_inside(ui, |ui| {
                ui.heading("Ideas");
                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::RED, format!("index error: {err}"));
                    return (false, None);
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
                (filter_changed, clicked)
            });
        let (filter_changed, clicked) = left.inner;
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
                    }
                }
                clicked
            });
        if let Some(id) = right.inner {
            self.select(id);
        }

        // --- centre: editor ---
        let central = egui::CentralPanel::default().show_inside(ui, |ui| {
            let mut save_requested = false;
            if self.selected_idea.is_none() {
                ui.weak("Select a note from the left.");
                return false;
            }

            ui.horizontal(|ui| {
                let view = self.mode == Mode::View;
                if ui.selectable_label(view, "View").clicked() {
                    self.mode = Mode::View;
                }
                if ui.selectable_label(!view, "Edit").clicked() {
                    self.mode = Mode::Edit;
                }
                ui.separator();
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
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| match self.mode {
                Mode::View => {
                    CommonMarkViewer::new().show(ui, &mut self.md_cache, &self.buffer);
                }
                Mode::Edit => {
                    ui.add_sized(
                        egui::vec2(ui.available_width(), ui.available_height().max(400.0)),
                        egui::TextEdit::multiline(&mut self.buffer)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                }
            });

            if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                save_requested = true;
            }
            save_requested
        });
        if central.inner && self.dirty() {
            self.save();
        }
    }
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

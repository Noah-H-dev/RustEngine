// ── Tileset editor ───────────────────────────────────────────────────────────
//
// Owns two related concerns, one per tab:
//
//   * Definitions — the abstract tile namespace. Create / rename / delete tile
//     ids and edit their tileset-independent properties (collision, folder).
//     Writes the shared `textures.toml` via `TileDefs`.
//
//   * Tileset — per-skin PNG assignment. The user picks (or creates) a tileset
//     file in `tilesets/` and assigns a PNG from `assets/` to each abstract
//     tile id. New tilesets start as a copy of whichever tileset is currently
//     selected, so making a variant ("forest_dark" from "forest") is cheap.
//
// The active tileset path is persisted in `Settings.active_tileset` so the
// selection survives restarts. Only one inline text input is ever open at a
// time (`InlineEdit`); naming a tile, naming a folder, and naming a new
// tileset all go through that single FSM.
//
// Pattern mirrors the other pure-egui contexts (settings.rs): the egui closure
// only writes into an `Intent`; all state mutation happens afterwards in
// `apply`, so there are no borrow conflicts with `tile_defs` or the engine.

use std::collections::HashMap;
use std::path::PathBuf;

use RustEngine::game::game_engine::{Engine, GameContext};

use super::menus::MainMenuContext;
use super::tiledefs::{Category, TileDefs};
use super::tileset::{Tileset, assets_pngs};

#[derive(Clone, Copy, PartialEq)]
enum EditorTab { Definitions, Tileset }

/// Single inline text input shared across tabs (tile name, folder name, or
/// new-tileset name). Only one is active at a time.
enum InlineEdit {
    Idle,
    DefName    { id: i32, buf: String },
    DefFolder  { id: i32, buf: String },
    NewTileset { buf: String, copy_from: Option<PathBuf> },
}

impl InlineEdit {
    fn is_active(&self) -> bool { !matches!(self, InlineEdit::Idle) }
    fn label(&self) -> &'static str {
        match self {
            InlineEdit::DefName    { .. } => "Tile name:",
            InlineEdit::DefFolder  { .. } => "Folder name:",
            InlineEdit::NewTileset { .. } => "New tileset name:",
            InlineEdit::Idle              => "",
        }
    }
    fn buf_clone(&self) -> String {
        match self {
            InlineEdit::DefName { buf, .. }
            | InlineEdit::DefFolder { buf, .. }
            | InlineEdit::NewTileset { buf, .. } => buf.clone(),
            InlineEdit::Idle => String::new(),
        }
    }
}

/// UI intents collected during one `draw`, applied afterward.
#[derive(Default)]
struct Intent {
    back:    bool,
    new_tab: Option<EditorTab>,

    // Definitions tab
    def_create:             bool,
    def_new_category:       Option<Category>,        // switch the active Definitions sub-tab
    def_set_category:       Option<(i32, Category)>, // move a def to another category
    def_toggle_solid:       Option<i32>,
    def_start_rename:       Option<i32>,
    def_start_folder:       Option<i32>,
    def_remove_from_folder: Option<i32>,
    def_delete:             Option<i32>,

    // Tileset tab
    select_tileset:     Option<PathBuf>,
    start_new_tileset:  bool,
    /// (tile id, Some(filename) = assign / None = clear)
    assign_png:         Option<(i32, Option<String>)>,
    /// Open rfd to pick a PNG from disk for this tile id.
    pick_png_from_disk: Option<i32>,

    // Shared inline editor
    confirm: bool,
    cancel:  bool,
}

pub struct TilesetEditorContext {
    tile_defs:       TileDefs,
    edit:            InlineEdit,
    active_tab:      EditorTab,
    /// Which category sub-tab is showing in the Definitions tab. New tiles are
    /// created into this category.
    def_category:    Category,
    /// Currently selected tileset, if any. None = no tileset picked yet.
    current_tileset: Option<Tileset>,
    pending:         Option<Box<dyn GameContext>>,
}

impl TilesetEditorContext {
    /// `active_tileset` is the persisted path from `Settings.active_tileset`.
    /// Tileset::ensure_default has already been called by the menu, so the
    /// file is expected to exist; we open with it as the current tileset.
    pub fn new(active_tileset: String) -> Self {
        let path = PathBuf::from(&active_tileset);
        let current = if path.exists() { Some(Tileset::load(&path)) } else { None };
        TilesetEditorContext {
            tile_defs:       TileDefs::load(),
            edit:            InlineEdit::Idle,
            active_tab:      EditorTab::Definitions,
            def_category:    Category::Tiles,
            current_tileset: current,
            pending:         None,
        }
    }

    /// Apply collected intents to state, persisting on every change. `engine`
    /// is here so we can update `Settings.active_tileset` and save it when the
    /// selected tileset changes.
    fn apply(&mut self, intent: Intent, edit_buf: String, engine: &mut Engine) {
        // Mirror inline text buffer back into the FSM.
        match &mut self.edit {
            InlineEdit::DefName    { buf, .. } => *buf = edit_buf,
            InlineEdit::DefFolder  { buf, .. } => *buf = edit_buf,
            InlineEdit::NewTileset { buf, .. } => *buf = edit_buf,
            InlineEdit::Idle => {}
        }

        if intent.back {
            self.pending = Some(Box::new(MainMenuContext::new()));
            return;
        }
        if let Some(tab) = intent.new_tab { self.active_tab = tab; }

        // ── Definitions tab ──────────────────────────────────────────────────
        if let Some(cat) = intent.def_new_category { self.def_category = cat; }
        if intent.def_create {
            // New tiles land in whichever category sub-tab is open.
            let id = self.tile_defs.create(None, self.def_category);
            self.tile_defs.save();
            self.edit = InlineEdit::DefName { id, buf: String::new() };
        }
        if let Some((id, cat)) = intent.def_set_category {
            self.tile_defs.set_category(id, cat);
            self.tile_defs.save();
        }
        if let Some(id) = intent.def_toggle_solid {
            let new = !self.tile_defs.solid_of(id);
            self.tile_defs.set_solid(id, new);
            self.tile_defs.save();
        }
        if let Some(id) = intent.def_start_rename {
            let buf = self.tile_defs.name_of(id).unwrap_or_default();
            self.edit = InlineEdit::DefName { id, buf };
        }
        if let Some(id) = intent.def_start_folder {
            let buf = self.tile_defs.folder_of(id).unwrap_or_default();
            self.edit = InlineEdit::DefFolder { id, buf };
        }
        if let Some(id) = intent.def_remove_from_folder {
            self.tile_defs.set_folder(id, None);
            self.tile_defs.save();
        }
        if let Some(id) = intent.def_delete {
            if self.tile_defs.remove(id) { self.tile_defs.save(); }
            // Also clear this id from the current tileset — keeping a PNG
            // mapping for a non-existent definition would dangle.
            if let Some(ts) = &mut self.current_tileset {
                if ts.png_of(id).is_some() {
                    ts.set_png(id, None);
                    ts.save();
                }
            }
            // Drop an open inline editor that targeted the now-gone tile.
            if matches!(&self.edit,
                InlineEdit::DefName { id: e, .. } | InlineEdit::DefFolder { id: e, .. } if *e == id)
            {
                self.edit = InlineEdit::Idle;
            }
        }

        // ── Tileset tab ──────────────────────────────────────────────────────
        if let Some(path) = intent.select_tileset {
            self.current_tileset = Some(Tileset::load(&path));
            engine.settings.active_tileset = path.to_string_lossy().into_owned();
            engine.settings.save();
        }
        if intent.start_new_tileset {
            // New tilesets start as a copy of the currently-selected one (if any).
            let copy_from = self.current_tileset.as_ref().map(|t| t.path.clone());
            self.edit = InlineEdit::NewTileset { buf: String::new(), copy_from };
        }
        if let Some((id, png)) = intent.assign_png {
            if let Some(ts) = &mut self.current_tileset {
                ts.set_png(id, png);
                ts.save();
            }
        }
        if let Some(id) = intent.pick_png_from_disk {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG Image", &["png"])
                .set_directory("assets")
                .pick_file()
            {
                // Copy into assets/ if not already there, then assign filename.
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !name.is_empty() {
                    let dest = std::path::Path::new("assets").join(&name);
                    if !dest.exists() {
                        let _ = std::fs::copy(&path, &dest);
                    }
                    if let Some(ts) = &mut self.current_tileset {
                        ts.set_png(id, Some(name));
                        ts.save();
                    }
                }
            }
        }

        // ── Shared inline editor confirm / cancel ────────────────────────────
        if intent.cancel { self.edit = InlineEdit::Idle; }
        if intent.confirm {
            match std::mem::replace(&mut self.edit, InlineEdit::Idle) {
                InlineEdit::DefName { id, buf } => {
                    let name = buf.trim();
                    self.tile_defs.set_name(id, if name.is_empty() { None } else { Some(name.to_string()) });
                    self.tile_defs.save();
                }
                InlineEdit::DefFolder { id, buf } => {
                    let folder = buf.trim();
                    self.tile_defs.set_folder(id, if folder.is_empty() { None } else { Some(folder.to_string()) });
                    self.tile_defs.save();
                }
                InlineEdit::NewTileset { buf, copy_from } => {
                    let name = buf.trim();
                    if !name.is_empty() {
                        let source = copy_from.as_ref().map(Tileset::load);
                        let ts = Tileset::create(name, source.as_ref());
                        ts.save();
                        engine.settings.active_tileset = ts.path.to_string_lossy().into_owned();
                        engine.settings.save();
                        self.current_tileset = Some(ts);
                        // Hop into the Tileset tab so you're already where you can
                        // start assigning PNGs.
                        self.active_tab = EditorTab::Tileset;
                    }
                }
                InlineEdit::Idle => {}
            }
        }
    }
}

/// Render one row in the Definitions tab.
fn def_row(ui: &mut egui::Ui, def: &super::tiledefs::TileDef, intent: &mut Intent) {
    ui.horizontal(|ui| {
        let name = def.name.as_deref().unwrap_or("(unnamed)");
        ui.label(format!("{:>3} | {}", def.id, name));
        let solid_label = if def.solid { "Solid" } else { "Passable" };
        if ui.button(solid_label).clicked() { intent.def_toggle_solid = Some(def.id); }
        match &def.folder {
            Some(f) => { ui.weak(format!("[{}]", f)); }
            None    => { ui.weak("(no folder)"); }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button("...", |ui| {
                if ui.button("Rename").clicked()     { intent.def_start_rename = Some(def.id); ui.close(); }
                if ui.button("Set folder").clicked() { intent.def_start_folder = Some(def.id); ui.close(); }
                if def.folder.is_some() && ui.button("Remove from folder").clicked() {
                    intent.def_remove_from_folder = Some(def.id);
                    ui.close();
                }
                ui.menu_button("Move to category", |ui| {
                    for cat in Category::ALL {
                        if cat == def.category { continue; }
                        if ui.button(cat.label()).clicked() {
                            intent.def_set_category = Some((def.id, cat));
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui.button("Delete").clicked() { intent.def_delete = Some(def.id); ui.close(); }
            });
        });
    });
}

/// Render one row in the Tileset tab — shows the PNG assigned for this tile
/// in the currently-selected tileset, with an Assign PNG submenu.
fn skin_row(
    ui: &mut egui::Ui,
    def: &super::tiledefs::TileDef,
    current_png: Option<&str>,
    assets: &[String],
    intent: &mut Intent,
) {
    ui.horizontal(|ui| {
        let name = def.name.as_deref().unwrap_or("(unnamed)");
        ui.label(format!("{:>3} | {}", def.id, name));
        match current_png {
            Some(p) => { ui.label(format!("PNG: {}", p)); }
            None    => { ui.weak("(unassigned)"); }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button("...", |ui| {
                ui.menu_button("Assign PNG", |ui| {
                    if assets.is_empty() {
                        ui.weak("(no PNGs in assets/)");
                    } else {
                        for asset in assets {
                            let selected = current_png == Some(asset.as_str());
                            if ui.selectable_label(selected, asset).clicked() {
                                intent.assign_png = Some((def.id, Some(asset.clone())));
                                ui.close();
                            }
                        }
                    }
                    ui.separator();
                    if ui.button("Pick from disk...").clicked() {
                        intent.pick_png_from_disk = Some(def.id);
                        ui.close();
                    }
                });
                if current_png.is_some() && ui.button("Clear assignment").clicked() {
                    intent.assign_png = Some((def.id, None));
                    ui.close();
                }
            });
        });
    });
}

impl GameContext for TilesetEditorContext {
    fn update(&mut self, _engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        self.pending.take()
    }

    fn draw(&mut self, engine: &mut Engine) {
        let mut intent = Intent::default();

        // Snapshots taken before the egui closure to avoid borrow conflicts.
        let editing      = self.edit.is_active();
        let edit_label   = self.edit.label();
        let mut edit_buf = self.edit.buf_clone();

        let defs         = self.tile_defs.sorted();
        let active_tab   = self.active_tab;
        let def_category = self.def_category;
        let tileset_name = self.current_tileset.as_ref().map(|t| t.name());
        let tileset_path = self.current_tileset.as_ref().map(|t| t.path.clone());
        let tileset_list = Tileset::list_in_dir();
        // Only scan assets/ when actually on the Tileset tab — saves work and
        // also avoids running it when no skin work is happening.
        let assets = if active_tab == EditorTab::Tileset { assets_pngs() } else { Vec::new() };
        // Snapshot current PNG assignments for the visible defs.
        let png_for: HashMap<i32, String> = self.current_tileset.as_ref()
            .map(|t| defs.iter()
                .filter_map(|d| t.png_of(d.id).map(|p| (d.id, p.to_string())))
                .collect())
            .unwrap_or_default();

        let (w, h) = engine.screen_size();
        let input  = engine.egui_input.clone();

        engine.renderer.render(input, w, h, |ctx| {
            egui::TopBottomPanel::top("tileset_top").show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("< Menu").clicked() { intent.back = true; }
                    ui.separator();

                    // Tab bar.
                    if ui.selectable_label(active_tab == EditorTab::Definitions, "Definitions").clicked() {
                        intent.new_tab = Some(EditorTab::Definitions);
                    }
                    if ui.selectable_label(active_tab == EditorTab::Tileset, "Tileset").clicked() {
                        intent.new_tab = Some(EditorTab::Tileset);
                    }
                    ui.separator();

                    // Per-tab toolbar action.
                    match active_tab {
                        EditorTab::Definitions => {
                            // Category sub-tabs — Tiles / Creatures / Objects.
                            for cat in Category::ALL {
                                if ui.selectable_label(def_category == cat, cat.label()).clicked() {
                                    intent.def_new_category = Some(cat);
                                }
                            }
                            ui.separator();
                            if ui.button("+ New Tile").clicked() { intent.def_create = true; }
                            ui.separator();
                            let count = defs.iter().filter(|d| d.category == def_category).count();
                            ui.label(format!("{} in {}", count, def_category.label()));
                        }
                        EditorTab::Tileset => {
                            let label = match &tileset_name {
                                Some(n) => format!("Tileset: {}", n),
                                None    => "Tileset: (none)".to_string(),
                            };
                            ui.menu_button(label, |ui| {
                                for path in &tileset_list {
                                    let name = path.file_stem()
                                        .map(|s| s.to_string_lossy().into_owned())
                                        .unwrap_or_default();
                                    let selected = tileset_path.as_ref() == Some(path);
                                    if ui.selectable_label(selected, name).clicked() {
                                        intent.select_tileset = Some(path.clone());
                                        ui.close();
                                    }
                                }
                                if !tileset_list.is_empty() { ui.separator(); }
                                if ui.button("New tileset...").clicked() {
                                    intent.start_new_tileset = true;
                                    ui.close();
                                }
                            });
                        }
                    }
                });
                ui.add_space(6.0);
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                // Shared inline editor.
                if editing {
                    ui.add_space(6.0);
                    ui.label(edit_label);
                    let resp = ui.text_edit_singleline(&mut edit_buf);
                    ui.horizontal(|ui| {
                        if ui.button("Confirm").clicked()
                            || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            intent.confirm = true;
                        }
                        if ui.button("Cancel").clicked() { intent.cancel = true; }
                    });
                    ui.separator();
                }

                match active_tab {
                    EditorTab::Definitions => {
                        let in_category: Vec<&super::tiledefs::TileDef> =
                            defs.iter().filter(|d| d.category == def_category).collect();
                        if in_category.is_empty() {
                            ui.add_space(12.0);
                            ui.weak(format!(
                                "No {} yet - click \"+ New Tile\" to define one.",
                                def_category.label().to_lowercase(),
                            ));
                            return;
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for def in &in_category {
                                def_row(ui, def, &mut intent);
                            }
                        });
                    }
                    EditorTab::Tileset => {
                        if tileset_name.is_none() {
                            ui.add_space(12.0);
                            ui.weak("No tileset selected. Pick one from the menu above, or create a new one.");
                            return;
                        }
                        if defs.is_empty() {
                            ui.add_space(12.0);
                            ui.weak("No tile definitions to skin. Switch to Definitions tab to add tiles first.");
                            return;
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for def in &defs {
                                let current = png_for.get(&def.id).map(String::as_str);
                                skin_row(ui, def, current, &assets, &mut intent);
                            }
                        });
                    }
                }
            });
        });

        self.apply(intent, edit_buf, engine);
    }
}

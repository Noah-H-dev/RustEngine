// ── Tileset editor: tile-definitions manager ─────────────────────────────────
//
// First slice of the planned tileset editor. It owns the *abstract tile
// namespace*: creating, naming, and deleting tile ids, and editing the
// tileset-independent properties (collision, folder). It writes the shared
// `textures.toml` via `TileDefs` — the same model the map editor reads, so the
// two never clobber each other's fields.
//
// NOT yet here (deliberately left open): per-tileset PNG/skin assignment
// (id→png, today `id.txt`). Each row leaves visual room for it; see the "png"
// note in `tile_row`. When added, it edits a *tileset* file, not these defs.
//
// Pattern mirrors the other pure-egui contexts (settings.rs): the egui closure
// only writes into a `DefIntent`; all state mutation happens afterwards in
// `apply`, so there are no borrow conflicts with `tile_defs`.

use RustEngine::game::game_engine::{Engine, GameContext};

use super::menus::MainMenuContext;
use super::tiledefs::TileDefs;

/// Inline single-active text editor (top of panel), for naming a tile or
/// setting its folder. Only one row is edited at a time — mirrors the map
/// editor's folder-input FSM rather than juggling a buffer per row.
enum DefEdit {
    Idle,
    Name   { id: i32, buf: String },
    Folder { id: i32, buf: String },
}

impl DefEdit {
    fn is_active(&self) -> bool { !matches!(self, DefEdit::Idle) }
}

/// UI intents collected during one `draw`, applied afterward. Default = nothing.
#[derive(Default)]
struct DefIntent {
    back:               bool,
    create:             bool,
    toggle_solid:       Option<i32>,
    start_rename:       Option<i32>,
    start_folder:       Option<i32>,
    remove_from_folder: Option<i32>,
    delete:             Option<i32>,
    confirm:            bool,
    cancel:             bool,
}

pub struct TilesetEditorContext {
    tile_defs: TileDefs,
    edit:      DefEdit,
    pending:   Option<Box<dyn GameContext>>,
}

impl TilesetEditorContext {
    pub fn new() -> Self {
        TilesetEditorContext {
            tile_defs: TileDefs::load(),
            edit:      DefEdit::Idle,
            pending:   None,
        }
    }

    /// Apply collected intents to state, persisting on every change.
    fn apply(&mut self, intent: DefIntent, edit_buf: String) {
        // Mirror the inline text input back into the FSM.
        match &mut self.edit {
            DefEdit::Name   { buf, .. } => *buf = edit_buf,
            DefEdit::Folder { buf, .. } => *buf = edit_buf,
            DefEdit::Idle => {}
        }

        if intent.back {
            self.pending = Some(Box::new(MainMenuContext::new()));
            return;
        }
        if intent.create {
            // Allocate a new abstract id and immediately open its name editor.
            let id = self.tile_defs.create(None);
            self.tile_defs.save();
            self.edit = DefEdit::Name { id, buf: String::new() };
        }
        if let Some(id) = intent.toggle_solid {
            let new = !self.tile_defs.solid_of(id);
            self.tile_defs.set_solid(id, new);
            self.tile_defs.save();
        }
        if let Some(id) = intent.start_rename {
            let buf = self.tile_defs.name_of(id).unwrap_or_default();
            self.edit = DefEdit::Name { id, buf };
        }
        if let Some(id) = intent.start_folder {
            let buf = self.tile_defs.folder_of(id).unwrap_or_default();
            self.edit = DefEdit::Folder { id, buf };
        }
        if let Some(id) = intent.remove_from_folder {
            self.tile_defs.set_folder(id, None);
            self.tile_defs.save();
        }
        if let Some(id) = intent.delete {
            if self.tile_defs.remove(id) { self.tile_defs.save(); }
            // Drop an edit input targeting the now-gone tile.
            if matches!(&self.edit, DefEdit::Name { id: e, .. } | DefEdit::Folder { id: e, .. } if *e == id) {
                self.edit = DefEdit::Idle;
            }
        }
        if intent.cancel {
            self.edit = DefEdit::Idle;
        }
        if intent.confirm {
            match std::mem::replace(&mut self.edit, DefEdit::Idle) {
                DefEdit::Name { id, buf } => {
                    let name = buf.trim();
                    self.tile_defs.set_name(id, if name.is_empty() { None } else { Some(name.to_string()) });
                    self.tile_defs.save();
                }
                DefEdit::Folder { id, buf } => {
                    let folder = buf.trim();
                    self.tile_defs.set_folder(id, if folder.is_empty() { None } else { Some(folder.to_string()) });
                    self.tile_defs.save();
                }
                DefEdit::Idle => {}
            }
        }
    }
}

/// Render one tile-definition row, emitting intents only.
fn tile_row(ui: &mut egui::Ui, def: &super::tiledefs::TileDef, intent: &mut DefIntent) {
    ui.horizontal(|ui| {
        let name = def.name.as_deref().unwrap_or("(unnamed)");
        ui.label(format!("{:>3} | {}", def.id, name));

        // Quick collision toggle.
        let solid_label = if def.solid { "Solid" } else { "Passable" };
        if ui.button(solid_label).clicked() { intent.toggle_solid = Some(def.id); }

        // Folder indicator.
        match &def.folder {
            Some(f) => { ui.weak(format!("[{}]", f)); }
            None    => { ui.weak("(no folder)"); }
        }

        // Per-tileset PNG/skin lives elsewhere; placeholder marks where it'll go.
        ui.weak("png: -");

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button("...", |ui| {
                if ui.button("Rename").clicked()     { intent.start_rename = Some(def.id); ui.close(); }
                if ui.button("Set folder").clicked() { intent.start_folder = Some(def.id); ui.close(); }
                if def.folder.is_some() && ui.button("Remove from folder").clicked() {
                    intent.remove_from_folder = Some(def.id);
                    ui.close();
                }
                ui.separator();
                if ui.button("Delete").clicked() { intent.delete = Some(def.id); ui.close(); }
            });
        });
    });
}

impl GameContext for TilesetEditorContext {
    fn update(&mut self, _engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        self.pending.take()
    }

    fn draw(&mut self, engine: &mut Engine) {
        let mut intent = DefIntent::default();

        // Snapshot the active inline editor's buffer for the text field.
        let editing = self.edit.is_active();
        let (edit_label, mut edit_buf) = match &self.edit {
            DefEdit::Name   { .. }  => ("Tile name:",   self.edit_buf_clone()),
            DefEdit::Folder { .. }  => ("Folder name:", self.edit_buf_clone()),
            DefEdit::Idle           => ("",             String::new()),
        };

        let defs = self.tile_defs.sorted();
        let (w, h) = engine.screen_size();
        let input  = engine.egui_input.clone();

        engine.renderer.render(input, w, h, |ctx| {
            egui::TopBottomPanel::top("tileset_top").show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("< Menu").clicked() { intent.back = true; }
                    ui.separator();
                    ui.heading("Tile Definitions");
                    ui.separator();
                    if ui.button("+ New Tile").clicked() { intent.create = true; }
                    ui.separator();
                    ui.label(format!("{} tiles", defs.len()));
                });
                ui.add_space(6.0);
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                // Inline name / folder editor.
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

                if defs.is_empty() {
                    ui.add_space(12.0);
                    ui.weak("No tiles yet - click \"+ New Tile\" to define one.");
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for def in &defs {
                        tile_row(ui, def, &mut intent);
                    }
                });
            });
        });

        self.apply(intent, edit_buf);
    }
}

impl TilesetEditorContext {
    fn edit_buf_clone(&self) -> String {
        match &self.edit {
            DefEdit::Name { buf, .. } | DefEdit::Folder { buf, .. } => buf.clone(),
            DefEdit::Idle => String::new(),
        }
    }
}

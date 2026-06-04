// ── Game-data editor ───────────────────────────────────────────────────────────
//
// Tabbed editor for game-data templates, reached from the editor hub's "Game"
// button. Tabs:
//
//   * Creature — define unit templates (name, sprite, stats) written to
//     `units.toml`. This is the creature *creator*: it owns create / edit /
//     delete of templates and the "+ Add PNG" flow that mints a Creatures-
//     category tile def. The map editor's Spawner tab only *places* these
//     templates onto a map (positions / patrols); it no longer creates them.
//   * Item / Feat / Spell — placeholders for now.
//
// Definition (here) and placement (map editor) both round-trip the whole
// `UnitRecord`, so editing stats here preserves a template's placed positions
// and vice-versa. The two never run at once (separate contexts).
//
// Follows the intent/apply pattern of the other egui contexts: the closure only
// writes into `Intent`; all state mutation happens afterward in `apply`.

use RustEngine::game::game_engine::{Engine, GameContext, UnitFile, UnitRecord};
use RustEngine::game::stats::stats;
use RustEngine::tools::load_textures;

use super::editor_menu::EditorMenuContext;
use super::tiledefs::{Category, TileDefs};
use super::tileset::Tileset;

const UNITS_PATH: &str = "gamedata/units.toml";

#[derive(Clone, Copy, PartialEq)]
enum GameTab { Creature, Item, Feat, Spell }

impl GameTab {
    const ALL: [GameTab; 4] = [GameTab::Creature, GameTab::Item, GameTab::Feat, GameTab::Spell];
    fn label(self) -> &'static str {
        match self {
            GameTab::Creature => "Creature",
            GameTab::Item     => "Item",
            GameTab::Feat     => "Feat",
            GameTab::Spell    => "Spell",
        }
    }
}

// ── Creature create/edit FSM ───────────────────────────────────────────────────
enum CreatureMode {
    Idle,
    Creating,
    Editing { index: usize },
}

#[derive(Clone, Copy)]
enum FormAction { OpenCreate, OpenEdit(usize), Confirm, Cancel }

/// Ephemeral state while creating or editing a creature template.
#[derive(Clone)]
struct CreatureDraft {
    name: String,
    sprite_id: Option<i32>,
    /// True when `sprite_id` points at a creature def freshly minted by this
    /// form's "+ Add PNG"; its name is synced to the unit's final name on
    /// Confirm. Cleared when the user picks an existing sprite instead.
    sprite_def_is_new: bool,
    health: i64,
    speed: f64,
}

impl CreatureDraft {
    fn new() -> Self {
        CreatureDraft { name: String::new(), sprite_id: None, sprite_def_is_new: false, health: 1, speed: 1.0 }
    }
}

/// UI intents collected during one `draw`, applied afterward.
#[derive(Default)]
struct Intent {
    back:             bool,
    new_tab:          Option<GameTab>,
    form:             Option<FormAction>,
    delete_id:        Option<u32>,
    new_draft_sprite: Option<i32>,
    add_png:          bool,
}

pub struct GameEditorContext {
    units:     Vec<UnitRecord>,
    tab:       GameTab,
    mode:      CreatureMode,
    draft:     CreatureDraft,
    tile_defs: TileDefs,
    /// Active tileset (id -> png file), needed for the sprite picker and the
    /// "+ Add PNG" flow. Mirrors `Settings.active_tileset`.
    id_path:   String,
    pending:   Option<Box<dyn GameContext>>,
}

impl GameEditorContext {
    pub fn new(active_tileset: String) -> Self {
        GameEditorContext {
            units:     Self::load_units(),
            tab:       GameTab::Creature,
            mode:      CreatureMode::Idle,
            draft:     CreatureDraft::new(),
            tile_defs: TileDefs::load(),
            id_path:   active_tileset,
            pending:   None,
        }
    }

    fn load_units() -> Vec<UnitRecord> {
        if !std::path::Path::new(UNITS_PATH).exists() { return Vec::new(); }
        let content = std::fs::read_to_string(UNITS_PATH).unwrap_or_default();
        toml::from_str::<UnitFile>(&content).map(|f| f.unit).unwrap_or_default()
    }

    fn save_units(&self) {
        let file = UnitFile { unit: self.units.clone() };
        if let Ok(content) = toml::to_string(&file) {
            let _ = std::fs::create_dir_all("gamedata");
            let _ = std::fs::write(UNITS_PATH, content);
        }
    }

    /// Creature sprite choices: (id, png) pairs from the active tileset whose
    /// tile def is in the Creatures category.
    fn creature_sprites(&self) -> Vec<(i32, String)> {
        let mut v: Vec<(i32, String)> = load_textures(&self.id_path)
            .into_iter()
            .filter(|(id, _)| self.tile_defs.category_of(*id) == Category::Creatures)
            .collect();
        v.sort_by_key(|(id, _)| *id);
        v
    }

    /// "+ Add PNG": pick a PNG, copy into assets/ if needed, mint a new
    /// Creatures-category tile def, assign the PNG in the active tileset, and
    /// select it as the draft sprite (marked owned so Confirm syncs its name).
    fn add_creature_png(&mut self) {
        let Some(picked) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_directory("assets")
            .pick_file() else { return; };
        let Some(filename) = picked.file_name().map(|n| n.to_string_lossy().into_owned()) else { return; };
        if filename.is_empty() { return; }
        let dest = std::path::Path::new("assets").join(&filename);
        if !dest.exists() {
            let _ = std::fs::copy(&picked, &dest);
        }
        // Seed name from the draft (final name synced on Confirm).
        let tile_name = {
            let n = self.draft.name.trim();
            if n.is_empty() { None } else { Some(n.to_string()) }
        };
        let id = self.tile_defs.create(tile_name, Category::Creatures);
        self.tile_defs.save();
        let mut ts = Tileset::load(&self.id_path);
        ts.set_png(id, Some(filename));
        ts.save();
        self.draft.sprite_id = Some(id);
        self.draft.sprite_def_is_new = true;
    }

    fn apply(&mut self, intent: Intent, draft_name: String, draft_health: i64, draft_speed: f64) {
        // Mirror draft edits back while a form is open.
        if matches!(self.mode, CreatureMode::Creating | CreatureMode::Editing { .. }) {
            self.draft.name   = draft_name;
            self.draft.health = draft_health;
            self.draft.speed  = draft_speed;
            if let Some(s) = intent.new_draft_sprite {
                self.draft.sprite_id = Some(s);
                self.draft.sprite_def_is_new = false;
            }
        }

        if intent.back {
            self.pending = Some(Box::new(EditorMenuContext::new()));
            return;
        }
        if let Some(tab) = intent.new_tab { self.tab = tab; }

        if intent.add_png { self.add_creature_png(); }

        if let Some(del_id) = intent.delete_id {
            self.units.retain(|u| u.id != del_id);
            self.save_units();
        }

        let Some(action) = intent.form else { return; };
        match action {
            FormAction::OpenCreate => {
                self.mode  = CreatureMode::Creating;
                self.draft = CreatureDraft::new();
            }
            FormAction::OpenEdit(idx) => {
                let record = &self.units[idx];
                let (h, s) = record.stats.as_ref().map(|st| (st.health, st.speed)).unwrap_or((1, 1.0));
                self.draft = CreatureDraft {
                    name:      record.name.clone(),
                    sprite_id: record.sprite_id,
                    sprite_def_is_new: false,
                    health:    h,
                    speed:     s,
                };
                self.mode = CreatureMode::Editing { index: idx };
            }
            FormAction::Cancel => {
                self.mode  = CreatureMode::Idle;
                self.draft = CreatureDraft::new();
            }
            FormAction::Confirm => {
                match self.mode {
                    CreatureMode::Creating => {
                        let new_id = self.units.iter().map(|u| u.id).max().unwrap_or(0) + 1;
                        self.units.push(UnitRecord {
                            id:        new_id,
                            name:      self.draft.name.clone(),
                            sprite_id: self.draft.sprite_id,
                            positions: vec![],
                            patrols:   vec![],
                            stats:     Some(stats::new(self.draft.health, self.draft.speed, 0.0)), //fix
                        });
                    }
                    CreatureMode::Editing { index } => {
                        self.units[index].name      = self.draft.name.clone();
                        self.units[index].sprite_id = self.draft.sprite_id;
                        self.units[index].stats     = Some(stats::new(self.draft.health, self.draft.speed,0.0)); //fix
                    }
                    CreatureMode::Idle => {}
                }
                self.save_units();
                // Sync an owned, freshly-added sprite def's name to the final name.
                if self.draft.sprite_def_is_new {
                    if let Some(sid) = self.draft.sprite_id {
                        let name = self.draft.name.trim();
                        self.tile_defs.set_name(sid, if name.is_empty() { None } else { Some(name.to_string()) });
                        self.tile_defs.save();
                    }
                }
                self.mode  = CreatureMode::Idle;
                self.draft = CreatureDraft::new();
            }
        }
    }
}

impl GameContext for GameEditorContext {
    fn update(&mut self, _engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        self.pending.take()
    }

    fn draw(&mut self, engine: &mut Engine) {
        let mut intent = Intent::default();

        // Snapshots taken before the closure to avoid borrow conflicts.
        let tab        = self.tab;
        let form_open  = matches!(self.mode, CreatureMode::Creating | CreatureMode::Editing { .. });
        let is_editing = matches!(self.mode, CreatureMode::Editing { .. });
        let sprites    = if tab == GameTab::Creature { self.creature_sprites() } else { Vec::new() };
        let mut draft_name   = self.draft.name.clone();
        let draft_sprite     = self.draft.sprite_id;
        let mut draft_health = self.draft.health;
        let mut draft_speed  = self.draft.speed;

        let (w, h) = engine.screen_size();
        let input  = engine.egui_input.clone();

        engine.renderer.render(input, w, h, |ctx| {
            egui::TopBottomPanel::top("game_editor_top").show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("< Editors").clicked() { intent.back = true; }
                    ui.separator();
                    for t in GameTab::ALL {
                        if ui.selectable_label(tab == t, t.label()).clicked() {
                            intent.new_tab = Some(t);
                        }
                    }
                });
                ui.add_space(6.0);
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                match tab {
                    GameTab::Creature => {
                        if !form_open {
                            egui::ScrollArea::vertical()
                                .id_salt("creature_list")
                                .show(ui, |ui| {
                                    for (i, unit) in self.units.iter().enumerate() {
                                        ui.horizontal(|ui| {
                                            let label = format!(
                                                "{} ({} placed)",
                                                if unit.name.is_empty() { "(unnamed)" } else { &unit.name },
                                                unit.positions.len(),
                                            );
                                            ui.label(label);
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.small_button("Del").clicked() {
                                                    intent.delete_id = Some(unit.id);
                                                }
                                                if ui.small_button("Edit").clicked() {
                                                    intent.form = Some(FormAction::OpenEdit(i));
                                                }
                                            });
                                        });
                                    }
                                });
                            ui.separator();
                            if ui.button("+ Create New").clicked() {
                                intent.form = Some(FormAction::OpenCreate);
                            }
                        } else {
                            ui.label(if is_editing { "Edit Creature" } else { "New Creature" });
                            ui.separator();
                            ui.label("Name:");
                            ui.text_edit_singleline(&mut draft_name);
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label("Sprite:");
                                if ui.small_button("+ Add PNG").clicked() { intent.add_png = true; }
                            });
                            egui::ScrollArea::vertical()
                                .id_salt("creature_sprite")
                                .max_height(160.0)
                                .show(ui, |ui| {
                                    if sprites.is_empty() {
                                        ui.weak("(no Creatures sprites yet — use + Add PNG)");
                                    }
                                    for (id, path) in &sprites {
                                        let selected = draft_sprite == Some(*id);
                                        let label = match self.tile_defs.name_of(*id) {
                                            Some(n) if !n.is_empty() => format!("{} | {}", id, n),
                                            _                        => format!("{} | {}", id, path),
                                        };
                                        if ui.selectable_label(selected, label).clicked() {
                                            intent.new_draft_sprite = Some(*id);
                                        }
                                    }
                                });
                            ui.add_space(6.0);
                            ui.collapsing("Stats", |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Health:");
                                    ui.add(egui::DragValue::new(&mut draft_health).range(0..=i64::MAX));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Speed:");
                                    ui.add(egui::DragValue::new(&mut draft_speed).range(0.0..=f64::MAX).speed(0.1));
                                });
                            });
                            ui.add_space(2.0);
                            ui.collapsing("Feats", |ui| { ui.label("(not yet implemented)"); });
                            ui.add_space(8.0);
                            let confirm_label = if is_editing { "Save" } else { "Create" };
                            ui.horizontal(|ui| {
                                if ui.button(confirm_label).clicked() { intent.form = Some(FormAction::Confirm); }
                                if ui.button("Cancel").clicked()      { intent.form = Some(FormAction::Cancel);  }
                            });
                        }
                    }
                    GameTab::Item  => { ui.add_space(12.0); ui.weak("Item editor — not yet implemented."); }
                    GameTab::Feat  => { ui.add_space(12.0); ui.weak("Feat editor — not yet implemented."); }
                    GameTab::Spell => { ui.add_space(12.0); ui.weak("Spell editor — not yet implemented."); }
                }
            });
        });

        self.apply(intent, draft_name, draft_health, draft_speed);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// HOW THIS EDITOR WORKS — AND HOW TO EXTEND IT
// ══════════════════════════════════════════════════════════════════════════════
//
// ── The context system ────────────────────────────────────────────────────────
// Every screen in the game (menu, gameplay, editor, settings) is a struct that
// implements `GameContext` (defined in game_engine.rs).  The trait has two
// methods:
//
//   fn update(&mut self, engine, dt) -> Option<Box<dyn GameContext>>
//   fn draw(&mut self, engine)
//
// Returning Some(next) from `update` swaps the running context.  `EditorContext`
// exits by setting `pending_exit = true`, which `update` picks up and returns
// `Some(Box::new(MainMenuContext::new()))`.
//
// To make a brand-new screen/context:
//   1. Create `src/contexts/my_screen.rs`, define a `pub struct MyScreenContext`.
//   2. `impl GameContext for MyScreenContext` with your `update` and `draw`.
//   3. Add `mod my_screen;` and a `pub use` line in `src/contexts/mod.rs`.
//   4. Transition into it from any other context by returning it from `update`.
//
// ── The intent / handler pattern ─────────────────────────────────────────────
// `draw` is split into two phases because the egui closure borrows `engine`
// exclusively — you cannot call `&mut self` methods inside it.
//   Phase 1 (inside the closure): write UI events into `EditorIntent` fields.
//   Phase 2 (after the closure):  hand `intent` to per-feature handler methods
//                                 (`handle_paint`, `handle_spawner`, …).
// Add new UI inputs by adding a field to `EditorIntent`, setting it inside the
// closure, and reacting to it in the matching `handle_*` method (or adding a
// new handler if it's a whole new feature).
//
// ── Adding a new tab to the right panel ──────────────────────────────────────
// The right panel has three tabs controlled by `RightPanelTab`.  To add one:
//   1. Add a variant to `RightPanelTab` (e.g. `EventPainter`).
//   2. In `draw`, find the `ui.selectable_label` row and add your tab button.
//   3. Add a `RightPanelTab::EventPainter => { … }` arm to the `match active_tab`
//      block to render the panel content.
//   4. Add any new intent fields the panel needs and react to them in a handler.
//
// ── Adding a new field / feature to EditorContext ────────────────────────────
// All runtime state lives in `EditorContext`. Initialise new fields in the
// single `with_world` constructor (both `from_file` and `new_map` call it).
// For FSM-style features (e.g. a multi-step dialog), follow the
// `SpawnerFormAction` pattern: define an action enum, store it in `EditorIntent`,
// and apply it in a dedicated handler method.
//
// ── Persisting new data ───────────────────────────────────────────────────────
// Map tile data is saved via `World::save` (called when `intent.do_save` is set).
// Unit templates and their placements live in `units.toml` and are written by
// `save_units()`.  If you add a new type of placed object, follow the same
// pattern: a TOML-backed file, a `load_*` helper called from `with_world`, and
// a `save_*` helper called whenever state changes.
//
// ── Key helpers ───────────────────────────────────────────────────────────────
//   screen_to_tile_idx  — converts an egui Pos2 into an index into world.tiles
//   tile_display_name   — controls how palette entries are labelled in the list
//   build_sprite_cache  — loads GLObjects keyed by palette id for drawing sprites
// ══════════════════════════════════════════════════════════════════════════════

use RustEngine::game::game_engine::{Engine, GameContext, World, TILE_SIZE, UnitRecord, UnitFile};
use RustEngine::game::stats::stats;
use RustEngine::tools::{load_textures, GLObject, BL_RECTANGLE};
use RustEngine::shaders::{VERT_SHADER, FRAG_SHADER};
use std::collections::HashMap;
use super::menus::MainMenuContext;
use super::tiledefs::{Category, TileDefs};
use super::tileset::Tileset;

// ── Editor ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum RightPanelTab {
    TexturePalette,
    PhysicsPainter,
    CharacterSpawner,
}

/// Tile palette entry. The display string is built by `tile_display_name` —
/// change that one function to reformat how tiles appear in the list.
struct PaletteEntry {
    id: i32,
    path: String,
}

impl PaletteEntry {
    fn display(&self) -> String {
        tile_display_name(self.id, &self.path)
    }
}

/// Change this to alter how tiles are labelled in the palette panel.
fn tile_display_name(id: i32, path: &str) -> String {
    format!("{} | {}", id, path)
}

/// Render one texture row in the palette: a selectable label (caller composes
/// it — typically `id | <tile-def name>`), with a `...` menu on the far right
/// holding the collision dropdown and the move-to-folder submenu. Emits intents
/// only — never mutates editor state directly.
fn texture_row(
    ui: &mut egui::Ui,
    entry: &PaletteEntry,
    label: &str,
    selected: bool,
    solid: bool,
    current_folder: Option<&str>,
    folders: &[String],
    intent: &mut EditorIntent,
) {
    ui.horizontal(|ui| {
        if ui.selectable_label(selected, label).clicked() {
            intent.new_selected = Some(entry.id);
        }
        // Push the overflow menu to the far right of the row.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button("...", |ui| {
                ui.menu_button("Default collision", |ui| {
                    if ui.selectable_label(solid, "Solid").clicked() {
                        intent.set_texture_solid = Some((entry.id, true));
                        ui.close();
                    }
                    if ui.selectable_label(!solid, "Passable").clicked() {
                        intent.set_texture_solid = Some((entry.id, false));
                        ui.close();
                    }
                });
                ui.separator();
                ui.menu_button("Move to folder", |ui| {
                    if current_folder.is_some() && ui.button("(remove from folder)").clicked() {
                        intent.move_to_folder = Some((entry.id, None));
                        ui.close();
                    }
                    for f in folders {
                        if current_folder == Some(f.as_str()) { continue; }
                        if ui.button(f).clicked() {
                            intent.move_to_folder = Some((entry.id, Some(f.clone())));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("New folder...").clicked() {
                        intent.start_new_folder = Some(entry.id);
                        ui.close();
                    }
                });
            });
        });
    });
}

// ── Folder-naming FSM ────────────────────────────────────────────────────────
// Inline text input shown at the top of the Textures panel, used both to name a
// brand-new folder (while moving a texture into it) and to rename an existing
// folder. State lives in EditorContext::folder_edit; applied in handle_folders.
enum FolderEdit {
    Idle,
    /// Typing a name for a new folder; the texture moves into it on confirm.
    NamingNew { texture_id: i32, name: String },
    /// Renaming an existing folder; all member textures are reassigned on confirm.
    Renaming  { from: String, to: String },
}

// ── Spawner FSM ──────────────────────────────────────────────────────────────
// State lives in EditorContext::spawner_mode; form actions are emitted by the
// right-panel UI and applied in handle_spawner. PatrolPainting is entered from
// a map click (handle_unit_clicks) and exited via Esc (handle_patrol).

enum SpawnerMode {
    Idle,
    CreatingNew,
    Editing { index: usize },
    PatrolPainting { unit_id: u32, instance_idx: usize },
}

#[derive(Clone, Copy)]
enum SpawnerFormAction { OpenCreate, OpenEdit(usize), Confirm, Cancel }

/// What's selected as the spawner-tab brush. `Unit(id)` paints unit instances
/// (existing behavior). `PlayerSpawn` paints / clears the per-map player
/// spawnpoint stored on `World::spawn` — max 1 per map.
#[derive(Clone, Copy, PartialEq)]
enum SpawnerBrush {
    None,
    Unit(u32),
    PlayerSpawn,
}

/// Ephemeral editor state while creating or editing a Unit — never serialized.
#[derive(Clone)]
struct UnitDraft {
    name: String,
    sprite_id: Option<i32>,
    /// True when `sprite_id` points at a creature def freshly minted by this
    /// form's "+ Add PNG". Such a def is owned by the unit, so its name is
    /// synced to the unit's final name on Confirm. Cleared when the user picks
    /// an existing sprite instead (we never rename a sprite we didn't create).
    sprite_def_is_new: bool,
    health: i64,
    speed: f64,
}

impl UnitDraft {
    fn new() -> Self {
        UnitDraft { name: String::new(), sprite_id: None, sprite_def_is_new: false, health: 1, speed: 1.0 }
    }
}

/// All UI intents collected during one `draw` call. The egui closure writes
/// here; post-closure handlers read from here. Default = "nothing happened".
#[derive(Default)]
struct EditorIntent {
    // Toolbar
    do_save:           bool,
    do_exit:           bool,
    toggle_panel:      bool,
    new_selected:      Option<i32>,
    new_tab:           Option<RightPanelTab>,
    new_physics_brush: Option<bool>,
    // Central-panel paint input (texture / physics painters)
    paint_pos: Option<egui::Pos2>,
    click_pos: Option<egui::Pos2>,
    erase_pos: Option<egui::Pos2>,
    // Held right-mouse erase on the texture tab.
    erase_paint_pos: Option<egui::Pos2>,
    // Patrol-painting input
    patrol_click_pos: Option<egui::Pos2>,
    patrol_erase_pos: Option<egui::Pos2>,
    patrol_esc:       bool,
    // Spawner panel
    spawner_form:         Option<SpawnerFormAction>,
    new_spawner_brush:    Option<SpawnerBrush>,
    delete_spawner_id:    Option<u32>,
    new_draft_sprite:     Option<i32>,
    /// Open file dialog to pick a PNG, register it as a new tile, and select
    /// it as the draft unit's sprite.
    add_unit_sprite_png:  bool,
    // Set a texture's default collision: (texture id, solid).
    set_texture_solid: Option<(i32, bool)>,
    // Folder management
    move_to_folder:      Option<(i32, Option<String>)>, // move texture to folder (None = loose)
    start_new_folder:    Option<i32>,                   // open new-folder input for this texture
    start_rename_folder: Option<String>,                // open rename input for this folder
    delete_folder:       Option<String>,                // dissolve folder, members become loose
    folder_confirm:      bool,                           // confirm the active folder input
    folder_cancel:       bool,                           // cancel the active folder input
    // Resize dialog
    open_resize_dialog:  bool,
    confirm_resize:      Option<(usize, usize)>,
    close_resize_dialog: bool,
    // Camera pan / zoom
    cam_scroll_y:  f32,
    cam_pan_delta: Option<egui::Vec2>,
    cam_cursor:    Option<egui::Pos2>,
}


pub struct EditorContext {
    world: World,
    map_path: String,
    palette: Vec<PaletteEntry>,
    selected_id: Option<i32>,
    pending_exit: bool,
    /// False until the first draw call, when we center the camera on the map.
    camera_init: bool,
    active_tab: RightPanelTab,
    /// Physics painter brush: true = paint solid, false = paint passable.
    physics_brush_solid: bool,
    right_panel_open: bool,
    /// Path of the tileset (id->png file) this editor session opened with.
    /// Used to write back when adding a unit-sprite PNG and to reload the palette.
    id_path: String,
    spawner_mode: SpawnerMode,
    spawner_draft: UnitDraft,
    spawner_units: Vec<UnitRecord>,
    /// Active brush for the Spawner tab — a unit template, the player-spawn
    /// marker, or nothing selected.
    spawner_brush: SpawnerBrush,
    /// Cached GLObjects keyed by palette id, used to draw unit sprites in the editor.
    unit_sprite_cache: HashMap<i32, GLObject>,
    /// Some((draft_w, draft_h)) while the resize dialog is open.
    resize_dialog: Option<(usize, usize)>,
    zoom: f32,
    /// Shared tile definitions (collision default, folder, name, future options).
    tile_defs: TileDefs,
    /// Inline folder-name input state (new folder / rename folder).
    folder_edit: FolderEdit,
}

impl EditorContext {
    const UNITS_PATH: &'static str = "gamedata/units.toml";

    /// Open an existing map file for editing.
    pub fn from_file(map_path: &str, id_path: &str) -> Result<Self, String> {
        if !std::path::Path::new(map_path).exists() {
            return Err(format!("Map file not found: {}", map_path));
        }
        if !std::path::Path::new(id_path).exists() {
            return Err(format!("ID file not found: {}", id_path));
        }
        Ok(Self::with_world(World::load(map_path, id_path), map_path, id_path))
    }

    /// Create a new blank map of the given dimensions.
    pub fn new_map(map_path: &str, id_path: &str, width: usize, height: usize) -> Result<Self, String> {
        if !std::path::Path::new(id_path).exists() {
            return Err(format!("ID file not found: {}", id_path));
        }
        Ok(Self::with_world(World::new_empty(width, height), map_path, id_path))
    }

    fn with_world(world: World, map_path: &str, id_path: &str) -> Self {
        let palette = Self::load_palette(id_path);
        let unit_sprite_cache = Self::build_sprite_cache(&palette);
        EditorContext {
            world,
            map_path: map_path.to_string(),
            palette,
            selected_id: None,
            pending_exit: false,
            camera_init: false,
            active_tab: RightPanelTab::TexturePalette,
            physics_brush_solid: true,
            right_panel_open: true,
            id_path: id_path.to_string(),
            spawner_mode: SpawnerMode::Idle,
            spawner_draft: UnitDraft::new(),
            spawner_units: Self::load_units(),
            spawner_brush: SpawnerBrush::None,
            unit_sprite_cache,
            resize_dialog: None,
            zoom: 1.0,
            tile_defs: TileDefs::load(),
            folder_edit: FolderEdit::Idle,
        }
    }

    fn build_sprite_cache(palette: &[PaletteEntry]) -> HashMap<i32, GLObject> {
        const FALLBACK: &str = "assets/temp3.png";
        palette.iter()
            .filter(|e| e.id != 0)
            .filter_map(|e| {
                let path = format!("assets/{}", e.path);
                let resolved = if std::path::Path::new(&path).exists() {
                    path
                } else if std::path::Path::new(FALLBACK).exists() {
                    FALLBACK.to_string()
                } else {
                    return None; // neither file nor fallback exists — skip
                };
                Some((e.id, GLObject::new(BL_RECTANGLE, &resolved, VERT_SHADER, FRAG_SHADER)))
            })
            .collect()
    }

    /// Build the palette purely from the loaded tileset (`id_path`) — the
    /// tileset is the source of truth for what's paintable. Stray PNGs in
    /// `assets/` that aren't bound to a tile in the active tileset do NOT
    /// appear here; add them via the Tileset editor's "Assign PNG" flow first.
    fn load_palette(id_path: &str) -> Vec<PaletteEntry> {
        if !std::path::Path::new(id_path).exists() {
            return Vec::new();
        }
        let mut entries: Vec<PaletteEntry> = load_textures(id_path)
            .into_iter()
            .map(|(id, path)| PaletteEntry { id, path })
            .collect();
        entries.sort_by_key(|e| e.id);
        entries
    }

    fn load_units() -> Vec<UnitRecord> {
        if !std::path::Path::new(Self::UNITS_PATH).exists() { return Vec::new(); }
        let content = std::fs::read_to_string(Self::UNITS_PATH).unwrap_or_default();
        toml::from_str::<UnitFile>(&content)
            .map(|f| f.unit)
            .unwrap_or_default()
    }

    fn save_units(&self) {
        let file    = UnitFile { unit: self.spawner_units.clone() };
        let content = toml::to_string(&file).expect("Failed to serialize units");
        let _ = std::fs::create_dir_all("gamedata");
        std::fs::write(Self::UNITS_PATH, content).expect("Failed to save units.toml");
    }

    /// Convert a screen-space position (egui coords, y=0 at top) to a
    /// tile index into `self.world.tiles`, accounting for the engine camera.
    /// World coords are snapped down to the nearest TILE_SIZE multiple first.
    fn screen_to_tile_idx(&self, sx: f32, sy: f32, screen_h: u32, camera: (i32, i32), zoom: f32) -> Option<usize> {
        // OpenGL y=0 is bottom; egui y=0 is top — flip y.
        // Divide by zoom to convert screen pixels back to world pixels before adding camera offset.
        let world_x = (sx / zoom) as i32 + camera.0;
        let world_y = ((screen_h as f32 - sy) / zoom) as i32 + camera.1;
        if world_x < 0 || world_y < 0 { return None; }
        // Snap to tile grid (floor to nearest TILE_SIZE multiple).
        let tx = (world_x / TILE_SIZE) as usize;
        let ty = (world_y / TILE_SIZE) as usize;
        if tx >= self.world.width || ty >= self.world.height { return None; }
        Some(ty * self.world.width + tx)
    }

    fn texture_for_id(&self, id: i32) -> Option<&str> {
        self.palette.iter().find(|e| e.id == id).map(|e| e.path.as_str())
    }

    /// Returns the (unit_id, instance_idx) of the first placed unit at `tile_pos`.
    fn find_unit_at_tile(&self, tile_pos: (i32, i32)) -> Option<(u32, usize)> {
        for record in &self.spawner_units {
            if let Some(i) = record.positions.iter().position(|&p| p == tile_pos) {
                return Some((record.id, i));
            }
        }
        None
    }

    fn find_unit_mut(&mut self, id: u32) -> Option<&mut UnitRecord> {
        self.spawner_units.iter_mut().find(|u| u.id == id)
    }

    /// Resolve a screen-space pointer position to a tile and invoke `f` with
    /// `(self, tile_idx, tile_position)`. No-op when the pointer is off-map.
    fn at_tile<F>(&mut self, pos: egui::Pos2, screen_h: u32, camera: (i32, i32), f: F)
    where
        F: FnOnce(&mut Self, usize, (i32, i32)),
    {
        let zoom = self.zoom;
        if let Some(idx) = self.screen_to_tile_idx(pos.x, pos.y, screen_h, camera, zoom) {
            let tile_pos = self.world.tiles[idx].position;
            f(self, idx, tile_pos);
        }
    }

    // ── Intent handlers (called after the egui closure returns) ───────────────

    /// Held-mouse painting on the texture / physics tabs. No-op on the spawner tab.
    fn handle_paint(&mut self, active_tab: RightPanelTab, intent: &EditorIntent, screen_h: u32, camera: (i32, i32)) {
        if let Some(pos) = intent.paint_pos {
            self.at_tile(pos, screen_h, camera, |this, idx, _tp| match active_tab {
                RightPanelTab::TexturePalette => {
                    if let Some(sel_id) = this.selected_id {
                        let tex = if sel_id == 0 { None } else { this.texture_for_id(sel_id).map(str::to_owned) };
                        this.world.tiles[idx].set_sprite(sel_id, tex.as_deref());
                        // Stamp this texture's default collision onto the tile.
                        // The Physics tab can still override individual tiles afterward.
                        // Eraser (id 0) leaves collision untouched.
                        if sel_id != 0 {
                            let solid = this.tile_defs.solid_of(sel_id);
                            this.world.tiles[idx].physics.solid = solid;
                        }
                    }
                }
                RightPanelTab::PhysicsPainter => {
                    this.world.tiles[idx].physics.solid = this.physics_brush_solid;
                }
                RightPanelTab::CharacterSpawner => {}
            });
        }
        // Held right-mouse on the texture tab erases whatever tile the cursor is over.
        if active_tab == RightPanelTab::TexturePalette {
            if let Some(pos) = intent.erase_paint_pos {
                self.at_tile(pos, screen_h, camera, |this, idx, _tp| {
                    this.world.tiles[idx].set_sprite(0, None);
                });
            }
        }
    }

    /// Single click / right-click on the map while the spawner tab is active.
    /// Left-click on empty + brush selected → place. Left-click on a unit → patrol.
    /// Right-click on a unit → delete the instance.
    fn handle_unit_clicks(&mut self, active_tab: RightPanelTab, intent: &EditorIntent, screen_h: u32, camera: (i32, i32)) {
        if active_tab != RightPanelTab::CharacterSpawner { return; }
        let brush = self.spawner_brush;

        // Player-spawn brush has its own click semantics — no unit interaction.
        if brush == SpawnerBrush::PlayerSpawn {
            if let Some(pos) = intent.click_pos {
                self.at_tile(pos, screen_h, camera, |this, _idx, tile_pos| {
                    // Overwrites any existing spawn (max 1 per map). Allowed on
                    // solid tiles for now — the player will simply be stuck if
                    // it's not walkable, which is the user's choice.
                    this.world.spawn = Some(tile_pos);
                });
            }
            if intent.erase_pos.is_some() {
                // Right-click clears the spawn regardless of which tile is
                // under the cursor — there's only ever one to clear.
                self.world.spawn = None;
            }
            return;
        }

        // Unit brush (or no brush) — original behaviour.
        if let Some(pos) = intent.click_pos {
            self.at_tile(pos, screen_h, camera, |this, idx, tile_pos| {
                if let Some((unit_id, instance_idx)) = this.find_unit_at_tile(tile_pos) {
                    // Click on a placed unit → enter patrol painting for that instance.
                    // The right panel auto-hides via the `!is_patrol_painting` render guard.
                    this.spawner_mode = SpawnerMode::PatrolPainting { unit_id, instance_idx };
                } else if let SpawnerBrush::Unit(sel_id) = brush {
                    // Empty tile + unit brush selected → place a new instance
                    // (skip solid tiles).
                    if !this.world.tiles[idx].physics.solid {
                        if let Some(record) = this.find_unit_mut(sel_id) {
                            record.positions.push(tile_pos);
                            record.patrols.push(vec![]);
                        }
                        this.save_units();
                    }
                }
            });
        }
        if let Some(pos) = intent.erase_pos {
            self.at_tile(pos, screen_h, camera, |this, _idx, tile_pos| {
                if let Some((unit_id, instance_idx)) = this.find_unit_at_tile(tile_pos) {
                    if let Some(record) = this.find_unit_mut(unit_id) {
                        record.positions.remove(instance_idx);
                        if instance_idx < record.patrols.len() { record.patrols.remove(instance_idx); }
                        this.save_units();
                    }
                }
            });
        }
    }

    fn handle_patrol(&mut self, intent: &EditorIntent, screen_h: u32, camera: (i32, i32)) {
        if intent.patrol_esc && matches!(self.spawner_mode, SpawnerMode::PatrolPainting { .. }) {
            self.spawner_mode = SpawnerMode::Idle;
        }
        if let Some(pos) = intent.patrol_click_pos {
            if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
                self.at_tile(pos, screen_h, camera, move |this, _idx, tile_pos| {
                    if let Some(record) = this.find_unit_mut(unit_id) {
                        while record.patrols.len() <= instance_idx { record.patrols.push(vec![]); }
                        record.patrols[instance_idx].push(tile_pos);
                        this.save_units();
                    }
                });
            }
        }
        if let Some(pos) = intent.patrol_erase_pos {
            if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
                self.at_tile(pos, screen_h, camera, move |this, _idx, tile_pos| {
                    if let Some(record) = this.find_unit_mut(unit_id) {
                        if let Some(patrol) = record.patrols.get_mut(instance_idx) {
                            if let Some(i) = patrol.iter().position(|&p| p == tile_pos) {
                                patrol.remove(i);
                                this.save_units();
                            }
                        }
                    }
                });
            }
        }
    }

    fn handle_spawner(&mut self, intent: &EditorIntent, draft_name: String, draft_health: i64, draft_speed: f64) {
        // Mirror draft edits back into self while the form is open.
        if matches!(self.spawner_mode, SpawnerMode::CreatingNew | SpawnerMode::Editing { .. }) {
            self.spawner_draft.name   = draft_name;
            self.spawner_draft.health = draft_health;
            self.spawner_draft.speed  = draft_speed;
            if let Some(s) = intent.new_draft_sprite {
                self.spawner_draft.sprite_id = Some(s);
                // User picked an existing sprite — not one we own, so don't rename it.
                self.spawner_draft.sprite_def_is_new = false;
            }
        }
        if let Some(b) = intent.new_spawner_brush { self.spawner_brush = b; }
        if let Some(del_id) = intent.delete_spawner_id {
            self.spawner_units.retain(|u| u.id != del_id);
            // If the deleted unit was the active brush, drop back to None so the
            // brush doesn't dangle on a non-existent template.
            if matches!(self.spawner_brush, SpawnerBrush::Unit(id) if id == del_id) {
                self.spawner_brush = SpawnerBrush::None;
            }
            // Defensive: if a future code path lets you delete while patrol-painting
            // that unit, drop back to Idle so we're not pointing at a dead instance.
            if let SpawnerMode::PatrolPainting { unit_id, .. } = self.spawner_mode {
                if unit_id == del_id { self.spawner_mode = SpawnerMode::Idle; }
            }
            self.save_units();
        }
        let Some(action) = intent.spawner_form else { return; };
        match action {
            SpawnerFormAction::OpenCreate => {
                self.spawner_mode  = SpawnerMode::CreatingNew;
                self.spawner_draft = UnitDraft::new();
            }
            SpawnerFormAction::OpenEdit(idx) => {
                let record = &self.spawner_units[idx];
                let (h, s) = record.stats.as_ref().map(|st| (st.health, st.speed)).unwrap_or((1, 1.0));
                self.spawner_draft = UnitDraft {
                    name:      record.name.clone(),
                    sprite_id: record.sprite_id,
                    sprite_def_is_new: false,
                    health:    h,
                    speed:     s,
                };
                self.spawner_mode = SpawnerMode::Editing { index: idx };
            }
            SpawnerFormAction::Cancel => {
                self.spawner_mode  = SpawnerMode::Idle;
                self.spawner_draft = UnitDraft::new();
            }
            SpawnerFormAction::Confirm => {
                match self.spawner_mode {
                    SpawnerMode::CreatingNew => {
                        let new_id = self.spawner_units.iter().map(|u| u.id).max().unwrap_or(0) + 1;
                        self.spawner_units.push(UnitRecord {
                            id:        new_id,
                            name:      self.spawner_draft.name.clone(),
                            sprite_id: self.spawner_draft.sprite_id,
                            positions: vec![],
                            patrols:   vec![],
                            stats:     Some(stats::new(self.spawner_draft.health, self.spawner_draft.speed)),
                        });
                    }
                    SpawnerMode::Editing { index } => {
                        self.spawner_units[index].name      = self.spawner_draft.name.clone();
                        self.spawner_units[index].sprite_id = self.spawner_draft.sprite_id;
                        self.spawner_units[index].stats     = Some(stats::new(self.spawner_draft.health, self.spawner_draft.speed));
                    }
                    SpawnerMode::Idle | SpawnerMode::PatrolPainting { .. } => {}
                }
                self.save_units();
                // If this unit owns a freshly-added creature sprite, name that
                // def after the finished unit so the two stay in sync.
                if self.spawner_draft.sprite_def_is_new {
                    if let Some(sid) = self.spawner_draft.sprite_id {
                        let name = self.spawner_draft.name.trim();
                        self.tile_defs.set_name(sid, if name.is_empty() { None } else { Some(name.to_string()) });
                        self.tile_defs.save();
                    }
                }
                self.spawner_mode  = SpawnerMode::Idle;
                self.spawner_draft = UnitDraft::new();
            }
        }
    }

    /// "+ Add PNG" from the spawner unit form: pick a PNG via rfd, copy it into
    /// `assets/` if needed, allocate a new abstract tile id in the **Creatures**
    /// category, assign the PNG in the active tileset, reload the palette so the
    /// new entry shows up immediately, and select it as the draft unit's sprite.
    /// This is the "thin shortcut that delegates to central allocation" mentioned
    /// in the architecture memory — the alloc goes through `TileDefs::create`.
    /// The def's name is seeded from the current draft name but is authoritatively
    /// (re)synced to the unit's final name when the form is confirmed, so we mark
    /// it `sprite_def_is_new`. Must be dispatched AFTER `handle_spawner` so
    /// `spawner_draft.name` is fresh.
    fn handle_add_unit_sprite_png(&mut self, intent: &EditorIntent) {
        if !intent.add_unit_sprite_png { return; }
        let Some(picked) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_directory("assets")
            .pick_file() else { return; };
        let Some(filename) = picked.file_name().map(|n| n.to_string_lossy().into_owned()) else { return; };
        if filename.is_empty() { return; }
        // Copy into assets/ if not already there.
        let dest = std::path::Path::new("assets").join(&filename);
        if !dest.exists() {
            let _ = std::fs::copy(&picked, &dest);
        }
        // Seed the def name from the unit's current name (the final name is
        // synced on Confirm). e.g. naming a unit "Goblin" then + Add PNG yields
        // a provisional "Goblin" creature def.
        let tile_name = {
            let n = self.spawner_draft.name.trim();
            if n.is_empty() { None } else { Some(n.to_string()) }
        };
        // Allocate a new Creatures-category id, assign the PNG in the loaded tileset.
        let id = self.tile_defs.create(tile_name, Category::Creatures);
        self.tile_defs.save();
        let mut ts = Tileset::load(&self.id_path);
        ts.set_png(id, Some(filename));
        ts.save();
        // Reload palette + sprite cache so the new entry is immediately visible.
        self.palette           = Self::load_palette(&self.id_path);
        self.unit_sprite_cache = Self::build_sprite_cache(&self.palette);
        // Auto-select it as the draft unit's sprite — and mark it owned so Confirm
        // syncs its name.
        self.spawner_draft.sprite_id = Some(id);
        self.spawner_draft.sprite_def_is_new = true;
    }

    /// Apply folder intents: open/confirm/cancel the inline name input, move a
    /// texture between folders, and rename or dissolve a folder. Folders are
    /// implicit, so rename/delete operate by rewriting member tiles' folder.
    fn handle_folders(&mut self, intent: &EditorIntent, folder_text: String) {
        // Mirror the inline text input back into the FSM.
        match &mut self.folder_edit {
            FolderEdit::NamingNew { name, .. } => *name = folder_text,
            FolderEdit::Renaming  { to, .. }   => *to   = folder_text,
            FolderEdit::Idle => {}
        }

        if let Some(id) = intent.start_new_folder {
            self.folder_edit = FolderEdit::NamingNew { texture_id: id, name: String::new() };
        }
        if let Some(from) = &intent.start_rename_folder {
            self.folder_edit = FolderEdit::Renaming { from: from.clone(), to: from.clone() };
        }
        if let Some((id, target)) = &intent.move_to_folder {
            self.tile_defs.set_folder(*id, target.clone());
            self.tile_defs.save();
        }
        if let Some(folder) = &intent.delete_folder {
            if self.tile_defs.clear_folder(folder) { self.tile_defs.save(); }
            // Drop a rename input that was targeting the now-gone folder.
            if matches!(&self.folder_edit, FolderEdit::Renaming { from, .. } if from == folder) {
                self.folder_edit = FolderEdit::Idle;
            }
        }
        if intent.folder_cancel {
            self.folder_edit = FolderEdit::Idle;
        }
        if intent.folder_confirm {
            match std::mem::replace(&mut self.folder_edit, FolderEdit::Idle) {
                FolderEdit::NamingNew { texture_id, name } => {
                    let name = name.trim().to_string();
                    if !name.is_empty() {
                        self.tile_defs.set_folder(texture_id, Some(name));
                        self.tile_defs.save();
                    }
                }
                FolderEdit::Renaming { from, to } => {
                    let to = to.trim().to_string();
                    if !to.is_empty() && to != from && self.tile_defs.rename_folder(&from, &to) {
                        self.tile_defs.save();
                    }
                }
                FolderEdit::Idle => {}
            }
        }
    }

    fn handle_resize(&mut self, intent: &EditorIntent, draft_w: usize, draft_h: usize) {
        if intent.open_resize_dialog  { self.resize_dialog = Some((self.world.width, self.world.height)); }
        if intent.close_resize_dialog { self.resize_dialog = None; }
        if let Some((nw, nh)) = intent.confirm_resize {
            self.world.resize(nw, nh);
            let mut units_changed = false;
            for record in &mut self.spawner_units {
                if record.clamp_to_bounds(nw, nh) { units_changed = true; }
            }
            if units_changed { self.save_units(); }
            self.world.save(&self.map_path);
            self.resize_dialog = None;
        }
        // Mirror draft values back while dialog is open.
        if let Some(ref mut d) = self.resize_dialog { *d = (draft_w, draft_h); }
    }

    fn handle_camera(&mut self, engine: &mut Engine, intent: &EditorIntent, screen_h: u32) {
        if let Some(delta) = intent.cam_pan_delta {
            engine.camera.0 -= (delta.x / self.zoom) as i32;
            engine.camera.1 += (delta.y / self.zoom) as i32;
        }
        if intent.cam_scroll_y != 0.0 {
            let old_z = self.zoom;
            self.zoom = (self.zoom * 1.1_f32.powf(intent.cam_scroll_y / 50.0)).clamp(0.125, 8.0);
            let new_z = self.zoom;
            if let Some(cur) = intent.cam_cursor {
                let cx = cur.x;
                let cy = screen_h as f32 - cur.y;
                engine.camera.0 += (cx * (1.0 / old_z - 1.0 / new_z)) as i32;
                engine.camera.1 += (cy * (1.0 / old_z - 1.0 / new_z)) as i32;
            }
        }
    }
}

impl GameContext for EditorContext {
    fn update(&mut self, _engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        if self.pending_exit {
            return Some(Box::new(MainMenuContext::new()));
        }
        None
    }

    fn draw(&mut self, engine: &mut Engine) {
        let mut intent = EditorIntent::default();

        // 1. On the first frame, place the camera so the map starts at the bottom-left.
        let (w, h) = engine.screen_size();
        if !self.camera_init {
            engine.camera = (0, 0);
            self.camera_init = true;
        }

        // 2. Draw the OpenGL world first so the egui overlay appears on top.
        self.world.draw(engine.camera, self.zoom);

        // Screen-space scalars + tile_rect helper used by both the GL pass below
        // and the egui closure further down.
        let ts    = TILE_SIZE as f32;
        let z     = self.zoom;
        let tsz   = ts * z;  // tile size in screen pixels at current zoom
        let cam_x = engine.camera.0 as f32;
        let cam_y = engine.camera.1 as f32;
        let sh    = h as f32;
        let map_w = self.world.width  as f32 * ts;
        let map_h = self.world.height as f32 * ts;
        let tile_rect = |tx: f32, ty: f32| -> egui::Rect {
            let x0 = (tx * ts - cam_x) * z;
            let y0 = sh - ((ty + 1.0) * ts - cam_y) * z;
            egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x0 + tsz, y0 + tsz))
        };

        // Single pass over placed units: draw the GL sprite (if any) and
        // accumulate the egui-side position marker overlay.
        let mut unit_overlay: Vec<(egui::Rect, egui::Color32)> = Vec::new();
        for record in &self.spawner_units {
            let gl_obj = record.sprite_id.and_then(|id| self.unit_sprite_cache.get(&id));
            for &(tx, ty) in &record.positions {
                let r = tile_rect(tx as f32, ty as f32);
                if let Some(obj) = gl_obj {
                    // GL y=0 is bottom; convert tile_rect's egui top-y into GL bottom-y.
                    obj.draw(r.min.x as i32, (sh - r.max.y) as i32, tsz);
                }
                unit_overlay.push((r, egui::Color32::from_rgba_unmultiplied(60, 200, 180, 160)));
            }
        }

        // 3. Precompute values needed inside the closure (avoids borrow conflicts with tile data).
        let active_tab       = self.active_tab;
        let physics_brush    = self.physics_brush_solid;
        let panel_open       = self.right_panel_open;
        // Folder-input snapshot: editable name + which mode (new vs rename) is active.
        let folder_naming   = matches!(self.folder_edit, FolderEdit::NamingNew { .. });
        let folder_renaming = matches!(self.folder_edit, FolderEdit::Renaming  { .. });
        let mut folder_text = match &self.folder_edit {
            FolderEdit::NamingNew { name, .. } => name.clone(),
            FolderEdit::Renaming  { to, .. }   => to.clone(),
            FolderEdit::Idle                   => String::new(),
        };
        let spawner_form_open  = matches!(self.spawner_mode, SpawnerMode::CreatingNew | SpawnerMode::Editing { .. });
        let spawner_is_editing = matches!(self.spawner_mode, SpawnerMode::Editing { .. });
        let is_patrol_painting = matches!(self.spawner_mode, SpawnerMode::PatrolPainting { .. });
        let patrol_unit_name: String = if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
            self.spawner_units.iter().find(|u| u.id == unit_id)
                .map(|u| format!("{} (instance {})", if u.name.is_empty() { "(unnamed)" } else { &u.name }, instance_idx))
                .unwrap_or_default()
        } else { String::new() };
        let spawner_brush       = self.spawner_brush;
        let mut draft_name     = self.spawner_draft.name.clone();
        let draft_sprite       = self.spawner_draft.sprite_id;
        let mut draft_health   = self.spawner_draft.health;
        let mut draft_speed    = self.spawner_draft.speed;

        let physics_overlay: Vec<(egui::Rect, egui::Color32)> =
            if active_tab == RightPanelTab::PhysicsPainter || active_tab == RightPanelTab::CharacterSpawner {
                self.world.tiles.iter().filter_map(|tile| {
                    let r = tile_rect(tile.position.0 as f32, tile.position.1 as f32);
                    if r.max.x < 0.0 || r.min.x > w as f32 || r.max.y < 0.0 || r.min.y > sh { return None; }
                    let color = if tile.physics.solid {
                        egui::Color32::from_rgba_unmultiplied(220, 60, 60, 140)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 50)
                    };
                    Some((r, color))
                }).collect()
            } else {
                vec![]
            };

        // Patrol waypoint nodes for the currently selected unit instance.
        let patrol_nodes: Vec<(egui::Rect, usize)> =
            if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
                self.spawner_units.iter()
                    .find(|u| u.id == unit_id)
                    .and_then(|r| r.patrols.get(instance_idx))
                    .map(|patrol| {
                        patrol.iter().enumerate().map(|(i, &(tx, ty))| {
                            (tile_rect(tx as f32, ty as f32), i)
                        }).collect()
                    })
                    .unwrap_or_default()
            } else { vec![] };

        // Player-spawn marker (only visible in the Spawner tab — like patrol
        // nodes and physics tint, it's editor metadata, not gameplay).
        let spawn_marker: Option<egui::Rect> = if active_tab == RightPanelTab::CharacterSpawner {
            self.world.spawn.map(|(sx, sy)| tile_rect(sx as f32, sy as f32))
        } else {
            None
        };

        let patrol_lines: Vec<(egui::Pos2, egui::Pos2)> = patrol_nodes.windows(2)
            .map(|w| (w[0].0.center(), w[1].0.center()))
            .collect();

        let resize_open = self.resize_dialog.is_some();
        let mut resize_draft_w = self.resize_dialog.map(|(w, _)| w).unwrap_or(self.world.width);
        let mut resize_draft_h = self.resize_dialog.map(|(_, h)| h).unwrap_or(self.world.height);

        // 4. egui overlay: toolbar + right panel + central paint input.
        let camera = engine.camera;
        let input  = engine.egui_input.clone();
        engine.renderer.render(input, w, h, |ctx| {
            // ── Toolbar ──
            egui::TopBottomPanel::top("editor_toolbar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked()   { intent.do_save = true; }
                    ui.separator();
                    if ui.button("Resize").clicked() { intent.open_resize_dialog = true; }
                    ui.separator();
                    if ui.button("Exit").clicked()   { intent.do_exit = true; }
                    ui.separator();
                    ui.label(format!("Map: {}", self.map_path));
                    ui.separator();
                    if is_patrol_painting {
                        ui.colored_label(
                            egui::Color32::from_rgb(100, 220, 100),
                            format!("Patrol: {} — left-click to add waypoint, right-click to remove, Enter to finish", patrol_unit_name),
                        );
                    } else {
                        match active_tab {
                            RightPanelTab::TexturePalette => {
                                match self.selected_id {
                                    Some(0)  => { ui.label("Brush: eraser"); }
                                    Some(id) => { ui.label(format!("Brush: tile {}", id)); }
                                    None     => { ui.colored_label(
                                        egui::Color32::from_rgb(220, 180, 60),
                                        "No tile selected — pick one from the panel",
                                    ); }
                                }
                            }
                            RightPanelTab::PhysicsPainter => {
                                ui.label(if physics_brush { "Brush: solid" } else { "Brush: passable" });
                            }
                            RightPanelTab::CharacterSpawner => {
                                match spawner_brush {
                                    SpawnerBrush::PlayerSpawn => {
                                        ui.label("Brush: player spawn — left-click to place (max 1 per map), right-click to clear");
                                    }
                                    SpawnerBrush::Unit(id) => {
                                        match self.spawner_units.iter().find(|u| u.id == id) {
                                            Some(u) => { ui.label(format!("Brush: {} — left-click to place / edit patrol, right-click to delete", u.name)); }
                                            None    => { ui.colored_label(
                                                egui::Color32::from_rgb(220, 180, 60),
                                                "Selected unit is gone — pick another from the panel",
                                            ); }
                                        }
                                    }
                                    SpawnerBrush::None => {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(220, 180, 60),
                                            "No brush selected — pick one from the panel (right-click to delete any placed unit)",
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Panel toggle button — rightmost item.
                        let toggle_label = if panel_open { "»" } else { "«" };
                        if ui.button(toggle_label).clicked() { intent.toggle_panel = true; }
                        ui.separator();
                        ui.label(format!("{}×{}", self.world.width, self.world.height));
                    });
                });
            });

            // ── Right panel with tabs (conditionally shown, never during patrol painting) ──
            if panel_open && !is_patrol_painting {
                egui::SidePanel::right("right_panel")
                    .min_width(180.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if ui.selectable_label(active_tab == RightPanelTab::TexturePalette,  "Textures").clicked() {
                                intent.new_tab = Some(RightPanelTab::TexturePalette);
                            }
                            if ui.selectable_label(active_tab == RightPanelTab::PhysicsPainter,  "Physics").clicked() {
                                intent.new_tab = Some(RightPanelTab::PhysicsPainter);
                            }
                            if ui.selectable_label(active_tab == RightPanelTab::CharacterSpawner, "Spawner").clicked() {
                                intent.new_tab = Some(RightPanelTab::CharacterSpawner);
                            }
                        });
                        ui.separator();

                        match active_tab {
                            RightPanelTab::TexturePalette => {
                                // Inline folder-name input (creating a new folder or renaming one).
                                if folder_naming || folder_renaming {
                                    ui.label(if folder_naming { "New folder name:" } else { "Rename folder:" });
                                    ui.text_edit_singleline(&mut folder_text);
                                    ui.horizontal(|ui| {
                                        if ui.button("Confirm").clicked() { intent.folder_confirm = true; }
                                        if ui.button("Cancel").clicked()  { intent.folder_cancel  = true; }
                                    });
                                    ui.separator();
                                }

                                if ui.selectable_label(self.selected_id == Some(0), "(eraser)").clicked() {
                                    intent.new_selected = Some(0);
                                }
                                ui.separator();
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    // The tile palette only paints map tiles, so it shows
                                    // Tiles-category defs only. Creatures/Objects live in the
                                    // Definitions tab of the tileset editor (and the spawner).
                                    let is_tile = |id: i32| self.tile_defs.category_of(id) == Category::Tiles;
                                    // Folders are implicit: the sorted set of folder names referenced
                                    // by any Tiles-category entry in the current palette.
                                    let folders: Vec<String> = {
                                        let mut set = std::collections::BTreeSet::new();
                                        for e in &self.palette {
                                            if e.id == 0 || !is_tile(e.id) { continue; }
                                            if let Some(f) = self.tile_defs.folder_of(e.id) {
                                                set.insert(f);
                                            }
                                        }
                                        set.into_iter().collect()
                                    };
                                    let solid_of  = |id: i32| self.tile_defs.solid_of(id);
                                    let folder_of = |id: i32| self.tile_defs.folder_of(id);
                                    // Prefer the tile-def name; fall back to the PNG filename
                                    // so unnamed tiles are still distinguishable in the row.
                                    let label_of = |e: &PaletteEntry| -> String {
                                        match self.tile_defs.name_of(e.id) {
                                            Some(n) if !n.is_empty() => format!("{} | {}", e.id, n),
                                            _                        => format!("{} | {}", e.id, e.path),
                                        }
                                    };

                                    // Folders first — each a collapsible group with its own ... menu.
                                    for folder in &folders {
                                        let header_id = ui.make_persistent_id(("tex_folder", folder));
                                        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, true)
                                            .show_header(ui, |ui| {
                                                ui.label(folder);
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    ui.menu_button("...", |ui| {
                                                        if ui.button("Rename").clicked() {
                                                            intent.start_rename_folder = Some(folder.clone());
                                                            ui.close();
                                                        }
                                                        if ui.button("Delete folder").clicked() {
                                                            intent.delete_folder = Some(folder.clone());
                                                            ui.close();
                                                        }
                                                    });
                                                });
                                            })
                                            .body(|ui| {
                                                for entry in &self.palette {
                                                    if entry.id == 0 || !is_tile(entry.id) { continue; }
                                                    if folder_of(entry.id).as_deref() != Some(folder.as_str()) { continue; }
                                                    let selected = self.selected_id == Some(entry.id);
                                                    let label = label_of(entry);
                                                    texture_row(ui, entry, &label, selected, solid_of(entry.id), Some(folder.as_str()), &folders, &mut intent);
                                                }
                                            });
                                    }

                                    // Loose textures (no folder) below the folders.
                                    for entry in &self.palette {
                                        if entry.id == 0 || !is_tile(entry.id) { continue; }
                                        if folder_of(entry.id).is_some() { continue; }
                                        let selected = self.selected_id == Some(entry.id);
                                        let label = label_of(entry);
                                        texture_row(ui, entry, &label, selected, solid_of(entry.id), None, &folders, &mut intent);
                                    }
                                });
                            }
                            RightPanelTab::PhysicsPainter => {
                                ui.label("Paint physics onto tiles:");
                                ui.add_space(4.0);
                                if ui.selectable_label(physics_brush,  "Solid").clicked()    { intent.new_physics_brush = Some(true);  }
                                if ui.selectable_label(!physics_brush, "Passable").clicked() { intent.new_physics_brush = Some(false); }
                                ui.add_space(8.0);
                                ui.separator();
                                ui.label("Red   = solid");
                                ui.label("White = passable");
                            }
                            RightPanelTab::CharacterSpawner => {
                                if !spawner_form_open {
                                    // Pinned "Player Spawn" brush at the top — singleton across
                                    // the whole map, shown as a green tag here and an "S" marker
                                    // on the editor map. Indicator (set) / (unset) tells the user
                                    // whether a spawn has been placed for the current map.
                                    let spawn_active = spawner_brush == SpawnerBrush::PlayerSpawn;
                                    let spawn_label = if self.world.spawn.is_some() {
                                        "Player Spawn (set)"
                                    } else {
                                        "Player Spawn (unset)"
                                    };
                                    if ui.selectable_label(spawn_active, spawn_label).clicked() {
                                        intent.new_spawner_brush = Some(SpawnerBrush::PlayerSpawn);
                                    }
                                    ui.separator();

                                    // Unit template list — click a row to select as brush
                                    egui::ScrollArea::vertical()
                                        .id_salt("spawner_list")
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            for (i, unit) in self.spawner_units.iter().enumerate() {
                                                let is_selected = matches!(spawner_brush, SpawnerBrush::Unit(id) if id == unit.id);
                                                let label = format!(
                                                    "{} ({})",
                                                    if unit.name.is_empty() { "(unnamed)" } else { &unit.name },
                                                    unit.positions.len(),
                                                );
                                                ui.horizontal(|ui| {
                                                    if ui.selectable_label(is_selected, &label).clicked() {
                                                        intent.new_spawner_brush = Some(SpawnerBrush::Unit(unit.id));
                                                    }
                                                    if ui.small_button("Edit").clicked() {
                                                        intent.spawner_form = Some(SpawnerFormAction::OpenEdit(i));
                                                    }
                                                    if ui.small_button("Del").clicked() {
                                                        intent.delete_spawner_id = Some(unit.id);
                                                    }
                                                });
                                            }
                                        });
                                    if !self.spawner_units.is_empty() { ui.separator(); }
                                    if ui.button("+ Create New").clicked() {
                                        intent.spawner_form = Some(SpawnerFormAction::OpenCreate);
                                    }
                                } else {
                                    // Create / Edit form
                                    ui.label(if spawner_is_editing { "Edit Unit" } else { "New Unit" });
                                    ui.separator();
                                    ui.label("Name:");
                                    ui.text_edit_singleline(&mut draft_name);
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Sprite:");
                                        if ui.small_button("+ Add PNG").clicked() {
                                            intent.add_unit_sprite_png = true;
                                        }
                                    });
                                    egui::ScrollArea::vertical()
                                        .id_salt("spawner_sprite")
                                        .max_height(120.0)
                                        .show(ui, |ui| {
                                            // Creatures only — units are drawn from creature defs.
                                            // Move a def to Creatures in the tileset editor to make
                                            // it selectable here (or use "+ Add PNG" above).
                                            for entry in &self.palette {
                                                if entry.id == 0 { continue; }
                                                if self.tile_defs.category_of(entry.id) != Category::Creatures { continue; }
                                                let selected = draft_sprite == Some(entry.id);
                                                if ui.selectable_label(selected, entry.display()).clicked() {
                                                    intent.new_draft_sprite = Some(entry.id);
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
                                    let confirm_label = if spawner_is_editing { "Save" } else { "Create" };
                                    ui.horizontal(|ui| {
                                        if ui.button(confirm_label).clicked() { intent.spawner_form = Some(SpawnerFormAction::Confirm); }
                                        if ui.button("Cancel").clicked()      { intent.spawner_form = Some(SpawnerFormAction::Cancel);  }
                                    });
                                }
                            }
                        }
                    });
            }

            // ── Resize dialog ──
            if resize_open {
                egui::Window::new("Resize Map")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        egui::Grid::new("resize_grid").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                            ui.label("Width:");
                            ui.add(egui::DragValue::new(&mut resize_draft_w).range(1..=512));
                            ui.end_row();
                            ui.label("Height:");
                            ui.add(egui::DragValue::new(&mut resize_draft_h).range(1..=512));
                            ui.end_row();
                        });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("Confirm").clicked() {
                                intent.confirm_resize = Some((resize_draft_w, resize_draft_h));
                            }
                            if ui.button("Cancel").clicked() {
                                intent.close_resize_dialog = true;
                            }
                        });
                    });
            }

            // ── Central panel: gridlines + map border + physics overlay + paint input ──
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let painter = ui.painter_at(rect);

                // Grid lines.
                let grid_stroke = egui::Stroke::new(1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30));
                let first_tx = (cam_x / ts).floor() as i32;
                let last_tx  = ((cam_x + rect.width()  / z) / ts).ceil() as i32 + 1;
                for tx in first_tx..=last_tx {
                    let sx = (tx as f32 * ts - cam_x) * z;
                    painter.line_segment(
                        [egui::pos2(sx, rect.top()), egui::pos2(sx, rect.bottom())],
                        grid_stroke,
                    );
                }
                let first_ty = (cam_y / ts).floor() as i32;
                let last_ty  = ((cam_y + rect.height() / z) / ts).ceil() as i32 + 1;
                for ty in first_ty..=last_ty {
                    let sy = sh - (ty as f32 * ts - cam_y) * z;
                    painter.line_segment(
                        [egui::pos2(rect.left(), sy), egui::pos2(rect.right(), sy)],
                        grid_stroke,
                    );
                }

                // Map border outline.
                let border_rect = egui::Rect::from_min_max(
                    egui::pos2(-cam_x * z,              sh - (map_h - cam_y) * z),
                    egui::pos2((map_w - cam_x) * z,     sh + cam_y * z),
                );
                painter.rect_stroke(
                    border_rect, 0.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 210, 50)),
                    egui::StrokeKind::Outside,
                );

                // Physics tint overlay.
                for (tile_rect, color) in &physics_overlay {
                    painter.rect_filled(*tile_rect, 0.0, *color);
                }

                // Unit position markers.
                for (tile_rect, color) in &unit_overlay {
                    painter.rect_filled(*tile_rect, 0.0, *color);
                }

                // Patrol waypoint nodes + connecting lines.
                for (a, b) in &patrol_lines {
                    painter.line_segment(
                        [*a, *b],
                        egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(100, 220, 100, 200)),
                    );
                }
                for &(node_rect, idx) in &patrol_nodes {
                    painter.rect_filled(node_rect, 4.0, egui::Color32::from_rgba_unmultiplied(80, 210, 80, 140));
                    painter.text(
                        node_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        idx.to_string(),
                        egui::FontId::proportional(13.0),
                        egui::Color32::WHITE,
                    );
                }

                // Player-spawn marker — bright yellow tile with an "S" label,
                // distinct from the green patrol nodes.
                if let Some(rect) = spawn_marker {
                    painter.rect_filled(
                        rect, 2.0,
                        egui::Color32::from_rgba_unmultiplied(255, 210, 50, 170),
                    );
                    painter.rect_stroke(
                        rect, 2.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 230, 90)),
                        egui::StrokeKind::Outside,
                    );
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "S",
                        egui::FontId::proportional(16.0),
                        egui::Color32::from_rgb(40, 30, 10),
                    );
                }

                // Paint input: held for texture/physics, single press for spawner/patrol.
                let (primary_down, primary_pressed, secondary_pressed, secondary_down, pointer_pos, esc,
                     scroll_y, mid_down, mid_delta) =
                    ctx.input(|i| (
                        i.pointer.primary_down(), i.pointer.primary_pressed(),
                        i.pointer.secondary_pressed(), i.pointer.secondary_down(), i.pointer.hover_pos(),
                        i.key_pressed(egui::Key::Enter),
                        i.raw_scroll_delta.y,
                        i.pointer.middle_down(),
                        i.pointer.delta(),
                    ));
                if esc { intent.patrol_esc = true; }
                if scroll_y != 0.0 { intent.cam_scroll_y = scroll_y; intent.cam_cursor = pointer_pos; }
                if mid_down && mid_delta != egui::Vec2::ZERO { intent.cam_pan_delta = Some(mid_delta); }
                if let Some(pos) = pointer_pos {
                    if rect.contains(pos) {
                        if is_patrol_painting {
                            if primary_pressed   { intent.patrol_click_pos = Some(pos); }
                            if secondary_pressed { intent.patrol_erase_pos = Some(pos); }
                        } else {
                            if primary_down      { intent.paint_pos = Some(pos); }
                            if primary_pressed   { intent.click_pos = Some(pos); }
                            if secondary_pressed { intent.erase_pos = Some(pos); }
                            if secondary_down    { intent.erase_paint_pos = Some(pos); }
                        }
                    }
                }
                ui.allocate_rect(rect, egui::Sense::click_and_drag());
            });
        });

        // 5. Dispatch the intents collected above.
        if intent.toggle_panel                  { self.right_panel_open    = !self.right_panel_open; }
        if let Some(tab)   = intent.new_tab     { self.active_tab          = tab;       }
        if let Some(brush) = intent.new_physics_brush { self.physics_brush_solid = brush; }
        if let Some(id)    = intent.new_selected      { self.selected_id   = Some(id);  }
        if let Some((id, solid)) = intent.set_texture_solid {
            self.tile_defs.set_solid(id, solid);
            self.tile_defs.save();
        }

        self.handle_paint(active_tab, &intent, h, camera);
        self.handle_unit_clicks(active_tab, &intent, h, camera);
        self.handle_patrol(&intent, h, camera);
        self.handle_spawner(&intent, draft_name, draft_health, draft_speed);
        self.handle_add_unit_sprite_png(&intent);
        self.handle_folders(&intent, folder_text);
        self.handle_resize(&intent, resize_draft_w, resize_draft_h);
        self.handle_camera(engine, &intent, h);

        if intent.do_save { self.world.save(&self.map_path); }
        if intent.do_exit { self.pending_exit = true; }
    }
}

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
// ── Adding a new tab to the right panel ──────────────────────────────────────
// The right panel has three tabs controlled by `RightPanelTab`.  To add one:
//   1. Add a variant to `RightPanelTab` (e.g. `EventPainter`).
//   2. In `draw`, find the `ui.selectable_label` row and add your tab button.
//   3. Add a `RightPanelTab::EventPainter => { … }` arm to the `match active_tab`
//      block to render the panel content.
//   4. Handle any paint/click input in the "Act on flags" section (step 5).
//
// ── Adding a new field / feature to EditorContext ────────────────────────────
// All runtime state lives in `EditorContext`.  The pattern is:
//   1. Add the field (e.g. `my_flag: bool`).
//   2. Initialise it in both `from_file` and `new_map`.
//   3. Declare a local `let mut my_flag = false;` at the top of `draw`.
//   4. Set it inside the egui closure (`if ui.button(…).clicked() { my_flag = true; }`).
//   5. After the closure, act on it (`if my_flag { self.do_thing(); }`).
//   This two-phase pattern (collect intent inside closure, act outside) is
//   necessary because the egui closure borrows `engine` exclusively — you cannot
//   call `&mut self` methods inside it.
//
// ── Persisting new data ───────────────────────────────────────────────────────
// Map tile data is saved via `World::save` (called when `do_save` is true).
// Unit templates and their placements live in `units.toml` and are written by
// `save_units()`.  If you add a new type of placed object, follow the same
// pattern: a TOML-backed file, a `load_*` helper called in both constructors,
// and a `save_*` helper called whenever state changes.
//
// ── Key helpers ───────────────────────────────────────────────────────────────
//   screen_to_tile_idx  — converts an egui Pos2 into an index into world.tiles
//   tile_display_name   — controls how palette entries are labelled in the list
//   build_sprite_cache  — loads GLObjects keyed by palette id for drawing sprites
// ══════════════════════════════════════════════════════════════════════════════

use RustEngine::game_engine::{Engine, GameContext, World, TILE_SIZE, UnitRecord, UnitFile};
use RustEngine::tools::{load_textures, GLObject, BL_RECTANGLE};
use RustEngine::shaders::{VERT_SHADER, FRAG_SHADER};
use std::collections::HashMap;

use super::menus::MainMenuContext;

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

enum TexNewState {
    Idle,
    Conflict { source: std::path::PathBuf, proposed_name: String },
    Renaming { source: std::path::PathBuf, new_name: String },
}

enum SpawnerMode {
    Idle,
    CreatingNew,
    Editing { index: usize },
    PatrolPainting { unit_id: u32, instance_idx: usize },
}

/// Ephemeral editor state while creating or editing a Unit — never serialized.
#[derive(Clone)]
struct UnitDraft {
    name: String,
    sprite_id: Option<i32>,
    // Future fields: stats: UnitStats, feats: Vec<String>
}

impl UnitDraft {
    fn new() -> Self {
        UnitDraft { name: String::new(), sprite_id: None }
    }
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
    id_path: String,
    tex_new_state: TexNewState,
    spawner_mode: SpawnerMode,
    spawner_draft: UnitDraft,
    spawner_units: Vec<UnitRecord>,
    /// The id of the template currently selected as the active placement brush.
    selected_spawner_id: Option<u32>,
    /// Cached GLObjects keyed by palette id, used to draw unit sprites in the editor.
    unit_sprite_cache: HashMap<i32, GLObject>,
    /// Panel state saved when entering patrol painting mode, restored on exit.
    patrol_panel_was_open: bool,
}

impl EditorContext {
    /// Open an existing map file for editing.
    pub fn from_file(map_path: &str, id_path: &str) -> Result<Self, String> {
        if !std::path::Path::new(map_path).exists() {
            return Err(format!("Map file not found: {}", map_path));
        }
        if !std::path::Path::new(id_path).exists() {
            return Err(format!("ID file not found: {}", id_path));
        }
        let palette = Self::load_palette(id_path);
        let unit_sprite_cache = Self::build_sprite_cache(&palette);
        Ok(EditorContext {
            world: World::load(map_path, id_path),
            map_path: map_path.to_string(),
            palette,
            selected_id: None,
            pending_exit: false,
            camera_init: false,
            active_tab: RightPanelTab::TexturePalette,
            physics_brush_solid: true,
            right_panel_open: true,
            id_path: id_path.to_string(),
            tex_new_state: TexNewState::Idle,
            spawner_mode: SpawnerMode::Idle,
            spawner_draft: UnitDraft::new(),
            spawner_units: Self::load_units(),
            selected_spawner_id: None,
            unit_sprite_cache,
            patrol_panel_was_open: true,
        })
    }

    /// Create a new blank map of the given dimensions.
    pub fn new_map(map_path: &str, id_path: &str, width: usize, height: usize) -> Result<Self, String> {
        if !std::path::Path::new(id_path).exists() {
            return Err(format!("ID file not found: {}", id_path));
        }
        let palette = Self::load_palette(id_path);
        let unit_sprite_cache = Self::build_sprite_cache(&palette);
        Ok(EditorContext {
            world: World::new_empty(width, height),
            map_path: map_path.to_string(),
            palette,
            selected_id: None,
            pending_exit: false,
            camera_init: false,
            active_tab: RightPanelTab::TexturePalette,
            physics_brush_solid: true,
            right_panel_open: true,
            id_path: id_path.to_string(),
            tex_new_state: TexNewState::Idle,
            spawner_mode: SpawnerMode::Idle,
            spawner_draft: UnitDraft::new(),
            spawner_units: Self::load_units(),
            selected_spawner_id: None,
            unit_sprite_cache,
            patrol_panel_was_open: true,
        })
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

    /// Scan `assets/` for PNG files and reconcile with the id file.
    /// Files in the id file that no longer exist on disk are dropped.
    /// New files found in assets get auto-assigned IDs.
    fn load_palette(id_path: &str) -> Vec<PaletteEntry> {
        // Load existing id→path mappings from the id file.
        let existing: HashMap<String, i32> = if std::path::Path::new(id_path).exists() {
            load_textures(id_path).into_iter().map(|(id, path)| (path, id)).collect()
        } else {
            HashMap::new()
        };

        // Scan assets/ for PNGs — these are the source of truth.
        let mut png_files: Vec<String> = std::fs::read_dir("assets")
            .map(|dir| {
                dir.filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        if name.to_lowercase().ends_with(".png") { Some(name) } else { None }
                    })
                    .collect()
            })
            .unwrap_or_default();
        png_files.sort();

        let mut next_id = existing.values().copied().max().unwrap_or(0) + 1;
        let mut entries: Vec<PaletteEntry> = png_files.into_iter().map(|filename| {
            let id = existing.get(&filename).copied().unwrap_or_else(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            PaletteEntry { id, path: filename }
        }).collect();

        entries.sort_by_key(|e| e.id);
        entries
    }

    fn load_units() -> Vec<UnitRecord> {
        if !std::path::Path::new("units.toml").exists() { return Vec::new(); }
        let content = std::fs::read_to_string("units.toml").unwrap_or_default();
        toml::from_str::<UnitFile>(&content)
            .map(|f| f.unit)
            .unwrap_or_default()
    }

    fn save_units(&self) {
        let file    = UnitFile { unit: self.spawner_units.clone() };
        let content = toml::to_string(&file).expect("Failed to serialize units");
        std::fs::write("units.toml", content).expect("Failed to save units.toml");
    }

    fn save_palette(&self) {
        let content = self.palette.iter()
            .filter(|e| e.id != 0)
            .map(|e| format!("{} {}", e.id, e.path))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&self.id_path, content).expect("Failed to save id file");
    }

    /// Copy `source` into `assets/<filename>` and register it in the palette + id file.
    fn register_texture(&mut self, source: &std::path::Path, filename: &str) {
        let dest = std::path::Path::new("assets").join(filename);
        std::fs::copy(source, &dest).expect("Failed to copy texture into assets/");
        let new_id = self.palette.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        self.palette.push(PaletteEntry { id: new_id, path: filename.to_string() });
        self.palette.sort_by_key(|e| e.id);
        self.save_palette();
        self.unit_sprite_cache = Self::build_sprite_cache(&self.palette);
    }

    /// Convert a screen-space position (egui coords, y=0 at top) to a
    /// tile index into `self.world.tiles`, accounting for the engine camera.
    /// World coords are snapped down to the nearest TILE_SIZE multiple first.
    fn screen_to_tile_idx(&self, sx: f32, sy: f32, screen_h: u32, camera: (i32, i32)) -> Option<usize> {
        // OpenGL y=0 is bottom; egui y=0 is top — flip y.
        let world_x = sx as i32 + camera.0;
        let world_y = (screen_h as i32 - sy as i32) + camera.1;
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
}

impl GameContext for EditorContext {
    fn update(&mut self, _engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        if self.pending_exit {
            return Some(Box::new(MainMenuContext::new()));
        }
        None
    }

    fn draw(&mut self, engine: &mut Engine) {
        let mut do_save            = false;
        let mut do_exit            = false;
        let mut toggle_panel       = false;
        let mut new_selected:      Option<i32>           = None;
        let mut new_tab:           Option<RightPanelTab> = None;
        let mut new_physics_brush: Option<bool>          = None;
        let mut paint_pos:  Option<egui::Pos2> = None;
        let mut click_pos:  Option<egui::Pos2> = None;
        let mut erase_pos:  Option<egui::Pos2> = None;
        let mut open_create        = false;
        let mut cancel_create      = false;
        let mut confirm_create     = false;
        let mut new_draft_sprite:     Option<i32>  = None;
        let mut open_edit:            Option<usize> = None;
        let mut new_selected_spawner: Option<u32>  = None;
        let mut delete_spawner_id:    Option<u32>  = None;
        let mut tex_new_clicked    = false;
        let mut tex_overwrite      = false;
        let mut tex_start_rename   = false;
        let mut tex_confirm_rename = false;
        let mut tex_cancel_new     = false;
        let mut delete_texture_id: Option<i32> = None;
        let mut patrol_click_pos:  Option<egui::Pos2> = None;
        let mut patrol_erase_pos:  Option<egui::Pos2> = None;
        let mut patrol_esc         = false;

        // 1. On the first frame, place the camera so the map starts at the bottom-left.
        let (w, h) = engine.screen_size();
        if !self.camera_init {
            engine.camera = (0, 0);
            self.camera_init = true;
        }

        // 2. Draw the OpenGL world first so the egui overlay appears on top.
        self.world.draw(engine.camera);

        // Draw placed unit sprites at their tile positions.
        for record in &self.spawner_units {
            if let Some(sprite_id) = record.sprite_id {
                if let Some(gl_obj) = self.unit_sprite_cache.get(&sprite_id) {
                    for &(tx, ty) in &record.positions {
                        gl_obj.draw(
                            tx * TILE_SIZE - engine.camera.0,
                            ty * TILE_SIZE - engine.camera.1,
                            TILE_SIZE as f32,
                        );
                    }
                }
            }
        }

        // 3. Precompute values needed inside the closure (avoids borrow conflicts with tile data).
        let active_tab       = self.active_tab;
        let physics_brush    = self.physics_brush_solid;
        let panel_open       = self.right_panel_open;
        let tex_in_conflict  = matches!(self.tex_new_state, TexNewState::Conflict { .. });
        let tex_in_renaming  = matches!(self.tex_new_state, TexNewState::Renaming  { .. });
        let (conflict_filename, mut rename_text) = match &self.tex_new_state {
            TexNewState::Conflict { proposed_name, .. } => (proposed_name.clone(), proposed_name.clone()),
            TexNewState::Renaming { new_name, .. }      => (String::new(),          new_name.clone()),
            TexNewState::Idle                           => (String::new(),          String::new()),
        };
        let spawner_form_open  = matches!(self.spawner_mode, SpawnerMode::CreatingNew | SpawnerMode::Editing { .. });
        let spawner_is_editing = matches!(self.spawner_mode, SpawnerMode::Editing { .. });
        let is_patrol_painting = matches!(self.spawner_mode, SpawnerMode::PatrolPainting { .. });
        let patrol_unit_name: String = if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
            self.spawner_units.iter().find(|u| u.id == unit_id)
                .map(|u| format!("{} (instance {})", if u.name.is_empty() { "(unnamed)" } else { &u.name }, instance_idx))
                .unwrap_or_default()
        } else { String::new() };
        let selected_spawner_id = self.selected_spawner_id;
        let mut draft_name     = self.spawner_draft.name.clone();
        let draft_sprite       = self.spawner_draft.sprite_id;
        let ts            = TILE_SIZE as f32;
        let cam_x         = engine.camera.0 as f32;
        let cam_y         = engine.camera.1 as f32;
        let sh            = h as f32;
        let map_w         = self.world.width  as f32 * ts;
        let map_h         = self.world.height as f32 * ts;

        let physics_overlay: Vec<(egui::Rect, egui::Color32)> =
            if active_tab == RightPanelTab::PhysicsPainter || active_tab == RightPanelTab::CharacterSpawner {
                self.world.tiles.iter().filter_map(|tile| {
                    let tx = tile.position.0 as f32;
                    let ty = tile.position.1 as f32;
                    let x0 = tx * ts - cam_x;
                    let y0 = sh - (ty + 1.0) * ts + cam_y;
                    let x1 = x0 + ts;
                    let y1 = y0 + ts;
                    if x1 < 0.0 || x0 > w as f32 || y1 < 0.0 || y0 > sh { return None; }
                    let color = if tile.physics.solid {
                        egui::Color32::from_rgba_unmultiplied(220, 60, 60, 140)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 50)
                    };
                    Some((egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)), color))
                }).collect()
            } else {
                vec![]
            };

        // Unit position markers — always visible regardless of active tab.
        let unit_overlay: Vec<(egui::Rect, egui::Color32)> = self.spawner_units.iter()
            .flat_map(|record| record.positions.iter().map(|&(tx, ty)| {
                let x0 = tx as f32 * ts - cam_x;
                let y0 = sh - (ty as f32 + 1.0) * ts + cam_y;
                (
                    egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x0 + ts, y0 + ts)),
                    egui::Color32::from_rgba_unmultiplied(60, 200, 180, 160),
                )
            }))
            .collect();

        // Patrol waypoint nodes for the currently selected unit instance.
        let patrol_nodes: Vec<(egui::Rect, usize)> =
            if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
                self.spawner_units.iter()
                    .find(|u| u.id == unit_id)
                    .and_then(|r| r.patrols.get(instance_idx))
                    .map(|patrol| {
                        patrol.iter().enumerate().map(|(i, &(tx, ty))| {
                            let x0 = tx as f32 * ts - cam_x;
                            let y0 = sh - (ty as f32 + 1.0) * ts + cam_y;
                            (egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x0 + ts, y0 + ts)), i)
                        }).collect()
                    })
                    .unwrap_or_default()
            } else { vec![] };

        let patrol_lines: Vec<(egui::Pos2, egui::Pos2)> = patrol_nodes.windows(2)
            .map(|w| (w[0].0.center(), w[1].0.center()))
            .collect();

        // 4. egui overlay: toolbar + right panel + central paint input.
        let camera = engine.camera;
        let input  = engine.egui_input.clone();
        engine.renderer.render(input, w, h, |ctx| {
            // ── Toolbar ──
            egui::TopBottomPanel::top("editor_toolbar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() { do_save = true; }
                    ui.separator();
                    if ui.button("Exit").clicked() { do_exit = true; }
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
                                Some(0) => { ui.label("Brush: eraser"); }
                                Some(id) => { ui.label(format!("Brush: tile {}", id)); }
                                None    => { ui.colored_label(
                                    egui::Color32::from_rgb(220, 180, 60),
                                    "No tile selected — pick one from the panel",
                                ); }
                            }
                        }
                        RightPanelTab::PhysicsPainter => {
                            let label = if physics_brush { "Brush: solid" } else { "Brush: passable" };
                            ui.label(label);
                        }
                        RightPanelTab::CharacterSpawner => {
                            match selected_spawner_id.and_then(|id| {
                                self.spawner_units.iter().find(|u| u.id == id)
                            }) {
                                Some(u) => { ui.label(format!("Brush: {} — left-click placed unit to edit patrol", u.name)); }
                                None    => { ui.colored_label(
                                    egui::Color32::from_rgb(220, 180, 60),
                                    "No unit selected — pick one from the panel",
                                ); }
                            }
                        }
                    }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Panel toggle button — rightmost item.
                        let toggle_label = if panel_open { "»" } else { "«" };
                        if ui.button(toggle_label).clicked() { toggle_panel = true; }
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
                                new_tab = Some(RightPanelTab::TexturePalette);
                            }
                            if ui.selectable_label(active_tab == RightPanelTab::PhysicsPainter,  "Physics").clicked() {
                                new_tab = Some(RightPanelTab::PhysicsPainter);
                            }
                            if ui.selectable_label(active_tab == RightPanelTab::CharacterSpawner, "Spawner").clicked() {
                                new_tab = Some(RightPanelTab::CharacterSpawner);
                            }
                        });
                        ui.separator();

                        match active_tab {
                            RightPanelTab::TexturePalette => {
                                // Conflict / rename prompts shown at the top of the panel.
                                if tex_in_conflict {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(220, 180, 60),
                                        format!("'{}' already exists.", conflict_filename),
                                    );
                                    ui.horizontal(|ui| {
                                        if ui.button("Overwrite").clicked() { tex_overwrite    = true; }
                                        if ui.button("Rename").clicked()    { tex_start_rename = true; }
                                        if ui.button("Cancel").clicked()    { tex_cancel_new   = true; }
                                    });
                                    ui.separator();
                                } else if tex_in_renaming {
                                    ui.label("New filename:");
                                    ui.text_edit_singleline(&mut rename_text);
                                    ui.horizontal(|ui| {
                                        if ui.button("Confirm").clicked() { tex_confirm_rename = true; }
                                        if ui.button("Cancel").clicked()  { tex_cancel_new     = true; }
                                    });
                                    ui.separator();
                                }

                                if ui.selectable_label(self.selected_id == Some(0), "0 | (eraser)").clicked() {
                                    new_selected = Some(0);
                                }
                                ui.separator();
                                if ui.button("+ New Texture").clicked() { tex_new_clicked = true; }
                                ui.separator();
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    for entry in &self.palette {
                                        if entry.id == 0 { continue; }
                                        let selected = self.selected_id == Some(entry.id);
                                        ui.horizontal(|ui| {
                                            if ui.selectable_label(selected, entry.display()).clicked() {
                                                new_selected = Some(entry.id);
                                            }
                                            if ui.small_button("Del").clicked() {
                                                delete_texture_id = Some(entry.id);
                                            }
                                        });
                                    }
                                });
                            }
                            RightPanelTab::PhysicsPainter => {
                                ui.label("Paint physics onto tiles:");
                                ui.add_space(4.0);
                                if ui.selectable_label(physics_brush,  "Solid").clicked()    { new_physics_brush = Some(true);  }
                                if ui.selectable_label(!physics_brush, "Passable").clicked() { new_physics_brush = Some(false); }
                                ui.add_space(8.0);
                                ui.separator();
                                ui.label("Red   = solid");
                                ui.label("White = passable");
                            }
                            RightPanelTab::CharacterSpawner => {
                                if !spawner_form_open {
                                    // Unit template list — click a row to select as brush
                                    egui::ScrollArea::vertical()
                                        .id_salt("spawner_list")
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            for (i, unit) in self.spawner_units.iter().enumerate() {
                                                let is_selected = selected_spawner_id == Some(unit.id);
                                                let label = format!(
                                                    "{} ({})",
                                                    if unit.name.is_empty() { "(unnamed)" } else { &unit.name },
                                                    unit.positions.len(),
                                                );
                                                ui.horizontal(|ui| {
                                                    if ui.selectable_label(is_selected, &label).clicked() {
                                                        new_selected_spawner = Some(unit.id);
                                                    }
                                                    if ui.small_button("Edit").clicked() {
                                                        open_edit = Some(i);
                                                    }
                                                    if ui.small_button("Del").clicked() {
                                                        delete_spawner_id = Some(unit.id);
                                                    }
                                                });
                                            }
                                        });
                                    if !self.spawner_units.is_empty() { ui.separator(); }
                                    if ui.button("+ Create New").clicked() { open_create = true; }
                                } else {
                                    // Create / Edit form
                                    let title = if spawner_is_editing { "Edit Unit" } else { "New Unit" };
                                    ui.label(title);
                                    ui.separator();
                                    ui.label("Name:");
                                    ui.text_edit_singleline(&mut draft_name);
                                    ui.add_space(6.0);
                                    ui.label("Sprite:");
                                    egui::ScrollArea::vertical()
                                        .id_salt("spawner_sprite")
                                        .max_height(120.0)
                                        .show(ui, |ui| {
                                            for entry in &self.palette {
                                                if entry.id == 0 { continue; }
                                                let selected = draft_sprite == Some(entry.id);
                                                if ui.selectable_label(selected, entry.display()).clicked() {
                                                    new_draft_sprite = Some(entry.id);
                                                }
                                            }
                                        });
                                    ui.add_space(6.0);
                                    ui.collapsing("Stats", |ui| { ui.label("(not yet implemented)"); });
                                    ui.add_space(2.0);
                                    ui.collapsing("Feats", |ui| { ui.label("(not yet implemented)"); });
                                    ui.add_space(8.0);
                                    let confirm_label = if spawner_is_editing { "Save" } else { "Create" };
                                    ui.horizontal(|ui| {
                                        if ui.button(confirm_label).clicked() { confirm_create = true; }
                                        if ui.button("Cancel").clicked()      { cancel_create  = true; }
                                    });
                                }
                            }
                        }
                    });
            }

            // ── Central panel: gridlines + map border + physics overlay + paint input ──
            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let painter = ui.painter_at(rect);

                // Grid lines.
                let grid_stroke = egui::Stroke::new(1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30));
                let first_tx = (cam_x / ts).floor() as i32;
                let last_tx  = ((cam_x + rect.width())  / ts).ceil() as i32 + 1;
                for tx in first_tx..=last_tx {
                    let sx = tx as f32 * ts - cam_x;
                    painter.line_segment(
                        [egui::pos2(sx, rect.top()), egui::pos2(sx, rect.bottom())],
                        grid_stroke,
                    );
                }
                let first_ty = (cam_y / ts).floor() as i32;
                let last_ty  = ((cam_y + rect.height()) / ts).ceil() as i32 + 1;
                for ty in first_ty..=last_ty {
                    let sy = sh - ty as f32 * ts + cam_y;
                    painter.line_segment(
                        [egui::pos2(rect.left(), sy), egui::pos2(rect.right(), sy)],
                        grid_stroke,
                    );
                }

                // Map border outline.
                let border_rect = egui::Rect::from_min_max(
                    egui::pos2(-cam_x,          sh - map_h + cam_y),
                    egui::pos2(map_w - cam_x,   sh         + cam_y),
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

                // Paint input: held for texture/physics, single press for spawner/patrol.
                let (primary_down, primary_pressed, secondary_pressed, pointer_pos, esc) =
                    ctx.input(|i| (i.pointer.primary_down(), i.pointer.primary_pressed(), i.pointer.secondary_pressed(), i.pointer.hover_pos(), i.key_pressed(egui::Key::Enter)));
                if esc { patrol_esc = true; }
                if let Some(pos) = pointer_pos {
                    if rect.contains(pos) {
                        if is_patrol_painting {
                            if primary_pressed   { patrol_click_pos = Some(pos); }
                            if secondary_pressed { patrol_erase_pos = Some(pos); }
                        } else {
                            if primary_down      { paint_pos = Some(pos); }
                            if primary_pressed   { click_pos = Some(pos); }
                            if secondary_pressed { erase_pos = Some(pos); }
                        }
                    }
                }
                ui.allocate_rect(rect, egui::Sense::click_and_drag());
            });
        });

        // 5. Act on flags now that the closure has returned.
        if toggle_panel                { self.right_panel_open   = !self.right_panel_open; }
        if let Some(tab)   = new_tab   { self.active_tab         = tab;   }
        if let Some(brush) = new_physics_brush { self.physics_brush_solid = brush; }
        if let Some(id)    = new_selected      { self.selected_id = Some(id); }

        if let Some(pos) = paint_pos {
            if let Some(idx) = self.screen_to_tile_idx(pos.x, pos.y, h, camera) {
                match active_tab {
                    RightPanelTab::TexturePalette => {
                        if let Some(sel_id) = self.selected_id {
                            let tex: Option<String> = if sel_id == 0 { None } else {
                                self.texture_for_id(sel_id).map(|s| s.to_owned())
                            };
                            self.world.tiles[idx].set_sprite(sel_id, tex.as_deref());
                        }
                    }
                    RightPanelTab::PhysicsPainter => {
                        self.world.tiles[idx].physics.solid = self.physics_brush_solid;
                    }
                    RightPanelTab::CharacterSpawner => {}
                }
            }
        }

        if let Some(pos) = click_pos {
            if let Some(idx) = self.screen_to_tile_idx(pos.x, pos.y, h, camera) {
                if active_tab == RightPanelTab::CharacterSpawner {
                    let tile_pos = self.world.tiles[idx].position;
                    if let Some((unit_id, instance_idx)) = self.find_unit_at_tile(tile_pos) {
                        // Click on a placed unit → enter patrol painting for that instance.
                        self.patrol_panel_was_open = self.right_panel_open;
                        self.right_panel_open = false;
                        self.spawner_mode = SpawnerMode::PatrolPainting { unit_id, instance_idx };
                    } else if let Some(sel_id) = self.selected_spawner_id {
                        // Empty tile + brush selected → place a new instance.
                        let tile = &self.world.tiles[idx];
                        if !tile.physics.solid {
                            if let Some(record) = self.spawner_units.iter_mut().find(|u| u.id == sel_id) {
                                record.positions.push(tile_pos);
                                record.patrols.push(vec![]);
                            }
                            self.save_units();
                        }
                    }
                }
            }
        }

        if let Some(pos) = erase_pos {
            if let Some(idx) = self.screen_to_tile_idx(pos.x, pos.y, h, camera) {
                if active_tab == RightPanelTab::CharacterSpawner {
                    let tile_pos = self.world.tiles[idx].position;
                    if let Some(sel_id) = self.selected_spawner_id {
                        // Remove the most recent instance of the selected template at this tile.
                        if let Some(record) = self.spawner_units.iter_mut().find(|u| u.id == sel_id) {
                            if let Some(i) = record.positions.iter().rposition(|&p| p == tile_pos) {
                                record.positions.remove(i);
                                if i < record.patrols.len() { record.patrols.remove(i); }
                                self.save_units();
                            }
                        }
                    }
                }
            }
        }

        // ── Patrol painting handlers ──────────────────────────────────────────
        if patrol_esc {
            if is_patrol_painting {
                self.right_panel_open = self.patrol_panel_was_open;
                self.spawner_mode = SpawnerMode::Idle;
            }
        }

        if let Some(pos) = patrol_click_pos {
            if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
                if let Some(idx) = self.screen_to_tile_idx(pos.x, pos.y, h, camera) {
                    let tile_pos = self.world.tiles[idx].position;
                    if let Some(record) = self.spawner_units.iter_mut().find(|u| u.id == unit_id) {
                        while record.patrols.len() <= instance_idx {
                            record.patrols.push(vec![]);
                        }
                        record.patrols[instance_idx].push(tile_pos);
                        self.save_units();
                    }
                }
            }
        }

        if let Some(pos) = patrol_erase_pos {
            if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
                if let Some(idx) = self.screen_to_tile_idx(pos.x, pos.y, h, camera) {
                    let tile_pos = self.world.tiles[idx].position;
                    if let Some(record) = self.spawner_units.iter_mut().find(|u| u.id == unit_id) {
                        if let Some(patrol) = record.patrols.get_mut(instance_idx) {
                            if let Some(i) = patrol.iter().position(|&p| p == tile_pos) {
                                patrol.remove(i);
                                self.save_units();
                            }
                        }
                    }
                }
            }
        }

        if let Some(del_id) = delete_spawner_id {
            self.spawner_units.retain(|u| u.id != del_id);
            if self.selected_spawner_id == Some(del_id) {
                self.selected_spawner_id = None;
            }
            if let SpawnerMode::PatrolPainting { unit_id, .. } = self.spawner_mode {
                if unit_id == del_id {
                    self.right_panel_open = self.patrol_panel_was_open;
                    self.spawner_mode = SpawnerMode::Idle;
                }
            }
            self.save_units();
        }

        // ── Texture new / conflict / rename handlers ──────────────────────────
        if tex_in_renaming {
            if let TexNewState::Renaming { new_name, .. } = &mut self.tex_new_state {
                *new_name = rename_text;
            }
        }
        if tex_new_clicked {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG Image", &["png"])
                .set_directory("assets")
                .pick_file()
            {
                let filename = path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let dest = std::path::Path::new("assets").join(&filename);
                if dest.exists() {
                    self.tex_new_state = TexNewState::Conflict { source: path, proposed_name: filename };
                } else {
                    self.register_texture(&path.clone(), &filename);
                }
            }
        }
        if tex_overwrite {
            if let TexNewState::Conflict { source, proposed_name } = &self.tex_new_state {
                let (src, name) = (source.clone(), proposed_name.clone());
                self.register_texture(&src, &name);
                self.tex_new_state = TexNewState::Idle;
            }
        }
        if tex_start_rename {
            if let TexNewState::Conflict { source, proposed_name } = &self.tex_new_state {
                self.tex_new_state = TexNewState::Renaming {
                    source: source.clone(),
                    new_name: proposed_name.clone(),
                };
            }
        }
        if tex_confirm_rename {
            if let TexNewState::Renaming { source, new_name } = &self.tex_new_state {
                let (src, name) = (source.clone(), new_name.clone());
                let dest = std::path::Path::new("assets").join(&name);
                if dest.exists() {
                    // New name also conflicts — loop back to conflict prompt.
                    self.tex_new_state = TexNewState::Conflict { source: src, proposed_name: name };
                } else {
                    self.register_texture(&src, &name);
                    self.tex_new_state = TexNewState::Idle;
                }
            }
        }
        if tex_cancel_new { self.tex_new_state = TexNewState::Idle; }
        if let Some(id) = delete_texture_id {
            self.palette.retain(|e| e.id != id);
            if self.selected_id == Some(id) { self.selected_id = None; }
            self.save_palette();
            self.unit_sprite_cache = Self::build_sprite_cache(&self.palette);
        }

        // Write back draft edits made inside the closure.
        if spawner_form_open {
            self.spawner_draft.name = draft_name;
            if let Some(s) = new_draft_sprite { self.spawner_draft.sprite_id = Some(s); }
        }
        if let Some(id) = new_selected_spawner { self.selected_spawner_id = Some(id); }
        if let Some(idx) = open_edit {
            self.spawner_mode  = SpawnerMode::Editing { index: idx };
            self.spawner_draft = UnitDraft {
                name:      self.spawner_units[idx].name.clone(),
                sprite_id: self.spawner_units[idx].sprite_id,
            };
        }
        if open_create {
            self.spawner_mode  = SpawnerMode::CreatingNew;
            self.spawner_draft = UnitDraft::new();
        }
        if cancel_create {
            self.spawner_mode  = SpawnerMode::Idle;
            self.spawner_draft = UnitDraft::new();
        }
        if confirm_create {
            match self.spawner_mode {
                SpawnerMode::CreatingNew => {
                    let new_id = self.spawner_units.iter().map(|u| u.id).max().unwrap_or(0) + 1;
                    self.spawner_units.push(UnitRecord {
                        id:        new_id,
                        name:      self.spawner_draft.name.clone(),
                        sprite_id: self.spawner_draft.sprite_id,
                        positions: vec![],
                        patrols:   vec![],
                    });
                }
                SpawnerMode::Editing { index } => {
                    self.spawner_units[index].name      = self.spawner_draft.name.clone();
                    self.spawner_units[index].sprite_id = self.spawner_draft.sprite_id;
                }
                SpawnerMode::Idle => {}
                SpawnerMode::PatrolPainting { .. } => {}
            }
            self.save_units();
            self.spawner_mode  = SpawnerMode::Idle;
            self.spawner_draft = UnitDraft::new();
        }

        if do_save { self.world.save(&self.map_path); }
        if do_exit { self.pending_exit = true; }
    }
}

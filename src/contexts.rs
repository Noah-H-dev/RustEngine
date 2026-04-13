use RustEngine::game_engine::{Engine, GameContext, World, TILE_SIZE};
use RustEngine::tools::load_textures;

// ── Game sub-menu state (owned by MainMenuContext) ─────────────────────────────
enum GameSub {
    Hidden,
    Open { map_path: String, id_path: String },
}

impl GameSub {
    fn is_visible(&self) -> bool { !matches!(self, GameSub::Hidden) }
}

// ── Editor sub-menu state (owned by MainMenuContext) ───────────────────────────
enum EditorSub {
    Hidden,
    Open { map_path: String, id_path: String },
    New  { map_path: String, id_path: String, width: String, height: String },
}

impl EditorSub {
    fn is_visible(&self) -> bool { !matches!(self, EditorSub::Hidden) }
    fn is_open(&self)    -> bool { matches!(self, EditorSub::Open { .. }) }
    fn is_new(&self)     -> bool { matches!(self, EditorSub::New  { .. }) }

    /// Preserve whatever paths the user has already typed when switching sub-forms.
    fn current_paths(&self) -> (String, String) {
        match self {
            EditorSub::Open { map_path, id_path } |
            EditorSub::New  { map_path, id_path, .. } => (map_path.clone(), id_path.clone()),
            EditorSub::Hidden => ("./map.txt".into(), "./id.txt".into()),
        }
    }
}

// ── Main menu ──────────────────────────────────────────────────────────────────
pub struct MainMenuContext {
    pending_transition: Option<Box<dyn GameContext>>,
    game_sub: GameSub,
    editor_sub: EditorSub,
    error_msg: Option<String>,
}

impl MainMenuContext {
    pub fn new() -> Self {
        MainMenuContext {
            pending_transition: None,
            game_sub: GameSub::Hidden,
            editor_sub: EditorSub::Hidden,
            error_msg: None,
        }
    }
}

impl GameContext for MainMenuContext {
    fn update(&mut self, _engine: &mut Engine) -> Option<Box<dyn GameContext>> {
        self.pending_transition.take()
    }

    fn draw(&mut self, engine: &mut Engine) {
        // Boolean flags written inside the closure, acted on after it returns.
        let mut toggle_game  = false;
        let mut confirm_game = false;
        let mut toggle_editor = false;
        let mut show_open    = false;
        let mut show_new     = false;
        let mut confirm_open = false;
        let mut confirm_new  = false;

        let (w, h) = engine.screen_size();
        let input = engine.egui_input.clone();

        engine.renderer.render(input, w, h, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(ui.available_height() / 4.0);
                ui.vertical_centered(|ui| {
                    ui.heading("RustEngine");
                    ui.add_space(16.0);

                    // ── New Game ──
                    let game_fill = if self.game_sub.is_visible() {
                        egui::Color32::from_rgb(80, 100, 180)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    if ui.add_sized([160.0, 40.0], egui::Button::new("New Game").fill(game_fill)).clicked() {
                        toggle_game = true;
                    }

                    if self.game_sub.is_visible() {
                        ui.add_space(4.0);
                        if let GameSub::Open { map_path, id_path } = &mut self.game_sub {
                            egui::Grid::new("game_form").num_columns(2).show(ui, |ui| {
                                ui.label("Filepath: "); ui.text_edit_singleline(map_path); ui.end_row();
                                ui.label("IDs:");      ui.text_edit_singleline(id_path);  ui.end_row();
                            });
                            if ui.button("Start").clicked() { confirm_game = true; }
                        }
                    }
                    ui.add_space(8.0);

                    // ── Editor button (centered) ──
                    let editor_fill = if self.editor_sub.is_visible() {
                        egui::Color32::from_rgb(80, 100, 180)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    if ui.add_sized([160.0, 40.0], egui::Button::new("Editor").fill(editor_fill)).clicked() {
                        toggle_editor = true;
                    }

                    // ── Open File / New File sub-buttons (centered row) ──
                    if self.editor_sub.is_visible() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let sub_w = 100.0 + 4.0 + 100.0;
                            let pad = (ui.available_width() - sub_w) / 2.0;
                            if pad > 0.0 { ui.add_space(pad); }
                            let open_fill = if self.editor_sub.is_open() {
                                egui::Color32::from_rgb(50, 130, 50)
                            } else {
                                ui.visuals().widgets.inactive.bg_fill
                            };
                            if ui.add_sized([100.0, 32.0], egui::Button::new("Open File").fill(open_fill)).clicked() {
                                show_open = true;
                            }
                            ui.add_space(4.0);
                            let new_fill = if self.editor_sub.is_new() {
                                egui::Color32::from_rgb(50, 130, 50)
                            } else {
                                ui.visuals().widgets.inactive.bg_fill
                            };
                            if ui.add_sized([100.0, 32.0], egui::Button::new("New File").fill(new_fill)).clicked() {
                                show_new = true;
                            }
                        });
                    }

                    // ── File form (shown below when Open or New is active) ──
                    ui.add_space(12.0);
                    match &mut self.editor_sub {
                        EditorSub::Open { map_path, id_path } => {
                            egui::Grid::new("open_form").num_columns(2).show(ui, |ui| {
                                ui.label("Filepath: ");  ui.text_edit_singleline(map_path);  ui.end_row();
                                ui.label("IDs:"); ui.text_edit_singleline(id_path);   ui.end_row();
                            });
                            if ui.button("Open").clicked() { confirm_open = true; }
                        }
                        EditorSub::New { map_path, id_path, width, height } => {
                            egui::Grid::new("new_form").num_columns(2).show(ui, |ui| {
                                ui.label("Map: ");   ui.text_edit_singleline(map_path);                           ui.end_row();
                                ui.label("Tiles:");  ui.text_edit_singleline(id_path);                            ui.end_row();
                                ui.label("Width:");  ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(width));  ui.end_row();
                                ui.label("Height:"); ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(height)); ui.end_row();
                            });
                            if ui.button("Create").clicked() { confirm_new = true; }
                        }
                        EditorSub::Hidden => {}
                    }

                    if let Some(msg) = &self.error_msg {
                        ui.add_space(8.0);
                        ui.colored_label(egui::Color32::from_rgb(220, 60, 60), msg);
                    }
                });
            });
        });

        // ── Act on flags now that the closure (and its borrows) have ended ──

        if toggle_game {
            self.error_msg = None;
            self.game_sub = if self.game_sub.is_visible() {
                GameSub::Hidden
            } else {
                GameSub::Open { map_path: "./map.txt".into(), id_path: "./id.txt".into() }
            };
        }

        if confirm_game {
            if let GameSub::Open { map_path, id_path } = &self.game_sub {
                let map = map_path.clone();
                let ids = id_path.clone();
                if !std::path::Path::new(&map).exists() {
                    self.error_msg = Some(format!("Map file not found: {}", map));
                } else if !std::path::Path::new(&ids).exists() {
                    self.error_msg = Some(format!("ID file not found: {}", ids));
                } else {
                    self.error_msg = None;
                    self.pending_transition = Some(Box::new(GameRunningContext::new(&map, &ids)));
                }
            }
        }

        if toggle_editor {
            self.error_msg = None;
            self.editor_sub = if self.editor_sub.is_visible() {
                EditorSub::Hidden
            } else {
                EditorSub::Open { map_path: "./map.txt".into(), id_path: "./id.txt".into() }
            };
        }

        if show_open && !self.editor_sub.is_open() {
            self.error_msg = None;
            let (map_path, id_path) = self.editor_sub.current_paths();
            self.editor_sub = EditorSub::Open { map_path, id_path };
        }

        if show_new && !self.editor_sub.is_new() {
            self.error_msg = None;
            let (map_path, id_path) = self.editor_sub.current_paths();
            let (sw, sh) = engine.screen_size();
            let dw = ((sw as i32 + TILE_SIZE - 1) / TILE_SIZE).max(20).to_string();
            let dh = ((sh as i32 + TILE_SIZE - 1) / TILE_SIZE).max(15).to_string();
            self.editor_sub = EditorSub::New { map_path, id_path, width: dw, height: dh };
        }

        if confirm_open {
            if let EditorSub::Open { map_path, id_path } = &self.editor_sub {
                let map = map_path.clone();
                let ids = id_path.clone();
                match EditorContext::from_file(&map, &ids) {
                    Ok(ctx) => { self.error_msg = None; self.pending_transition = Some(Box::new(ctx)); }
                    Err(e)  => { self.error_msg = Some(e); }
                }
            }
        }

        if confirm_new {
            if let EditorSub::New { map_path, id_path, width, height } = &self.editor_sub {
                if let (Ok(w), Ok(h)) = (width.parse::<usize>(), height.parse::<usize>()) {
                    let map = map_path.clone();
                    let ids = id_path.clone();
                    match EditorContext::new_map(&map, &ids, w, h) {
                        Ok(ctx) => { self.error_msg = None; self.pending_transition = Some(Box::new(ctx)); }
                        Err(e)  => { self.error_msg = Some(e); }
                    }
                } else {
                    self.error_msg = Some("Width and height must be valid integers.".into());
                }
            }
        }
    }
}

// ── Gameplay ───────────────────────────────────────────────────────────────────
pub struct GameRunningContext {
    world: World,
}

impl GameRunningContext {
    pub fn new(map_path: &str, id_path: &str) -> Self {
        GameRunningContext { world: World::load(map_path, id_path) }
    }
}

impl GameContext for GameRunningContext {
    fn update(&mut self, _engine: &mut Engine) -> Option<Box<dyn GameContext>> {
        // TODO: handle player input, update world state, return Some(...) to transition
        None
    }

    fn draw(&mut self, engine: &mut Engine) {
        self.world.draw(engine.camera);
        // TODO: draw game HUD via engine.renderer
    }
}

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
}

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
    spawner_units: Vec<UnitDraft>,
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
        Ok(EditorContext {
            world: World::load(map_path, id_path),
            map_path: map_path.to_string(),
            palette: Self::load_palette(id_path),
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
            spawner_units: Vec::new(),
        })
    }

    /// Create a new blank map of the given dimensions.
    pub fn new_map(map_path: &str, id_path: &str, width: usize, height: usize) -> Result<Self, String> {
        if !std::path::Path::new(id_path).exists() {
            return Err(format!("ID file not found: {}", id_path));
        }
        Ok(EditorContext {
            world: World::new_empty(width, height),
            map_path: map_path.to_string(),
            palette: Self::load_palette(id_path),
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
            spawner_units: Vec::new(),
        })
    }

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
}

impl GameContext for EditorContext {
    fn update(&mut self, _engine: &mut Engine) -> Option<Box<dyn GameContext>> {
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
        let mut paint_pos:         Option<egui::Pos2>    = None;
        let mut open_create        = false;
        let mut cancel_create      = false;
        let mut confirm_create     = false;
        let mut new_draft_sprite:  Option<i32>           = None;
        let mut open_edit:         Option<usize>         = None;
        let mut tex_new_clicked    = false;
        let mut tex_overwrite      = false;
        let mut tex_start_rename   = false;
        let mut tex_confirm_rename = false;
        let mut tex_cancel_new     = false;
        let mut delete_texture_id: Option<i32> = None;

        // 1. On the first frame, place the camera so the map starts at the bottom-left.
        let (w, h) = engine.screen_size();
        if !self.camera_init {
            engine.camera = (0, 0);
            self.camera_init = true;
        }

        // 2. Draw the OpenGL world first so the egui overlay appears on top.
        self.world.draw(engine.camera);

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
        let mut draft_name     = self.spawner_draft.name.clone();
        let draft_sprite       = self.spawner_draft.sprite_id;
        let ts            = TILE_SIZE as f32;
        let cam_x         = engine.camera.0 as f32;
        let cam_y         = engine.camera.1 as f32;
        let sh            = h as f32;
        let map_w         = self.world.width  as f32 * ts;
        let map_h         = self.world.height as f32 * ts;

        let physics_overlay: Vec<(egui::Rect, egui::Color32)> =
            if active_tab == RightPanelTab::PhysicsPainter {
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
                            ui.label("Character Spawner");
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

            // ── Right panel with tabs (conditionally shown) ──
            if panel_open {
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
                                    // Unit list
                                    egui::ScrollArea::vertical()
                                        .id_salt("spawner_list")
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            for (i, unit) in self.spawner_units.iter().enumerate() {
                                                ui.horizontal(|ui| {
                                                    ui.label(if unit.name.is_empty() { "(unnamed)" } else { &unit.name });
                                                    if ui.small_button("Edit").clicked() {
                                                        open_edit = Some(i);
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

                // Paint input — fires every frame the primary button is held.
                let (primary_down, pointer_pos) =
                    ctx.input(|i| (i.pointer.primary_down(), i.pointer.hover_pos()));
                if primary_down {
                    if let Some(pos) = pointer_pos {
                        if rect.contains(pos) {
                            paint_pos = Some(pos);
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
        }

        // Write back draft edits made inside the closure.
        if spawner_form_open {
            self.spawner_draft.name = draft_name;
            if let Some(s) = new_draft_sprite { self.spawner_draft.sprite_id = Some(s); }
        }
        if let Some(idx) = open_edit {
            self.spawner_mode  = SpawnerMode::Editing { index: idx };
            self.spawner_draft = self.spawner_units[idx].clone();
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
            let saved = self.spawner_draft.clone();
            match self.spawner_mode {
                SpawnerMode::CreatingNew          => self.spawner_units.push(saved),
                SpawnerMode::Editing { index }    => self.spawner_units[index] = saved,
                SpawnerMode::Idle                 => {}
            }
            // TODO: persist to units.toml
            self.spawner_mode  = SpawnerMode::Idle;
            self.spawner_draft = UnitDraft::new();
        }

        if do_save { self.world.save(&self.map_path); }
        if do_exit { self.pending_exit = true; }
    }
}

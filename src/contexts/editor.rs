use RustEngine::game::game_engine::{Engine, GameContext, World, TILE_SIZE, UnitRecord, UnitFile};
use RustEngine::tools::{load_textures, GLObject, BL_RECTANGLE};
use RustEngine::shaders::{VERT_SHADER, FRAG_SHADER};
use std::collections::HashMap;
use super::menus::MainMenuContext;
use super::tiledefs::{Category, TileDefs};

#[derive(Clone, Copy, PartialEq)]
enum RightPanelTab {
    TexturePalette,
    PhysicsPainter,
    CharacterSpawner,
}

struct PaletteEntry {
    id: i32,
    path: String,
}

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

enum FolderEdit {
    Idle,
    NamingNew { texture_id: i32, name: String },
    Renaming  { from: String, to: String },
}

enum SpawnerMode {
    Idle,
    PatrolPainting { unit_id: u32, instance_idx: usize },
}

#[derive(Clone, Copy, PartialEq)]
enum SpawnerBrush {
    None,
    Unit(u32),
    PlayerSpawn,
}

#[derive(Default)]
struct EditorIntent {
    do_save:           bool,
    do_exit:           bool,
    toggle_panel:      bool,
    new_selected:      Option<i32>,
    new_tab:           Option<RightPanelTab>,
    new_physics_brush: Option<bool>,
    paint_pos: Option<egui::Pos2>,
    click_pos: Option<egui::Pos2>,
    erase_pos: Option<egui::Pos2>,
    erase_paint_pos: Option<egui::Pos2>,
    patrol_click_pos: Option<egui::Pos2>,
    patrol_erase_pos: Option<egui::Pos2>,
    patrol_esc:       bool,
    new_spawner_brush:    Option<SpawnerBrush>,
    set_texture_solid: Option<(i32, bool)>,
    move_to_folder:      Option<(i32, Option<String>)>,
    start_new_folder:    Option<i32>,
    start_rename_folder: Option<String>,
    delete_folder:       Option<String>,
    folder_confirm:      bool,
    folder_cancel:       bool,
    open_resize_dialog:  bool,
    confirm_resize:      Option<(usize, usize)>,
    close_resize_dialog: bool,
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
    camera_init: bool,
    active_tab: RightPanelTab,
    physics_brush_solid: bool,
    right_panel_open: bool,
    spawner_mode: SpawnerMode,
    spawner_units: Vec<UnitRecord>,
    spawner_brush: SpawnerBrush,
    unit_sprite_cache: HashMap<i32, GLObject>,
    resize_dialog: Option<(usize, usize)>,
    zoom: f32,
    tile_defs: TileDefs,
    folder_edit: FolderEdit,
}

impl EditorContext {
    const UNITS_PATH: &'static str = "gamedata/units.toml";

    pub fn from_file(map_path: &str, id_path: &str) -> Result<Self, String> {
        if !std::path::Path::new(map_path).exists() {
            return Err(format!("Map file not found: {}", map_path));
        }
        if !std::path::Path::new(id_path).exists() {
            return Err(format!("ID file not found: {}", id_path));
        }
        Ok(Self::with_world(World::load(map_path, id_path), map_path, id_path))
    }

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
            spawner_mode: SpawnerMode::Idle,
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
                    return None;
                };
                Some((e.id, GLObject::new(BL_RECTANGLE, &resolved, VERT_SHADER, FRAG_SHADER)))
            })
            .collect()
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

    fn screen_to_tile_idx(&self, sx: f32, sy: f32, screen_h: u32, camera: (i32, i32), zoom: f32) -> Option<usize> {
        let world_x = (sx / zoom) as i32 + camera.0;
        let world_y = ((screen_h as f32 - sy) / zoom) as i32 + camera.1;
        if world_x < 0 || world_y < 0 { return None; }
        let tx = (world_x / TILE_SIZE) as usize;
        let ty = (world_y / TILE_SIZE) as usize;
        if tx >= self.world.width || ty >= self.world.height { return None; }
        Some(ty * self.world.width + tx)
    }

    fn texture_for_id(&self, id: i32) -> Option<&str> {
        self.palette.iter().find(|e| e.id == id).map(|e| e.path.as_str())
    }

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

    fn handle_paint(&mut self, active_tab: RightPanelTab, intent: &EditorIntent, screen_h: u32, camera: (i32, i32)) {
        if let Some(pos) = intent.paint_pos {
            self.at_tile(pos, screen_h, camera, |this, idx, _tp| match active_tab {
                RightPanelTab::TexturePalette => {
                    if let Some(sel_id) = this.selected_id {
                        let tex = if sel_id == 0 { None } else { this.texture_for_id(sel_id).map(str::to_owned) };
                        this.world.tiles[idx].set_sprite(sel_id, tex.as_deref());
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
        if active_tab == RightPanelTab::TexturePalette {
            if let Some(pos) = intent.erase_paint_pos {
                self.at_tile(pos, screen_h, camera, |this, idx, _tp| {
                    this.world.tiles[idx].set_sprite(0, None);
                });
            }
        }
    }

    fn handle_unit_clicks(&mut self, active_tab: RightPanelTab, intent: &EditorIntent, screen_h: u32, camera: (i32, i32)) {
        if active_tab != RightPanelTab::CharacterSpawner { return; }
        let brush = self.spawner_brush;

        if brush == SpawnerBrush::PlayerSpawn {
            if let Some(pos) = intent.click_pos {
                self.at_tile(pos, screen_h, camera, |this, _idx, tile_pos| {
                    this.world.spawn = Some(tile_pos);
                });
            }
            if intent.erase_pos.is_some() {
                self.world.spawn = None;
            }
            return;
        }

        if let Some(pos) = intent.click_pos {
            self.at_tile(pos, screen_h, camera, |this, idx, tile_pos| {
                if let Some((unit_id, instance_idx)) = this.find_unit_at_tile(tile_pos) {
                    this.spawner_mode = SpawnerMode::PatrolPainting { unit_id, instance_idx };
                } else if let SpawnerBrush::Unit(sel_id) = brush {
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

    fn handle_spawner(&mut self, intent: &EditorIntent) {
        if let Some(b) = intent.new_spawner_brush { self.spawner_brush = b; }
    }

    fn handle_folders(&mut self, intent: &EditorIntent, folder_text: String) {
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

        let (w, h) = engine.screen_size();
        if !self.camera_init {
            engine.camera = (0, 0);
            self.camera_init = true;
        }

        self.world.draw(engine.camera, self.zoom);

        let ts    = TILE_SIZE as f32;
        let z     = self.zoom;
        let tsz   = ts * z;
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

        let mut unit_overlay: Vec<(egui::Rect, egui::Color32)> = Vec::new();
        for record in &self.spawner_units {
            let gl_obj = record.sprite_id.and_then(|id| self.unit_sprite_cache.get(&id));
            for &(tx, ty) in &record.positions {
                let r = tile_rect(tx as f32, ty as f32);
                if let Some(obj) = gl_obj {
                    obj.draw(r.min.x as i32, (sh - r.max.y) as i32, tsz);
                }
                unit_overlay.push((r, egui::Color32::from_rgba_unmultiplied(60, 200, 180, 160)));
            }
        }

        let active_tab       = self.active_tab;
        let physics_brush    = self.physics_brush_solid;
        let panel_open       = self.right_panel_open;
        let folder_naming   = matches!(self.folder_edit, FolderEdit::NamingNew { .. });
        let folder_renaming = matches!(self.folder_edit, FolderEdit::Renaming  { .. });
        let mut folder_text = match &self.folder_edit {
            FolderEdit::NamingNew { name, .. } => name.clone(),
            FolderEdit::Renaming  { to, .. }   => to.clone(),
            FolderEdit::Idle                   => String::new(),
        };
        let is_patrol_painting = matches!(self.spawner_mode, SpawnerMode::PatrolPainting { .. });
        let patrol_unit_name: String = if let SpawnerMode::PatrolPainting { unit_id, instance_idx } = self.spawner_mode {
            self.spawner_units.iter().find(|u| u.id == unit_id)
                .map(|u| format!("{} (instance {})", if u.name.is_empty() { "(unnamed)" } else { &u.name }, instance_idx))
                .unwrap_or_default()
        } else { String::new() };
        let spawner_brush       = self.spawner_brush;

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

        let camera = engine.camera;
        let input  = engine.egui_input.clone();
        engine.renderer.render(input, w, h, |ctx| {
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
                        let toggle_label = if panel_open { "»" } else { "«" };
                        if ui.button(toggle_label).clicked() { intent.toggle_panel = true; }
                        ui.separator();
                        ui.label(format!("{}×{}", self.world.width, self.world.height));
                    });
                });
            });

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
                                    let is_tile = |id: i32| self.tile_defs.category_of(id) == Category::Tiles;
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
                                    let label_of = |e: &PaletteEntry| -> String {
                                        match self.tile_defs.name_of(e.id) {
                                            Some(n) if !n.is_empty() => format!("{} | {}", e.id, n),
                                            _                        => format!("{} | {}", e.id, e.path),
                                        }
                                    };

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

                                egui::ScrollArea::vertical()
                                    .id_salt("spawner_list")
                                    .max_height(220.0)
                                    .show(ui, |ui| {
                                        for unit in &self.spawner_units {
                                            let is_selected = matches!(spawner_brush, SpawnerBrush::Unit(id) if id == unit.id);
                                            let label = format!(
                                                "{} ({})",
                                                if unit.name.is_empty() { "(unnamed)" } else { &unit.name },
                                                unit.positions.len(),
                                            );
                                            if ui.selectable_label(is_selected, &label).clicked() {
                                                intent.new_spawner_brush = Some(SpawnerBrush::Unit(unit.id));
                                            }
                                        }
                                    });
                                ui.separator();
                                if self.spawner_units.is_empty() {
                                    ui.weak("No creatures defined.");
                                }
                                ui.weak("Create creatures in Editor -> Game -> Creature.");
                            }
                        }
                    });
            }

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

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                let painter = ui.painter_at(rect);

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

                let border_rect = egui::Rect::from_min_max(
                    egui::pos2(-cam_x * z,              sh - (map_h - cam_y) * z),
                    egui::pos2((map_w - cam_x) * z,     sh + cam_y * z),
                );
                painter.rect_stroke(
                    border_rect, 0.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 210, 50)),
                    egui::StrokeKind::Outside,
                );

                for (tile_rect, color) in &physics_overlay {
                    painter.rect_filled(*tile_rect, 0.0, *color);
                }

                for (tile_rect, color) in &unit_overlay {
                    painter.rect_filled(*tile_rect, 0.0, *color);
                }

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
        self.handle_spawner(&intent);
        self.handle_folders(&intent, folder_text);
        self.handle_resize(&intent, resize_draft_w, resize_draft_h);
        self.handle_camera(engine, &intent, h);

        if intent.do_save { self.world.save(&self.map_path); }
        if intent.do_exit { self.pending_exit = true; }
    }
}

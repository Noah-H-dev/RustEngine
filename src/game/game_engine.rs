use std::sync::Arc;
use std::time::{Duration, Instant};
use super::entity::Unit;
use super::stats::stats;
use crate::tools::*;
use crate::gui::EguiRenderer;
use crate::shaders::{FRAG_SHADER, TRANS_FRAG_SHADER, VERT_SHADER};
use serde::{Deserialize, Serialize};
use crate::game::item::item;

pub const TILE_SIZE: i32 = 32;
/// Persisted unit template, written to / read from units.toml.
#[derive(Clone, Serialize, Deserialize)]
pub struct UnitRecord {
    pub id: u32,
    pub name: String,
    pub sprite_id: Option<i32>,
    /// Every tile position where this template has been placed on the map.
    #[serde(default)]
    pub positions: Vec<(i32, i32)>,
    /// Per-instance patrol paths, parallel to `positions`.
    /// `patrols[i]` is the ordered waypoint list for the unit at `positions[i]`.
    #[serde(default)]
    pub patrols: Vec<Vec<(i32, i32)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<stats>,
}
impl UnitRecord {
    /// Drop placed instances (and their patrols) whose position falls outside
    /// the given map bounds, then trim surviving patrols the same way.
    /// Returns true if anything was removed.
    pub fn clamp_to_bounds(&mut self, width: usize, height: usize) -> bool {
        let in_bounds = |x: i32, y: i32| x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height;
        let mut changed = false;
        let mut i = 0;
        while i < self.positions.len() {
            let (px, py) = self.positions[i];
            if !in_bounds(px, py) {
                self.positions.remove(i);
                if i < self.patrols.len() { self.patrols.remove(i); }
                changed = true;
            } else {
                i += 1;
            }
        }
        for patrol in &mut self.patrols {
            let before = patrol.len();
            patrol.retain(|&(wx, wy)| in_bounds(wx, wy));
            if patrol.len() != before { changed = true; }
        }
        return changed
    }
}
/// Top-level TOML wrapper — produces `[[unit]]` array-of-tables syntax.
#[derive(Serialize, Deserialize, Default)]
pub struct UnitFile {
    #[serde(default)]
    pub unit: Vec<UnitRecord>,
}

/// Persisted game settings, written to / read from settings.toml.
#[derive(Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub real_time: bool,
    /// Request per-monitor-v2 DPI awareness on Windows. Read once at startup
    /// (before SDL init) — changing it requires a restart to take effect.
    #[serde(default)]
    pub dpi_per_monitor: bool,
    /// Path of the active tileset. Used by the tileset editor (which tileset
    /// is open for editing) AND by the game and map editor (which `id.txt`-
    /// style file resolves tile ids -> PNGs). One global selection.
    #[serde(default = "default_tileset")]
    pub active_tileset: String,
}

fn default_tileset() -> String { "tilesets/Default.txt".to_string() }

impl Default for Settings {
    fn default() -> Self {
        Settings {
            real_time: false,
            dpi_per_monitor: false,
            active_tileset: default_tileset(),
        }
    }
}

impl Settings {
    const PATH: &'static str = "gamedata/settings.toml";

    pub fn load() -> Self {
        if !std::path::Path::new(Self::PATH).exists() { return Self::default(); }
        let content = std::fs::read_to_string(Self::PATH).unwrap_or_default();
        toml::from_str(&content).unwrap_or_default()
    }

    pub fn save(&self) {
        let content = toml::to_string(self).expect("Failed to serialize settings");
        let _ = std::fs::create_dir_all("gamedata");
        std::fs::write(Self::PATH, content).expect("Failed to save settings.toml");
    }
}

// ── Save game ────────────────────────────────────────────────────────────────
/// The live runtime snapshot, written to / read from game.toml. Pairs with
/// `player.toml` `spawn` (the player's position): game.toml records which map /
/// tileset the session is on plus the live state of placed units, while the
/// player's own position lives in player.toml. Together they let "Continue"
/// resume a session. units.toml stays the template; `units` here are the live
/// overrides applied on top of it, matched by load order.
#[derive(Serialize, Deserialize, Default)]
pub struct SaveGame {
    pub map: String,
    pub tileset: String,
    #[serde(default)]
    pub units: Vec<UnitState>,
}

/// Live position + patrol progress for one placed unit, indexed parallel to the
/// units produced by `Engine::load_units`.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct UnitState {
    pub position: (i32, i32),
    pub patrol_idx: i32,
}

impl SaveGame {
    pub const PATH: &'static str = "gamedata/game.toml";

    pub fn exists() -> bool {
        std::path::Path::new(Self::PATH).exists()
    }

    pub fn load() -> Option<Self> {
        if !Self::exists() { return None; }
        let content = std::fs::read_to_string(Self::PATH).ok()?;
        toml::from_str(&content).ok()
    }

    pub fn save(&self) {
        if let Ok(content) = toml::to_string_pretty(self) {
            let _ = std::fs::create_dir_all("gamedata");
            let _ = std::fs::write(Self::PATH, content);
        }
    }
}

// ── Context trait ──────────────────────────────────────────────────────────────
/// Implement this for every screen/mode (main menu, gameplay, editor, pause, …).
/// Return `Some(next_context)` from `update` to transition, `None` to stay.
pub trait GameContext {
    /// Called at a fixed rate (TICK_RATE). `dt` is seconds per tick.
    fn update(&mut self, engine: &mut Engine, dt: f32) -> Option<Box<dyn GameContext>>;
    fn draw(&mut self, engine: &mut Engine);
}

///IDEA: effects. each unit has a vector of effects which every iteration gets run through and applied to
/// anything they do (every action that is). each effect will have a type (on move, on attach, on attacked,
/// on update (an enum)), and each loop the current action will be checked against the effect action. this will
/// resemble an ECS system, however will slot nicely into how I am approaching this. EX. you could have a sword which on attach applies
/// another effect of 'guard'. guard lasts for 5 actions (ticked down the same way the speed counter does) and for as long as
/// it exists, it is an on attacked effect which does damage to the attacker if the attack is successfuly blocked.
///


// ── Engine ─────────────────────────────────────────────────────────────────────
pub struct Engine {
    pub renderer: EguiRenderer,
    pub player: Unit,
    pub units: Vec<Unit>,
    pub items: Vec<item>,
    pub world: World,
    win: GlWindow,
    sdl: Sdl,
    pub win_open: bool,
    /// The player's action for this tick, read from input by `poll_input` and
    /// consumed by the gameplay context. Reset to NONE after each tick. Each
    /// non-player Unit holds its own `current_action` (see entity.rs).
    pub player_action: actions,
    /// Neutral keys pressed since the last tick consumed them, filled by the
    /// event pump (`poll_input`) and interpreted by the active context. Cleared
    /// alongside `player_action` after each tick. See `tools::Key`.
    pub keys_pressed: Vec<Key>,
    pub egui_input: egui::RawInput,
    pub mouse_pos: egui::Pos2,
    pub camera: (i32, i32),
    pub settings: Settings,
}

impl Engine {
    pub fn new( title: &str) -> Self {
        // Load settings before SDL init so DPI awareness can be applied (the
        // hint is read when SDL's video subsystem comes up). Reused below.
        let settings = Settings::load();
        set_dpi_awareness(settings.dpi_per_monitor);
        let sdl = init_sdl();
        let win = init_window(title, &sdl);
        let gl = Arc::new(unsafe {
            egui_glow::glow::Context::from_loader_function(|s| {
                let c_str = std::ffi::CString::new(s).unwrap();
                win.get_proc_address(c_str.as_ptr() as *const u8) as *const _
            })
        });
        win.set_swap_interval(GlSwapInterval::Vsync).unwrap();
        clear_color(0.0, 0.0, 0.0, 1.0);
        // Build the player from player.toml — resolves sprite_id via the active
        // tileset. Game start later overrides position from the map's ---SPAWN---
        // marker. Stats come straight from the file (no longer forced here).
        let player = super::entity::load_player(&settings.active_tileset);
        let engine = Engine {
            win,
            sdl,
            renderer: EguiRenderer::new(gl),
            player,
            win_open: true,
            player_action: actions::NONE,
            keys_pressed: Vec::new(),
            egui_input: egui::RawInput::default(),
            mouse_pos: egui::Pos2::default(),
            units: Vec::new(),
            items: vec![],//update this
            world: World::new_empty(0, 0),
            camera: (0, 0),
            settings,
        };
        return engine;
    }

    /// Drives the context state machine until the window is closed.
    pub fn run(&mut self, mut context: Box<dyn GameContext>) {
        let mut TICK_RATE: Duration = Duration::from_millis(16);
        let mut DT: f32 = TICK_RATE.as_secs_f32();
        let mut accumulator = Duration::ZERO;
        let mut last = Instant::now();

        while self.win_open {
            let now = Instant::now();
            accumulator += now.duration_since(last);
            last = now;

            self.poll_input();
            while accumulator >= TICK_RATE {
                if let Some(next) = context.update(self, DT) {
                    context = next;
                }
                self.player_action = actions::NONE;
                self.keys_pressed.clear();
                accumulator = Duration::ZERO;
            }

            clear();
            context.draw(self);
            self.win.swap_window();
        }
    }

    pub fn screen_size(&self) -> (u32, u32) {
        unsafe { (SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32) }
    }

    fn poll_input(&mut self) {

        self.egui_input.events.clear();
        self.egui_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(
                unsafe { SCREEN_WIDTH as f32 },
                unsafe { SCREEN_HEIGHT as f32 },
            ),
        ));
        update(
            &mut self.win_open,
            &self.sdl,
            &mut self.keys_pressed,
            &mut self.egui_input,
            &mut self.mouse_pos,
        );
    }
    /// Snapshot the running game: write the player's tile into `player.toml`
    /// `spawn` and the map/tileset + live unit state into `game.toml`. Called by
    /// the pause-menu Save button.
    pub fn save(&self, map_path: &str, id_path: &str) {
        super::entity::save_player_spawn(self.player.position);
        let units = self.units.iter()
            .map(|u| UnitState { position: u.position, patrol_idx: u.patrol_idx })
            .collect();
        SaveGame {
            map: map_path.to_string(),
            tileset: id_path.to_string(),
            units,
        }.save();
    }

    pub fn load_units(id_path: &str) -> Vec<Unit> {
        const UNITS_PATH: &str = "gamedata/units.toml";
        if !std::path::Path::new(UNITS_PATH).exists() { return Vec::new(); }
        let content = std::fs::read_to_string(UNITS_PATH).unwrap_or_default();
        let records = toml::from_str::<UnitFile>(&content)
            .map(|f| f.unit)
            .unwrap_or_default();
        let id_key = load_textures(id_path);
        let mut unit_vec: Vec<Unit> = Vec::new();
        for record in &records {
            if let Some(sprite_id) = record.sprite_id {
                if let Some(tex_path) = id_key.get(&sprite_id) {
                    for (i, &position) in record.positions.iter().enumerate() {
                        let sprite = GLObject::new(
                            BL_RECTANGLE,
                            &format!("assets/{}", tex_path),
                            VERT_SHADER,
                            FRAG_SHADER,
                        );
                        let mut unit = Unit::new(position, position, sprite);
                        unit.patrol = record.patrols.get(i).cloned().unwrap_or_default();
                        if let Some(s) = &record.stats {
                            unit.stats = s.clone();
                        }
                        unit_vec.push(unit);
                    }
                }
            }
        }
        return unit_vec
    }

}

// ── Physics component ──────────────────────────────────────────────────────────
/// Per-tile physics data. Extend this as the physics system grows.
/// `solid` blocks movement; add friction, bounciness, etc. here later.
pub struct PhysicsComponent {
    pub solid: bool,
}

impl Default for PhysicsComponent {
    fn default() -> Self {
        PhysicsComponent { solid: false }
    }
}

// ── Tile ───────────────────────────────────────────────────────────────────────
pub struct Tile {
    pub tile_id: i32,
    pub size: i32,
    pub position: (i32, i32),
    /// None when tile_id == 0 (empty). Replacing this drops the old GLObject
    /// but does NOT free its GPU resources — GLObject needs a Drop impl for that.
    pub sprite: Option<GLObject>,
    pub physics: PhysicsComponent,
}

impl Tile {
    pub fn new(
        tile_id: i32,
        size: i32,
        position: (i32, i32),
        sprite: Option<GLObject>,
        physics: PhysicsComponent,
    ) -> Self {
        Tile { tile_id, size, position, sprite, physics }
    }

    /// Paint this tile with a new id and texture. Pass `texture_path = None`
    /// (or `tile_id = 0`) to make this an empty tile.
    pub fn set_sprite(&mut self, tile_id: i32, texture_path: Option<&str>) {
        self.tile_id = tile_id;
        self.sprite = texture_path
            .filter(|_| tile_id != 0)
            .map(|p| GLObject::new(BL_RECTANGLE, &format!("assets/{}", p), VERT_SHADER, FRAG_SHADER));
    }

    /// Skips drawing if this is an empty tile (id == 0).
    pub fn draw(&self, camera: (i32, i32), zoom: f32) {
        if self.tile_id == 0 {
            return;
        }
        if let Some(sprite) = &self.sprite {
            let sx = ((self.position.0 * self.size - camera.0) as f32 * zoom) as i32;
            let sy = ((self.position.1 * self.size - camera.1) as f32 * zoom) as i32;
            sprite.draw(sx, sy, self.size as f32 * zoom);
        }
    }
}

// ── World ──────────────────────────────────────────────────────────────────────
pub struct World {
    pub tiles: Vec<Tile>,
    pub width: usize,
    pub height: usize,
    /// Per-map player spawn (tile coords). `None` = no spawn placed yet.
    /// Read from the map's `---SPAWN---` section; the game uses it to position
    /// the player at launch. Editor's Spawner tab places/clears it.
    pub spawn: Option<(i32, i32)>,
}

impl World {
    /// Load a world from a map file and a texture-id key file.
    /// Tiles with id 0 are created as empty (no sprite).
    pub fn load(map_path: &str, id_path: &str) -> Self {
        let id_map      = load_map(map_path);
        let physics_map = load_physics(map_path);
        let id_key      = load_textures(id_path);
        let height = id_map.len();
        let width  = id_map.first().map_or(0, |r| r.len());
        let mut tiles = Vec::with_capacity(width * height);
        let mut y = 0i32;
        for row in id_map {
            let mut x = 0i32;
            for id in row {
                let sprite = if id == 0 {
                    None
                } else {
                    id_key.get(&id)
                        .map(|p| GLObject::new(BL_RECTANGLE, &format!("assets/{}", p), VERT_SHADER, FRAG_SHADER))
                };
                let solid = physics_map
                    .get(y as usize)
                    .and_then(|r| r.get(x as usize))
                    .copied()
                    .unwrap_or(false);
                tiles.push(Tile::new(id, TILE_SIZE, (x, y), sprite, PhysicsComponent { solid }));
                x += 1;
            }
            y += 1;
        }
        let spawn = load_spawn(map_path);
        World { tiles, width, height, spawn }
    }

    /// Create a blank world of the given dimensions, all tiles empty (id = 0).
    pub fn new_empty(width: usize, height: usize) -> Self {
        let mut tiles = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                tiles.push(Tile::new(
                    0, TILE_SIZE, (x as i32, y as i32), None, PhysicsComponent::default(),
                ));
            }
        }
        World { tiles, width, height, spawn: None }
    }

    pub fn draw(&self, camera: (i32, i32), zoom: f32) {
        for tile in &self.tiles {
            tile.draw(camera, zoom);
        }
    }

    /// Resize the world, preserving tiles that fit in the new bounds.
    /// Tiles outside the new bounds are dropped; new cells start empty.
    pub fn resize(&mut self, new_width: usize, new_height: usize) {
        let old_tiles: Vec<Tile> = std::mem::take(&mut self.tiles);
        let mut by_pos: std::collections::HashMap<(i32, i32), Tile> =
            old_tiles.into_iter().map(|t| (t.position, t)).collect();
        let mut new_tiles = Vec::with_capacity(new_width * new_height);
        for y in 0..new_height {
            for x in 0..new_width {
                let pos = (x as i32, y as i32);
                new_tiles.push(by_pos.remove(&pos).unwrap_or_else(|| {
                    Tile::new(0, TILE_SIZE, pos, None, PhysicsComponent::default())
                }));
            }
        }
        self.tiles  = new_tiles;
        self.width  = new_width;
        self.height = new_height;
        // Drop the spawn if the resized map no longer contains it.
        if let Some((sx, sy)) = self.spawn {
            if sx < 0 || sy < 0 || (sx as usize) >= new_width || (sy as usize) >= new_height {
                self.spawn = None;
            }
        }
    }

    /// Write the map back to disk in the same space-separated format that load_map reads.
    pub fn save(&self, path: &str) {
        // ── Tile ID grid ──
        let mut id_rows: Vec<Vec<i32>> = vec![vec![0; self.width]; self.height];
        let mut ph_rows: Vec<Vec<i32>> = vec![vec![0; self.width]; self.height];
        for tile in &self.tiles {
            let (tx, ty) = tile.position;
            if tx >= 0 && ty >= 0 && (tx as usize) < self.width && (ty as usize) < self.height {
                id_rows[ty as usize][tx as usize] = tile.tile_id;
                ph_rows[ty as usize][tx as usize] = if tile.physics.solid { 1 } else { 0 };
            }
        }
        // Reverse both grids: row 0 in world = bottom row of file.
        id_rows.reverse();
        ph_rows.reverse();

        let serialize = |rows: &Vec<Vec<i32>>| -> String {
            rows.iter()
                .map(|row| row.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" "))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let mut content = format!(
            "{}\n{}\n{}",
            serialize(&id_rows),
            MAP_PHYSICS_DELIMITER,
            serialize(&ph_rows),
        );
        // Only emit ---SPAWN--- when one is set, so maps that never used the
        // feature stay byte-identical to before.
        if let Some((sx, sy)) = self.spawn {
            content.push_str(&format!("\n{}\n{} {}", MAP_SPAWN_DELIMITER, sx, sy));
        }
        // Ensure the target directory exists (e.g. maps/ on first save).
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() { let _ = std::fs::create_dir_all(parent); }
        }
        std::fs::write(path, content).expect("Failed to save map");
    }
}

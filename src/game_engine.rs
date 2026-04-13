use std::sync::Arc;
use crate::tools::*;
use crate::gui::EguiRenderer;
use crate::shaders::{FRAG_SHADER, VERT_SHADER};

pub const TILE_SIZE: i32 = 32;

// ── Context trait ──────────────────────────────────────────────────────────────
/// Implement this for every screen/mode (main menu, gameplay, editor, pause, …).
/// Return `Some(next_context)` from `update` to transition, `None` to stay.
pub trait GameContext {
    fn update(&mut self, engine: &mut Engine) -> Option<Box<dyn GameContext>>;
    // &mut self lets contexts track click state or other per-frame mutable data.
    fn draw(&mut self, engine: &mut Engine);
}

// ── Engine ─────────────────────────────────────────────────────────────────────
pub struct Engine {
    // GlWindow must be declared before Sdl — Rust drops fields in declaration
    // order, and the window must be destroyed before the SDL context.
    win: GlWindow,
    sdl: Sdl,
    pub renderer: EguiRenderer,
    pub win_open: bool,
    pub current_action: actions,
    pub egui_input: egui::RawInput,
    pub mouse_pos: egui::Pos2,
    /// World-space offset of the bottom-left corner of the screen.
    /// (0, 0) means the world origin is at the bottom-left of the screen.
    pub camera: (i32, i32),
}

impl Engine {
    pub fn new(title: &str) -> Self {
        let sdl = init_sdl();
        let win = init_window(title, &sdl);
        let gl = Arc::new(unsafe {
            egui_glow::glow::Context::from_loader_function(|s| {
                let c_str = std::ffi::CString::new(s).unwrap();
                win.get_proc_address(c_str.as_ptr() as *const u8) as *const _
            })
        });
        win.set_swap_interval(GlSwapInterval::Vsync).unwrap();
        clear_color(0.2, 0.3, 0.3, 1.0);
        Engine {
            win,
            sdl,
            renderer: EguiRenderer::new(gl),
            win_open: true,
            current_action: actions::NONE,
            egui_input: egui::RawInput::default(),
            mouse_pos: egui::Pos2::default(),
            camera: (0, 0),
        }
    }

    /// Drives the context state machine until the window is closed.
    pub fn run(&mut self, mut context: Box<dyn GameContext>) {
        while self.win_open {
            self.poll_input();
            if let Some(next) = context.update(self) {
                context = next;
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
        self.current_action = actions::NONE;
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
            &mut self.current_action,
            &mut self.egui_input,
            &mut self.mouse_pos,
        );
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
    pub fn draw(&self, camera: (i32, i32)) {
        if self.tile_id == 0 {
            return;
        }
        if let Some(sprite) = &self.sprite {
            sprite.draw(
                self.position.0 * self.size - camera.0,
                self.position.1 * self.size - camera.1,
                self.size as f32,
            );
        }
    }
}

// ── World ──────────────────────────────────────────────────────────────────────
pub struct World {
    pub tiles: Vec<Tile>,
    pub width: usize,
    pub height: usize,
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
        World { tiles, width, height }
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
        World { tiles, width, height }
    }

    pub fn draw(&self, camera: (i32, i32)) {
        for tile in &self.tiles {
            tile.draw(camera);
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

        let content = format!(
            "{}\n{}\n{}",
            serialize(&id_rows),
            MAP_PHYSICS_DELIMITER,
            serialize(&ph_rows),
        );
        std::fs::write(path, content).expect("Failed to save map");
    }
}

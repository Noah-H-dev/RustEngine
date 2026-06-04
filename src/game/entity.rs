use crate::tools::*;
use crate::shaders::{FRAG_SHADER, TRANS_FRAG_SHADER, VERT_SHADER};
use super::game_engine::World;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use super::stats::stats;
use serde::{Deserialize, Serialize};
use crate::game::item::item;
// ── Player loader ────────────────────────────────────────────────────────────
//
// Reads `player.toml` into a `Unit` ready to drop into the engine. The on-disk
// format is `[[unit]]` (array of tables) for forward compatibility with
// multi-player, but the loader currently takes the first entry and ignores any
// others — see the deferred-features comment at the top of main.rs.
//
// `sprite_id` is resolved via the active tileset (id.txt-style file passed as
// `id_path`), so the same player file looks different under a "forest" vs
// "dungeon" tileset. Anything missing falls back to defaults — never panics.

const PLAYER_FILE:     &str = "gamedata/player.toml";
const FALLBACK_SPRITE: &str = "assets/player.png";

#[derive(Deserialize, Default)]
struct PlayerFile {
    #[serde(default)]
    unit: Vec<PlayerRecord>,
}

/// One persisted player. `spawn` is stored for the future save-game flow
/// (see main.rs deferred list) — it is NOT used at game start today; the
/// per-map ---SPAWN--- marker determines the actual starting tile.
#[derive(Deserialize, Default)]
struct PlayerRecord {
    #[serde(default)] sprite_id: Option<i32>,
    #[serde(default)] spawn:     Option<[i32; 2]>,
    #[serde(default)] stats:     stats,
}

fn load_player_record() -> PlayerRecord {
    if !std::path::Path::new(PLAYER_FILE).exists() { return PlayerRecord::default(); }
    let content = std::fs::read_to_string(PLAYER_FILE).unwrap_or_default();
    toml::from_str::<PlayerFile>(&content)
        .map(|f| f.unit.into_iter().next().unwrap_or_default())
        .unwrap_or_default()
}

/// Assemble the player `Unit` from `player.toml`, resolving `sprite_id` against
/// `id_path` (the active tileset). Position is `(0, 0)` — callers should
/// override it from the map's spawn at game start.
pub fn load_player(id_path: &str) -> Unit {
    let record = load_player_record();
    let sprite_path = record.sprite_id
        .and_then(|id| load_textures(id_path).get(&id).map(|p| format!("assets/{}", p)))
        .filter(|p| std::path::Path::new(p).exists())
        .unwrap_or_else(|| FALLBACK_SPRITE.to_string());
    let sprite = GLObject::new(BL_RECTANGLE, &sprite_path, VERT_SHADER, FRAG_SHADER);
    let mut unit = Unit::new((0, 0), (0, 0), sprite);
    unit.stats = record.stats;
    unit
}

/// The default spawn coordinate persisted in `player.toml`, if any. Returned
/// as `(x, y)`. Used as a fallback when the loaded map has no per-map spawn.
pub fn player_default_spawn() -> Option<(i32, i32)> {
    load_player_record().spawn.map(|[x, y]| (x, y))
}

pub struct properties{

}

pub struct Unit {
    pub position: (i32,i32),
    pub target_position: (i32,i32),
    pub patrol: Vec<(i32,i32)>,
    pub patrol_idx: i32,
    pub path: Vec<(i32, i32)>,
    pub size: f32,
    pub stats: stats,
    pub action_time:f64,
    pub inventory: Vec<item>,
    sprite: GLObject,

}
impl Unit {
    pub fn new(position: (i32,i32),target_position: (i32,i32), sprite:GLObject) -> Unit {
        return Unit {position,target_position,size: 1.0, patrol: vec!(),patrol_idx:0, path: vec!(), stats:stats::new(1,1.0, 0.0),action_time: 0.0,inventory:Vec::new(), sprite}
    }
    pub fn draw(&self, camera: (i32, i32)){
        use super::game_engine::TILE_SIZE;
        self.sprite.draw(
            self.position.0 * TILE_SIZE - camera.0,
            self.position.1 * TILE_SIZE - camera.1,
            TILE_SIZE as f32,
        );
    }
    pub fn update(&mut self, world: &World){
        let mut current_move = (0,0);
        self.action_time += self.stats.speed;
        while self.action_time > 1.0 {
            if self.path.len() > 0 {
                current_move = self.path.pop().unwrap();
                self.position.0 += current_move.0;
                self.position.1 += current_move.1;
            } else if self.patrol.len() > 0 {
                self.target_position = self.patrol[self.patrol_idx as usize];
                self.update_path(world);
                self.patrol_idx += 1;
                if self.patrol_idx == (self.patrol.len()) as i32 {
                    self.patrol_idx = 0;
                }
            }
            self.action_time -= 1.0;
        }
    }

    /// Consumes one step from the pre-computed path and subtracts 1.0 from action_time.
    /// Hitting a wall (empty path) does not consume action_time.
    pub fn player_update(&mut self) {
        if let Some(step) = self.path.pop() {
            self.position.0 += step.0;
            self.position.1 += step.1;
            self.action_time -= 1.0;
        }
        self.target_position = self.position;
    }

    /// Runs A* from self.position to self.target_position using World tile solidity.
    /// Stores the resulting tile-coordinate path (excluding start, including goal) in self.path.
    /// self.path will be empty if already at the target or no path exists.
    pub fn update_path(&mut self, world: &World){
        let start = self.position;
        let goal  = self.target_position;

        if start == goal {
            self.path = vec![];
            return;
        }

        let is_walkable = |pos: (i32, i32)| -> bool {
            if pos.0 < 0 || pos.1 < 0 { return false; }
            let x = pos.0 as usize;
            let y = pos.1 as usize;
            if x >= world.width || y >= world.height { return false; }
            !world.tiles[y * world.width + x].physics.solid
        };

        let heuristic = |pos: (i32, i32)| -> i32 {
            (pos.0 - goal.0).abs() + (pos.1 - goal.1).abs()
        };

        // Min-heap keyed on f_score = g_score + heuristic.
        let mut open: BinaryHeap<(Reverse<i32>, (i32, i32))> = BinaryHeap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

        g_score.insert(start, 0);
        open.push((Reverse(heuristic(start)), start));

        const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

        while let Some((_, current)) = open.pop() {
            if current == goal {
                let mut path = vec![];
                let mut cur = goal;
                while cur != start {
                    let prev = came_from[&cur];
                    // Forward delta: the direction taken to step from prev into cur.
                    // Collecting goal→start means the vec is already reversed for pop().
                    path.push((cur.0 - prev.0, cur.1 - prev.1));
                    cur = prev;
                }
                self.path = path;
                return;
            }

            let current_g = g_score[&current];

            for &(dx, dy) in &DIRS {
                let neighbor = (current.0 + dx, current.1 + dy);
                if !is_walkable(neighbor) { continue; }

                let tentative_g = current_g + 1;
                if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    open.push((Reverse(tentative_g + heuristic(neighbor)), neighbor));
                }
            }
        }

        self.path = vec![]; // no path found
    }
}

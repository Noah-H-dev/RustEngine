use RustEngine::game::game_engine::{Engine, GameContext, SaveGame, UnitState, World};
use RustEngine::game::entity::player_default_spawn;
use RustEngine::tools::{actions, directions, Key};
use super::settings::SettingsContext;

// ── Gameplay ───────────────────────────────────────────────────────────────────
pub struct GameRunningContext {
    map_path:            String,
    id_path:             String,
    loaded:              bool,
    wants_settings:      bool,
    player_budget_ready: bool,
    /// `Some` only for a continued save: the player start tile (from player.toml
    /// `spawn`) and the live unit overrides to overlay after loading units. A
    /// fresh Run leaves both `None` and starts the player at the map spawn.
    resume_player_pos:   Option<(i32, i32)>,
    unit_overrides:      Option<Vec<UnitState>>,
}

impl GameRunningContext {
    pub fn new(map_path: &str, id_path: &str) -> Self {
        GameRunningContext {
            map_path:            map_path.to_string(),
            id_path:             id_path.to_string(),
            loaded:              false,
            wants_settings:      false,
            player_budget_ready: false,
            resume_player_pos:   None,
            unit_overrides:      None,
        }
    }

    /// Resume an already-loaded game — skips the world/unit reload (used when
    /// returning from the in-game settings/pause overlay).
    pub fn resume(map_path: &str, id_path: &str) -> Self {
        GameRunningContext {
            map_path:            map_path.to_string(),
            id_path:             id_path.to_string(),
            loaded:              true,
            wants_settings:      false,
            player_budget_ready: false,
            resume_player_pos:   None,
            unit_overrides:      None,
        }
    }

    /// Continue a saved session (game.toml). Loads the saved map/tileset, starts
    /// the player at their saved position (player.toml `spawn`), and overlays the
    /// saved unit positions / patrol progress onto the freshly-loaded units.
    pub fn continue_from(save: SaveGame) -> Self {
        GameRunningContext {
            map_path:            save.map.clone(),
            id_path:             save.tileset.clone(),
            loaded:              false,
            wants_settings:      false,
            player_budget_ready: false,
            resume_player_pos:   player_default_spawn(),
            unit_overrides:      Some(save.units),
        }
    }
}

impl GameContext for GameRunningContext {
    fn update(&mut self, engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        // Gameplay decides what the neutral keys mean. The pump no longer knows
        // that WASD is movement — that binding lives here, so menus/editors are
        // unaffected by it.
        engine.player_action = keys_to_action(&engine.keys_pressed);

        // Escape opens the in-game settings/pause overlay. Detected here (not in
        // draw) because the tick loop clears `keys_pressed` before draw runs.
        if engine.keys_pressed.contains(&Key::Escape) {
            self.wants_settings = true;
        }

        if !self.loaded {
            engine.world = World::load(&self.map_path, &self.id_path);
            engine.units = Engine::load_units(&self.id_path);
            // Fresh Run starts at the map's ---SPAWN--- marker (or (0, 0) when
            // none is set). A continued save instead starts at the saved player
            // position (player.toml `spawn`), carried in `resume_player_pos`.
            let start = self.resume_player_pos
                .or(engine.world.spawn)
                .unwrap_or((0, 0));
            engine.player.position        = start;
            engine.player.target_position = start;
            engine.player.path.clear();
            // Overlay saved unit state (continue only). Units load from units.toml
            // in a stable order; we match overrides by that index. Extra/missing
            // entries (units.toml edited since saving) are simply skipped.
            if let Some(overrides) = self.unit_overrides.take() {
                for (unit, saved) in engine.units.iter_mut().zip(overrides.iter()) {
                    unit.position        = saved.position;
                    unit.target_position = saved.position;
                    unit.patrol_idx      = saved.patrol_idx;
                    unit.path.clear();
                }
            }
            self.loaded = true;
        }
        if self.wants_settings {
            self.wants_settings = false;
            return Some(Box::new(SettingsContext::from_game(&self.map_path, &self.id_path)));
        }

        if engine.settings.real_time {
            if let actions::MOVE { ref dir } = engine.player_action {
                let (dx, dy) = dir.value();
                engine.player.target_position.0 += dx as i32;
                engine.player.target_position.1 += dy as i32;
                engine.player.update_path(&engine.world);
            }
            step_units(engine);
            engine.player.update(&engine.world);
            engine.player.target_position = engine.player.position;
            return None;
        }

        // Turn-based: give the player their speed budget once per round.
        let player_pressed = matches!(engine.player_action, actions::MOVE { .. });

        if !self.player_budget_ready && player_pressed {
            engine.player.action_time += engine.player.stats.speed;
            self.player_budget_ready = true;
        }

        // Execute one player move if they have remaining budget.
        if engine.player.action_time >= 1.0 && player_pressed {
            if let actions::MOVE { ref dir } = engine.player_action {
                let (dx, dy) = dir.value();
                engine.player.target_position.0 += dx as i32;
                engine.player.target_position.1 += dy as i32;
                engine.player.update_path(&engine.world);
                engine.player.player_update();
            }
            engine.player.action_time -= 1.0;
        }

        // Enemies take their turn once the player's budget is exhausted.
        if player_pressed && engine.player.action_time < 1.0 {
            step_units(engine);
            self.player_budget_ready = false;
        }

        None
    }

    fn draw(&mut self, engine: &mut Engine) {
        engine.world.draw(engine.camera, 1.0);
        for unit in &engine.units {
            unit.draw(engine.camera);
        }
        engine.player.draw(engine.camera);
    }
}

/// Map this tick's neutral key presses to the player's intended action. WASD →
/// movement; last directional key pressed this frame wins (matching the old
/// pump's overwrite behavior). Non-movement keys are ignored here.
fn keys_to_action(keys: &[Key]) -> actions {
    let mut action = actions::NONE;
    for key in keys {
        action = match key {
            Key::W => actions::MOVE { dir: directions::UP },
            Key::S => actions::MOVE { dir: directions::DOWN },
            Key::A => actions::MOVE { dir: directions::LEFT },
            Key::D => actions::MOVE { dir: directions::RIGHT },
            _ => continue,
        };
    }
    action
}

/// Run all non-player units for one tick: decide phase -> execute phase -> the
/// existing patrol/A* movement.
///
/// Mirrors the player channel: where the player's intent lives on the Engine as
/// `player_action`, each Unit owns its `current_action` (the source of truth).
///
/// NOTE (alongside, not drives-movement): `decide` is a stub returning NONE, so
/// `unit_actions` is empty today and patrol in `Unit::update` still does the
/// moving. The decide/execute plumbing is wired so AI can be layered on later.
/// TODO come back and switch to "drives movement" (see notes.txt / Unit::decide).
fn step_units(engine: &mut Engine) {
    // ── Decide phase: each unit sets its own current_action (AI hook). ──
    for unit in &mut engine.units {
        unit.current_action = unit.decide(&engine.world, &engine.player);
    }

    // ── Execute phase: sparse, sortable (unit_index, action) work-list. ──
    // Only units that actually want to act appear here; a future turn-order
    // system can sort this by speed/initiative before applying. Resolution is
    // sequential for now — each applied action is visible to the next.
    let unit_actions: Vec<(usize, actions)> = engine.units.iter().enumerate()
        .filter(|(_, u)| u.current_action != actions::NONE)
        .map(|(i, u)| (i, u.current_action))
        .collect();
    for (i, action) in unit_actions {
        engine.units[i].apply_action(action, &engine.world);
        engine.units[i].current_action = actions::NONE;
    }

    // Existing patrol/path movement — still the mover under the "alongside" plan.
    let world = &engine.world;
    for unit in &mut engine.units {
        unit.update(world);
    }
}

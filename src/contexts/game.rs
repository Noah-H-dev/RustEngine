use RustEngine::game::game_engine::{Engine, GameContext, World};
use RustEngine::tools::actions;
use super::settings::SettingsContext;

// ── Gameplay ───────────────────────────────────────────────────────────────────
pub struct GameRunningContext {
    map_path:            String,
    id_path:             String,
    loaded:              bool,
    wants_settings:      bool,
    player_budget_ready: bool,
}

impl GameRunningContext {
    pub fn new(map_path: &str, id_path: &str) -> Self {
        GameRunningContext {
            map_path:            map_path.to_string(),
            id_path:             id_path.to_string(),
            loaded:              false,
            wants_settings:      false,
            player_budget_ready: false,
        }
    }

    /// Resume an already-loaded game — skips the world/unit reload.
    pub fn resume(map_path: &str, id_path: &str) -> Self {
        GameRunningContext {
            map_path:            map_path.to_string(),
            id_path:             id_path.to_string(),
            loaded:              true,
            wants_settings:      false,
            player_budget_ready: false,
        }
    }
}

impl GameContext for GameRunningContext {
    fn update(&mut self, engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        if !self.loaded {
            engine.world = World::load(&self.map_path, &self.id_path);
            engine.units = Engine::load_units(&self.id_path);
            //here we edit the player position
            self.loaded = true;
        }
        if self.wants_settings {
            self.wants_settings = false;
            return Some(Box::new(SettingsContext::from_game(&self.map_path, &self.id_path)));
        }

        if engine.settings.real_time {
            if let actions::MOVE { ref dir } = engine.current_action {
                let (dx, dy) = dir.value();
                engine.player.target_position.0 += dx as i32;
                engine.player.target_position.1 += dy as i32;
                engine.player.update_path(&engine.world);
            }
            let world = &engine.world;
            for unit in &mut engine.units {
                unit.update(world);
            }
            engine.player.update(&engine.world);
            engine.player.target_position = engine.player.position;
            return None;
        }

        // Turn-based: give the player their speed budget once per round.
        let player_pressed = matches!(engine.current_action, actions::MOVE { .. });

        if !self.player_budget_ready && player_pressed {
            engine.player.action_time += engine.player.stats.speed;
            self.player_budget_ready = true;
        }

        // Execute one player move if they have remaining budget.
        if engine.player.action_time >= 1.0 && player_pressed {
            if let actions::MOVE { ref dir } = engine.current_action {
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
            let world = &engine.world;
            for unit in &mut engine.units {
                unit.update(world);
            }
            self.player_budget_ready = false;
        }

        None
    }

    fn draw(&mut self, engine: &mut Engine) {
        let esc = engine.egui_input.events.iter().any(|e| matches!(
            e, egui::Event::Key { key: egui::Key::Escape, pressed: true, .. }
        ));
        if esc { self.wants_settings = true; }

        engine.world.draw(engine.camera, 1.0);
        for unit in &engine.units {
            unit.draw(engine.camera);
        }
        engine.player.draw(engine.camera);
    }
}

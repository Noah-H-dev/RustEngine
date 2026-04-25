use RustEngine::game_engine::{Engine, GameContext, World};
use RustEngine::tools::actions;
use super::settings::SettingsContext;

// ── Gameplay ───────────────────────────────────────────────────────────────────
pub struct GameRunningContext {
    map_path:       String,
    id_path:        String,
    loaded:         bool,
    wants_settings: bool,
    pub real_time:  bool,
}

impl GameRunningContext {
    pub fn new(map_path: &str, id_path: &str) -> Self {
        GameRunningContext {
            map_path:       map_path.to_string(),
            id_path:        id_path.to_string(),
            loaded:         false,
            wants_settings: false,
            real_time:      false,
        }
    }

    /// Resume an already-loaded game — skips the world/unit reload.
    pub fn resume(map_path: &str, id_path: &str, real_time: bool) -> Self {
        GameRunningContext {
            map_path:       map_path.to_string(),
            id_path:        id_path.to_string(),
            loaded:         true,
            wants_settings: false,
            real_time,
        }
    }
}

impl GameContext for GameRunningContext {
    fn update(&mut self, engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        if !self.loaded {
            engine.world = World::load(&self.map_path, &self.id_path);
            engine.units = Engine::load_units(&self.id_path);
            self.loaded = true;
        }
        if self.wants_settings {
            self.wants_settings = false;
            return Some(Box::new(SettingsContext::from_game(&self.map_path, &self.id_path, self.real_time)));
        }
        let player_moved = matches!(engine.current_action, actions::MOVE { .. });

        if let actions::MOVE { ref dir } = engine.current_action {
            let (dx, dy) = dir.value();
            engine.player.target_position.0 += dx as i32;
            engine.player.target_position.1 += dy as i32;
            engine.player.update_path(&engine.world);
        }

        if self.real_time || player_moved {
            let world = &engine.world;
            for unit in &mut engine.units {
                unit.update(world);
            }
            engine.player.update(&engine.world);
        }



        None
    }

    fn draw(&mut self, engine: &mut Engine) {
        let esc = engine.egui_input.events.iter().any(|e| matches!(
            e, egui::Event::Key { key: egui::Key::Escape, pressed: true, .. }
        ));
        if esc { self.wants_settings = true; }

        engine.world.draw(engine.camera);
        for unit in &engine.units {
            unit.draw(engine.camera);
        }
        engine.player.draw(engine.camera);
    }
}

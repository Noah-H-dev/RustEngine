use RustEngine::game::game_engine::{Engine, GameContext, SaveGame, UnitState, World};
use RustEngine::game::entity::player_default_spawn;
use RustEngine::tools::{actions, directions, Key};
use super::settings::SettingsContext;

pub struct GameRunningContext {
    map_path:            String,
    id_path:             String,
    loaded:              bool,
    wants_settings:      bool,
    player_budget_ready: bool,
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
        engine.player_action = keys_to_action(&engine.keys_pressed);

        if engine.keys_pressed.contains(&Key::Escape) {
            self.wants_settings = true;
        }

        if !self.loaded {
            engine.world = World::load(&self.map_path, &self.id_path);
            engine.units = Engine::load_units(&self.id_path);
            let start = self.resume_player_pos
                .or(engine.world.spawn)
                .unwrap_or((0, 0));
            engine.player.position        = start;
            engine.player.target_position = start;
            engine.player.path.clear();
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

        let player_pressed = matches!(engine.player_action, actions::MOVE { .. });

        if !self.player_budget_ready && player_pressed {
            engine.player.action_time += engine.player.stats.speed;
            self.player_budget_ready = true;
        }

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

fn step_units(engine: &mut Engine) {
    for unit in &mut engine.units {
        unit.current_action = unit.decide(&engine.world, &engine.player);
    }

    // TODO drives-movement rework: sort this work-list by speed/initiative
    // before applying (see notes.txt / Unit::decide).
    let unit_actions: Vec<(usize, actions)> = engine.units.iter().enumerate()
        .filter(|(_, u)| u.current_action != actions::NONE)
        .map(|(i, u)| (i, u.current_action))
        .collect();
    for (i, action) in unit_actions {
        engine.units[i].apply_action(action, &engine.world);
        engine.units[i].current_action = actions::NONE;
    }

    let world = &engine.world;
    for unit in &mut engine.units {
        unit.update(world);
    }
}

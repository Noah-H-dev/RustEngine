use std::path::{Path, PathBuf};

use RustEngine::game::game_engine::{Engine, GameContext, SaveGame};

use super::editor_menu::EditorMenuContext;
use super::game::GameRunningContext;
use super::settings::SettingsContext;
use super::tileset::ensure_default;

pub(crate) const MAPS_DIR: &str = "maps";

pub(crate) fn resolve_map_path(s: &str) -> String {
    let p = Path::new(s);
    if p.is_absolute() || p.starts_with(MAPS_DIR) {
        s.to_string()
    } else {
        PathBuf::from(MAPS_DIR).join(p).to_string_lossy().into_owned()
    }
}

enum GameSub {
    Hidden,
    Open { map_path: String },
}

impl GameSub {
    fn is_visible(&self) -> bool { !matches!(self, GameSub::Hidden) }
}

pub struct MainMenuContext {
    pending_transition: Option<Box<dyn GameContext>>,
    game_sub: GameSub,
    error_msg: Option<String>,
}

impl MainMenuContext {
    pub fn new() -> Self {
        let _ = ensure_default();
        MainMenuContext {
            pending_transition: None,
            game_sub: GameSub::Hidden,
            error_msg: None,
        }
    }
}

impl GameContext for MainMenuContext {
    fn update(&mut self, _engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        self.pending_transition.take()
    }

    fn draw(&mut self, engine: &mut Engine) {
        let mut toggle_game   = false;
        let mut confirm_game  = false;
        let mut continue_game = false;
        let mut go_editor     = false;
        let mut go_settings   = false;
        let mut quit_app      = false;

        let has_save = SaveGame::exists();

        let (w, h) = engine.screen_size();
        let input = engine.egui_input.clone();

        engine.renderer.render(input, w, h, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(ui.available_height() / 4.0);
                ui.vertical_centered(|ui| {
                    ui.heading("RustEngine");
                    ui.add_space(16.0);

                    if has_save {
                        if ui.add_sized([160.0, 40.0], egui::Button::new("Continue")).clicked() {
                            continue_game = true;
                        }
                        ui.add_space(8.0);
                    }

                    let game_fill = if self.game_sub.is_visible() {
                        egui::Color32::from_rgb(80, 100, 180)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    if ui.add_sized([160.0, 40.0], egui::Button::new("Run").fill(game_fill)).clicked() {
                        toggle_game = true;
                    }
                    if self.game_sub.is_visible() {
                        ui.add_space(4.0);
                        if let GameSub::Open { map_path } = &mut self.game_sub {
                            ui.horizontal(|ui| {
                                let form_w = 280.0;
                                let pad = (ui.available_width() - form_w) / 2.0;
                                if pad > 0.0 { ui.add_space(pad); }
                                ui.vertical(|ui| {
                                    ui.set_max_width(form_w);
                                    egui::Grid::new("game_form").num_columns(2).show(ui, |ui| {
                                        ui.label("Filepath: "); ui.text_edit_singleline(map_path); ui.end_row();
                                    });
                                    if ui.button("Start").clicked() { confirm_game = true; }
                                });
                            });
                        }
                    }
                    ui.add_space(8.0);

                    if ui.add_sized([160.0, 40.0], egui::Button::new("Editor")).clicked() {
                        go_editor = true;
                    }

                    ui.add_space(8.0);
                    if ui.add_sized([160.0, 40.0], egui::Button::new("Settings")).clicked() {
                        go_settings = true;
                    }

                    ui.add_space(16.0);
                    if ui.add_sized([100.0, 28.0], egui::Button::new("Quit")).clicked() {
                        quit_app = true;
                    }

                    if let Some(msg) = &self.error_msg {
                        ui.add_space(8.0);
                        ui.colored_label(egui::Color32::from_rgb(220, 60, 60), msg);
                    }
                });
            });
        });

        if toggle_game {
            self.error_msg = None;
            self.game_sub = if self.game_sub.is_visible() {
                GameSub::Hidden
            } else {
                GameSub::Open { map_path: "map.txt".into() }
            };
        }

        if confirm_game {
            if let GameSub::Open { map_path } = &self.game_sub {
                let map = resolve_map_path(map_path);
                let ids = engine.settings.active_tileset.clone();
                if !std::path::Path::new(&map).exists() {
                    self.error_msg = Some(format!("Map file not found: {}", map));
                } else if !std::path::Path::new(&ids).exists() {
                    self.error_msg = Some(format!("Tileset file not found: {} (pick another in Settings -> Game -> Tileset)", ids));
                } else {
                    self.error_msg = None;
                    self.pending_transition = Some(Box::new(GameRunningContext::new(&map, &ids)));
                }
            }
        }

        if continue_game {
            match SaveGame::load() {
                Some(save) if !Path::new(&save.map).exists() => {
                    self.error_msg = Some(format!("Saved map not found: {}", save.map));
                }
                Some(save) if !Path::new(&save.tileset).exists() => {
                    self.error_msg = Some(format!("Saved tileset not found: {}", save.tileset));
                }
                Some(save) => {
                    self.error_msg = None;
                    self.pending_transition = Some(Box::new(GameRunningContext::continue_from(save)));
                }
                None => { self.error_msg = Some("Could not read save file (game.toml).".into()); }
            }
        }

        if go_editor {
            self.error_msg = None;
            self.pending_transition = Some(Box::new(EditorMenuContext::new()));
        }

        if go_settings {
            self.pending_transition = Some(Box::new(SettingsContext::from_menu()));
        }

        if quit_app { engine.win_open = false; }
    }
}

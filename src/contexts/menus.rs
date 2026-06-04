use std::path::{Path, PathBuf};

use RustEngine::game::game_engine::{Engine, GameContext};

use super::editor_menu::EditorMenuContext;
use super::game::GameRunningContext;
use super::settings::SettingsContext;
use super::tileset::ensure_default;

/// Directory all map files are looked up in. Relative paths typed into the
/// menu are resolved under this folder; absolute paths or paths that already
/// start with this folder are used verbatim (escape hatch for power users).
pub(crate) const MAPS_DIR: &str = "maps";

/// Resolve a user-entered map path against `MAPS_DIR`. Absolute paths and
/// paths already rooted in `maps/` pass through unchanged. Shared with the
/// editor hub, which hosts the map-editor Open/New flow.
pub(crate) fn resolve_map_path(s: &str) -> String {
    let p = Path::new(s);
    if p.is_absolute() || p.starts_with(MAPS_DIR) {
        s.to_string()
    } else {
        PathBuf::from(MAPS_DIR).join(p).to_string_lossy().into_owned()
    }
}

// ── Game sub-menu state (owned by MainMenuContext) ─────────────────────────────
enum GameSub {
    Hidden,
    Open { map_path: String },
}

impl GameSub {
    fn is_visible(&self) -> bool { !matches!(self, GameSub::Hidden) }
}

// ── Main menu ──────────────────────────────────────────────────────────────────
// Top-level entry point. "Run" launches gameplay; "Editor" opens the editor hub
// (EditorMenuContext), which in turn hosts the map editor, tileset editor, and
// game-data editor. The editor sub-screens used to live here directly — they were
// moved to the hub to keep this menu uncluttered as more editors are added.
pub struct MainMenuContext {
    pending_transition: Option<Box<dyn GameContext>>,
    game_sub: GameSub,
    error_msg: Option<String>,
}

impl MainMenuContext {
    pub fn new() -> Self {
        // Make sure tilesets/Default.txt exists so the Settings tileset dropdown
        // always has at least one option, and Run/Editor have a real file to
        // resolve to when the user hasn't picked anything else yet. Migrates a
        // legacy root-level id.txt on first run; otherwise creates it empty.
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
        // Boolean flags written inside the closure, acted on after it returns.
        let mut toggle_game  = false;
        let mut confirm_game = false;
        let mut go_editor    = false;
        let mut go_settings  = false;
        let mut quit_app     = false;

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

                    // ── Editor (opens the editor hub) ──
                    if ui.add_sized([160.0, 40.0], egui::Button::new("Editor")).clicked() {
                        go_editor = true;
                    }

                    // ── Settings ──
                    ui.add_space(8.0);
                    if ui.add_sized([160.0, 40.0], egui::Button::new("Settings")).clicked() {
                        go_settings = true;
                    }

                    // ── Quit (smaller, sits below) ──
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

        // ── Act on flags now that the closure (and its borrows) have ended ──

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

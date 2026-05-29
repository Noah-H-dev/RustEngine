use std::path::{Path, PathBuf};

use RustEngine::game::game_engine::{Engine, GameContext, TILE_SIZE};

use super::editor::EditorContext;
use super::game::GameRunningContext;
use super::settings::SettingsContext;
use super::tileset::ensure_default;
use super::tileset_editor::TilesetEditorContext;

/// Directory all map files are looked up in. Relative paths typed into the
/// menu are resolved under this folder; absolute paths or paths that already
/// start with this folder are used verbatim (escape hatch for power users).
const MAPS_DIR: &str = "maps";

/// Resolve a user-entered map path against `MAPS_DIR`. Absolute paths and
/// paths already rooted in `maps/` pass through unchanged.
fn resolve_map_path(s: &str) -> String {
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

// ── Editor sub-menu state (owned by MainMenuContext) ───────────────────────────
// The tileset (id->png file) is no longer asked here — it's a global setting
// (`Settings.active_tileset`), edited in Settings -> Game -> Tileset.
enum EditorSub {
    Hidden,
    Open { map_path: String },
    New  { map_path: String, width: String, height: String },
}

impl EditorSub {
    fn is_visible(&self) -> bool { !matches!(self, EditorSub::Hidden) }
    fn is_open(&self)    -> bool { matches!(self, EditorSub::Open { .. }) }
    fn is_new(&self)     -> bool { matches!(self, EditorSub::New  { .. }) }

    /// Preserve the map path the user has already typed when switching sub-forms.
    fn current_map_path(&self) -> String {
        match self {
            EditorSub::Open { map_path } |
            EditorSub::New  { map_path, .. } => map_path.clone(),
            EditorSub::Hidden => "map.txt".into(),
        }
    }
}

// ── Main menu ──────────────────────────────────────────────────────────────────
pub struct MainMenuContext {
    pending_transition: Option<Box<dyn GameContext>>,
    game_sub: GameSub,
    editor_sub: EditorSub,
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
            editor_sub: EditorSub::Hidden,
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
        let mut toggle_editor = false;
        let mut show_open    = false;
        let mut show_new     = false;
        let mut confirm_open = false;
        let mut confirm_new  = false;
        let mut go_settings  = false;
        let mut go_tileset   = false;
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

                    // ── Editor button (centered) ──
                    let editor_fill = if self.editor_sub.is_visible() {
                        egui::Color32::from_rgb(80, 100, 180)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    if ui.add_sized([160.0, 40.0], egui::Button::new("Editor").fill(editor_fill)).clicked() {
                        toggle_editor = true;
                    }

                    // ── Open File / New File sub-buttons (centered row) ──
                    if self.editor_sub.is_visible() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let sub_w = 100.0 + 4.0 + 100.0;
                            let pad = (ui.available_width() - sub_w) / 2.0;
                            if pad > 0.0 { ui.add_space(pad); }
                            let open_fill = if self.editor_sub.is_open() {
                                egui::Color32::from_rgb(50, 130, 50)
                            } else {
                                ui.visuals().widgets.inactive.bg_fill
                            };
                            if ui.add_sized([100.0, 32.0], egui::Button::new("Open File").fill(open_fill)).clicked() {
                                show_open = true;
                            }
                            ui.add_space(4.0);
                            let new_fill = if self.editor_sub.is_new() {
                                egui::Color32::from_rgb(50, 130, 50)
                            } else {
                                ui.visuals().widgets.inactive.bg_fill
                            };
                            if ui.add_sized([100.0, 32.0], egui::Button::new("New File").fill(new_fill)).clicked() {
                                show_new = true;
                            }
                        });
                    }

                    // ── File form (shown below when Open or New is active) ──
                    ui.add_space(12.0);
                    match &mut self.editor_sub {
                        EditorSub::Open { map_path } => {
                            ui.horizontal(|ui| {
                                let form_w = 280.0;
                                let pad = (ui.available_width() - form_w) / 2.0;
                                if pad > 0.0 { ui.add_space(pad); }
                                ui.vertical(|ui| {
                                    ui.set_max_width(form_w);
                                    egui::Grid::new("open_form").num_columns(2).show(ui, |ui| {
                                        ui.label("Filepath: "); ui.text_edit_singleline(map_path); ui.end_row();
                                    });
                                    if ui.button("Open").clicked() { confirm_open = true; }
                                });
                            });
                        }
                        EditorSub::New { map_path, width, height } => {
                            ui.horizontal(|ui| {
                                let form_w = 280.0;
                                let pad = (ui.available_width() - form_w) / 2.0;
                                if pad > 0.0 { ui.add_space(pad); }
                                ui.vertical(|ui| {
                                    ui.set_max_width(form_w);
                                    egui::Grid::new("new_form").num_columns(2).show(ui, |ui| {
                                        ui.label("Map: ");   ui.text_edit_singleline(map_path);                              ui.end_row();
                                        ui.label("Width:");  ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(width));  ui.end_row();
                                        ui.label("Height:"); ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(height)); ui.end_row();
                                    });
                                    if ui.button("Create").clicked() { confirm_new = true; }
                                });
                            });
                        }
                        EditorSub::Hidden => {}
                    }

                    // ── Tileset editor ──
                    ui.add_space(8.0);
                    if ui.add_sized([160.0, 40.0], egui::Button::new("Tileset")).clicked() {
                        go_tileset = true;
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

        if toggle_editor {
            self.error_msg = None;
            self.editor_sub = if self.editor_sub.is_visible() {
                EditorSub::Hidden
            } else {
                EditorSub::Open { map_path: "map.txt".into() }
            };
        }

        if show_open && !self.editor_sub.is_open() {
            self.error_msg = None;
            let map_path = self.editor_sub.current_map_path();
            self.editor_sub = EditorSub::Open { map_path };
        }

        if show_new && !self.editor_sub.is_new() {
            self.error_msg = None;
            let map_path = self.editor_sub.current_map_path();
            let (sw, sh) = engine.screen_size();
            let dw = ((sw as i32 + TILE_SIZE - 1) / TILE_SIZE).max(20).to_string();
            let dh = ((sh as i32 + TILE_SIZE - 1) / TILE_SIZE).max(15).to_string();
            self.editor_sub = EditorSub::New { map_path, width: dw, height: dh };
        }

        if confirm_open {
            if let EditorSub::Open { map_path } = &self.editor_sub {
                let map = resolve_map_path(map_path);
                let ids = engine.settings.active_tileset.clone();
                match EditorContext::from_file(&map, &ids) {
                    Ok(ctx) => { self.error_msg = None; self.pending_transition = Some(Box::new(ctx)); }
                    Err(e)  => { self.error_msg = Some(e); }
                }
            }
        }

        if go_settings {
            self.pending_transition = Some(Box::new(SettingsContext::from_menu()));
        }

        if go_tileset {
            self.error_msg = None;
            let active = engine.settings.active_tileset.clone();
            self.pending_transition = Some(Box::new(TilesetEditorContext::new(active)));
        }

        if quit_app { engine.win_open = false; }

        if confirm_new {
            if let EditorSub::New { map_path, width, height } = &self.editor_sub {
                if let (Ok(w), Ok(h)) = (width.parse::<usize>(), height.parse::<usize>()) {
                    let map = resolve_map_path(map_path);
                    let ids = engine.settings.active_tileset.clone();
                    match EditorContext::new_map(&map, &ids, w, h) {
                        Ok(ctx) => { self.error_msg = None; self.pending_transition = Some(Box::new(ctx)); }
                        Err(e)  => { self.error_msg = Some(e); }
                    }
                } else {
                    self.error_msg = Some("Width and height must be valid integers.".into());
                }
            }
        }
    }
}

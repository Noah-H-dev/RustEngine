use RustEngine::game_engine::{Engine, GameContext, TILE_SIZE};

use super::editor::EditorContext;
use super::game::GameRunningContext;
use super::settings::SettingsContext;

// ── Game sub-menu state (owned by MainMenuContext) ─────────────────────────────
enum GameSub {
    Hidden,
    Open { map_path: String, id_path: String },
}

impl GameSub {
    fn is_visible(&self) -> bool { !matches!(self, GameSub::Hidden) }
}

// ── Editor sub-menu state (owned by MainMenuContext) ───────────────────────────
enum EditorSub {
    Hidden,
    Open { map_path: String, id_path: String },
    New  { map_path: String, id_path: String, width: String, height: String },
}

impl EditorSub {
    fn is_visible(&self) -> bool { !matches!(self, EditorSub::Hidden) }
    fn is_open(&self)    -> bool { matches!(self, EditorSub::Open { .. }) }
    fn is_new(&self)     -> bool { matches!(self, EditorSub::New  { .. }) }

    /// Preserve whatever paths the user has already typed when switching sub-forms.
    fn current_paths(&self) -> (String, String) {
        match self {
            EditorSub::Open { map_path, id_path } |
            EditorSub::New  { map_path, id_path, .. } => (map_path.clone(), id_path.clone()),
            EditorSub::Hidden => ("map.txt".into(), "id.txt".into()),
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
                        if let GameSub::Open { map_path, id_path } = &mut self.game_sub {
                            ui.horizontal(|ui| {
                                let form_w = 280.0;
                                let pad = (ui.available_width() - form_w) / 2.0;
                                if pad > 0.0 { ui.add_space(pad); }
                                ui.vertical(|ui| {
                                    ui.set_max_width(form_w);
                                    egui::Grid::new("game_form").num_columns(2).show(ui, |ui| {
                                        ui.label("Filepath: "); ui.text_edit_singleline(map_path); ui.end_row();
                                        ui.label("IDs:");      ui.text_edit_singleline(id_path);  ui.end_row();
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
                        EditorSub::Open { map_path, id_path } => {
                            ui.horizontal(|ui| {
                                let form_w = 280.0;
                                let pad = (ui.available_width() - form_w) / 2.0;
                                if pad > 0.0 { ui.add_space(pad); }
                                ui.vertical(|ui| {
                                    ui.set_max_width(form_w);
                                    egui::Grid::new("open_form").num_columns(2).show(ui, |ui| {
                                        ui.label("Filepath: "); ui.text_edit_singleline(map_path); ui.end_row();
                                        ui.label("IDs:");       ui.text_edit_singleline(id_path);  ui.end_row();
                                    });
                                    if ui.button("Open").clicked() { confirm_open = true; }
                                });
                            });
                        }
                        EditorSub::New { map_path, id_path, width, height } => {
                            ui.horizontal(|ui| {
                                let form_w = 280.0;
                                let pad = (ui.available_width() - form_w) / 2.0;
                                if pad > 0.0 { ui.add_space(pad); }
                                ui.vertical(|ui| {
                                    ui.set_max_width(form_w);
                                    egui::Grid::new("new_form").num_columns(2).show(ui, |ui| {
                                        ui.label("Map: ");   ui.text_edit_singleline(map_path);                              ui.end_row();
                                        ui.label("Tiles:");  ui.text_edit_singleline(id_path);                               ui.end_row();
                                        ui.label("Width:");  ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(width));  ui.end_row();
                                        ui.label("Height:"); ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(height)); ui.end_row();
                                    });
                                    if ui.button("Create").clicked() { confirm_new = true; }
                                });
                            });
                        }
                        EditorSub::Hidden => {}
                    }

                    // ── Settings ──
                    ui.add_space(8.0);
                    if ui.add_sized([160.0, 40.0], egui::Button::new("Settings")).clicked() {
                        go_settings = true;
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
                GameSub::Open { map_path: "map.txt".into(), id_path: "id.txt".into() }
            };
        }

        if confirm_game {
            if let GameSub::Open { map_path, id_path } = &self.game_sub {
                let map = map_path.clone();
                let ids = id_path.clone();
                if !std::path::Path::new(&map).exists() {
                    self.error_msg = Some(format!("Map file not found: {}", map));
                } else if !std::path::Path::new(&ids).exists() {
                    self.error_msg = Some(format!("ID file not found: {}", ids));
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
                EditorSub::Open { map_path: "map.txt".into(), id_path: "id.txt".into() }
            };
        }

        if show_open && !self.editor_sub.is_open() {
            self.error_msg = None;
            let (map_path, id_path) = self.editor_sub.current_paths();
            self.editor_sub = EditorSub::Open { map_path, id_path };
        }

        if show_new && !self.editor_sub.is_new() {
            self.error_msg = None;
            let (map_path, id_path) = self.editor_sub.current_paths();
            let (sw, sh) = engine.screen_size();
            let dw = ((sw as i32 + TILE_SIZE - 1) / TILE_SIZE).max(20).to_string();
            let dh = ((sh as i32 + TILE_SIZE - 1) / TILE_SIZE).max(15).to_string();
            self.editor_sub = EditorSub::New { map_path, id_path, width: dw, height: dh };
        }

        if confirm_open {
            if let EditorSub::Open { map_path, id_path } = &self.editor_sub {
                let map = map_path.clone();
                let ids = id_path.clone();
                match EditorContext::from_file(&map, &ids) {
                    Ok(ctx) => { self.error_msg = None; self.pending_transition = Some(Box::new(ctx)); }
                    Err(e)  => { self.error_msg = Some(e); }
                }
            }
        }

        if go_settings {
            self.pending_transition = Some(Box::new(SettingsContext::from_menu()));
        }

        if confirm_new {
            if let EditorSub::New { map_path, id_path, width, height } = &self.editor_sub {
                if let (Ok(w), Ok(h)) = (width.parse::<usize>(), height.parse::<usize>()) {
                    let map = map_path.clone();
                    let ids = id_path.clone();
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

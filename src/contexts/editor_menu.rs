use RustEngine::game::game_engine::{Engine, GameContext, TILE_SIZE};

use super::editor::EditorContext;
use super::game_editor::GameEditorContext;
use super::menus::{resolve_map_path, MainMenuContext};
use super::tileset_editor::TilesetEditorContext;

enum MapSub {
    Hidden,
    Open { map_path: String },
    New  { map_path: String, width: String, height: String },
}

impl MapSub {
    fn is_visible(&self) -> bool { !matches!(self, MapSub::Hidden) }
    fn is_open(&self)    -> bool { matches!(self, MapSub::Open { .. }) }
    fn is_new(&self)     -> bool { matches!(self, MapSub::New  { .. }) }

    fn current_map_path(&self) -> String {
        match self {
            MapSub::Open { map_path } |
            MapSub::New  { map_path, .. } => map_path.clone(),
            MapSub::Hidden => "map.txt".into(),
        }
    }
}

pub struct EditorMenuContext {
    pending: Option<Box<dyn GameContext>>,
    map_sub: MapSub,
    error_msg: Option<String>,
}

impl EditorMenuContext {
    pub fn new() -> Self {
        EditorMenuContext {
            pending: None,
            map_sub: MapSub::Hidden,
            error_msg: None,
        }
    }
}

impl GameContext for EditorMenuContext {
    fn update(&mut self, _engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        self.pending.take()
    }

    fn draw(&mut self, engine: &mut Engine) {
        let mut go_back      = false;
        let mut toggle_map   = false;
        let mut show_open    = false;
        let mut show_new     = false;
        let mut confirm_open = false;
        let mut confirm_new  = false;
        let mut go_tileset   = false;
        let mut go_game      = false;

        let (w, h) = engine.screen_size();
        let input = engine.egui_input.clone();

        engine.renderer.render(input, w, h, |ctx| {
            egui::TopBottomPanel::top("editor_menu_top").show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("< Menu").clicked() { go_back = true; }
                    ui.separator();
                    ui.heading("Editors");
                });
                ui.add_space(6.0);
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(ui.available_height() / 5.0);
                ui.vertical_centered(|ui| {
                    let map_fill = if self.map_sub.is_visible() {
                        egui::Color32::from_rgb(80, 100, 180)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    ui.horizontal(|ui| {
                        let row_w = 180.0 * 3.0 + 8.0 * 2.0;
                        let pad = (ui.available_width() - row_w) / 2.0;
                        if pad > 0.0 { ui.add_space(pad); }
                        if ui.add_sized([180.0, 40.0], egui::Button::new("Map Editor").fill(map_fill)).clicked() {
                            toggle_map = true;
                        }
                        ui.add_space(8.0);
                        if ui.add_sized([180.0, 40.0], egui::Button::new("Tileset Editor")).clicked() {
                            go_tileset = true;
                        }
                        ui.add_space(8.0);
                        if ui.add_sized([180.0, 40.0], egui::Button::new("Game")).clicked() {
                            go_game = true;
                        }
                    });

                    if self.map_sub.is_visible() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let sub_w = 100.0 + 4.0 + 100.0;
                            let pad = (ui.available_width() - sub_w) / 2.0;
                            if pad > 0.0 { ui.add_space(pad); }
                            let open_fill = if self.map_sub.is_open() {
                                egui::Color32::from_rgb(50, 130, 50)
                            } else {
                                ui.visuals().widgets.inactive.bg_fill
                            };
                            if ui.add_sized([100.0, 32.0], egui::Button::new("Open File").fill(open_fill)).clicked() {
                                show_open = true;
                            }
                            ui.add_space(4.0);
                            let new_fill = if self.map_sub.is_new() {
                                egui::Color32::from_rgb(50, 130, 50)
                            } else {
                                ui.visuals().widgets.inactive.bg_fill
                            };
                            if ui.add_sized([100.0, 32.0], egui::Button::new("New File").fill(new_fill)).clicked() {
                                show_new = true;
                            }
                        });
                    }

                    ui.add_space(8.0);
                    match &mut self.map_sub {
                        MapSub::Open { map_path } => {
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
                        MapSub::New { map_path, width, height } => {
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
                        MapSub::Hidden => {}
                    }

                    if let Some(msg) = &self.error_msg {
                        ui.add_space(8.0);
                        ui.colored_label(egui::Color32::from_rgb(220, 60, 60), msg);
                    }
                });
            });
        });

        if go_back {
            self.pending = Some(Box::new(MainMenuContext::new()));
            return;
        }

        if toggle_map {
            self.error_msg = None;
            self.map_sub = if self.map_sub.is_visible() {
                MapSub::Hidden
            } else {
                MapSub::Open { map_path: "map.txt".into() }
            };
        }

        if show_open && !self.map_sub.is_open() {
            self.error_msg = None;
            let map_path = self.map_sub.current_map_path();
            self.map_sub = MapSub::Open { map_path };
        }

        if show_new && !self.map_sub.is_new() {
            self.error_msg = None;
            let map_path = self.map_sub.current_map_path();
            let (sw, sh) = engine.screen_size();
            let dw = ((sw as i32 + TILE_SIZE - 1) / TILE_SIZE).max(20).to_string();
            let dh = ((sh as i32 + TILE_SIZE - 1) / TILE_SIZE).max(15).to_string();
            self.map_sub = MapSub::New { map_path, width: dw, height: dh };
        }

        if confirm_open {
            if let MapSub::Open { map_path } = &self.map_sub {
                let map = resolve_map_path(map_path);
                let ids = engine.settings.active_tileset.clone();
                match EditorContext::from_file(&map, &ids) {
                    Ok(ctx) => { self.error_msg = None; self.pending = Some(Box::new(ctx)); }
                    Err(e)  => { self.error_msg = Some(e); }
                }
            }
        }

        if confirm_new {
            if let MapSub::New { map_path, width, height } = &self.map_sub {
                if let (Ok(mw), Ok(mh)) = (width.parse::<usize>(), height.parse::<usize>()) {
                    let map = resolve_map_path(map_path);
                    let ids = engine.settings.active_tileset.clone();
                    match EditorContext::new_map(&map, &ids, mw, mh) {
                        Ok(ctx) => { self.error_msg = None; self.pending = Some(Box::new(ctx)); }
                        Err(e)  => { self.error_msg = Some(e); }
                    }
                } else {
                    self.error_msg = Some("Width and height must be valid integers.".into());
                }
            }
        }

        if go_tileset {
            self.error_msg = None;
            let active = engine.settings.active_tileset.clone();
            self.pending = Some(Box::new(TilesetEditorContext::new(active)));
        }

        if go_game {
            self.error_msg = None;
            let active = engine.settings.active_tileset.clone();
            self.pending = Some(Box::new(GameEditorContext::new(active)));
        }
    }
}

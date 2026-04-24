// ── How to extend this settings menu ─────────────────────────────────────────
//
// ADDING A NEW SETTING VALUE
//   1. Add the field to `SettingsContext` (e.g. `pub volume: f32`).
//   2. Declare a local mirror variable at the top of `draw` (e.g. `let mut new_volume = None;`).
//   3. Inside `engine.renderer.render`, read the active tab and add the widget:
//        SettingsTab::Audio => {
//            let mut v = self.volume;
//            if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).text("Volume")).changed() {
//                new_volume = Some(v);
//            }
//        }
//   4. After the render closure, apply it: `if let Some(v) = new_volume { self.volume = v; }`
//      (The two-step pattern is necessary because the closure borrows `self` fields
//       that can't also be mutably borrowed inside it at the same time.)
//
// ADDING A NEW TAB
//   1. Add a variant to `SettingsTab`.
//   2. Add a `SelectableLabel` for it in the left-column block inside `draw`.
//   3. Add a match arm in the tab content block.
//
// PERSISTING SETTINGS
//   Store the values in a `Settings` struct, serialize with `serde`/`toml` in
//   `update` whenever a flag like `settings_changed` is true, and load them in
//   `SettingsContext::new` or from a shared `Arc<Mutex<Settings>>` passed to
//   every context that needs to read them (e.g. audio volume, resolution).
// ─────────────────────────────────────────────────────────────────────────────

use RustEngine::game_engine::{Engine, GameContext};

use super::menus::MainMenuContext;
use super::game::GameRunningContext;

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab { Game, Video, Audio }

enum ReturnDest {
    MainMenu,
    Game { map_path: String, id_path: String },
}

pub struct SettingsContext {
    return_dest: ReturnDest,
    active_tab:  SettingsTab,
    pending:     Option<Box<dyn GameContext>>,
    do_quit:     bool,
}

impl SettingsContext {
    pub fn from_menu() -> Self {
        SettingsContext {
            return_dest: ReturnDest::MainMenu,
            active_tab:  SettingsTab::Game,
            pending:     None,
            do_quit:     false,
        }
    }

    pub fn from_game(map_path: &str, id_path: &str) -> Self {
        SettingsContext {
            return_dest: ReturnDest::Game {
                map_path: map_path.to_string(),
                id_path:  id_path.to_string(),
            },
            active_tab: SettingsTab::Game,
            pending:    None,
            do_quit:    false,
        }
    }
}

impl GameContext for SettingsContext {
    fn update(&mut self, engine: &mut Engine, _dt: f32) -> Option<Box<dyn GameContext>> {
        if self.do_quit {
            engine.win_open = false;
            return None;
        }
        self.pending.take()
    }

    fn draw(&mut self, engine: &mut Engine) {
        let mut do_return    = false;
        let mut do_main_menu = false;
        let mut do_quit      = false;
        let mut new_tab: Option<SettingsTab> = None;

        let (w, h) = engine.screen_size();
        let input  = engine.egui_input.clone();

        // Render the game world behind the overlay when entered from in-game.
        if matches!(self.return_dest, ReturnDest::Game { .. }) {
            engine.world.draw(engine.camera);
            for unit in &engine.units {
                unit.draw(engine.camera);
            }
        }

        let active_tab = self.active_tab;
        engine.renderer.render(input, w, h, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(
                    egui::Color32::from_rgba_unmultiplied(80, 80, 80, 160),
                ))
                .show(ctx, |ui| {
                    ui.add_space(ui.available_height() / 4.0);

                    // ── Left-aligned section titles ───────────────────────────
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            if ui.add_sized([160.0, 40.0], egui::SelectableLabel::new(active_tab == SettingsTab::Game,  "Game")).clicked()  { new_tab = Some(SettingsTab::Game);  }
                            ui.add_space(4.0);
                            if ui.add_sized([160.0, 40.0], egui::SelectableLabel::new(active_tab == SettingsTab::Video, "Video")).clicked() { new_tab = Some(SettingsTab::Video); }
                            ui.add_space(4.0);
                            if ui.add_sized([160.0, 40.0], egui::SelectableLabel::new(active_tab == SettingsTab::Audio, "Audio")).clicked() { new_tab = Some(SettingsTab::Audio); }
                        });
                    });

                    ui.add_space(24.0);

                    // ── Tab content ───────────────────────────────────────────
                    // Add settings widgets for the active tab here (see file header).
                    match active_tab {
                        SettingsTab::Game  => {}
                        SettingsTab::Video => {}
                        SettingsTab::Audio => {}
                    }

                    ui.add_space(16.0);

                    // ── Action buttons — centered ─────────────────────────────
                    ui.vertical_centered(|ui| {
                        if ui.add_sized([160.0, 40.0], egui::Button::new("Return")).clicked() {
                            do_return = true;
                        }
                        ui.add_space(4.0);
                        if ui.add_sized([160.0, 40.0], egui::Button::new("Exit to Main Menu")).clicked() {
                            do_main_menu = true;
                        }
                        ui.add_space(4.0);
                        if ui.add_sized([160.0, 40.0], egui::Button::new("Quit")).clicked() {
                            do_quit = true;
                        }
                    });
                });
        });

        if let Some(tab) = new_tab { self.active_tab = tab; }

        if do_return {
            self.pending = Some(match &self.return_dest {
                ReturnDest::MainMenu => Box::new(MainMenuContext::new()),
                ReturnDest::Game { map_path, id_path } => {
                    let (m, i) = (map_path.clone(), id_path.clone());
                    Box::new(GameRunningContext::resume(&m, &i))
                }
            });
        }
        if do_main_menu {
            self.pending = Some(Box::new(MainMenuContext::new()));
        }
        if do_quit {
            self.do_quit = true;
        }
    }
}

// ── How to extend this settings menu ─────────────────────────────────────────
//
// ADDING A NEW SETTING VALUE
//   1. Add the field to `Settings` in game_engine.rs (e.g. `pub volume: f32`).
//   2. In draw(), add `let mut volume = engine.settings.volume;` before the closure.
//   3. Call `add_setting(ui, "Volume", SettingValue::Number(&mut volume));`
//      inside the relevant tab arm.
//   4. After the closure, add `engine.settings.volume = volume;`.
//   Settings are saved automatically when the user presses Return or Exit to Main Menu.
//
// ADDING A NEW TAB
//   1. Add a variant to `SettingsTab`.
//   2. Add a `selectable_label` for it in the tab bar inside `draw`.
//   3. Add a match arm in the settings content block.
// ─────────────────────────────────────────────────────────────────────────────

use RustEngine::game_engine::{Engine, GameContext};

use super::menus::MainMenuContext;
use super::game::GameRunningContext;

// ── Setting widget helper ──────────────────────────────────────────────────────
// TODO: Dropdown support will likely be needed in the future.
enum SettingValue<'a> {
    Checkbox(&'a mut bool),
    Number(&'a mut f64),
}

// Renders one labeled setting row and adds consistent spacing below it.
// Settings stack top-to-bottom automatically because egui's vertical layout
// places each call beneath the previous one.
fn add_setting(ui: &mut egui::Ui, name: &str, value: SettingValue<'_>) {
    match value {
        SettingValue::Checkbox(b) => { ui.checkbox(b, name); }
        SettingValue::Number(n) => {
            ui.horizontal(|ui| {
                ui.label(name);
                ui.add(egui::DragValue::new(n).speed(0.1));
            });
        }
    }
    ui.add_space(4.0);
}

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

        // Setting mirrors: copy from engine.settings before the closure, write back after.
        let mut rt = engine.settings.real_time;

        let (w, h) = engine.screen_size();
        let input  = engine.egui_input.clone();

        // Render the game world behind the overlay when entered from in-game.
        if matches!(self.return_dest, ReturnDest::Game { .. }) {
            engine.world.draw(engine.camera, 1.0);
            for unit in &engine.units {
                unit.draw(engine.camera);
            }
        }

        let active_tab = self.active_tab;
        engine.renderer.render(input, w, h, |ctx| {
            // ── Tab bar pinned to the top ─────────────────────────────────────
            egui::TopBottomPanel::top("settings_tabs").show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(96.0);
                    if ui.selectable_label(active_tab == SettingsTab::Game,  "Game").clicked()  { new_tab = Some(SettingsTab::Game);  }
                    ui.add_space(8.0);
                    if ui.selectable_label(active_tab == SettingsTab::Video, "Video").clicked() { new_tab = Some(SettingsTab::Video); }
                    ui.add_space(8.0);
                    if ui.selectable_label(active_tab == SettingsTab::Audio, "Audio").clicked() { new_tab = Some(SettingsTab::Audio); }
                });
                ui.add_space(6.0);
            });

            // ── Action buttons pinned to the bottom ───────────────────────────
            egui::TopBottomPanel::bottom("settings_actions").show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(96.0);
                    if ui.button("Return").clicked()            { do_return    = true; }
                    ui.add_space(8.0);
                    if ui.button("Exit to Main Menu").clicked() { do_main_menu = true; }
                    ui.add_space(8.0);
                    if ui.button("Quit").clicked()              { do_quit      = true; }
                });
                ui.add_space(8.0);
            });

            // ── Settings content: top-to-bottom, 1 inch from the left ─────────
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(96.0);
                    ui.vertical(|ui| {
                        match active_tab {
                            SettingsTab::Game  => {
                                add_setting(ui, "Realtime mode", SettingValue::Checkbox(&mut rt));
                            }
                            SettingsTab::Video => {}
                            SettingsTab::Audio => {}
                        }
                    });
                });
            });
        });

        if let Some(tab) = new_tab { self.active_tab = tab; }
        engine.settings.real_time = rt;

        if do_return {
            engine.settings.save();
            self.pending = Some(match &self.return_dest {
                ReturnDest::MainMenu => Box::new(MainMenuContext::new()),
                ReturnDest::Game { map_path, id_path } => {
                    let (m, i) = (map_path.clone(), id_path.clone());
                    Box::new(GameRunningContext::resume(&m, &i))
                }
            });
        }
        if do_main_menu {
            engine.settings.save();
            self.pending = Some(Box::new(MainMenuContext::new()));
        }
        if do_quit {
            self.do_quit = true;
        }
    }
}

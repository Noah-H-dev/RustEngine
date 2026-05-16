// ── How to extend this settings menu ─────────────────────────────────────────
//
// ADDING A NEW SETTING — two places to touch:
//   1. Add the field to `Settings` in game_engine.rs (e.g. `pub volume: f64`).
//   2. Add one line to `settings_for_tab` under the right tab:
//        SettingsTab::Audio => vec![
//            ("Volume", Number(&mut s.volume)),
//        ],
//   Settings save automatically when the user picks Return or Exit to Main Menu.
//
// ADDING A NEW TAB
//   1. Add a variant to `SettingsTab`.
//   2. Add a `selectable_label` for it in the tab bar inside `draw`.
//   3. Add a match arm in `settings_for_tab` (start with `vec![]`).
//
// ADDING A NEW WIDGET TYPE (slider, dropdown, …)
//   Extend `SettingValue` with the new variant and `add_setting` with its match arm.
// ─────────────────────────────────────────────────────────────────────────────

use RustEngine::game::game_engine::{Engine, GameContext, Settings};

use super::menus::MainMenuContext;
use super::game::GameRunningContext;

// ── Setting widgets ──────────────────────────────────────────────────────────

enum SettingValue<'a> {
    Checkbox(&'a mut bool),
    Number(&'a mut f64),
}

/// Renders one labeled setting row and adds consistent spacing below it.
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




/// Single source of truth for what settings exist and which tab they live on.
/// Each entry hands the UI a `&mut` directly into `Settings`, so edits land in
/// engine state with no mirror/write-back dance.
fn settings_for_tab<'a>(tab: SettingsTab, s: &'a mut Settings) -> Vec<(&'static str, SettingValue<'a>)> {
    use SettingValue::*;
    match tab {
        SettingsTab::Game  => vec![
            ("Realtime mode", Checkbox(&mut s.real_time)),
        ],
        SettingsTab::Video => vec![],
        SettingsTab::Audio => vec![],
    }
}







// ── Context ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab { Game, Video, Audio }

#[derive(Clone, Copy)]
enum SettingsAction { Return, MainMenu, Quit }

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
        Self::new(ReturnDest::MainMenu)
    }

    pub fn from_game(map_path: &str, id_path: &str) -> Self {
        Self::new(ReturnDest::Game {
            map_path: map_path.to_string(),
            id_path:  id_path.to_string(),
        })
    }

    fn new(return_dest: ReturnDest) -> Self {
        SettingsContext {
            return_dest,
            active_tab: SettingsTab::Game,
            pending:    None,
            do_quit:    false,
        }
    }

    /// The context to swap to when the user clicks Return.
    fn return_context(&self) -> Box<dyn GameContext> {
        match &self.return_dest {
            ReturnDest::MainMenu => Box::new(MainMenuContext::new()),
            ReturnDest::Game { map_path, id_path } => {
                Box::new(GameRunningContext::resume(map_path, id_path))
            }
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
        let mut action:  Option<SettingsAction> = None;
        let mut new_tab: Option<SettingsTab>    = None;

        // Render the game world behind the overlay when entered from in-game.
        if matches!(self.return_dest, ReturnDest::Game { .. }) {
            engine.world.draw(engine.camera, 1.0);
            for unit in &engine.units {
                unit.draw(engine.camera);
            }
        }

        let active_tab = self.active_tab;
        let (w, h)     = engine.screen_size();
        let input      = engine.egui_input.clone();
        // Hand the closure a reborrowable &mut into settings — disjoint from
        // engine.renderer, so the closure can mutate settings in place with
        // no mirror/write-back. Re-listed each pass since egui may invoke
        // the UI closure multiple times per frame for layout convergence.
        let settings   = &mut engine.settings;

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
                    if ui.button("Return").clicked()            { action = Some(SettingsAction::Return);   }
                    ui.add_space(8.0);
                    if ui.button("Exit to Main Menu").clicked() { action = Some(SettingsAction::MainMenu); }
                    ui.add_space(8.0);
                    if ui.button("Quit").clicked()              { action = Some(SettingsAction::Quit);     }
                });
                ui.add_space(8.0);
            });

            // ── Settings content: top-to-bottom, 1 inch from the left ─────────
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(96.0);
                    ui.vertical(|ui| {
                        for (label, val) in settings_for_tab(active_tab, settings) {
                            add_setting(ui, label, val);
                        }
                    });
                });
            });
        });

        if let Some(tab) = new_tab { self.active_tab = tab; }
        match action {
            Some(SettingsAction::Return)   => { engine.settings.save(); self.pending = Some(self.return_context()); }
            Some(SettingsAction::MainMenu) => { engine.settings.save(); self.pending = Some(Box::new(MainMenuContext::new())); }
            Some(SettingsAction::Quit)     => { self.do_quit = true; }
            None => {}
        }
    }
}

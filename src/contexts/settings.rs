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
use super::tileset::Tileset;

// ── Setting widgets ──────────────────────────────────────────────────────────

enum SettingValue<'a> {
    Checkbox(&'a mut bool),
    Number(&'a mut f64),
    /// Dropdown over a fixed set of choices. `value` is the stored selection;
    /// `options` are (stored value, display label) pairs. If `value` doesn't
    /// match any option (e.g. a stale entry), it's still shown as the current
    /// text so the user can see what's set.
    Choice {
        value:   &'a mut String,
        options: Vec<(String, String)>,
    },
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
        SettingValue::Choice { value, options } => {
            // Display the matching option's label, or the raw value if it's
            // not in the list (e.g. a stale or hand-edited setting).
            let current_display = options.iter()
                .find(|(v, _)| v == value)
                .map(|(_, d)| d.clone())
                .unwrap_or_else(|| value.clone());
            ui.horizontal(|ui| {
                ui.label(name);
                egui::ComboBox::from_id_salt(name)
                    .selected_text(current_display)
                    .show_ui(ui, |ui| {
                        for (v, d) in &options {
                            ui.selectable_value(value, v.clone(), d);
                        }
                    });
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
        SettingsTab::Game  => {
            // Each .txt file in tilesets/ becomes a dropdown option. Path is the
            // stored value; the file stem is the display label.
            let tileset_options: Vec<(String, String)> = Tileset::list_in_dir()
                .into_iter()
                .map(|p| {
                    let display = p.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.to_string_lossy().into_owned());
                    (p.to_string_lossy().into_owned(), display)
                })
                .collect();
            vec![
                ("Realtime mode", Checkbox(&mut s.real_time)),
                ("Tileset",       Choice { value: &mut s.active_tileset, options: tileset_options }),
            ]
        }
        SettingsTab::Video => vec![
            ("Per-monitor DPI v2 (restart to apply)", Checkbox(&mut s.dpi_per_monitor)),
        ],
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

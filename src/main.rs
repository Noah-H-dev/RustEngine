mod contexts;
use contexts::MainMenuContext;
use RustEngine::game::game_engine::Engine;


//Zincident Software
/* Notes
start thinking about stats and game logic - stats are there!
Bundled objects, dont allow base tile painting anymore. struct with vec of tilecoords, drawn from bottom left to top right but all holds 1 id. \ Maybe not
interactables? another layer? propperties of tiles in the tileset? (I prefer the second)
interactables first (an enum? probably a struct with a lot of fields, which then in a UI draw loop if interact__pressed for type in interact_menu, then combat interaction

 */

// ── Save / resume (IMPLEMENTED) ─────────────────────────────────────────────
// The pause menu's "Save" button calls `Engine::save`, which writes the player
// position into `player.toml` `spawn` and the map / tileset / live unit state
// into `gamedata/game.toml`. The main menu shows a "Continue" button whenever
// game.toml exists; it resumes the map, starts the player from player.toml
// `spawn`, and overlays the saved unit positions / patrol progress. A fresh
// "Run" still starts at the map's ---SPAWN--- and ignores player.toml `spawn`.
//
// ── Planned features deliberately deferred ──────────────────────────────────
// These are intentionally NOT implemented yet — the code is shaped to make room
// for them but does not include them. Each is on the roadmap.
//
//   * Multi-player support. `player.toml` uses [[unit]] (array of tables)
//     intentionally, but the loader currently takes the first entry and
//     ignores any others. A future change can expand this to multiple
//     players without re-shaping the file.
//   * Dev-mode lock on the Tileset editor's Definitions tab (so end users
//     can't mutate the abstract id namespace at runtime).
//   * Rename tilesets/*.txt -> *.tileset for self-documenting filenames
//     (touches game engine load paths + needs back-compat for existing
//     projects). (textures.toml -> tiledefs.toml is already done.)
static WINDOW_TITLE: &str = "RustEngine";

fn main() {
    let mut engine = Engine::new(WINDOW_TITLE);
    engine.run(Box::new(MainMenuContext::new()));
}

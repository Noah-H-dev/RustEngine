mod contexts;
use contexts::MainMenuContext;
use RustEngine::game::game_engine::Engine;


//Zincident Software
/* Notes
Clean up UI make it more useable - kinda done
add a settings menu (persistant across all contexts) - Yep
add panning and zooming to the level editor YAY!
add tile and physics painter combination YAY!
make levels resizeable from within the editor \ done
abstract the GUI further to make it easier to use \ yep
start thinking about stats and game logic - stats are there!
Bundled objects, dont allow base tile painting anymore. struct with vec of tilecoords, drawn from bottom left to top right but all holds 1 id. \ Maybe not
interactables? another layer? propperties of tiles in the tileset? (I prefer the second)
 */

// ── Planned features deliberately deferred ──────────────────────────────────
// These are intentionally NOT implemented yet — the code is shaped to make room
// for them but does not include them. Each is on the roadmap.
//
//   * Save button + game.toml. A "save game" action that snapshots current
//     runtime state into a top-level game.toml (last map loaded, player
//     position, future world state). Also unlocks: "load last session"
//     behavior on startup (skip the menu's map picker and resume).
//   * Persist runtime player position back to player.toml on save. The
//     player.toml `spawn = [x, y]` field is read and kept today but is not
//     used at game start — `Run` always spawns at the map's ---SPAWN---
//     marker. When the save button lands, `spawn` becomes the "last known
//     position" field that gets written there and consulted on resume.
//   * Multi-player support. `player.toml` uses [[unit]] (array of tables)
//     intentionally, but the loader currently takes the first entry and
//     ignores any others. A future change can expand this to multiple
//     players without re-shaping the file.
//   * Dev-mode lock on the Tileset editor's Definitions tab (so end users
//     can't mutate the abstract id namespace at runtime).
//   * Rename textures.toml -> tiledefs.toml and tilesets/*.txt -> *.tileset
//     for self-documenting filenames (touches game engine load paths +
//     needs back-compat for existing projects).
static WINDOW_TITLE: &str = "RustEngine";

fn main() {
    let mut engine = Engine::new(WINDOW_TITLE);
    engine.run(Box::new(MainMenuContext::new()));
}

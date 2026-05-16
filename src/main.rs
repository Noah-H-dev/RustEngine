mod contexts;
use contexts::MainMenuContext;
use RustEngine::game_engine::Engine;

/* Notes
Clean up UI make it more useable - kinda done
add a settings menu (persistant across all contexts - kinda done
add panning and zooming to the level editor
add tile and physics painter combination
make levels resizeable from within the editor
abstract the GUI further to make it easier to use
start thinking about stats and game logic - stats are there!
 */
static WINDOW_TITLE: &str = "RustEngine";

fn main() {
    let mut engine = Engine::new(WINDOW_TITLE);
    engine.run(Box::new(MainMenuContext::new()));
}

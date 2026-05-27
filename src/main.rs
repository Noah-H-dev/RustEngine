mod contexts;
use contexts::MainMenuContext;
use RustEngine::game::game_engine::Engine;


//Zincident Software
/* Notes
Clean up UI make it more useable - kinda done
add a settings menu (persistant across all contexts - kinda done
add panning and zooming to the level editor
add tile and physics painter combination
make levels resizeable from within the editor \ done
abstract the GUI further to make it easier to use \ yep
start thinking about stats and game logic - stats are there!
Bundled objects, dont allow base tile painting anymore. struct with vec of tilecoords, drawn from bottom left to top right but all holds 1 id. \ Maybe not

 */
static WINDOW_TITLE: &str = "RustEngine";

fn main() {
    let mut engine = Engine::new(WINDOW_TITLE);
    engine.run(Box::new(MainMenuContext::new()));
}

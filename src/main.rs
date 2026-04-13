mod contexts;
use contexts::MainMenuContext;
use RustEngine::game_engine::Engine;

static WINDOW_TITLE: &str = "RustEngine";

fn main() {
    let mut engine = Engine::new(WINDOW_TITLE);
    engine.run(Box::new(MainMenuContext::new()));
}

use serde::{Serialize, Deserialize};



//stats, skills, characteristics
#[derive(Clone, Serialize, Deserialize)]
pub struct characteristics{
    constitution: i32,
    strength: i32,
    reflex: i32,
    will: i32,
    intelligence: i32
}
// `#[serde(default)]` (container-level) fills any field missing from a TOML
// file from the `Default` impl below. Without it, adding a new field here
// silently breaks every older units.toml / player.toml that predates it: the
// whole file fails to parse and load_units/load_player fall back to empty.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct stats {
    pub health: i64,
    pub speed: f64,
    pub defense: f64
}
#[derive(Clone, Serialize, Deserialize)]
pub struct skills{
    //much work to do maybe a scaling struct? return to this
    guns:i32,
    melee:i32,
    throwing:i32,
    traps:i32,

    stealth:i32,
}

impl stats {
    pub fn new(health: i64, speed: f64, defense: f64) -> stats {
        stats { health, speed, defense}
    }
}

impl Default for stats {
    fn default() -> Self {
        stats { health: 1, speed: 1.0, defense: 0.0 }
    }
}
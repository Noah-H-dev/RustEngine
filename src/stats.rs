use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct stats {
    pub health: i64,
    pub speed: f64,
}

impl stats {
    pub fn new(health: i64, speed: f64) -> stats {
        stats { health, speed }
    }
}

impl Default for stats {
    fn default() -> Self {
        stats { health: 1, speed: 1.0 }
    }
}
use serde::{Deserialize, Serialize};
use crate::game::entity::Unit;
use crate::game::game_engine::Tile;
use crate::game::item::stats_mod;
use crate::tools::{actions, GLObject};

#[derive(Serialize,Deserialize,Clone)]
pub enum AbilityType{
    ATTACK,
    BUFF,
    DEBUFF,
    INTERACTION
}

#[derive(Serialize,Deserialize,Clone)]
pub enum TriggerType {
    ALWAYS,
    OnAttack,
    OnAttacked,
    OnTarget,
    OnTargeted,
}
#[derive(Serialize,Deserialize,Clone)]
pub struct Effect {
    trigger: TriggerType,
    stats_mods: stats_mod,
    duration: i32,
}
pub struct Ability {
    new_state: actions,
    ability_class: AbilityType,
    target_unit: (bool, Unit),
    target_tile: (bool, Tile),
    applied_effects: Vec<Effect>,
    sprite: GLObject
}
pub struct AbilityMenu {
    size: i32,
    scale: i32,
    ability_list: Vec<Ability>
}
impl AbilityMenu{

}

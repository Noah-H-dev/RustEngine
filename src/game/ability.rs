use serde::{Deserialize, Serialize};
use crate::game::entity::Unit;
use crate::game::game_engine::Tile;
use crate::game::item::stats_mod;
use crate::tools::{actions, GLObject};

//YAY ANOTHER SCAFFOLDED BUT UNIMPLIMENTED FEATURE

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
//each ability should get: a display sprite - does it need to be a globject or should it be something different through egui
pub struct Ability {
    new_state: actions,
    ability_class: AbilityType,
    target_unit: (bool, Unit),
    target_tile: (bool, Tile),
    applied_effects: Vec<Effect>,
    sprite: GLObject
}
//this should just loop through the list of abilities and draw their sprites (bottom, centered) when in game when its draw method is called
//do not actually use the draw method when it is made, we will adress that later. when an Abilities respective button is pressed
//either any keypad number 1-9 (the abilities number is dictated by its position in ability_list, or when it is clicked
//via EGUI, it should update the player_state based on new_state. AI will have access to these but I will handle that later.
//
pub struct AbilityMenu {
    size: i32,
    scale: i32,
    ability_list: Vec<Ability>
}
impl AbilityMenu{

}
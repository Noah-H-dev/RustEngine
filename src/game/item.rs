use crate::game::ability::Effect;
use crate::game::stats::stats;
use crate::shaders::{FRAG_SHADER, VERT_SHADER};
use crate::tools::{BL_RECTANGLE, GLObject, load_textures};
use serde::{Deserialize, Serialize};
pub const ITEMS_PATH: &str = "gamedata/items.toml";
#[derive(Serialize, Deserialize, Default)]
pub struct ItemFile {
    #[serde(rename = "item")]
    #[serde(default)]
    items: Vec<ItemRecord>,
}
#[derive(Serialize, Deserialize)]
pub struct ItemRecord {
    pub name: String,
    pub position: (i32, i32),
    pub carried: bool,
    pub id: i32,
    pub class: item_type,
    pub mods: stats_mod,
    pub action_list: Actions,
}
pub fn load_items(id_path: &str) -> Vec<item> {
    let mut itemVec: Vec<item> = Vec::new();
    let textures = load_textures(id_path);
    if !std::path::Path::new(ITEMS_PATH).exists() {
        return itemVec;
    }
    let content = std::fs::read_to_string(ITEMS_PATH).unwrap_or_default();
    let itemFile = toml::from_str::<ItemFile>(&content);
    for itemRecord in itemFile.unwrap_or_default().items {
        if let Some(tex_path) = textures.get(&itemRecord.id) {
            itemVec.push(item::new(
                itemRecord.name,
                itemRecord.position,
                itemRecord.carried,
                GLObject::new(
                    BL_RECTANGLE,
                    &format!("assets/{}", tex_path),
                    VERT_SHADER,
                    FRAG_SHADER,
                ),
                itemRecord.id,
                itemRecord.class,
                itemRecord.mods,
                itemRecord.action_list,
            ));
        }
    }

    return itemVec;
}
pub fn store_items(items: &[item]) {
    let mut storage: ItemFile = Default::default();
    for item in items {
        storage.items.push(ItemRecord {
            name: item.name.clone(),
            position: item.position,
            carried: item.carried,
            id: item.sprite_id,
            class: item.class.clone(),
            mods: item.mods.clone(),
            action_list: item.action_list.clone(),
        });
    }
    let _ = std::fs::create_dir_all("gamedata");
    if let Ok(s) = toml::to_string_pretty(&storage) {
        let _ = std::fs::write(ITEMS_PATH, s);
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub enum GearType {
    #[default]
    HEAD,
    BODY,
    FEET,
    BELT,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Armor {
    defense: i32,
    stat_mods: stats_mod,
    effects: Vec<Effect>,
}
#[derive(Serialize, Deserialize, Clone)]
pub enum WeaponType {
    FIREARM { ft: FirearmType },
    MELEE { mt: MeleeType },
}
#[derive(Serialize, Deserialize, Clone)]
pub enum MeleeType {
    SWORD,
    AXE,
    HAMMER,
    SPEAR,
}
#[derive(Serialize, Deserialize, Clone)]
pub enum FirearmType {
    PISTOL,
    RIFLE,
    SHOTGUN,
    MINIGUN,
    CANNON,
}
#[derive(Serialize, Deserialize, Clone)]
pub enum ConsumeableType {
    TRAP,
    THROWABLE,
    FOOD,
    MEDICAL,
}
#[derive(Serialize, Deserialize, Clone)]
pub enum ammo_type {
    MUSKET { am: ammunition },
    NineMill { am: ammunition },
    TwelveGauge { am: ammunition },
    FiveFiveSix { am: ammunition },
}
#[derive(Serialize, Deserialize, Clone)]
pub struct ammunition {
    quantity: i32,
    damage: i32,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub enum item_type {
    WEAPON {
        wt: WeaponType,
    },
    GEAR {
        gt: GearType,
    },
    CONSUMABLE {
        ct: ConsumeableType,
    },
    AMMUNITION {
        at: ammo_type,
    },
    KEY,
    TOOL,
    VALUABLE,
    #[default]
    CHAFF,
}
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct stats_mod {
    mods: stats,
}
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Actions {
    pub unequip: bool,
}

pub struct item {
    pub name: String,
    pub position: (i32, i32),
    pub carried: bool,
    pub sprite: GLObject,
    pub sprite_id: i32,
    pub class: item_type,
    pub mods: stats_mod,
    pub action_list: Actions,
}

impl item {
    pub fn new(
        name: String,
        position: (i32, i32),
        carried: bool,
        sprite: GLObject,
        sprite_id: i32,
        class: item_type,
        mods: stats_mod,
        action_list: Actions,
    ) -> item {
        item {
            name,
            position,
            carried,
            sprite,
            sprite_id,
            class,
            mods,
            action_list,
        }
    }
    pub fn draw(&self, camera: (i32, i32)) {
        use super::game_engine::TILE_SIZE;
        if !self.carried {
            self.sprite.draw(
                self.position.0 * TILE_SIZE - camera.0,
                self.position.1 * TILE_SIZE - camera.1,
                TILE_SIZE as f32,
            );
        }
    }
}

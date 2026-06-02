use serde::{Deserialize, Serialize};
use crate::shaders::{FRAG_SHADER, VERT_SHADER};
use crate::tools::{load_textures, GLObject, BL_RECTANGLE};
pub const ITEMS_PATH: &str = "gamedata/items.toml";
#[derive(Serialize,Deserialize,Default)]
pub struct ItemFile{
    #[serde(rename = "item")]
    #[serde(default)]
    items: Vec<ItemRecord>
}
#[derive(Serialize, Deserialize)]
pub struct ItemRecord{
    pub name: String,
    pub position:(i32,i32),
    pub carried: bool,
    pub id: i32
}
pub fn load_items(id_path:&str) -> Vec<item>{
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
                GLObject::new(BL_RECTANGLE, &format!("assets/{}", tex_path), VERT_SHADER, FRAG_SHADER),
                itemRecord.id
            ));
        }
    }

    return itemVec;
}
pub fn store_items(items: &[item]){
    let mut storage: ItemFile = Default::default();
    for item in items {
        storage.items.push(ItemRecord{
            name: item.name.clone(),
            position: item.position,
            carried: item.carried,
            id: item.sprite_id
        });
    }
    let _ = std::fs::create_dir_all("gamedata");
    if let Ok(s) = toml::to_string_pretty(&storage) {
        let _ = std::fs::write(ITEMS_PATH, s);
    }
}

pub struct item{
    pub name: String,
    pub position: (i32,i32), //inherit from a field getting added to Unit
    carried: bool,
    sprite: GLObject,
    sprite_id: i32
    //add effects here, maybe call it a struct mod or something
}

impl item{
    pub fn new(name: String, position: (i32,i32), carried: bool, sprite: GLObject,sprite_id:i32) -> item {
        item{name,position,carried,sprite,sprite_id}
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
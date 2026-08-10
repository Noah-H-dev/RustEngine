use std::collections::{BTreeSet, HashMap};
use serde::{Deserialize, Serialize};

pub const TILEDEFS_PATH: &str = "gamedata/tiledefs.toml";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    #[default]
    Tiles,
    Creatures,
    Objects,
}

impl Category {
    pub const ALL: [Category; 3] = [Category::Tiles, Category::Creatures, Category::Objects];

    pub fn label(self) -> &'static str {
        match self {
            Category::Tiles     => "Tiles",
            Category::Creatures => "Creatures",
            Category::Objects   => "Objects",
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TileDef {
    pub id: i32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub solid: bool,
    #[serde(default)]
    pub category: Category,
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct TileDefFile {
    #[serde(default, rename = "texture")]
    tiles: Vec<TileDef>,
}

#[derive(Default)]
pub struct TileDefs {
    map: HashMap<i32, TileDef>,
}

impl TileDefs {
    pub fn load() -> Self {
        if !std::path::Path::new(TILEDEFS_PATH).exists() {
            return Self::default();
        }
        let content = std::fs::read_to_string(TILEDEFS_PATH).unwrap_or_default();
        let map = toml::from_str::<TileDefFile>(&content)
            .map(|f| f.tiles.into_iter().map(|t| (t.id, t)).collect())
            .unwrap_or_default();
        TileDefs { map }
    }

    pub fn save(&self) {
        let mut tiles: Vec<TileDef> = self.map.values().cloned().collect();
        tiles.sort_by_key(|t| t.id);
        let content = toml::to_string(&TileDefFile { tiles })
            .expect("Failed to serialize tile definitions");
        if let Some(parent) = std::path::Path::new(TILEDEFS_PATH).parent() {
            if !parent.as_os_str().is_empty() { let _ = std::fs::create_dir_all(parent); }
        }
        std::fs::write(TILEDEFS_PATH, content).expect("Failed to save tile definitions");
    }

    #[allow(dead_code)]
    pub fn get(&self, id: i32) -> Option<&TileDef> {
        self.map.get(&id)
    }

    pub fn solid_of(&self, id: i32) -> bool {
        self.map.get(&id).map(|t| t.solid).unwrap_or(false)
    }

    pub fn folder_of(&self, id: i32) -> Option<String> {
        self.map.get(&id).and_then(|t| t.folder.clone())
    }

    pub fn category_of(&self, id: i32) -> Category {
        self.map.get(&id).map(|t| t.category).unwrap_or_default()
    }

    pub fn name_of(&self, id: i32) -> Option<String> {
        self.map.get(&id).and_then(|t| t.name.clone())
    }

    #[allow(dead_code)]
    pub fn folders(&self) -> Vec<String> {
        let set: BTreeSet<String> =
            self.map.values().filter_map(|t| t.folder.clone()).collect();
        set.into_iter().collect()
    }

    pub fn sorted(&self) -> Vec<TileDef> {
        let mut v: Vec<TileDef> = self.map.values().cloned().collect();
        v.sort_by_key(|t| t.id);
        v
    }

    pub fn entry_mut(&mut self, id: i32) -> &mut TileDef {
        let e = self.map.entry(id).or_default();
        e.id = id;
        e
    }

    pub fn set_solid(&mut self, id: i32, solid: bool) {
        self.entry_mut(id).solid = solid;
    }

    pub fn set_folder(&mut self, id: i32, folder: Option<String>) {
        self.entry_mut(id).folder = folder;
    }

    pub fn set_category(&mut self, id: i32, category: Category) {
        let e = self.entry_mut(id);
        if e.category != category {
            e.category = category;
            e.folder = None;
        }
    }

    pub fn set_name(&mut self, id: i32, name: Option<String>) {
        self.entry_mut(id).name = name;
    }

    pub fn rename_folder(&mut self, from: &str, to: &str) -> bool {
        let mut changed = false;
        for t in self.map.values_mut() {
            if t.folder.as_deref() == Some(from) {
                t.folder = Some(to.to_string());
                changed = true;
            }
        }
        changed
    }

    pub fn clear_folder(&mut self, folder: &str) -> bool {
        let mut changed = false;
        for t in self.map.values_mut() {
            if t.folder.as_deref() == Some(folder) {
                t.folder = None;
                changed = true;
            }
        }
        changed
    }

    pub fn create(&mut self, name: Option<String>, category: Category) -> i32 {
        let id = self.map.keys().copied().max().unwrap_or(0) + 1;
        self.map.insert(id, TileDef { id, name, solid: false, category, folder: None });
        id
    }

    pub fn remove(&mut self, id: i32) -> bool {
        self.map.remove(&id).is_some()
    }
}

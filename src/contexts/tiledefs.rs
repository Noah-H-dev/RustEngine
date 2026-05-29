// ── Tile definitions ─────────────────────────────────────────────────────────
//
// A *tile definition* is the abstract, tileset-independent identity of a tile:
// its id, an optional human name, and properties shared across every tileset
// (collision, folder, and — later — color shift, etc.). The PNG shown for a tile
// is NOT here: that is per-tileset (the id→png "skin", today `id.txt`). Keeping
// path out of this struct is deliberate — it lets one map be re-skinned by
// swapping tilesets while the definitions stay constant.
//
// Source of truth on disk is `textures.toml` (name kept for now; a rename to
// something like `tiledefs.toml` is planned). The `[[texture]]` table key is
// preserved for backward compatibility with files the map editor already wrote.
//
// ADDING A NEW PROPERTY: add a `#[serde(default)]` field to `TileDef` so old
// files keep loading. Render-time properties (e.g. color shift) will also need
// the game engine to load these.

use std::collections::{BTreeSet, HashMap};
use serde::{Deserialize, Serialize};

/// On-disk path for tile definitions. Shared by the map editor and the tileset
/// editor; both must use this same model so neither drops the other's fields.
pub const TILEDEFS_PATH: &str = "gamedata/textures.toml";

/// One abstract tile definition. Properties here are shared across all tilesets.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TileDef {
    pub id: i32,
    /// Human-readable name (e.g. "stone wall"). None = unnamed.
    #[serde(default)]
    pub name: Option<String>,
    /// Default collision stamped onto tiles painted with this id.
    #[serde(default)]
    pub solid: bool,
    /// Organizational folder this tile lives in. None = loose. Folders are
    /// implicit: one exists exactly while ≥1 tile references it.
    #[serde(default)]
    pub folder: Option<String>,
}

/// On-disk wrapper. `texture` table key kept for back-compat with existing files.
#[derive(Default, Serialize, Deserialize)]
struct TileDefFile {
    #[serde(default, rename = "texture")]
    tiles: Vec<TileDef>,
}

/// In-memory tile definitions keyed by id. The single source of truth for which
/// abstract tile ids exist and their shared properties.
#[derive(Default)]
pub struct TileDefs {
    map: HashMap<i32, TileDef>,
}

impl TileDefs {
    /// Load from `textures.toml`; empty (not an error) if the file is absent.
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

    /// Persist to `textures.toml`, sorted by id for stable diffs.
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

    // Part of the shared API surface; not all consumers exist yet.
    #[allow(dead_code)]
    pub fn get(&self, id: i32) -> Option<&TileDef> {
        self.map.get(&id)
    }

    /// Default collision for `id` (false if no definition exists).
    pub fn solid_of(&self, id: i32) -> bool {
        self.map.get(&id).map(|t| t.solid).unwrap_or(false)
    }

    /// Folder `id` belongs to (None if loose or undefined).
    pub fn folder_of(&self, id: i32) -> Option<String> {
        self.map.get(&id).and_then(|t| t.folder.clone())
    }

    /// Human name for `id`, if any.
    pub fn name_of(&self, id: i32) -> Option<String> {
        self.map.get(&id).and_then(|t| t.name.clone())
    }

    /// All distinct folder names currently in use, sorted.
    #[allow(dead_code)]
    pub fn folders(&self) -> Vec<String> {
        let set: BTreeSet<String> =
            self.map.values().filter_map(|t| t.folder.clone()).collect();
        set.into_iter().collect()
    }

    /// Definitions sorted by id (for stable list rendering).
    pub fn sorted(&self) -> Vec<TileDef> {
        let mut v: Vec<TileDef> = self.map.values().cloned().collect();
        v.sort_by_key(|t| t.id);
        v
    }

    /// Mutable access to a definition, creating a default entry (with id set) if
    /// none exists. Use for in-place property edits.
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

    pub fn set_name(&mut self, id: i32, name: Option<String>) {
        self.entry_mut(id).name = name;
    }

    /// Rename a folder by rewriting every member tile's `folder`. Returns whether
    /// anything changed.
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

    /// Dissolve a folder: every member tile becomes loose. Returns whether
    /// anything changed.
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

    /// Allocate the next abstract id (max + 1, ids start at 1) and insert a new
    /// definition with the given name. Returns the new id. This is the central
    /// id-allocation point the tileset editor owns.
    pub fn create(&mut self, name: Option<String>) -> i32 {
        let id = self.map.keys().copied().max().unwrap_or(0) + 1;
        self.map.insert(id, TileDef { id, name, solid: false, folder: None });
        id
    }

    /// Remove a definition. Returns whether it existed.
    pub fn remove(&mut self, id: i32) -> bool {
        self.map.remove(&id).is_some()
    }
}

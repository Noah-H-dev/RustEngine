// ── Tileset (skin) ───────────────────────────────────────────────────────────
//
// A *tileset* is the per-skin mapping from abstract tile id to a PNG filename
// in `assets/`. Tile properties (collision, folder, name) live elsewhere in
// `tiledefs.toml` and are shared across all tilesets — see `tiledefs.rs` and
// the project-tileset-editor-architecture memory.
//
// On-disk format is the same plain `id filename` text the legacy `id.txt` /
// game engine's `load_textures` already use, so existing tilesets and the game
// loader keep working unchanged. We just add a directory convention: new
// tilesets live in `tilesets/` so the editor can scan and list them.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Directory tilesets are discovered in. Created on first save if absent.
pub const TILESETS_DIR: &str = "tilesets";

/// Filename (within `TILESETS_DIR`) of the always-present default tileset.
pub const DEFAULT_TILESET_FILE: &str = "Default.txt";

/// In-memory tileset: a file path plus id → filename mapping (filename is
/// relative to `assets/`, matching the on-disk format).
#[derive(Clone)]
pub struct Tileset {
    pub path: PathBuf,
    map: HashMap<i32, String>,
}

impl Tileset {
    /// Load from a tileset file (plain text, "id filename" per line, '#' comments).
    /// Returns a tileset bound to `path` with an empty map if the file is absent.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut map = HashMap::new();
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() || parts[0].starts_with('#') { continue; }
                if parts.len() >= 2 {
                    if let Ok(id) = parts[0].parse::<i32>() {
                        map.insert(id, parts[1].to_string());
                    }
                }
            }
        }
        Tileset { path, map }
    }

    /// Persist to `self.path`. Creates parent directories as needed. Output is
    /// sorted by id for stable diffs and matches the format `load_textures`
    /// (game engine) and the map editor's `load_palette` already read.
    pub fn save(&self) {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }
        let mut entries: Vec<(&i32, &String)> = self.map.iter().collect();
        entries.sort_by_key(|(id, _)| *id);
        let content = entries.iter()
            .map(|(id, p)| format!("{} {}", id, p))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&self.path, content).expect("Failed to save tileset");
    }

    /// PNG filename assigned to `id` in this tileset, if any.
    pub fn png_of(&self, id: i32) -> Option<&str> {
        self.map.get(&id).map(String::as_str)
    }

    /// Assign (Some) or clear (None) the PNG for `id`. Caller persists via `save`.
    pub fn set_png(&mut self, id: i32, png: Option<String>) {
        match png {
            Some(p) => { self.map.insert(id, p); }
            None    => { self.map.remove(&id); }
        }
    }

    /// Display name (file stem) of this tileset, e.g. `forest` for `tilesets/forest.txt`.
    pub fn name(&self) -> String {
        self.path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

    /// Scan `tilesets/` for `.txt` files. Returns paths sorted by name. Missing
    /// directory yields an empty list (not an error).
    pub fn list_in_dir() -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = fs::read_dir(TILESETS_DIR)
            .map(|dir| {
                dir.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("txt"))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }

    /// Build a new tileset bound to `tilesets/<name>.txt`. If `copy_from` is
    /// Some, the new tileset starts as a duplicate of that one (per the
    /// "copy from currently-selected" creation behavior); otherwise empty.
    /// The file is NOT written until `save()` is called.
    pub fn create(name: &str, copy_from: Option<&Tileset>) -> Self {
        let path = Path::new(TILESETS_DIR).join(format!("{}.txt", name));
        let map = copy_from.map(|t| t.map.clone()).unwrap_or_default();
        Tileset { path, map }
    }
}

/// Ensure `tilesets/Default.txt` exists, creating it if absent. On a fresh
/// install with a legacy `id.txt` at the project root, migrate that file's
/// contents into the new Default tileset so existing maps keep their skin.
/// Returns the path of the Default tileset.
///
/// Idempotent — safe to call on every startup.
pub fn ensure_default() -> PathBuf {
    let dir  = Path::new(TILESETS_DIR);
    let path = dir.join(DEFAULT_TILESET_FILE);
    if path.exists() { return path; }
    let _ = fs::create_dir_all(dir);
    // Migrate from a legacy root-level id.txt if one is sitting there, so
    // upgrades don't silently lose a user's existing tileset.
    let legacy = Path::new("id.txt");
    if legacy.exists() {
        let _ = fs::copy(legacy, &path);
    } else {
        let _ = fs::write(&path, "");
    }
    path
}

/// All PNG filenames currently in `assets/`, sorted. Used by the tileset editor
/// to populate the per-tile "Assign PNG" submenu. Filenames are relative to
/// `assets/`, matching what gets stored in a tileset entry.
pub fn assets_pngs() -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir("assets")
        .map(|dir| {
            dir.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name.to_lowercase().ends_with(".png") { Some(name) } else { None }
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

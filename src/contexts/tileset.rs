use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const TILESETS_DIR: &str = "tilesets";

pub const DEFAULT_TILESET_FILE: &str = "Default.txt";

#[derive(Clone)]
pub struct Tileset {
    pub path: PathBuf,
    map: HashMap<i32, String>,
}

impl Tileset {
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

    pub fn png_of(&self, id: i32) -> Option<&str> {
        self.map.get(&id).map(String::as_str)
    }

    pub fn set_png(&mut self, id: i32, png: Option<String>) {
        match png {
            Some(p) => { self.map.insert(id, p); }
            None    => { self.map.remove(&id); }
        }
    }

    pub fn name(&self) -> String {
        self.path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

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

    pub fn create(name: &str, copy_from: Option<&Tileset>) -> Self {
        let path = Path::new(TILESETS_DIR).join(format!("{}.txt", name));
        let map = copy_from.map(|t| t.map.clone()).unwrap_or_default();
        Tileset { path, map }
    }
}

pub fn ensure_default() -> PathBuf {
    let dir  = Path::new(TILESETS_DIR);
    let path = dir.join(DEFAULT_TILESET_FILE);
    if path.exists() { return path; }
    let _ = fs::create_dir_all(dir);
    let legacy = Path::new("id.txt");
    if legacy.exists() {
        let _ = fs::copy(legacy, &path);
    } else {
        let _ = fs::write(&path, "");
    }
    path
}

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

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{MempalaceError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSpec {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub wing: String,
    #[serde(default)]
    pub rooms: Vec<RoomSpec>,
}

pub fn load_project_config(project_dir: &Path) -> Result<ProjectConfig> {
    let primary = project_dir.join("mempalace.yaml");
    let legacy = project_dir.join("mempal.yaml");
    let path = if primary.exists() {
        primary
    } else if legacy.exists() {
        legacy
    } else {
        return Err(MempalaceError::message(format!(
            "No mempalace.yaml found in {}",
            project_dir.display()
        )));
    };

    let mut config: ProjectConfig = serde_yaml::from_str(&fs::read_to_string(path)?)?;
    if config.rooms.is_empty() {
        config.rooms.push(RoomSpec {
            name: "general".to_string(),
            description: "All project files".to_string(),
            keywords: Vec::new(),
        });
    }
    Ok(config)
}

pub fn detect_room(filepath: &Path, content: &str, rooms: &[RoomSpec], project_path: &Path) -> String {
    let relative = filepath
        .strip_prefix(project_path)
        .unwrap_or(filepath)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let filename = filepath
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let content_lower = content.chars().take(2_000).collect::<String>().to_ascii_lowercase();

    let path_parts = relative.split('/').collect::<Vec<_>>();
    for part in path_parts.iter().take(path_parts.len().saturating_sub(1)) {
        for room in rooms {
            let room_name = room.name.to_ascii_lowercase();
            if room_name.contains(part) || part.contains(&room_name) {
                return room.name.clone();
            }
        }
    }

    for room in rooms {
        let room_name = room.name.to_ascii_lowercase();
        if room_name.contains(&filename) || filename.contains(&room_name) {
            return room.name.clone();
        }
    }

    let mut best_score = 0;
    let mut best_room = "general".to_string();
    for room in rooms {
        let mut score = 0;
        for keyword in room
            .keywords
            .iter()
            .chain(std::iter::once(&room.name))
            .map(|item| item.to_ascii_lowercase())
        {
            score += content_lower.matches(&keyword).count();
        }
        if score > best_score {
            best_score = score;
            best_room = room.name.clone();
        }
    }

    best_room
}

pub fn detect_rooms_from_folders(project_dir: &Path) -> Result<Vec<RoomSpec>> {
    let folder_room_map = folder_room_map();
    let mut found = HashMap::<String, String>::new();
    for entry in fs::read_dir(project_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_skip_dir(&name) {
            continue;
        }
        let normalized = name.to_ascii_lowercase().replace('-', "_");
        let room_name = folder_room_map
            .get(normalized.as_str())
            .map(|value| (*value).to_string())
            .unwrap_or_else(|| normalized.clone());
        found.entry(room_name).or_insert(name);
    }

    let mut rooms = found
        .into_iter()
        .map(|(room_name, original)| RoomSpec {
            name: room_name.clone(),
            description: format!("Files from {original}/"),
            keywords: vec![room_name, original.to_ascii_lowercase()],
        })
        .collect::<Vec<_>>();

    rooms.sort_by(|left, right| left.name.cmp(&right.name));
    if !rooms.iter().any(|room| room.name == "general") {
        rooms.push(RoomSpec {
            name: "general".to_string(),
            description: "Files that don't fit other rooms".to_string(),
            keywords: Vec::new(),
        });
    }
    Ok(rooms)
}

pub fn detect_rooms_from_files(project_dir: &Path) -> Result<Vec<RoomSpec>> {
    let folder_room_map = folder_room_map();
    let mut counts = HashMap::<String, usize>::new();
    for entry in WalkDir::new(project_dir) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .path()
            .components()
            .any(|component| is_skip_dir(&component.as_os_str().to_string_lossy()))
        {
            continue;
        }
        let name = entry
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .replace('-', "_");
        for (keyword, room) in &folder_room_map {
            if name.contains(keyword) {
                *counts.entry((*room).to_string()).or_default() += 1;
            }
        }
    }

    let mut rooms = counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|(room, _)| RoomSpec {
            name: room.clone(),
            description: format!("Files related to {room}"),
            keywords: vec![room],
        })
        .collect::<Vec<_>>();
    rooms.sort_by(|left, right| left.name.cmp(&right.name));
    rooms.truncate(6);

    if rooms.is_empty() {
        rooms.push(RoomSpec {
            name: "general".to_string(),
            description: "All project files".to_string(),
            keywords: Vec::new(),
        });
    }
    Ok(rooms)
}

pub fn init_project(project_dir: &Path) -> Result<PathBuf> {
    let project_name = project_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project")
        .to_ascii_lowercase()
        .replace([' ', '-'], "_");
    let mut rooms = detect_rooms_from_folders(project_dir)?;
    if rooms.len() <= 1 {
        rooms = detect_rooms_from_files(project_dir)?;
    }
    save_project_config(project_dir, &project_name, &rooms)
}

pub fn save_project_config(project_dir: &Path, wing: &str, rooms: &[RoomSpec]) -> Result<PathBuf> {
    let config = ProjectConfig {
        wing: wing.to_string(),
        rooms: rooms.to_vec(),
    };
    let path = project_dir.join("mempalace.yaml");
    fs::write(&path, serde_yaml::to_string(&config)?)?;
    Ok(path)
}

fn folder_room_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("frontend", "frontend"),
        ("front_end", "frontend"),
        ("client", "frontend"),
        ("backend", "backend"),
        ("server", "backend"),
        ("api", "backend"),
        ("docs", "documentation"),
        ("documentation", "documentation"),
        ("notes", "documentation"),
        ("design", "design"),
        ("research", "research"),
        ("planning", "planning"),
        ("tests", "testing"),
        ("scripts", "scripts"),
        ("config", "configuration"),
        ("infra", "configuration"),
        ("deploy", "configuration"),
    ])
}

fn is_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "__pycache__" | ".venv" | "venv" | "env" | "dist" | "build" | ".next" | "coverage" | ".mempalace"
    )
}

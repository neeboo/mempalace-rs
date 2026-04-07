use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::room_detector::{detect_room, load_project_config};
use crate::storage::{NewDrawer, PalaceStore};
use crate::{Result, room_detector::ProjectConfig};

const CHUNK_SIZE: usize = 800;
const CHUNK_OVERLAP: usize = 100;
const MIN_CHUNK_SIZE: usize = 50;

#[derive(Debug, Clone, Default)]
pub struct MineSummary {
    pub files_processed: usize,
    pub files_skipped: usize,
    pub drawers_filed: usize,
    pub room_counts: BTreeMap<String, usize>,
}

pub fn mine_project(
    project_dir: &Path,
    palace_path: &Path,
    wing_override: Option<&str>,
    agent: &str,
    limit: usize,
    dry_run: bool,
) -> Result<MineSummary> {
    let project_path = project_dir.canonicalize()?;
    let config = load_project_config(&project_path)?;
    let wing = wing_override.unwrap_or(&config.wing).to_string();
    let files = scan_project(&project_path, limit)?;

    let store = if dry_run {
        None
    } else {
        Some(PalaceStore::open(palace_path)?)
    };

    let mut summary = MineSummary::default();
    for filepath in files {
        let processed = process_file(
            &filepath,
            &project_path,
            &config,
            &wing,
            agent,
            store.as_ref(),
            dry_run,
        )?;
        summary.drawers_filed += processed.drawers_filed;
        summary.files_processed += usize::from(processed.drawers_filed > 0 || dry_run);
        summary.files_skipped += processed.files_skipped;
        if let Some(room) = processed.room {
            *summary.room_counts.entry(room).or_default() += 1;
        }
    }
    Ok(summary)
}

pub fn scan_project(project_dir: &Path, limit: usize) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
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
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !readable_extensions().contains(&ext.to_ascii_lowercase().as_str()) {
            continue;
        }
        let filename = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
        if matches!(
            filename,
            "mempalace.yaml" | "mempalace.yml" | "mempal.yaml" | "mempal.yml" | ".gitignore" | "package-lock.json"
        ) {
            continue;
        }
        files.push(path.to_path_buf());
        if limit > 0 && files.len() >= limit {
            break;
        }
    }
    Ok(files)
}

pub fn chunk_text(content: &str) -> Vec<(usize, String)> {
    let content = content.trim();
    if content.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    let bytes = content.as_bytes();
    let len = content.len();
    while start < len {
        let mut end = (start + CHUNK_SIZE).min(len);
        if end < len {
            let window = &content[start..end];
            if let Some(pos) = window.rfind("\n\n") {
                if pos > CHUNK_SIZE / 2 {
                    end = start + pos;
                }
            } else if let Some(pos) = window.rfind('\n') {
                if pos > CHUNK_SIZE / 2 {
                    end = start + pos;
                }
            }
        }

        while !content.is_char_boundary(end) && end > start {
            end -= 1;
        }
        let chunk = content[start..end].trim().to_string();
        if chunk.len() >= MIN_CHUNK_SIZE {
            chunks.push((chunks.len(), chunk));
        }
        if end >= len {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP);
        while start < len && !bytes.is_empty() && !content.is_char_boundary(start) {
            start += 1;
        }
    }
    chunks
}

struct FileProcessSummary {
    drawers_filed: usize,
    files_skipped: usize,
    room: Option<String>,
}

fn process_file(
    filepath: &Path,
    project_path: &Path,
    config: &ProjectConfig,
    wing: &str,
    agent: &str,
    store: Option<&PalaceStore>,
    dry_run: bool,
) -> Result<FileProcessSummary> {
    let source_file = filepath.to_string_lossy().to_string();
    if let Some(store) = store {
        if store.source_file_exists(&source_file)? {
            return Ok(FileProcessSummary {
                drawers_filed: 0,
                files_skipped: 1,
                room: None,
            });
        }
    }

    let content = std::fs::read_to_string(filepath).unwrap_or_default();
    if content.trim().len() < MIN_CHUNK_SIZE {
        return Ok(FileProcessSummary {
            drawers_filed: 0,
            files_skipped: 0,
            room: None,
        });
    }

    let room = detect_room(filepath, &content, &config.rooms, project_path);
    let chunks = chunk_text(&content);
    if dry_run {
        return Ok(FileProcessSummary {
            drawers_filed: chunks.len(),
            files_skipped: 0,
            room: Some(room),
        });
    }

    let Some(store) = store else {
        return Ok(FileProcessSummary {
            drawers_filed: 0,
            files_skipped: 0,
            room: Some(room),
        });
    };

    let mut drawers_filed = 0;
    for (chunk_index, chunk) in chunks {
        let drawer = NewDrawer {
            id: drawer_id(wing, &room, &source_file, chunk_index),
            wing: wing.to_string(),
            room: room.clone(),
            source_file: source_file.clone(),
            chunk_index,
            added_by: agent.to_string(),
            filed_at: filed_at(),
            content: chunk,
            ingest_mode: Some("projects".to_string()),
            extract_mode: None,
            hall: None,
            topic: None,
            drawer_type: None,
            date: None,
        };
        if store.insert_drawer(&drawer)? {
            drawers_filed += 1;
        }
    }

    Ok(FileProcessSummary {
        drawers_filed,
        files_skipped: 0,
        room: Some(room),
    })
}

pub(crate) fn drawer_id(wing: &str, room: &str, source_file: &str, chunk_index: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_file.as_bytes());
    hasher.update(chunk_index.to_string().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    format!("drawer_{wing}_{room}_{}", &hash[..16])
}

pub(crate) fn filed_at() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn readable_extensions() -> &'static [&'static str] {
    &[
        "txt", "md", "py", "js", "ts", "jsx", "tsx", "json", "yaml", "yml", "html", "css",
        "java", "go", "rs", "rb", "sh", "csv", "sql", "toml",
    ]
}

fn is_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "__pycache__" | ".venv" | "venv" | "env" | "dist" | "build" | ".next" | "coverage" | ".mempalace"
    )
}

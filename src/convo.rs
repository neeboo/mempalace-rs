use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use walkdir::WalkDir;

use crate::general_extractor::extract_memories;
use crate::miner::{MineSummary, drawer_id, filed_at};
use crate::normalize::normalize_file;
use crate::storage::{NewDrawer, PalaceStore};
use crate::Result;

const MIN_CHUNK_SIZE: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExtractMode {
    Exchange,
    General,
}

pub fn mine_conversations(
    convo_dir: &Path,
    palace_path: &Path,
    wing: Option<&str>,
    agent: &str,
    limit: usize,
    dry_run: bool,
) -> Result<MineSummary> {
    mine_conversations_with_extract_mode(
        convo_dir,
        palace_path,
        wing,
        agent,
        limit,
        dry_run,
        ExtractMode::Exchange,
    )
}

pub fn mine_conversations_with_extract_mode(
    convo_dir: &Path,
    palace_path: &Path,
    wing: Option<&str>,
    agent: &str,
    limit: usize,
    dry_run: bool,
    extract_mode: ExtractMode,
) -> Result<MineSummary> {
    let convo_path = convo_dir.canonicalize()?;
    let wing = wing
        .map(str::to_string)
        .unwrap_or_else(|| normalized_name(&convo_path));
    let files = scan_conversations(&convo_path, limit)?;
    let store = if dry_run {
        None
    } else {
        Some(PalaceStore::open(palace_path)?)
    };

    let mut summary = MineSummary {
        room_counts: BTreeMap::new(),
        ..MineSummary::default()
    };

    for filepath in files {
        let source_file = filepath.to_string_lossy().to_string();
        if let Some(store) = store.as_ref() {
            if store.source_file_exists(&source_file)? {
                summary.files_skipped += 1;
                continue;
            }
        }

        let content = normalize_file(&filepath)?;
        if content.trim().len() < MIN_CHUNK_SIZE {
            continue;
        }
        if !looks_like_conversation(&filepath, &content) {
            continue;
        }
        let chunks = match extract_mode {
            ExtractMode::Exchange => chunk_exchanges(&content)
                .into_iter()
                .map(|(chunk_index, content)| ConvoChunk {
                    content,
                    chunk_index,
                    memory_type: None,
                })
                .collect::<Vec<_>>(),
            ExtractMode::General => extract_memories(&content, 0.3)
                .into_iter()
                .map(|memory| ConvoChunk {
                    content: memory.content,
                    chunk_index: memory.chunk_index,
                    memory_type: Some(memory.memory_type),
                })
                .collect::<Vec<_>>(),
        };
        if chunks.is_empty() {
            continue;
        }
        summary.files_processed += 1;

        if dry_run {
            summary.drawers_filed += chunks.len();
            for chunk in &chunks {
                let room = chunk
                    .memory_type
                    .clone()
                    .unwrap_or_else(|| detect_convo_room(&content));
                *summary.room_counts.entry(room).or_default() += 1;
            }
            continue;
        }

        let Some(store) = store.as_ref() else {
            continue;
        };

        let fallback_room = detect_convo_room(&content);
        for chunk in chunks {
            let room = chunk.memory_type.clone().unwrap_or_else(|| fallback_room.clone());
            let hall = general_hall(chunk.memory_type.as_deref());
            let drawer = NewDrawer {
                id: drawer_id(&wing, &room, &source_file, chunk.chunk_index),
                wing: wing.clone(),
                room: room.clone(),
                source_file: source_file.clone(),
                chunk_index: chunk.chunk_index,
                added_by: agent.to_string(),
                filed_at: filed_at(),
                content: chunk.content,
                ingest_mode: Some("convos".to_string()),
                extract_mode: Some(
                    match extract_mode {
                        ExtractMode::Exchange => "exchange",
                        ExtractMode::General => "general",
                    }
                    .to_string(),
                ),
                hall,
                topic: Some(room.clone()),
                drawer_type: chunk.memory_type.clone(),
                date: None,
            };
            if store.insert_drawer(&drawer)? {
                summary.drawers_filed += 1;
                *summary.room_counts.entry(room).or_default() += 1;
            }
        }
    }

    Ok(summary)
}

fn scan_conversations(convo_dir: &Path, limit: usize) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(convo_dir) {
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
        let Some(ext) = entry.path().extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !matches!(ext.to_ascii_lowercase().as_str(), "txt" | "md" | "json" | "jsonl") {
            continue;
        }
        files.push(entry.path().to_path_buf());
        if limit > 0 && files.len() >= limit {
            break;
        }
    }
    Ok(files)
}

#[derive(Debug, Clone)]
struct ConvoChunk {
    content: String,
    chunk_index: usize,
    memory_type: Option<String>,
}

fn chunk_exchanges(content: &str) -> Vec<(usize, String)> {
    let lines = content.lines().collect::<Vec<_>>();
    let quote_lines = lines
        .iter()
        .filter(|line| line.trim_start().starts_with('>'))
        .count();
    if quote_lines >= 3 {
        chunk_by_exchange(&lines)
    } else {
        chunk_by_paragraph(content)
    }
}

fn general_hall(memory_type: Option<&str>) -> Option<String> {
    let hall = match memory_type {
        Some("decision") => "hall_facts",
        Some("preference") => "hall_preferences",
        Some("milestone") => "hall_discoveries",
        Some("problem") => "hall_advice",
        Some("emotional") => "hall_events",
        _ => return None,
    };
    Some(hall.to_string())
}

fn chunk_by_exchange(lines: &[&str]) -> Vec<(usize, String)> {
    let mut chunks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if !line.starts_with('>') {
            index += 1;
            continue;
        }
        let user_turn = line.to_string();
        index += 1;

        let mut ai_lines = Vec::new();
        while index < lines.len() {
            let next = lines[index].trim();
            if next.starts_with('>') || next.starts_with("---") {
                break;
            }
            if !next.is_empty() {
                ai_lines.push(next.to_string());
            }
            index += 1;
        }

        let ai_response = ai_lines.into_iter().take(8).collect::<Vec<_>>().join(" ");
        let chunk = if ai_response.is_empty() {
            user_turn
        } else {
            format!("{user_turn}\n{ai_response}")
        };
        if chunk.trim().len() > MIN_CHUNK_SIZE {
            chunks.push((chunks.len(), chunk));
        }
    }
    chunks
}

fn chunk_by_paragraph(content: &str) -> Vec<(usize, String)> {
    let paragraphs = content
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .collect::<Vec<_>>();
    if paragraphs.len() <= 1 && content.lines().count() > 20 {
        return content
            .lines()
            .collect::<Vec<_>>()
            .chunks(25)
            .filter_map(|group| {
                let chunk = group.join("\n");
                (chunk.trim().len() > MIN_CHUNK_SIZE).then(|| chunk)
            })
            .enumerate()
            .collect();
    }

    paragraphs
        .into_iter()
        .filter(|paragraph| paragraph.len() > MIN_CHUNK_SIZE)
        .map(str::to_string)
        .enumerate()
        .collect()
}

fn detect_convo_room(content: &str) -> String {
    let topic_keywords: [(&str, &[&str]); 5] = [
        (
            "technical",
            &[
                "code", "python", "function", "bug", "error", "api", "database", "server",
                "deploy", "git", "test", "debug", "refactor",
            ],
        ),
        (
            "architecture",
            &[
                "architecture", "design", "pattern", "structure", "schema", "interface",
                "module", "component", "service", "layer",
            ],
        ),
        (
            "planning",
            &[
                "plan", "roadmap", "milestone", "deadline", "priority", "sprint", "backlog",
                "scope", "requirement", "spec",
            ],
        ),
        (
            "decisions",
            &[
                "decided", "chose", "picked", "switched", "migrated", "replaced", "trade-off",
                "alternative", "option", "approach",
            ],
        ),
        (
            "problems",
            &[
                "problem", "issue", "broken", "failed", "crash", "stuck", "workaround", "fix",
                "solved", "resolved",
            ],
        ),
    ];
    let haystack = content.chars().take(3_000).collect::<String>().to_ascii_lowercase();
    topic_keywords
        .iter()
        .map(|(room, keywords)| {
            let score = keywords.iter().filter(|keyword| haystack.contains(**keyword)).count();
            (*room, score)
        })
        .max_by_key(|(_, score)| *score)
        .filter(|(_, score)| *score > 0)
        .map(|(room, _)| room.to_string())
        .unwrap_or_else(|| "general".to_string())
}

fn looks_like_conversation(path: &Path, content: &str) -> bool {
    let ext = path.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase();
    if matches!(ext.as_str(), "json" | "jsonl") {
        return true;
    }
    if content.lines().any(|line| line.trim_start().starts_with('>')) {
        return true;
    }
    if content.contains("Claude Code v") {
        return true;
    }
    content.split("\n\n").filter(|paragraph| !paragraph.trim().is_empty()).count() >= 2
}

fn normalized_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("general")
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
}

fn is_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "__pycache__" | ".venv" | "venv" | "env" | "dist" | "build" | ".next" | ".mempalace"
    )
}

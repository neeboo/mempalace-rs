use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::Result;

const PERSON_VERB_PATTERNS: &[&str] = &[
    r"\b{name}\s+said\b",
    r"\b{name}\s+asked\b",
    r"\b{name}\s+told\b",
    r"\b{name}\s+replied\b",
    r"\b{name}\s+felt\b",
    r"\b{name}\s+thinks?\b",
    r"\b{name}\s+wants?\b",
    r"\b{name}\s+decided\b",
    r"\b{name}\s+pushed\b",
    r"\b{name}\s+wrote\b",
    r"\bhey\s+{name}\b",
    r"\bthanks?\s+{name}\b",
    r"\bhi\s+{name}\b",
];

const PROJECT_VERB_PATTERNS: &[&str] = &[
    r"\bbuilding\s+{name}\b",
    r"\bbuilt\s+{name}\b",
    r"\bship(?:ping|ped)?\s+{name}\b",
    r"\blaunch(?:ing|ed)?\s+{name}\b",
    r"\bdeploy(?:ing|ed)?\s+{name}\b",
    r"\bthe\s+{name}\s+architecture\b",
    r"\bthe\s+{name}\s+pipeline\b",
    r"\bthe\s+{name}\s+system\b",
    r"\bthe\s+{name}\s+repo\b",
    r"\b{name}\s+v\d+\b",
    r"\b{name}\.py\b",
    r"\b{name}-core\b",
    r"\bimport\s+{name}\b",
];

const DIALOGUE_PATTERNS: &[&str] = &[
    r"^>\s*{name}[:\s]",
    r"^{name}:\s",
    r#"\"{name}\s+said"#,
];

const PRONOUN_PATTERNS: &[&str] = &[
    r"\bshe\b",
    r"\bher\b",
    r"\bhe\b",
    r"\bhim\b",
    r"\bhis\b",
    r"\bthey\b",
    r"\bthem\b",
    r"\btheir\b",
];

const PROSE_EXTENSIONS: &[&str] = &["txt", "md", "rst", "csv"];
const READABLE_EXTENSIONS: &[&str] = &[
    "txt", "md", "rst", "csv", "json", "jsonl", "yaml", "yml", "py", "rs", "js", "ts",
];
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "__pycache__",
    ".venv",
    "venv",
    "env",
    "dist",
    "build",
    ".next",
    "coverage",
    ".mempalace",
    "target",
];

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "as", "is", "was", "are", "were", "be", "been", "being", "have", "has", "had", "do",
    "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall", "can",
    "this", "that", "these", "those", "it", "its", "they", "them", "their", "we", "our", "you",
    "your", "i", "my", "me", "he", "she", "his", "her", "who", "what", "when", "where", "why",
    "how", "which", "if", "then", "so", "not", "no", "yes", "ok", "okay", "just", "very",
    "really", "also", "already", "still", "even", "only", "here", "there", "now", "too", "up",
    "out", "about", "like", "use", "get", "got", "make", "made", "take", "put", "come", "go",
    "see", "know", "think", "true", "false", "none", "null", "new", "old", "all", "any", "some",
    "return", "print", "def", "class", "import", "step", "usage", "run", "check", "find", "add",
    "set", "list", "args", "dict", "str", "int", "bool", "path", "file", "type", "name", "note",
    "example", "option", "result", "error", "warning", "info", "first", "second", "stack", "layer",
    "mode", "test", "stop", "start", "copy", "move", "source", "target", "output", "input", "data",
    "item", "key",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectedEntity {
    pub name: String,
    pub entity_type: String,
    pub confidence: f64,
    pub frequency: usize,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DetectionReport {
    pub people: Vec<DetectedEntity>,
    pub projects: Vec<DetectedEntity>,
    pub uncertain: Vec<DetectedEntity>,
}

pub fn scan_for_detection(project_dir: &Path, max_files: usize) -> Result<Vec<PathBuf>> {
    let mut prose_files = Vec::new();
    let mut all_files = Vec::new();

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

        let Some(ext) = entry.path().extension().and_then(|value| value.to_str()) else {
            continue;
        };
        let ext = ext.to_ascii_lowercase();
        if PROSE_EXTENSIONS.contains(&ext.as_str()) {
            prose_files.push(entry.path().to_path_buf());
        } else if READABLE_EXTENSIONS.contains(&ext.as_str()) {
            all_files.push(entry.path().to_path_buf());
        }
    }

    let mut files = if prose_files.len() >= 3 {
        prose_files
    } else {
        prose_files.into_iter().chain(all_files).collect::<Vec<_>>()
    };
    files.truncate(max_files);
    Ok(files)
}

pub fn detect_entities(file_paths: &[PathBuf], max_files: usize) -> Result<DetectionReport> {
    let mut combined = String::new();
    let mut all_lines = Vec::new();

    for path in file_paths.iter().take(max_files) {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        let snippet = content.chars().take(5_000).collect::<String>();
        combined.push_str(&snippet);
        combined.push('\n');
        all_lines.extend(snippet.lines().map(str::to_string));
    }

    let candidates = extract_candidates(&combined);
    let mut report = DetectionReport::default();
    for (name, frequency) in candidates {
        let scored = score_entity(&name, &combined, &all_lines);
        let entity = classify_entity(&name, frequency, &scored);
        match entity.entity_type.as_str() {
            "person" => report.people.push(entity),
            "project" => report.projects.push(entity),
            _ => report.uncertain.push(entity),
        }
    }

    report
        .people
        .sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
    report
        .projects
        .sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
    report
        .uncertain
        .sort_by(|left, right| right.frequency.cmp(&left.frequency));
    report.people.truncate(15);
    report.projects.truncate(10);
    report.uncertain.truncate(8);
    Ok(report)
}

fn extract_candidates(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    let single = Regex::new(r"\b([A-Z][A-Za-z0-9]{1,29})\b").unwrap();
    for cap in single.captures_iter(text) {
        let word = cap.get(1).unwrap().as_str();
        if !STOPWORDS.contains(&word.to_ascii_lowercase().as_str()) {
            *counts.entry(word.to_string()).or_default() += 1;
        }
    }

    let multi = Regex::new(r"\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+)+)\b").unwrap();
    for cap in multi.captures_iter(text) {
        let phrase = cap.get(1).unwrap().as_str();
        if phrase
            .split_whitespace()
            .all(|part| !STOPWORDS.contains(&part.to_ascii_lowercase().as_str()))
        {
            *counts.entry(phrase.to_string()).or_default() += 1;
        }
    }

    counts.into_iter().filter(|(_, count)| *count >= 3).collect()
}

struct EntityScores {
    person_score: usize,
    project_score: usize,
    person_signals: Vec<String>,
    project_signals: Vec<String>,
}

fn score_entity(name: &str, text: &str, lines: &[String]) -> EntityScores {
    let name_regex = regex::escape(name);
    let mut person_score = 0usize;
    let mut project_score = 0usize;
    let mut person_signals = Vec::new();
    let mut project_signals = Vec::new();

    for pattern in DIALOGUE_PATTERNS {
        let regex = Regex::new(&pattern.replace("{name}", &name_regex)).unwrap();
        let matches = regex.find_iter(text).count();
        if matches > 0 {
            person_score += matches * 3;
            person_signals.push(format!("dialogue marker ({matches}x)"));
        }
    }

    for pattern in PERSON_VERB_PATTERNS {
        let regex = Regex::new(&pattern.replace("{name}", &name_regex)).unwrap();
        let matches = regex.find_iter(text).count();
        if matches > 0 {
            person_score += matches * 2;
            person_signals.push(format!("action signal ({matches}x)"));
        }
    }

    let name_lower = name.to_ascii_lowercase();
    let name_line_indices = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.to_ascii_lowercase().contains(&name_lower))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut pronoun_hits = 0usize;
    for index in name_line_indices {
        let start = index.saturating_sub(2);
        let end = usize::min(index + 3, lines.len());
        let window = lines[start..end].join(" ").to_ascii_lowercase();
        if PRONOUN_PATTERNS
            .iter()
            .any(|pattern| Regex::new(pattern).unwrap().is_match(&window))
        {
            pronoun_hits += 1;
        }
    }
    if pronoun_hits > 0 {
        person_score += pronoun_hits * 2;
        person_signals.push(format!("pronoun nearby ({pronoun_hits}x)"));
    }

    let direct = Regex::new(&format!(r"\b(?:hey|thanks?|hi)\s+{name_regex}\b")).unwrap();
    let direct_hits = direct.find_iter(text).count();
    if direct_hits > 0 {
        person_score += direct_hits * 4;
        person_signals.push(format!("addressed directly ({direct_hits}x)"));
    }

    for pattern in PROJECT_VERB_PATTERNS {
        let regex = Regex::new(&pattern.replace("{name}", &name_regex)).unwrap();
        let matches = regex.find_iter(text).count();
        if matches > 0 {
            project_score += matches * 2;
            project_signals.push(format!("project signal ({matches}x)"));
        }
    }

    let versioned = Regex::new(&format!(r"\b{name_regex}[-v]\w+")).unwrap();
    let versioned_hits = versioned.find_iter(text).count();
    if versioned_hits > 0 {
        project_score += versioned_hits * 3;
        project_signals.push(format!("versioned ({versioned_hits}x)"));
    }

    let code_ref = Regex::new(&format!(r"\b{name_regex}\.(?:py|js|ts|yaml|yml|json|sh)\b")).unwrap();
    let code_hits = code_ref.find_iter(text).count();
    if code_hits > 0 {
        project_score += code_hits * 3;
        project_signals.push(format!("code file reference ({code_hits}x)"));
    }

    EntityScores {
        person_score,
        project_score,
        person_signals,
        project_signals,
    }
}

fn classify_entity(name: &str, frequency: usize, scores: &EntityScores) -> DetectedEntity {
    let total = scores.person_score + scores.project_score;
    if total == 0 {
        return DetectedEntity {
            name: name.to_string(),
            entity_type: "uncertain".to_string(),
            confidence: f64::min(0.4, frequency as f64 / 50.0),
            frequency,
            signals: vec![format!("appears {frequency}x, no strong type signals")],
        };
    }

    let person_ratio = scores.person_score as f64 / total as f64;
    let signal_categories = scores
        .person_signals
        .iter()
        .map(|signal| {
            if signal.contains("dialogue") {
                "dialogue"
            } else if signal.contains("action") {
                "action"
            } else if signal.contains("pronoun") {
                "pronoun"
            } else {
                "addressed"
            }
        })
        .collect::<BTreeSet<_>>();
    let has_two_signal_types = signal_categories.len() >= 2;

    let (entity_type, confidence, mut signals) =
        if person_ratio >= 0.7 && has_two_signal_types && scores.person_score >= 5 {
            (
                "person",
                f64::min(0.99, 0.5 + person_ratio * 0.5),
                scores.person_signals.clone(),
            )
        } else if person_ratio <= 0.3 {
            (
                "project",
                f64::min(0.99, 0.5 + (1.0 - person_ratio) * 0.5),
                scores.project_signals.clone(),
            )
        } else {
            let mut mixed = scores.person_signals.clone();
            mixed.extend(scores.project_signals.clone());
            mixed.push("mixed signals — needs review".to_string());
            ("uncertain", 0.5, mixed)
        };

    signals.truncate(3);
    DetectedEntity {
        name: name.to_string(),
        entity_type: entity_type.to_string(),
        confidence: (confidence * 100.0).round() / 100.0,
        frequency,
        signals,
    }
}

fn is_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

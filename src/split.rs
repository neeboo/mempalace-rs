use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::Result;

pub fn find_session_boundaries(lines: &[String]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            (line.contains("Claude Code v") && is_true_session_start(lines, index)).then_some(index)
        })
        .collect()
}

pub fn split_file(filepath: &Path, output_dir: Option<&Path>, dry_run: bool) -> Result<Vec<PathBuf>> {
    let path = filepath.to_path_buf();
    let lines = fs::read_to_string(&path)?
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut boundaries = find_session_boundaries(&lines);
    if boundaries.len() < 2 {
        return Ok(Vec::new());
    }
    boundaries.push(lines.len());

    let out_dir = output_dir.map(PathBuf::from).unwrap_or_else(|| path.parent().unwrap().to_path_buf());
    fs::create_dir_all(&out_dir)?;
    let mut outputs = Vec::new();
    for (index, window) in boundaries.windows(2).enumerate() {
        let chunk = &lines[window[0]..window[1]];
        if chunk.len() < 2 {
            continue;
        }
        let ts_part = extract_timestamp(chunk).unwrap_or_else(|| format!("part{:02}", index + 1));
        let people = extract_people(chunk);
        let subject = extract_subject(chunk);
        let src_stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("session")
            .chars()
            .map(|char| if char.is_ascii_alphanumeric() || char == '-' { char } else { '_' })
            .collect::<String>();
        let people_part = if people.is_empty() {
            "unknown".to_string()
        } else {
            people.join("-")
        };
        let file_name = format!("{src_stem}__{ts_part}_{people_part}_{subject}.txt");
        let out_path = out_dir.join(sanitize_file_name(&file_name));
        if !dry_run {
            fs::write(&out_path, chunk.join("\n"))?;
        }
        outputs.push(out_path);
    }

    if !dry_run && !outputs.is_empty() {
        fs::rename(&path, path.with_extension("mega_backup"))?;
    }

    Ok(outputs)
}

fn is_true_session_start(lines: &[String], index: usize) -> bool {
    for line in lines.iter().skip(index + 1).take(5) {
        if line.contains("Claude Code v") {
            break;
        }
        if line.contains("Ctrl+E") || line.contains("previous messages") {
            return false;
        }
    }
    true
}

fn extract_timestamp(lines: &[String]) -> Option<String> {
    let regex = Regex::new(r"⏺\s+(\d{1,2}):(\d{2})\s+([AP]M)\s+\w+,\s+(\w+)\s+(\d{1,2}),\s+(\d{4})").unwrap();
    let month_map = [
        ("January", "01"),
        ("February", "02"),
        ("March", "03"),
        ("April", "04"),
        ("May", "05"),
        ("June", "06"),
        ("July", "07"),
        ("August", "08"),
        ("September", "09"),
        ("October", "10"),
        ("November", "11"),
        ("December", "12"),
    ]
    .into_iter()
    .collect::<std::collections::HashMap<_, _>>();

    for line in lines.iter().take(50) {
        if let Some(captures) = regex.captures(line) {
            let hour = captures.get(1)?.as_str();
            let minute = captures.get(2)?.as_str();
            let meridiem = captures.get(3)?.as_str();
            let month = month_map.get(captures.get(4)?.as_str()).copied().unwrap_or("00");
            let day = format!("{:02}", captures.get(5)?.as_str().parse::<usize>().ok()?);
            let year = captures.get(6)?.as_str();
            return Some(format!("{year}-{month}-{day}_{hour}{minute}{meridiem}"));
        }
    }
    None
}

fn extract_people(lines: &[String]) -> Vec<String> {
    let known = ["Alice", "Ben", "Riley", "Max", "Sam", "Devon", "Jordan"];
    let text = lines.iter().take(100).cloned().collect::<Vec<_>>().join("\n");
    known
        .into_iter()
        .filter(|name| text.to_ascii_lowercase().contains(&name.to_ascii_lowercase()))
        .map(|name| name.to_string())
        .collect()
}

fn extract_subject(lines: &[String]) -> String {
    for line in lines {
        if let Some(prompt) = line.strip_prefix("> ") {
            let trimmed = prompt.trim();
            if trimmed.len() <= 5 {
                continue;
            }
            let lower = trimmed.to_ascii_lowercase();
            if ["./", "cd ", "ls ", "python", "bash", "git ", "cat ", "source ", "export "]
                .iter()
                .any(|prefix| lower.starts_with(prefix))
            {
                continue;
            }
            return sanitize_file_name(&trimmed.replace(' ', "-"));
        }
    }
    "session".to_string()
}

fn sanitize_file_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char == '.' || char == '-' || char == '_' {
                char
            } else {
                '_'
            }
        })
        .collect::<String>();
    sanitized
        .trim_matches('_')
        .chars()
        .take(160)
        .collect::<String>()
}

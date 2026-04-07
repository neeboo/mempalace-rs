use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::{MempalaceError, Result};

pub fn normalize_file(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path).map_err(|err| {
        MempalaceError::message(format!("Could not read {}: {err}", path.display()))
    })?;

    if content.trim().is_empty() {
        return Ok(content);
    }

    let quote_lines = content
        .lines()
        .filter(|line| line.trim_start().starts_with('>'))
        .count();
    if quote_lines >= 3 {
        return Ok(content);
    }

    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let starts_like_json = matches!(content.trim_start().chars().next(), Some('{') | Some('['));
    if matches!(ext.as_str(), "json" | "jsonl") || starts_like_json {
        if let Some(normalized) = try_normalize_json(&content)? {
            return Ok(normalized);
        }
    }

    Ok(content)
}

fn try_normalize_json(content: &str) -> Result<Option<String>> {
    if let Some(normalized) = try_claude_code_jsonl(content)? {
        return Ok(Some(normalized));
    }

    let parsed = match serde_json::from_str::<Value>(content) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    for parser in [try_claude_ai_json as fn(&Value) -> Option<String>, try_chatgpt_json, try_slack_json]
    {
        if let Some(normalized) = parser(&parsed) {
            return Ok(Some(normalized));
        }
    }

    Ok(None)
}

fn try_claude_code_jsonl(content: &str) -> Result<Option<String>> {
    let mut messages = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let msg_type = entry.get("type").and_then(Value::as_str).unwrap_or_default();
        let message = entry.get("message").cloned().unwrap_or(Value::Null);
        let Some(text) = extract_content(message.get("content")) else {
            continue;
        };
        match msg_type {
            "human" => messages.push(("user".to_string(), text)),
            "assistant" => messages.push(("assistant".to_string(), text)),
            _ => {}
        }
    }

    if messages.len() >= 2 {
        Ok(Some(messages_to_transcript(&messages)))
    } else {
        Ok(None)
    }
}

fn try_claude_ai_json(data: &Value) -> Option<String> {
    let items = if let Some(messages) = data.get("messages").and_then(Value::as_array) {
        messages
    } else if let Some(messages) = data.get("chat_messages").and_then(Value::as_array) {
        messages
    } else {
        data.as_array()?
    };

    let mut messages = Vec::new();
    for item in items {
        let role = item.get("role").and_then(Value::as_str).unwrap_or_default();
        let Some(text) = extract_content(item.get("content")) else {
            continue;
        };
        match role {
            "user" | "human" => messages.push(("user".to_string(), text)),
            "assistant" | "ai" => messages.push(("assistant".to_string(), text)),
            _ => {}
        }
    }

    if messages.len() >= 2 {
        Some(messages_to_transcript(&messages))
    } else {
        None
    }
}

fn try_chatgpt_json(data: &Value) -> Option<String> {
    let mapping = data.get("mapping")?.as_object()?;
    let mut current_id = mapping
        .iter()
        .find_map(|(key, value)| {
            if value.get("parent").is_none() && value.get("message").is_some() {
                Some(key.clone())
            } else {
                None
            }
        })
        .or_else(|| {
            mapping
                .iter()
                .find_map(|(key, value)| value.get("parent").is_none().then(|| key.clone()))
        })?;

    let mut messages = Vec::new();
    let mut visited = std::collections::HashSet::new();
    while visited.insert(current_id.clone()) {
        let Some(node) = mapping.get(&current_id) else {
            break;
        };
        if let Some(message) = node.get("message") {
            let role = message
                .get("author")
                .and_then(|author| author.get("role"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let text = message
                .get("content")
                .and_then(|content| content.get("parts"))
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string()
                })
                .filter(|text| !text.is_empty());
            if let Some(text) = text {
                match role {
                    "user" => messages.push(("user".to_string(), text)),
                    "assistant" => messages.push(("assistant".to_string(), text)),
                    _ => {}
                }
            }
        }

        let Some(next) = node
            .get("children")
            .and_then(Value::as_array)
            .and_then(|children| children.first())
            .and_then(Value::as_str)
        else {
            break;
        };
        current_id = next.to_string();
    }

    if messages.len() >= 2 {
        Some(messages_to_transcript(&messages))
    } else {
        None
    }
}

fn try_slack_json(data: &Value) -> Option<String> {
    let items = data.as_array()?;
    let mut seen_users = std::collections::HashMap::<String, String>::new();
    let mut last_role: Option<String> = None;
    let mut messages = Vec::new();
    for item in items {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let user_id = item
            .get("user")
            .or_else(|| item.get("username"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let text = item
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        if user_id.is_empty() || text.is_empty() {
            continue;
        }
        let computed_role = if let Some(existing) = seen_users.get(&user_id) {
            existing.clone()
        } else if seen_users.is_empty() {
            "user".to_string()
        } else if last_role.as_deref() == Some("user") {
            "assistant".to_string()
        } else {
            "user".to_string()
        };
        let role = seen_users.entry(user_id).or_insert(computed_role);
        last_role = Some(role.clone());
        messages.push((role.clone(), text));
    }

    if messages.len() >= 2 {
        Some(messages_to_transcript(&messages))
    } else {
        None
    }
}

fn extract_content(content: Option<&Value>) -> Option<String> {
    let content = content?;
    match content {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(text) => Some(text.clone()),
                    Value::Object(map) => map
                        .get("type")
                        .and_then(Value::as_str)
                        .filter(|kind| *kind == "text")
                        .and_then(|_| map.get("text"))
                        .and_then(Value::as_str)
                        .map(|text| text.to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ");
            let trimmed = parts.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string),
        _ => None,
    }
}

fn messages_to_transcript(messages: &[(String, String)]) -> String {
    let mut lines = Vec::new();
    let mut index = 0;
    while index < messages.len() {
        let (role, text) = &messages[index];
        if role == "user" {
            lines.push(format!("> {text}"));
            if let Some((next_role, next_text)) = messages.get(index + 1) {
                if next_role == "assistant" {
                    lines.push(next_text.clone());
                    index += 2;
                } else {
                    index += 1;
                }
            } else {
                index += 1;
            }
        } else {
            lines.push(text.clone());
            index += 1;
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

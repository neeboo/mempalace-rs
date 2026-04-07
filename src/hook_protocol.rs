use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::Result;
use crate::convo::{ExtractMode, mine_conversations_with_extract_mode};

pub fn handle_stop_hook(
    input: &str,
    state_dir: &Path,
    mempal_dir: Option<&Path>,
    palace_path: &Path,
    save_interval: usize,
) -> Result<String> {
    fs::create_dir_all(state_dir)?;
    let payload: Value = serde_json::from_str(input)?;
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let stop_hook_active = payload
        .get("stop_hook_active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let transcript_path = payload
        .get("transcript_path")
        .and_then(Value::as_str)
        .map(expand_home)
        .unwrap_or_default();

    if stop_hook_active {
        return Ok("{}".to_string());
    }

    let exchange_count = if transcript_path.is_file() {
        count_user_messages(&transcript_path)?
    } else {
        0
    };

    let last_save_file = state_dir.join(format!("{session_id}_last_save"));
    let last_save = if last_save_file.exists() {
        fs::read_to_string(&last_save_file)
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0)
    } else {
        0
    };
    let since_last = exchange_count.saturating_sub(last_save);
    append_log(
        state_dir,
        &format!(
            "Session {session_id}: {exchange_count} exchanges, {since_last} since last save"
        ),
    )?;

    if since_last >= save_interval && exchange_count > 0 {
        fs::write(&last_save_file, exchange_count.to_string())?;
        append_log(
            state_dir,
            &format!("TRIGGERING SAVE at exchange {exchange_count}"),
        )?;
        maybe_mine_dir(mempal_dir, palace_path, state_dir)?;
        Ok(json!({
            "decision": "block",
            "reason": "AUTO-SAVE checkpoint. Save key topics, decisions, quotes, and code from this session to your memory system. Organize into appropriate categories. Use verbatim quotes where possible. Continue conversation after saving."
        })
        .to_string())
    } else {
        Ok("{}".to_string())
    }
}

pub fn handle_precompact_hook(
    input: &str,
    state_dir: &Path,
    mempal_dir: Option<&Path>,
    palace_path: &Path,
) -> Result<String> {
    fs::create_dir_all(state_dir)?;
    let payload: Value = serde_json::from_str(input)?;
    let session_id = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    append_log(
        state_dir,
        &format!("PRE-COMPACT triggered for session {session_id}"),
    )?;
    maybe_mine_dir(mempal_dir, palace_path, state_dir)?;
    Ok(json!({
        "decision": "block",
        "reason": "COMPACTION IMMINENT. Save ALL topics, decisions, quotes, code, and important context from this session to your memory system. Be thorough — after compaction, detailed context will be lost. Organize into appropriate categories. Use verbatim quotes where possible. Save everything, then allow compaction to proceed."
    })
    .to_string())
}

fn maybe_mine_dir(mempal_dir: Option<&Path>, palace_path: &Path, state_dir: &Path) -> Result<()> {
    let Some(mempal_dir) = mempal_dir else {
        return Ok(());
    };
    if !mempal_dir.is_dir() {
        return Ok(());
    }
    match mine_conversations_with_extract_mode(
        mempal_dir,
        palace_path,
        None,
        "mempalace-hook",
        0,
        false,
        ExtractMode::Exchange,
    ) {
        Ok(summary) => append_log(
            state_dir,
            &format!(
                "Auto-ingested {} files and filed {} drawers",
                summary.files_processed, summary.drawers_filed
            ),
        )?,
        Err(err) => append_log(state_dir, &format!("Auto-ingest failed: {err}"))?,
    }
    Ok(())
}

fn count_user_messages(transcript_path: &Path) -> Result<usize> {
    let content = fs::read_to_string(transcript_path)?;
    let mut count = 0usize;
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(message) = entry.get("message") else {
            continue;
        };
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let content = message.get("content");
        if content
            .and_then(Value::as_str)
            .map(|text| text.contains("<command-message>"))
            == Some(true)
        {
            continue;
        }
        count += 1;
    }
    Ok(count)
}

fn append_log(state_dir: &Path, message: &str) -> Result<()> {
    let log_path = state_dir.join("hook.log");
    let prefix = chrono_like_timestamp();
    let line = format!("[{prefix}] {message}\n");
    if log_path.exists() {
        let mut existing = fs::read_to_string(&log_path)?;
        existing.push_str(&line);
        fs::write(log_path, existing)?;
    } else {
        fs::write(log_path, line)?;
    }
    Ok(())
}

fn chrono_like_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let seconds = now % 86_400;
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn expand_home(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        return PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(rest);
    }
    if raw == "~" {
        return PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    }
    PathBuf::from(raw)
}

use std::fs;

use mempalace_rs::hook_protocol::{handle_precompact_hook, handle_stop_hook};
use tempfile::tempdir;

#[test]
fn stop_hook_blocks_when_save_interval_is_reached() {
    let dir = tempdir().unwrap();
    let transcript = dir.path().join("transcript.jsonl");
    fs::write(
        &transcript,
        r#"{"message":{"role":"user","content":"one"}}
{"message":{"role":"assistant","content":"ack"}}
{"message":{"role":"user","content":"two"}}
"#,
    )
    .unwrap();

    let input = format!(
        r#"{{"session_id":"abc123","stop_hook_active":false,"transcript_path":"{}"}}"#,
        transcript.display()
    );
    let output = handle_stop_hook(&input, dir.path(), None, dir.path(), 2).unwrap();

    assert!(output.contains(r#""decision":"block""#));
    assert!(dir.path().join("abc123_last_save").exists());
}

#[test]
fn stop_hook_allows_exit_when_already_in_save_cycle() {
    let dir = tempdir().unwrap();
    let output = handle_stop_hook(
        r#"{"session_id":"abc123","stop_hook_active":true,"transcript_path":""}"#,
        dir.path(),
        None,
        dir.path(),
        2,
    )
    .unwrap();

    assert_eq!(output, "{}");
}

#[test]
fn precompact_hook_always_blocks() {
    let dir = tempdir().unwrap();
    let output = handle_precompact_hook(
        r#"{"session_id":"precompact-1"}"#,
        dir.path(),
        None,
        dir.path(),
    )
    .unwrap();

    assert!(output.contains("COMPACTION IMMINENT"));
}

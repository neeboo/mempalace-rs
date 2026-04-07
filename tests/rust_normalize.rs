use std::fs;

use mempalace_rs::normalize::normalize_file;
use tempfile::tempdir;

#[test]
fn keeps_plain_text_as_is() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("chat.txt");
    fs::write(&file, "Hello world\nSecond line\n").unwrap();

    let text = normalize_file(&file).unwrap();
    assert!(text.contains("Hello world"));
    assert!(text.contains("Second line"));
}

#[test]
fn normalizes_simple_claude_json() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("chat.json");
    fs::write(
        &file,
        r#"[{"role":"user","content":"Hi"},{"role":"assistant","content":"Hello"}]"#,
    )
    .unwrap();

    let text = normalize_file(&file).unwrap();
    assert!(text.contains("> Hi"));
    assert!(text.contains("Hello"));
}

#[test]
fn preserves_empty_files() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("empty.txt");
    fs::write(&file, "").unwrap();

    let text = normalize_file(&file).unwrap();
    assert!(text.trim().is_empty());
}

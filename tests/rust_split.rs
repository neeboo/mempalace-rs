use std::fs;

use mempalace_rs::split::{find_session_boundaries, split_file};
use tempfile::tempdir;

#[test]
fn detects_true_session_boundaries() {
    let text = [
        "Claude Code v1.0",
        "⏺ 10:20 AM Tuesday, April 07, 2026",
        "> first prompt",
        "answer",
        "Claude Code v1.0",
        "Ctrl+E to show 30 previous messages",
        "restored context",
        "Claude Code v1.0",
        "⏺ 11:45 AM Tuesday, April 07, 2026",
        "> second prompt",
    ]
    .join("\n");
    let lines = text.lines().map(str::to_string).collect::<Vec<_>>();

    let boundaries = find_session_boundaries(&lines);
    assert_eq!(boundaries, vec![0, 7]);
}

#[test]
fn splits_mega_file_into_timestamped_sessions() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("sessions.txt");
    fs::write(
        &src,
        [
            "Claude Code v1.0",
            "⏺ 10:20 AM Tuesday, April 07, 2026",
            "> Build the auth service",
            "Working on it",
            "Claude Code v1.0",
            "⏺ 11:45 AM Tuesday, April 07, 2026",
            "> Review deployment notes",
            "Done",
        ]
        .join("\n"),
    )
    .unwrap();

    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    let outputs = split_file(&src, Some(&out_dir), false).unwrap();

    assert_eq!(outputs.len(), 2);
    assert!(outputs[0].file_name().unwrap().to_string_lossy().contains("2026-04-07_1020AM"));
    assert!(outputs[1].exists());
    assert!(src.with_extension("mega_backup").exists());
}

use std::fs;

use mempalace_rs::entity_detector::{detect_entities, scan_for_detection};
use tempfile::tempdir;

#[test]
fn detects_people_and_projects_from_prose_files() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("notes.md"),
        r#"
Riley said we should ship it.
Riley asked for the deploy log.
hey Riley, can you review this?

We are building MemPalace.
The MemPalace architecture is stable.
We shipped MemPalace v2 yesterday.
"#,
    )
    .unwrap();
    fs::write(dir.path().join("script.py"), "print('ignore code noise')\n").unwrap();

    let files = scan_for_detection(dir.path(), 10).unwrap();
    let detected = detect_entities(&files, 10).unwrap();

    assert!(detected.people.iter().any(|entity| entity.name == "Riley"));
    assert!(detected.projects.iter().any(|entity| entity.name == "MemPalace"));
}

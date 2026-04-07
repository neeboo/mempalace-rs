use std::fs;

use mempalace_rs::convo::mine_conversations;
use mempalace_rs::miner::mine_project;
use mempalace_rs::search::search_memories;
use mempalace_rs::storage::PalaceStore;
use tempfile::tempdir;

#[test]
fn mines_project_files_into_the_palace() {
    let dir = tempdir().unwrap();
    let backend_dir = dir.path().join("backend");
    fs::create_dir_all(&backend_dir).unwrap();
    fs::write(
        backend_dir.join("app.py"),
        "def main():\n    print('hello world')\n".repeat(20),
    )
    .unwrap();
    fs::write(
        dir.path().join("mempalace.yaml"),
        r#"wing: test_project
rooms:
  - name: backend
    description: Backend code
  - name: general
    description: General
"#,
    )
    .unwrap();

    let palace_dir = dir.path().join("palace");
    let summary = mine_project(dir.path(), &palace_dir, None, "mempalace", 0, false).unwrap();
    assert!(summary.drawers_filed > 0);

    let store = PalaceStore::open(&palace_dir).unwrap();
    assert!(store.drawer_count().unwrap() > 0);
}

#[test]
fn mines_conversations_and_can_search_them() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("chat.txt"),
        "> What is memory?\nMemory is persistence.\n\n> Why does it matter?\nIt enables continuity.\n\n> How do we build it?\nWith structured storage.\n",
    )
    .unwrap();

    let palace_dir = dir.path().join("palace");
    let summary = mine_conversations(dir.path(), &palace_dir, Some("test_convos"), "mempalace", 0, false).unwrap();
    assert!(summary.drawers_filed >= 2);

    let store = PalaceStore::open(&palace_dir).unwrap();
    assert!(store.drawer_count().unwrap() >= 2);

    let hits = search_memories(&store, "memory persistence", None, None, 1).unwrap();
    assert!(!hits.is_empty());
}

use std::collections::BTreeSet;
use std::fs;

use mempalace_rs::convo::{ExtractMode, mine_conversations_with_extract_mode};
use mempalace_rs::general_extractor::extract_memories;
use mempalace_rs::storage::PalaceStore;
use tempfile::tempdir;

#[test]
fn extracts_general_memory_types_from_prose() {
    let text = r#"
We decided to switch to Rust because the Python runtime kept drifting.

I prefer small, direct tools and never use giant framework wrappers.

The build kept failing with a linker error, but the fix was setting the wrapper correctly and now it works.

I feel proud that the demo finally shipped.
"#;

    let memories = extract_memories(text, 0.3);
    let kinds = memories
        .iter()
        .map(|memory| memory.memory_type.as_str())
        .collect::<BTreeSet<_>>();

    assert!(kinds.contains("decision"));
    assert!(kinds.contains("preference"));
    assert!(kinds.contains("milestone"));
    assert!(kinds.contains("emotional"));
}

#[test]
fn mines_general_conversations_into_typed_rooms() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("chat.txt"),
        r#"
We decided to ship the Rust CLI first because it closes the runtime gap.

I prefer direct binaries and never use wrapper launchers for hooks.

The hook kept failing before, but the fix was switching the command to the Rust binary and now it works.
"#,
    )
    .unwrap();

    let palace_dir = dir.path().join("palace");
    let summary = mine_conversations_with_extract_mode(
        dir.path(),
        &palace_dir,
        Some("typed_memories"),
        "mempalace",
        0,
        false,
        ExtractMode::General,
    )
    .unwrap();
    assert!(summary.drawers_filed >= 3);

    let store = PalaceStore::open(&palace_dir).unwrap();
    let drawers = store.list_drawers(Some("typed_memories"), None).unwrap();
    let rooms = drawers.iter().map(|drawer| drawer.room.as_str()).collect::<BTreeSet<_>>();

    assert!(rooms.contains("decision"));
    assert!(rooms.contains("preference"));
    assert!(rooms.contains("milestone"));
    assert!(
        drawers
            .iter()
            .any(|drawer| drawer.hall.as_deref() == Some("hall_facts"))
    );
    assert!(
        drawers
            .iter()
            .any(|drawer| drawer.hall.as_deref() == Some("hall_preferences"))
    );
}

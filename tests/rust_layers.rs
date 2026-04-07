use std::fs;

use mempalace_rs::convo::mine_conversations;
use mempalace_rs::layers::MemoryStack;
use tempfile::tempdir;

#[test]
fn builds_wake_up_recall_and_search_layers() {
    let dir = tempdir().unwrap();
    let palace = dir.path().join("palace");
    let identity = dir.path().join("identity.txt");
    fs::write(&identity, "I am Atlas, a personal AI assistant for Alice.").unwrap();
    fs::write(
        dir.path().join("chat.txt"),
        "> Why did we switch to GraphQL?\nBecause the REST API kept failing.\n\n> What was the deployment issue?\nThe auth service was missing an env var.\n",
    )
    .unwrap();

    mine_conversations(dir.path(), &palace, Some("wing_code"), "mempalace", 0, false).unwrap();
    let stack = MemoryStack::new(palace.clone(), Some(identity.clone()));

    let wake_up = stack.wake_up(Some("wing_code")).unwrap();
    assert!(wake_up.contains("I am Atlas"));
    assert!(wake_up.contains("ESSENTIAL STORY"));

    let recall = stack.recall(Some("wing_code"), Some("problems"), 10).unwrap();
    assert!(recall.contains("ON-DEMAND"));

    let search = stack.search("REST API failing", Some("wing_code"), None, 5).unwrap();
    assert!(search.contains("SEARCH RESULTS"));

    let status = stack.status().unwrap();
    assert_eq!(status.total_drawers, 2);
}

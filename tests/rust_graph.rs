use mempalace_rs::palace_graph::{build_graph, find_tunnels, graph_stats, traverse};
use mempalace_rs::storage::{NewDrawer, PalaceStore};
use tempfile::tempdir;

#[test]
fn builds_room_graph_and_traverses_tunnels() {
    let dir = tempdir().unwrap();
    let store = PalaceStore::open(dir.path()).unwrap();

    for (id, wing, room, source) in [
        ("a", "wing_code", "auth", "a.txt"),
        ("b", "wing_ops", "auth", "b.txt"),
        ("c", "wing_code", "deploy", "c.txt"),
        ("d", "wing_ops", "deploy", "d.txt"),
    ] {
        store
            .insert_drawer(&NewDrawer {
                id: id.to_string(),
                wing: wing.to_string(),
                room: room.to_string(),
                source_file: source.to_string(),
                chunk_index: 0,
                added_by: "test".to_string(),
                filed_at: "2026-04-07".to_string(),
                content: format!("{wing} {room}"),
                ingest_mode: Some("projects".to_string()),
                extract_mode: None,
                hall: None,
                topic: None,
                drawer_type: None,
                date: Some("2026-04-07".to_string()),
            })
            .unwrap();
    }

    let (nodes, edges) = build_graph(&store).unwrap();
    assert!(nodes.contains_key("auth"));
    assert!(!edges.is_empty());

    let walked = traverse(&store, "auth", 2).unwrap();
    assert!(walked.iter().any(|node| node.room == "deploy"));

    let tunnels = find_tunnels(&store, Some("wing_code"), Some("wing_ops")).unwrap();
    assert_eq!(tunnels.len(), 2);

    let stats = graph_stats(&store).unwrap();
    assert_eq!(stats.tunnel_rooms, 2);
}

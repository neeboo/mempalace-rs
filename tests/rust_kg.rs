use mempalace_rs::knowledge_graph::{KnowledgeGraph, QueryDirection};
use tempfile::tempdir;

#[test]
fn adds_queries_and_invalidates_triples() {
    let dir = tempdir().unwrap();
    let kg = KnowledgeGraph::new(&dir.path().join("kg.sqlite3")).unwrap();

    kg.add_triple("Max", "child_of", "Alice", Some("2015-04-01"), None, 1.0, None)
        .unwrap();
    kg.add_triple("Max", "does", "swimming", Some("2025-01-01"), None, 1.0, None)
        .unwrap();

    let facts = kg.query_entity("Max", Some("2026-01-15"), QueryDirection::Outgoing).unwrap();
    assert_eq!(facts.len(), 2);
    assert!(facts.iter().any(|fact| fact.object == "Alice"));

    kg.invalidate("Max", "does", "swimming", Some("2026-02-15")).unwrap();
    let as_of_march = kg.query_entity("Max", Some("2026-03-01"), QueryDirection::Outgoing).unwrap();
    assert_eq!(as_of_march.len(), 1);

    let stats = kg.stats().unwrap();
    assert_eq!(stats.entities, 3);
    assert_eq!(stats.triples, 2);
    assert_eq!(stats.current_facts, 1);
}

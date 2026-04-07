use std::collections::HashMap;

use mempalace_rs::dialect::{CompressionMetadata, Dialect};

#[test]
fn compresses_plain_text_into_aaak_like_output() {
    let mut entities = HashMap::new();
    entities.insert("Alice".to_string(), "ALC".to_string());
    let dialect = Dialect::new(entities, Vec::new());

    let compressed = dialect.compress(
        "Alice decided to switch the GraphQL API because the old REST path kept failing.",
        Some(&CompressionMetadata {
            source_file: Some("notes/auth.txt".to_string()),
            wing: Some("wing_code".to_string()),
            room: Some("backend".to_string()),
            date: Some("2026-04-07".to_string()),
        }),
    );

    assert!(compressed.contains("wing_code|backend|2026-04-07|auth"));
    assert!(compressed.contains("0:ALC"));
    assert!(compressed.contains("graphql"));
    assert!(compressed.contains("DECISION"));
}

#[test]
fn reports_smaller_token_estimate_for_compressed_text() {
    let dialect = Dialect::default();
    let original = "We decided to use GraphQL instead of REST because the old API kept failing.";
    let compressed = dialect.compress(original, None);
    let stats = dialect.compression_stats(original, &compressed);

    assert!(stats.original_chars > stats.compressed_chars);
    assert!(stats.ratio > 1.0);
}

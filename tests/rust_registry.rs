use std::collections::HashMap;

use mempalace_rs::entity_registry::{EntityRegistry, RegistryPerson};
use tempfile::tempdir;

#[test]
fn registry_seeds_aliases_and_disambiguates_ambiguous_names() {
    let dir = tempdir().unwrap();
    let mut registry = EntityRegistry::load(Some(dir.path())).unwrap();
    registry
        .seed(
            "combo",
            &[
                RegistryPerson::new("Will", "friend", "personal"),
                RegistryPerson::new("Maxwell", "coworker", "work"),
            ],
            &["MemPalace".to_string()],
            &HashMap::from([("Max".to_string(), "Maxwell".to_string())]),
        )
        .unwrap();

    let alias = registry.lookup("Max", "thanks Max for reviewing this");
    assert_eq!(alias.entity_type, "person");
    assert_eq!(alias.name, "Max");

    let person = registry.lookup("Will", "Will said the deploy is clean");
    assert_eq!(person.entity_type, "person");

    let concept = registry.lookup("Will", "if you will ever need a fallback");
    assert_eq!(concept.entity_type, "concept");
}

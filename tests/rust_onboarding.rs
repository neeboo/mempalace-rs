use std::fs;

use mempalace_rs::onboarding::{RegistryBootstrap, bootstrap_project, generate_bootstrap_files};
use tempfile::tempdir;

#[test]
fn writes_bootstrap_files_for_known_people_and_projects() {
    let dir = tempdir().unwrap();
    generate_bootstrap_files(
        &[
            RegistryBootstrap::person("Riley", "daughter", "personal"),
            RegistryBootstrap::person("Devon", "engineer", "work"),
        ],
        &["MemPalace".to_string()],
        &["family".to_string(), "projects".to_string()],
        "combo",
        Some(dir.path()),
    )
    .unwrap();

    let aaak = fs::read_to_string(dir.path().join("aaak_entities.md")).unwrap();
    let facts = fs::read_to_string(dir.path().join("critical_facts.md")).unwrap();

    assert!(aaak.contains("RIL=Riley"));
    assert!(aaak.contains("MEMP=MemPalace"));
    assert!(facts.contains("Wings: family, projects"));
    assert!(facts.contains("Mode: combo"));
}

#[test]
fn bootstrap_project_writes_entities_registry_and_room_config() {
    let project_dir = tempdir().unwrap();
    let config_dir = project_dir.path().join(".bootstrap");
    fs::create_dir_all(project_dir.path().join("backend")).unwrap();
    fs::write(
        project_dir.path().join("journal.md"),
        r#"
Riley said the launch can wait.
Riley asked for the API notes.
hey Riley, let's review the release.

We are building MemPalace.
The MemPalace architecture is stable.
We shipped MemPalace v2 yesterday.
"#,
    )
    .unwrap();
    fs::write(
        project_dir.path().join("backend").join("app.py"),
        "def main():\n    return 'ok'\n",
    )
    .unwrap();

    let summary = bootstrap_project(project_dir.path(), Some(&config_dir), true).unwrap();

    let entities = fs::read_to_string(summary.entities_path.unwrap()).unwrap();
    assert!(entities.contains("\"Riley\""));
    assert!(entities.contains("\"MemPalace\""));
    assert!(summary.config_path.exists());
    assert!(config_dir.join("entity_registry.json").exists());
    assert!(config_dir.join("aaak_entities.md").exists());
    assert!(config_dir.join("critical_facts.md").exists());
}

use std::fs;

use mempalace_rs::config::MempalaceConfig;
use tempfile::tempdir;

#[test]
fn uses_default_config_when_no_file_exists() {
    let dir = tempdir().unwrap();
    let cfg = MempalaceConfig::new(Some(dir.path().to_path_buf())).unwrap();

    assert!(cfg.palace_path().to_string_lossy().contains("palace"));
    assert_eq!(cfg.collection_name(), "mempalace_drawers");
}

#[test]
fn prefers_config_file_when_present() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("config.json"),
        r#"{"palace_path":"/custom/palace","collection_name":"custom_drawers"}"#,
    )
    .unwrap();

    let cfg = MempalaceConfig::new(Some(dir.path().to_path_buf())).unwrap();
    assert_eq!(cfg.palace_path().to_string_lossy(), "/custom/palace");
    assert_eq!(cfg.collection_name(), "custom_drawers");
}

#[test]
fn env_override_wins() {
    let dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("MEMPALACE_PALACE_PATH", "/env/palace");
    }

    let cfg = MempalaceConfig::new(Some(dir.path().to_path_buf())).unwrap();
    assert_eq!(cfg.palace_path().to_string_lossy(), "/env/palace");

    unsafe {
        std::env::remove_var("MEMPALACE_PALACE_PATH");
    }
}

#[test]
fn init_writes_default_config() {
    let dir = tempdir().unwrap();
    let cfg = MempalaceConfig::new(Some(dir.path().to_path_buf())).unwrap();
    let path = cfg.init().unwrap();

    assert!(path.exists());
}

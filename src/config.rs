use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Result;

pub const DEFAULT_PALACE_PATH: &str = "~/.mempalace/palace";
pub const DEFAULT_COLLECTION_NAME: &str = "mempalace_drawers";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FileConfig {
    palace_path: Option<String>,
    collection_name: Option<String>,
    people_map: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct MempalaceConfig {
    config_dir: PathBuf,
    config_file: PathBuf,
    file_config: FileConfig,
}

impl MempalaceConfig {
    pub fn new(config_dir: Option<PathBuf>) -> Result<Self> {
        let dir = config_dir.unwrap_or_else(default_config_dir);
        let file = dir.join("config.json");
        let file_config = if file.exists() {
            serde_json::from_str(&fs::read_to_string(&file).unwrap_or_default()).unwrap_or_default()
        } else {
            FileConfig::default()
        };
        Ok(Self {
            config_dir: dir,
            config_file: file,
            file_config,
        })
    }

    pub fn palace_path(&self) -> PathBuf {
        let env = std::env::var("MEMPALACE_PALACE_PATH")
            .ok()
            .or_else(|| std::env::var("MEMPAL_PALACE_PATH").ok());
        let raw = env
            .or_else(|| self.file_config.palace_path.clone())
            .unwrap_or_else(|| DEFAULT_PALACE_PATH.to_string());
        expand_home(raw)
    }

    pub fn collection_name(&self) -> &str {
        self.file_config
            .collection_name
            .as_deref()
            .unwrap_or(DEFAULT_COLLECTION_NAME)
    }

    pub fn init(&self) -> Result<PathBuf> {
        fs::create_dir_all(&self.config_dir)?;
        if !self.config_file.exists() {
            let payload = FileConfig {
                palace_path: Some(DEFAULT_PALACE_PATH.to_string()),
                collection_name: Some(DEFAULT_COLLECTION_NAME.to_string()),
                people_map: Some(HashMap::new()),
            };
            fs::write(&self.config_file, serde_json::to_string_pretty(&payload)?)?;
        }
        Ok(self.config_file.clone())
    }
}

fn default_config_dir() -> PathBuf {
    expand_home("~/.mempalace")
}

fn expand_home(raw: impl Into<String>) -> PathBuf {
    let raw = raw.into();
    if raw == "~" {
        return PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(rest);
    }
    PathBuf::from(raw)
}

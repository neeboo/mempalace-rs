use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::search::search_memories;
use crate::storage::PalaceStore;
use crate::Result;

#[derive(Debug, Clone, Serialize)]
pub struct MemoryStackStatus {
    pub palace_path: PathBuf,
    pub identity_path: PathBuf,
    pub total_drawers: usize,
}

#[derive(Debug, Clone)]
pub struct MemoryStack {
    palace_path: PathBuf,
    identity_path: PathBuf,
}

impl MemoryStack {
    pub fn new(palace_path: PathBuf, identity_path: Option<PathBuf>) -> Self {
        let identity_path = identity_path.unwrap_or_else(|| {
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".mempalace/identity.txt")
        });
        Self {
            palace_path,
            identity_path,
        }
    }

    pub fn wake_up(&self, wing: Option<&str>) -> Result<String> {
        let identity = self.render_identity();
        let essential = self.generate_layer1(wing)?;
        Ok(format!("{identity}\n\n{essential}"))
    }

    pub fn recall(&self, wing: Option<&str>, room: Option<&str>, n_results: usize) -> Result<String> {
        let store = PalaceStore::open(&self.palace_path)?;
        let drawers = store.list_drawers(wing, room)?;
        let mut lines = vec![format!("## L2 — ON-DEMAND ({} drawers)", drawers.len())];
        if drawers.is_empty() {
            lines.push("No drawers found.".to_string());
            return Ok(lines.join("\n"));
        }
        for drawer in drawers.into_iter().take(n_results) {
            let mut snippet = drawer.content.replace('\n', " ");
            if snippet.len() > 300 {
                snippet.truncate(297);
                snippet.push_str("...");
            }
            lines.push(format!("  [{}] {}  ({})", drawer.room, snippet, drawer.source_file));
        }
        Ok(lines.join("\n"))
    }

    pub fn search(&self, query: &str, wing: Option<&str>, room: Option<&str>, n_results: usize) -> Result<String> {
        let store = PalaceStore::open(&self.palace_path)?;
        let hits = search_memories(&store, query, wing, room, n_results)?;
        if hits.is_empty() {
            return Ok("No results found.".to_string());
        }
        let mut lines = vec![format!("## L3 — SEARCH RESULTS for \"{query}\"")];
        for (index, hit) in hits.into_iter().enumerate() {
            let mut snippet = hit.text.replace('\n', " ");
            if snippet.len() > 300 {
                snippet.truncate(297);
                snippet.push_str("...");
            }
            lines.push(format!("  [{}] {}/{} (sim={})", index + 1, hit.wing, hit.room, hit.score));
            lines.push(format!("      {snippet}"));
            lines.push(format!("      src: {}", hit.source_file));
        }
        Ok(lines.join("\n"))
    }

    pub fn status(&self) -> Result<MemoryStackStatus> {
        let store = PalaceStore::open(&self.palace_path)?;
        Ok(MemoryStackStatus {
            palace_path: self.palace_path.clone(),
            identity_path: self.identity_path.clone(),
            total_drawers: store.drawer_count()?,
        })
    }

    fn render_identity(&self) -> String {
        if let Ok(text) = std::fs::read_to_string(&self.identity_path) {
            text.trim().to_string()
        } else {
            "## L0 — IDENTITY\nNo identity configured. Create ~/.mempalace/identity.txt".to_string()
        }
    }

    fn generate_layer1(&self, wing: Option<&str>) -> Result<String> {
        let store = PalaceStore::open(&self.palace_path)?;
        let drawers = store.list_drawers(wing, None)?;
        if drawers.is_empty() {
            return Ok("## L1 — No memories yet.".to_string());
        }
        let mut by_room = BTreeMap::<String, Vec<String>>::new();
        for drawer in drawers.into_iter().take(15) {
            let mut snippet = drawer.content.replace('\n', " ");
            if snippet.len() > 200 {
                snippet.truncate(197);
                snippet.push_str("...");
            }
            let source = PathBuf::from(drawer.source_file)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("?")
                .to_string();
            by_room
                .entry(drawer.room)
                .or_default()
                .push(format!("  - {snippet}  ({source})"));
        }

        let mut lines = vec!["## L1 — ESSENTIAL STORY".to_string()];
        for (room, entries) in by_room {
            lines.push(format!("\n[{room}]"));
            lines.extend(entries);
        }
        Ok(lines.join("\n"))
    }
}

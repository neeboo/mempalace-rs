use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::config::MempalaceConfig;
use crate::knowledge_graph::{KnowledgeGraph, QueryDirection};
use crate::palace_graph::{find_tunnels, graph_stats, traverse};
use crate::search::search_memories;
use crate::storage::{NewDrawer, PalaceStore};
use crate::{MempalaceError, Result};

pub const PALACE_PROTOCOL: &str = "IMPORTANT — MemPalace Memory Protocol:\n1. ON WAKE-UP: Call mempalace_status to load palace overview + AAAK spec.\n2. BEFORE RESPONDING about any person, project, or past event: call mempalace_kg_query or mempalace_search FIRST. Never guess — verify.\n3. IF UNSURE about a fact: say \"let me check\" and query the palace.\n4. AFTER EACH SESSION: call mempalace_diary_write.\n5. WHEN FACTS CHANGE: invalidate old fact then add the new one.";
pub const AAAK_SPEC: &str = "AAAK is a compressed memory dialect that MemPalace uses for efficient storage.\nFORMAT: entities are short codes, emotions are compact markers, fields are pipe-separated.";

pub struct McpServer {
    config: MempalaceConfig,
    palace_path: PathBuf,
    kg: KnowledgeGraph,
}

impl McpServer {
    pub fn new(config_dir: Option<PathBuf>, palace_path: Option<PathBuf>) -> Result<Self> {
        let config = MempalaceConfig::new(config_dir)?;
        let palace_path = palace_path.unwrap_or_else(|| config.palace_path());
        let kg_path = config
            .init()
            .unwrap_or_else(|_| PathBuf::from("."))
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("knowledge_graph.sqlite3");
        let kg = KnowledgeGraph::new(&kg_path)?;
        Ok(Self {
            config,
            palace_path,
            kg,
        })
    }

    pub fn handle_request(&self, request: Value) -> Result<Option<Value>> {
        let method = request.get("method").and_then(Value::as_str).unwrap_or_default();
        let request_id = request.get("id").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => Ok(Some(json!({
                "jsonrpc":"2.0",
                "id":request_id,
                "result":{
                    "protocolVersion":"2024-11-05",
                    "capabilities":{"tools":{}},
                    "serverInfo":{"name":"mempalace","version":"0.2.0"}
                }
            }))),
            "notifications/initialized" => Ok(None),
            "tools/list" => Ok(Some(json!({
                "jsonrpc":"2.0",
                "id":request_id,
                "result":{"tools": self.tool_descriptions()}
            }))),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or(Value::Null);
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let result = self.call_tool(name, arguments)?;
                Ok(Some(json!({
                    "jsonrpc":"2.0",
                    "id":request_id,
                    "result":{"content":[{"type":"text","text":serde_json::to_string_pretty(&result)?}]}
                })))
            }
            _ => Ok(Some(json!({
                "jsonrpc":"2.0",
                "id":request_id,
                "error":{"code":-32601,"message":format!("Unknown method: {method}")}
            }))),
        }
    }

    pub fn serve_stdio(&self) -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let request: Value = serde_json::from_str(&line)?;
            if let Some(response) = self.handle_request(request)? {
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
            }
        }
        Ok(())
    }

    fn call_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        match name {
            "mempalace_status" => self.tool_status(),
            "mempalace_list_wings" => self.tool_list_wings(),
            "mempalace_list_rooms" => self.tool_list_rooms(arguments.get("wing").and_then(Value::as_str)),
            "mempalace_get_taxonomy" => self.tool_get_taxonomy(),
            "mempalace_search" => self.tool_search(
                arguments.get("query").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("limit").and_then(Value::as_u64).unwrap_or(5) as usize,
                arguments.get("wing").and_then(Value::as_str),
                arguments.get("room").and_then(Value::as_str),
            ),
            "mempalace_check_duplicate" => self.tool_check_duplicate(
                arguments.get("content").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("threshold").and_then(Value::as_f64).unwrap_or(0.9),
            ),
            "mempalace_get_aaak_spec" => Ok(json!({"aaak_spec": AAAK_SPEC})),
            "mempalace_traverse" => self.tool_traverse(
                arguments.get("start_room").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("max_hops").and_then(Value::as_u64).unwrap_or(2) as usize,
            ),
            "mempalace_find_tunnels" => self.tool_find_tunnels(
                arguments.get("wing_a").and_then(Value::as_str),
                arguments.get("wing_b").and_then(Value::as_str),
            ),
            "mempalace_graph_stats" => self.tool_graph_stats(),
            "mempalace_add_drawer" => self.tool_add_drawer(
                arguments.get("wing").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("room").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("content").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("source_file").and_then(Value::as_str),
                arguments.get("added_by").and_then(Value::as_str).unwrap_or("mcp"),
            ),
            "mempalace_delete_drawer" => self.tool_delete_drawer(
                arguments.get("drawer_id").and_then(Value::as_str).unwrap_or_default(),
            ),
            "mempalace_kg_query" => self.tool_kg_query(
                arguments.get("entity").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("as_of").and_then(Value::as_str),
                arguments.get("direction").and_then(Value::as_str).unwrap_or("both"),
            ),
            "mempalace_kg_add" => self.tool_kg_add(
                arguments.get("subject").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("predicate").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("object").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("valid_from").and_then(Value::as_str),
            ),
            "mempalace_kg_invalidate" => self.tool_kg_invalidate(
                arguments.get("subject").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("predicate").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("object").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("ended").and_then(Value::as_str),
            ),
            "mempalace_kg_timeline" => self.tool_kg_timeline(arguments.get("entity").and_then(Value::as_str)),
            "mempalace_kg_stats" => Ok(json!(self.kg.stats()?)),
            "mempalace_diary_write" => self.tool_diary_write(
                arguments.get("agent_name").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("entry").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("topic").and_then(Value::as_str).unwrap_or("general"),
            ),
            "mempalace_diary_read" => self.tool_diary_read(
                arguments.get("agent_name").and_then(Value::as_str).unwrap_or_default(),
                arguments.get("last_n").and_then(Value::as_u64).unwrap_or(10) as usize,
            ),
            _ => Err(MempalaceError::message(format!("Unknown tool: {name}"))),
        }
    }

    fn tool_status(&self) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        let total_drawers = store.drawer_count()?;
        let taxonomy = self.collect_taxonomy(&store)?;
        let wings = taxonomy
            .iter()
            .map(|(wing, rooms)| {
                (
                    wing.clone(),
                    rooms.values().copied().sum::<usize>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut rooms = BTreeMap::<String, usize>::new();
        for room_counts in taxonomy.values() {
            for (room, count) in room_counts {
                *rooms.entry(room.clone()).or_default() += count;
            }
        }
        Ok(json!({
            "total_drawers": total_drawers,
            "wings": wings,
            "rooms": rooms,
            "palace_path": self.palace_path,
            "collection_name": self.config.collection_name(),
            "protocol": PALACE_PROTOCOL,
            "aaak_dialect": AAAK_SPEC,
        }))
    }

    fn tool_list_wings(&self) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        let taxonomy = self.collect_taxonomy(&store)?;
        let wings = taxonomy
            .into_iter()
            .map(|(wing, rooms)| (wing, rooms.values().copied().sum::<usize>()))
            .collect::<BTreeMap<_, _>>();
        Ok(json!({ "wings": wings }))
    }

    fn tool_list_rooms(&self, wing: Option<&str>) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        let taxonomy = self.collect_taxonomy(&store)?;
        let rooms = if let Some(wing) = wing {
            taxonomy.get(wing).cloned().unwrap_or_default()
        } else {
            let mut rooms = BTreeMap::<String, usize>::new();
            for room_counts in taxonomy.values() {
                for (room, count) in room_counts {
                    *rooms.entry(room.clone()).or_default() += count;
                }
            }
            rooms
        };
        Ok(json!({ "wing": wing.unwrap_or("all"), "rooms": rooms }))
    }

    fn tool_get_taxonomy(&self) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        Ok(json!({ "taxonomy": self.collect_taxonomy(&store)? }))
    }

    fn tool_search(&self, query: &str, limit: usize, wing: Option<&str>, room: Option<&str>) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        Ok(json!(search_memories(&store, query, wing, room, limit)?))
    }

    fn tool_check_duplicate(&self, content: &str, threshold: f64) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        let candidates = store.list_drawers(None, None)?;
        let matches = candidates
            .into_iter()
            .filter_map(|drawer| {
                let similarity = similarity(content, &drawer.content);
                (similarity >= threshold).then(|| {
                    json!({
                        "id": drawer.id,
                        "wing": drawer.wing,
                        "room": drawer.room,
                        "similarity": similarity,
                        "content": if drawer.content.len() > 200 { format!("{}...", &drawer.content[..200]) } else { drawer.content }
                    })
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "is_duplicate": !matches.is_empty(),
            "matches": matches
        }))
    }

    fn tool_traverse(&self, start_room: &str, max_hops: usize) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        Ok(json!(traverse(&store, start_room, max_hops)?))
    }

    fn tool_find_tunnels(&self, wing_a: Option<&str>, wing_b: Option<&str>) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        Ok(json!(find_tunnels(&store, wing_a, wing_b)?))
    }

    fn tool_graph_stats(&self) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        Ok(json!(graph_stats(&store)?))
    }

    fn tool_add_drawer(
        &self,
        wing: &str,
        room: &str,
        content: &str,
        source_file: Option<&str>,
        added_by: &str,
    ) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        let duplicate = self.tool_check_duplicate(content, 0.9)?;
        if duplicate.get("is_duplicate").and_then(Value::as_bool) == Some(true) {
            return Ok(json!({
                "success": false,
                "reason": "duplicate",
                "matches": duplicate.get("matches").cloned().unwrap_or_else(|| json!([]))
            }));
        }

        let drawer = NewDrawer {
            id: format!("drawer_{}", crate::miner::drawer_id(wing, room, content, 0)),
            wing: wing.to_string(),
            room: room.to_string(),
            source_file: source_file.unwrap_or("").to_string(),
            chunk_index: 0,
            added_by: added_by.to_string(),
            filed_at: crate::miner::filed_at(),
            content: content.to_string(),
            ingest_mode: Some("mcp".to_string()),
            extract_mode: None,
            hall: None,
            topic: None,
            drawer_type: None,
            date: None,
        };
        let success = store.insert_drawer(&drawer)?;
        Ok(json!({
            "success": success,
            "drawer_id": drawer.id,
            "wing": wing,
            "room": room
        }))
    }

    fn tool_delete_drawer(&self, drawer_id: &str) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        let success = store.delete_drawer(drawer_id)?;
        Ok(json!({ "success": success, "drawer_id": drawer_id }))
    }

    fn tool_kg_query(&self, entity: &str, as_of: Option<&str>, direction: &str) -> Result<Value> {
        let direction = match direction {
            "outgoing" => QueryDirection::Outgoing,
            "incoming" => QueryDirection::Incoming,
            _ => QueryDirection::Both,
        };
        let facts = self.kg.query_entity(entity, as_of, direction)?;
        Ok(json!({ "entity": entity, "as_of": as_of, "facts": facts, "count": facts.len() }))
    }

    fn tool_kg_add(&self, subject: &str, predicate: &str, object: &str, valid_from: Option<&str>) -> Result<Value> {
        let triple_id = self
            .kg
            .add_triple(subject, predicate, object, valid_from, None, 1.0, None)?;
        Ok(json!({ "success": true, "triple_id": triple_id }))
    }

    fn tool_kg_invalidate(&self, subject: &str, predicate: &str, object: &str, ended: Option<&str>) -> Result<Value> {
        self.kg.invalidate(subject, predicate, object, ended)?;
        Ok(json!({ "success": true }))
    }

    fn tool_kg_timeline(&self, entity: Option<&str>) -> Result<Value> {
        let timeline = self.kg.timeline(entity)?;
        Ok(json!({ "entity": entity.unwrap_or("all"), "timeline": timeline, "count": timeline.len() }))
    }

    fn tool_diary_write(&self, agent_name: &str, entry: &str, topic: &str) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        let wing = format!("wing_{}", agent_name.to_ascii_lowercase().replace(' ', "_"));
        let drawer = NewDrawer {
            id: format!("diary_{}", crate::miner::drawer_id(&wing, "diary", entry, 0)),
            wing: wing.clone(),
            room: "diary".to_string(),
            source_file: String::new(),
            chunk_index: 0,
            added_by: agent_name.to_string(),
            filed_at: crate::miner::filed_at(),
            content: entry.to_string(),
            ingest_mode: Some("diary".to_string()),
            extract_mode: None,
            hall: Some("hall_diary".to_string()),
            topic: Some(topic.to_string()),
            drawer_type: Some("diary_entry".to_string()),
            date: Some(crate::miner::filed_at()),
        };
        store.insert_drawer(&drawer)?;
        Ok(json!({ "success": true, "entry_id": drawer.id, "agent": agent_name, "topic": topic }))
    }

    fn tool_diary_read(&self, agent_name: &str, last_n: usize) -> Result<Value> {
        let store = PalaceStore::open(&self.palace_path)?;
        let wing = format!("wing_{}", agent_name.to_ascii_lowercase().replace(' ', "_"));
        let mut entries = store
            .list_drawers(Some(&wing), Some("diary"))?
            .into_iter()
            .map(|drawer| {
                json!({
                    "entry_id": drawer.id,
                    "timestamp": drawer.filed_at,
                    "topic": drawer.topic.unwrap_or_else(|| "general".to_string()),
                    "content": drawer.content
                })
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| right["timestamp"].as_str().cmp(&left["timestamp"].as_str()));
        entries.truncate(last_n);
        Ok(json!({
            "agent": agent_name,
            "entries": entries,
            "showing": entries.len()
        }))
    }

    fn collect_taxonomy(&self, store: &PalaceStore) -> Result<BTreeMap<String, BTreeMap<String, usize>>> {
        let drawers = store.list_drawers(None, None)?;
        let mut taxonomy = BTreeMap::<String, BTreeMap<String, usize>>::new();
        for drawer in drawers {
            *taxonomy
                .entry(drawer.wing)
                .or_default()
                .entry(drawer.room)
                .or_default() += 1;
        }
        Ok(taxonomy)
    }

    fn tool_descriptions(&self) -> Vec<Value> {
        [
            ("mempalace_status", "Palace overview — total drawers, wing and room counts"),
            ("mempalace_list_wings", "List all wings with drawer counts"),
            ("mempalace_list_rooms", "List rooms within a wing"),
            ("mempalace_get_taxonomy", "Full taxonomy: wing → room → drawer count"),
            ("mempalace_search", "Search the palace"),
            ("mempalace_check_duplicate", "Check whether content already exists"),
            ("mempalace_get_aaak_spec", "Get the AAAK dialect specification"),
            ("mempalace_traverse", "Walk the palace graph from a room"),
            ("mempalace_find_tunnels", "Find rooms that bridge two wings"),
            ("mempalace_graph_stats", "Palace graph overview"),
            ("mempalace_add_drawer", "Add a drawer"),
            ("mempalace_delete_drawer", "Delete a drawer"),
            ("mempalace_kg_query", "Query the knowledge graph"),
            ("mempalace_kg_add", "Add a knowledge graph fact"),
            ("mempalace_kg_invalidate", "Invalidate a knowledge graph fact"),
            ("mempalace_kg_timeline", "Get knowledge graph timeline"),
            ("mempalace_kg_stats", "Get knowledge graph stats"),
            ("mempalace_diary_write", "Write an agent diary entry"),
            ("mempalace_diary_read", "Read recent diary entries"),
        ]
        .into_iter()
        .map(|(name, description)| json!({"name": name, "description": description, "inputSchema": {"type":"object","properties":{}}}))
        .collect()
    }
}

fn similarity(left: &str, right: &str) -> f64 {
    let left = left.to_ascii_lowercase();
    let right = right.to_ascii_lowercase();
    if left == right {
        return 1.0;
    }
    let left_terms = left.split_whitespace().collect::<std::collections::BTreeSet<_>>();
    let right_terms = right.split_whitespace().collect::<std::collections::BTreeSet<_>>();
    let intersection = left_terms.intersection(&right_terms).count();
    let union = left_terms.union(&right_terms).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

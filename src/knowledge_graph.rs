use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryDirection {
    Outgoing,
    Incoming,
    Both,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fact {
    pub direction: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub confidence: f64,
    pub source_file: Option<String>,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphStats {
    pub entities: usize,
    pub triples: usize,
    pub current_facts: usize,
    pub expired_facts: usize,
    pub relationship_types: Vec<String>,
}

pub struct KnowledgeGraph {
    db_path: PathBuf,
}

impl KnowledgeGraph {
    pub fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let graph = Self {
            db_path: path.to_path_buf(),
        };
        graph.init_db()?;
        Ok(graph)
    }

    pub fn add_triple(
        &self,
        subject: &str,
        predicate: &str,
        object: &str,
        valid_from: Option<&str>,
        valid_to: Option<&str>,
        confidence: f64,
        source_file: Option<&str>,
    ) -> Result<String> {
        let subject_id = entity_id(subject);
        let object_id = entity_id(object);
        let predicate = predicate.to_ascii_lowercase().replace(' ', "_");
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO entities (id, name) VALUES (?1, ?2)",
            params![subject_id, subject],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO entities (id, name) VALUES (?1, ?2)",
            params![object_id, object],
        )?;

        if let Ok(existing) = conn.query_row(
            "SELECT id FROM triples WHERE subject = ?1 AND predicate = ?2 AND object = ?3 AND valid_to IS NULL",
            params![subject_id, predicate, object_id],
            |row| row.get::<_, String>(0),
        ) {
            return Ok(existing);
        }

        let triple_id = format!("t_{}_{}_{}_{}", subject_id, predicate, object_id, random_suffix());
        conn.execute(
            "INSERT INTO triples (
                id, subject, predicate, object, valid_from, valid_to, confidence, source_file
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                triple_id,
                subject_id,
                predicate,
                object_id,
                valid_from,
                valid_to,
                confidence,
                source_file
            ],
        )?;
        Ok(triple_id)
    }

    pub fn invalidate(
        &self,
        subject: &str,
        predicate: &str,
        object: &str,
        ended: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE triples SET valid_to = ?1 WHERE subject = ?2 AND predicate = ?3 AND object = ?4 AND valid_to IS NULL",
            params![
                ended.unwrap_or("today"),
                entity_id(subject),
                predicate.to_ascii_lowercase().replace(' ', "_"),
                entity_id(object)
            ],
        )?;
        Ok(())
    }

    pub fn query_entity(
        &self,
        name: &str,
        as_of: Option<&str>,
        direction: QueryDirection,
    ) -> Result<Vec<Fact>> {
        let entity_id = entity_id(name);
        let conn = self.conn()?;
        let mut facts = Vec::new();

        if matches!(direction, QueryDirection::Outgoing | QueryDirection::Both) {
            let mut stmt = conn.prepare(
                "SELECT t.predicate, o.name, t.valid_from, t.valid_to, t.confidence, t.source_file
                 FROM triples t
                 JOIN entities o ON t.object = o.id
                 WHERE t.subject = ?1",
            )?;
            let rows = stmt.query_map([entity_id.as_str()], |row| {
                Ok(Fact {
                    direction: "outgoing".to_string(),
                    subject: name.to_string(),
                    predicate: row.get(0)?,
                    object: row.get(1)?,
                    valid_from: row.get(2)?,
                    valid_to: row.get(3)?,
                    confidence: row.get(4)?,
                    source_file: row.get(5)?,
                    current: false,
                })
            })?;
            for row in rows {
                let mut fact = row?;
                fact.current = fact.valid_to.is_none();
                if valid_at(&fact, as_of) {
                    facts.push(fact);
                }
            }
        }

        if matches!(direction, QueryDirection::Incoming | QueryDirection::Both) {
            let mut stmt = conn.prepare(
                "SELECT t.predicate, s.name, t.valid_from, t.valid_to, t.confidence, t.source_file
                 FROM triples t
                 JOIN entities s ON t.subject = s.id
                 WHERE t.object = ?1",
            )?;
            let rows = stmt.query_map([entity_id.as_str()], |row| {
                Ok(Fact {
                    direction: "incoming".to_string(),
                    subject: row.get(1)?,
                    predicate: row.get(0)?,
                    object: name.to_string(),
                    valid_from: row.get(2)?,
                    valid_to: row.get(3)?,
                    confidence: row.get(4)?,
                    source_file: row.get(5)?,
                    current: false,
                })
            })?;
            for row in rows {
                let mut fact = row?;
                fact.current = fact.valid_to.is_none();
                if valid_at(&fact, as_of) {
                    facts.push(fact);
                }
            }
        }

        Ok(facts)
    }

    pub fn timeline(&self, entity_name: Option<&str>) -> Result<Vec<Fact>> {
        let conn = self.conn()?;
        let base = "SELECT s.name, t.predicate, o.name, t.valid_from, t.valid_to, t.confidence, t.source_file
                    FROM triples t
                    JOIN entities s ON t.subject = s.id
                    JOIN entities o ON t.object = o.id";
        let sql = if entity_name.is_some() {
            format!("{base} WHERE t.subject = ?1 OR t.object = ?1 ORDER BY t.valid_from ASC")
        } else {
            format!("{base} ORDER BY t.valid_from ASC")
        };
        let mut stmt = conn.prepare(&sql)?;
        let mut facts = Vec::new();
        if let Some(entity_name) = entity_name {
            let rows = stmt.query_map([entity_id(entity_name)], map_fact_row)?;
            for row in rows {
                let mut fact = row?;
                fact.current = fact.valid_to.is_none();
                facts.push(fact);
            }
        } else {
            let rows = stmt.query_map([], map_fact_row)?;
            for row in rows {
                let mut fact = row?;
                fact.current = fact.valid_to.is_none();
                facts.push(fact);
            }
        }
        Ok(facts)
    }

    pub fn stats(&self) -> Result<GraphStats> {
        let conn = self.conn()?;
        let entities = conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get::<_, i64>(0))?;
        let triples = conn.query_row("SELECT COUNT(*) FROM triples", [], |row| row.get::<_, i64>(0))?;
        let current = conn.query_row(
            "SELECT COUNT(*) FROM triples WHERE valid_to IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let mut stmt = conn.prepare("SELECT DISTINCT predicate FROM triples ORDER BY predicate")?;
        let predicates = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(GraphStats {
            entities: entities as usize,
            triples: triples as usize,
            current_facts: current as usize,
            expired_facts: (triples - current) as usize,
            relationship_types: predicates,
        })
    }

    fn init_db(&self) -> Result<()> {
        let conn = self.conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entities (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS triples (
                id TEXT PRIMARY KEY,
                subject TEXT NOT NULL,
                predicate TEXT NOT NULL,
                object TEXT NOT NULL,
                valid_from TEXT,
                valid_to TEXT,
                confidence REAL NOT NULL DEFAULT 1.0,
                source_file TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_triples_subject ON triples(subject);
            CREATE INDEX IF NOT EXISTS idx_triples_object ON triples(object);
            CREATE INDEX IF NOT EXISTS idx_triples_predicate ON triples(predicate);",
        )?;
        Ok(())
    }

    fn conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }
}

fn entity_id(name: &str) -> String {
    name.to_ascii_lowercase()
        .replace(' ', "_")
        .replace('\'', "")
}

fn valid_at(fact: &Fact, as_of: Option<&str>) -> bool {
    let Some(as_of) = as_of else {
        return true;
    };
    let valid_from_ok = fact
        .valid_from
        .as_deref()
        .map(|valid_from| valid_from <= as_of)
        .unwrap_or(true);
    let valid_to_ok = fact
        .valid_to
        .as_deref()
        .map(|valid_to| valid_to >= as_of)
        .unwrap_or(true);
    valid_from_ok && valid_to_ok
}

fn random_suffix() -> String {
    use std::hash::{Hash, Hasher};
    let now = std::time::SystemTime::now();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    now.hash(&mut hasher);
    format!("{:08x}", hasher.finish())
}

fn map_fact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Fact> {
    Ok(Fact {
        direction: "timeline".to_string(),
        subject: row.get(0)?,
        predicate: row.get(1)?,
        object: row.get(2)?,
        valid_from: row.get(3)?,
        valid_to: row.get(4)?,
        confidence: row.get(5)?,
        source_file: row.get(6)?,
        current: false,
    })
}

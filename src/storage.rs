use std::path::Path;

use rusqlite::{Connection, params};

use crate::Result;

#[derive(Debug, Clone)]
pub struct NewDrawer {
    pub id: String,
    pub wing: String,
    pub room: String,
    pub source_file: String,
    pub chunk_index: usize,
    pub added_by: String,
    pub filed_at: String,
    pub content: String,
    pub ingest_mode: Option<String>,
    pub extract_mode: Option<String>,
    pub hall: Option<String>,
    pub topic: Option<String>,
    pub drawer_type: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredDrawer {
    pub id: String,
    pub wing: String,
    pub room: String,
    pub source_file: String,
    pub content: String,
    pub filed_at: String,
    pub hall: Option<String>,
    pub topic: Option<String>,
    pub drawer_type: Option<String>,
    pub date: Option<String>,
}

pub struct PalaceStore {
    conn: Connection,
}

impl PalaceStore {
    pub fn open(palace_path: &Path) -> Result<Self> {
        std::fs::create_dir_all(palace_path)?;
        let conn = Connection::open(palace_path.join("drawers.sqlite3"))?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn drawer_count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM drawers", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn source_file_exists(&self, source_file: &str) -> Result<bool> {
        let exists: i64 = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM drawers WHERE source_file = ?1 LIMIT 1)",
            [source_file],
            |row| row.get(0),
        )?;
        Ok(exists == 1)
    }

    pub fn insert_drawer(&self, drawer: &NewDrawer) -> Result<bool> {
        let changed = self.conn.execute(
            "INSERT OR IGNORE INTO drawers (
                id, wing, room, source_file, chunk_index, added_by, filed_at, content, ingest_mode, extract_mode, hall, topic, drawer_type, date
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                drawer.id,
                drawer.wing,
                drawer.room,
                drawer.source_file,
                drawer.chunk_index as i64,
                drawer.added_by,
                drawer.filed_at,
                drawer.content,
                drawer.ingest_mode,
                drawer.extract_mode,
                drawer.hall,
                drawer.topic,
                drawer.drawer_type,
                drawer.date,
            ],
        )?;
        Ok(changed > 0)
    }

    pub fn delete_drawer(&self, drawer_id: &str) -> Result<bool> {
        let changed = self
            .conn
            .execute("DELETE FROM drawers WHERE id = ?1", [drawer_id])?;
        Ok(changed > 0)
    }

    pub fn list_drawers(&self, wing: Option<&str>, room: Option<&str>) -> Result<Vec<StoredDrawer>> {
        let mut sql =
            "SELECT id, wing, room, source_file, content, filed_at, hall, topic, drawer_type, date FROM drawers WHERE 1=1".to_string();
        let mut values = Vec::new();
        if let Some(wing) = wing {
            sql.push_str(" AND wing = ?");
            values.push(wing.to_string());
        }
        if let Some(room) = room {
            sql.push_str(" AND room = ?");
            values.push(room.to_string());
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values.iter()), |row| {
            Ok(StoredDrawer {
                id: row.get(0)?,
                wing: row.get(1)?,
                room: row.get(2)?,
                source_file: row.get(3)?,
                content: row.get(4)?,
                filed_at: row.get(5)?,
                hall: row.get(6)?,
                topic: row.get(7)?,
                drawer_type: row.get(8)?,
                date: row.get(9)?,
            })
        })?;

        let mut drawers = Vec::new();
        for row in rows {
            drawers.push(row?);
        }
        Ok(drawers)
    }

    pub fn status_counts(&self) -> Result<Vec<(String, String, usize)>> {
        let mut stmt = self.conn.prepare(
            "SELECT wing, room, COUNT(*) FROM drawers GROUP BY wing, room ORDER BY wing ASC, COUNT(*) DESC, room ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as usize,
            ))
        })?;

        let mut counts = Vec::new();
        for row in rows {
            counts.push(row?);
        }
        Ok(counts)
    }

    pub fn upsert_compressed_drawer(
        &self,
        drawer_id: &str,
        wing: &str,
        room: &str,
        source_file: &str,
        content: &str,
        compression_ratio: f64,
        original_tokens: usize,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO compressed_drawers (
                drawer_id, wing, room, source_file, content, compression_ratio, original_tokens
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                drawer_id,
                wing,
                room,
                source_file,
                content,
                compression_ratio,
                original_tokens as i64
            ],
        )?;
        Ok(())
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS drawers (
                id TEXT PRIMARY KEY,
                wing TEXT NOT NULL,
                room TEXT NOT NULL,
                source_file TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                added_by TEXT NOT NULL,
                filed_at TEXT NOT NULL,
                content TEXT NOT NULL,
                ingest_mode TEXT,
                extract_mode TEXT,
                hall TEXT,
                topic TEXT,
                drawer_type TEXT,
                date TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_drawers_source_file ON drawers(source_file);
            CREATE INDEX IF NOT EXISTS idx_drawers_wing_room ON drawers(wing, room);
            CREATE TABLE IF NOT EXISTS compressed_drawers (
                drawer_id TEXT PRIMARY KEY,
                wing TEXT NOT NULL,
                room TEXT NOT NULL,
                source_file TEXT NOT NULL,
                content TEXT NOT NULL,
                compression_ratio REAL NOT NULL,
                original_tokens INTEGER NOT NULL
            );",
        )?;
        self.add_column_if_missing("drawers", "hall", "TEXT")?;
        self.add_column_if_missing("drawers", "topic", "TEXT")?;
        self.add_column_if_missing("drawers", "drawer_type", "TEXT")?;
        self.add_column_if_missing("drawers", "date", "TEXT")?;
        Ok(())
    }

    fn add_column_if_missing(&self, table: &str, column: &str, definition: &str) -> Result<()> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let mut existing = Vec::new();
        for row in rows {
            existing.push(row?);
        }
        if !existing.iter().any(|name| name == column) {
            self.conn
                .execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"), [])?;
        }
        Ok(())
    }
}

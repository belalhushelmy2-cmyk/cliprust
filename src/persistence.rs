use crate::{
    model::{Bundle, Entry},
    search::{self, Weights},
    snippets::Snippet,
};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

pub struct Store {
    conn: Connection,
}
#[derive(Default, Clone)]
pub struct Filter {
    pub kind: Option<String>,
    pub pinned: bool,
    pub source: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
}
#[derive(Serialize, Deserialize)]
pub struct Archive {
    pub version: u32,
    pub entries: Vec<(Entry, Bundle)>,
    pub snippets: Vec<Snippet>,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path).context("open history database")?;
        conn.busy_timeout(Duration::from_secs(3))?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;",
        )?;
        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            bail!("database was created by a newer Cliprust version");
        }
        if version == 0 {
            let tx = conn.transaction()?;
            tx.execute_batch("CREATE TABLE entries (
                id INTEGER PRIMARY KEY, identity TEXT NOT NULL UNIQUE, kind TEXT NOT NULL, preview TEXT NOT NULL,
                created_at INTEGER NOT NULL, last_used INTEGER NOT NULL, copy_count INTEGER NOT NULL DEFAULT 1,
                pinned INTEGER NOT NULL DEFAULT 0, pin_order INTEGER NOT NULL DEFAULT 0, source TEXT);
                CREATE TABLE formats (entry_id INTEGER NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
                mime TEXT NOT NULL, data BLOB NOT NULL, PRIMARY KEY(entry_id,mime));
                CREATE INDEX history_recent ON entries(last_used DESC);
                CREATE VIRTUAL TABLE history_fts USING fts5(preview, content='entries', content_rowid='id', tokenize='unicode61');
                CREATE TRIGGER entries_ai AFTER INSERT ON entries BEGIN INSERT INTO history_fts(rowid,preview) VALUES(new.id,new.preview); END;
                CREATE TRIGGER entries_ad AFTER DELETE ON entries BEGIN INSERT INTO history_fts(history_fts,rowid,preview) VALUES('delete',old.id,old.preview); END;
                CREATE TRIGGER entries_au AFTER UPDATE OF preview ON entries BEGIN
                INSERT INTO history_fts(history_fts,rowid,preview) VALUES('delete',old.id,old.preview);
                INSERT INTO history_fts(rowid,preview) VALUES(new.id,new.preview); END;
                CREATE TABLE snippets(name TEXT PRIMARY KEY, json TEXT NOT NULL);
                CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                PRAGMA user_version=1;")?;
            tx.commit()?;
        }
        Ok(Self { conn })
    }
    pub fn memory() -> Result<Self> {
        Self::open(Path::new(":memory:"))
    }
    pub fn capture(&mut self, bundle: &Bundle, source: Option<&str>, now: i64) -> Result<i64> {
        let id = self.capture_identity(bundle, source, now, &bundle.identity())?;
        self.set_setting("latest_captured_id", &id.to_string())?;
        Ok(id)
    }
    fn capture_identity(
        &mut self,
        bundle: &Bundle,
        source: Option<&str>,
        now: i64,
        identity: &str,
    ) -> Result<i64> {
        bundle.validate()?;
        let tx = self.conn.savepoint()?;
        tx.execute("INSERT INTO entries(identity,kind,preview,created_at,last_used,source) VALUES(?1,?2,?3,?4,?4,?5)
            ON CONFLICT(identity) DO UPDATE SET last_used=excluded.last_used,copy_count=entries.copy_count+1,source=COALESCE(excluded.source,entries.source)",
            params![identity,bundle.kind(),bundle.summary(),now,source])?;
        let id = tx.query_row(
            "SELECT id FROM entries WHERE identity=?1",
            [&identity],
            |r| r.get(0),
        )?;
        for (mime, data) in &bundle.formats {
            tx.execute(
                "INSERT OR IGNORE INTO formats VALUES(?1,?2,?3)",
                params![id, mime, data],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }
    pub fn duplicate(&mut self, id: i64, now: i64) -> Result<i64> {
        let bundle = self.bundle(id)?;
        let source: Option<String> =
            self.conn
                .query_row("SELECT source FROM entries WHERE id=?1", [id], |row| {
                    row.get(0)
                })?;
        let suffix: String =
            self.conn
                .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))?;
        let identity = format!("{}:copy:{suffix}", bundle.identity());
        self.capture_identity(&bundle, source.as_deref(), now, &identity)
    }
    fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
        Ok(Entry {
            id: row.get(0)?,
            identity: row.get(1)?,
            kind: row.get(2)?,
            preview: row.get(3)?,
            created_at: row.get(4)?,
            last_used: row.get(5)?,
            copy_count: row.get(6)?,
            pinned: row.get(7)?,
            pin_order: row.get(8)?,
            source: row.get(9)?,
        })
    }
    pub fn list(&self, query: &str, filter: &Filter, now: i64, limit: usize) -> Result<Vec<Entry>> {
        // FTS candidates plus bounded recency/favorite candidates give typo matching without scanning blobs.
        let normalized = search::normalize(query);
        let fts = normalized
            .split_whitespace()
            .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" AND ");
        let mut entries = Vec::new();
        // Apply filters before the candidate cap so old matching entries are
        // not crowded out by recent entries from other apps/types/dates.
        let predicates = "(?1 = 0 OR e.pinned = 1)
            AND (?2 IS NULL OR e.kind = ?2)
            AND (?3 IS NULL OR e.source = ?3)
            AND (?4 IS NULL OR e.created_at >= ?4)
            AND (?5 IS NULL OR e.created_at < ?5)";
        if !fts.is_empty() {
            let mut stmt = self.conn.prepare(&format!("SELECT e.* FROM entries e JOIN history_fts f ON e.id=f.rowid WHERE {predicates} AND history_fts MATCH ?6 ORDER BY rank, e.id LIMIT 1000"))?;
            entries.extend(
                stmt.query_map(
                    params![
                        filter.pinned,
                        filter.kind,
                        filter.source,
                        filter.since,
                        filter.until,
                        fts
                    ],
                    Self::row,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?,
            );
        }
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT e.* FROM entries e WHERE {predicates} ORDER BY e.pinned DESC, CASE WHEN e.pinned = 1 THEN e.pin_order END ASC, e.last_used DESC, e.id DESC LIMIT 1000"))?;
        entries.extend(
            stmt.query_map(
                params![
                    filter.pinned,
                    filter.kind,
                    filter.source,
                    filter.since,
                    filter.until
                ],
                Self::row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        );
        entries.sort_by_key(|e| e.id);
        entries.dedup_by_key(|e| e.id);
        let weights = self.search_weights();
        let mut ranked = search::rank(entries, query, now, &weights);
        if query.trim().is_empty() {
            ranked.sort_by(|a, b| {
                b.pinned
                    .cmp(&a.pinned)
                    .then_with(|| {
                        if a.pinned && b.pinned {
                            a.pin_order.cmp(&b.pin_order)
                        } else {
                            b.last_used.cmp(&a.last_used)
                        }
                    })
                    .then_with(|| b.id.cmp(&a.id))
            });
        }
        ranked.truncate(limit.min(2000));
        Ok(ranked)
    }
    /// Metadata only: blobs stay in SQLite while an open history session is frozen.
    pub fn history_snapshot(&self) -> Result<Vec<Entry>> {
        let mut statement = self.conn.prepare("SELECT * FROM entries ORDER BY id")?;
        Ok(statement
            .query_map([], Self::row)?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn latest_captured_id(&self) -> Option<i64> {
        self.setting("latest_captured_id")
            .and_then(|id| id.parse().ok())
    }
    pub fn search_weights(&self) -> Weights {
        self.setting("search_weights")
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_default()
    }
    pub fn sources(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT source FROM entries WHERE source IS NOT NULL AND trim(source) <> '' ORDER BY source COLLATE NOCASE LIMIT 100",
        )?;
        Ok(stmt
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn bundle(&self, id: i64) -> Result<Bundle> {
        let mut stmt = self
            .conn
            .prepare("SELECT mime,data FROM formats WHERE entry_id=?1")?;
        let formats = stmt
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let bundle = Bundle { formats };
        bundle.validate()?;
        Ok(bundle)
    }
    pub fn pin(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE entries SET pin_order=CASE WHEN pinned=0 THEN COALESCE((SELECT MAX(pin_order)+1 FROM entries WHERE pinned=1),0) ELSE pin_order END,pinned=NOT pinned WHERE id=?1",
            [id],
        )?;
        Ok(())
    }
    pub fn move_pin(&self, id: i64, delta: isize) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM entries WHERE pinned=1 ORDER BY pin_order,id")?;
        let mut ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        let Some(current) = ids.iter().position(|pin_id| *pin_id == id) else {
            return Ok(false);
        };
        let Some(next) = current.checked_add_signed(delta).filter(|i| *i < ids.len()) else {
            return Ok(false);
        };
        ids.swap(current, next);
        let tx = self.conn.unchecked_transaction()?;
        for (order, pin_id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE entries SET pin_order=?2 WHERE id=?1",
                params![pin_id, order as i64],
            )?;
        }
        tx.commit()?;
        Ok(true)
    }
    pub fn used(&self, id: i64, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE entries SET last_used=?2 WHERE id=?1",
            params![id, now],
        )?;
        Ok(())
    }
    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM entries WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn clear_unpinned(&self) -> Result<()> {
        self.conn
            .execute("DELETE FROM entries WHERE pinned=0", [])?;
        Ok(())
    }
    pub fn retention(&self) -> (usize, u32) {
        (
            self.setting("max_entries")
                .and_then(|v| v.parse().ok())
                .unwrap_or(10000),
            self.setting("retention_days")
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        )
    }
    pub fn cleanup(&self, max: usize, days: u32, now: i64) -> Result<usize> {
        let mut count = 0;
        if days > 0 {
            count += self.conn.execute(
                "DELETE FROM entries WHERE pinned=0 AND last_used<?1",
                [now.saturating_sub(i64::from(days) * 86400)],
            )?;
        }
        count+=self.conn.execute("DELETE FROM entries WHERE pinned=0 AND id NOT IN (SELECT id FROM entries WHERE pinned=0 ORDER BY last_used DESC,id DESC LIMIT ?1)",[max as i64])?;
        Ok(count)
    }
    pub fn setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .ok()
    }
    /// Apply capture policy before inserting clipboard contents. Invalid policy fails closed.
    pub fn capture_allowed(&self, source: Option<&str>) -> bool {
        if self.setting("paused").as_deref() == Some("true") {
            return false;
        }
        let policy = self.setting("excluded_apps").unwrap_or_else(|| "[]".into());
        let Ok(excluded) = crate::security::parse_exclusions(&policy) else {
            return false;
        };
        !source.is_some_and(|source| excluded.iter().any(|app| app.eq_ignore_ascii_case(source)))
    }
    pub fn recent_searches(&self) -> Vec<String> {
        self.setting("recent_searches")
            .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|query| !query.trim().is_empty() && query.len() <= 512)
            .take(20)
            .collect()
    }
    /// Remember completed searches, avoiding a disk write for every keystroke.
    pub fn remember_search(&self, query: &str) -> Result<()> {
        let query = query.trim();
        if query.is_empty() || query.len() > 512 || !self.capture_allowed(None) {
            return Ok(());
        }
        let mut recent = self.recent_searches();
        if recent.first().is_some_and(|first| first == query) {
            return Ok(());
        }
        recent.retain(|previous| previous != query);
        recent.insert(0, query.to_owned());
        recent.truncate(20);
        self.set_setting("recent_searches", &serde_json::to_string(&recent)?)
    }
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute("INSERT INTO settings VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;
        Ok(())
    }
    pub fn save_snippet(&self, snippet: &Snippet) -> Result<()> {
        if snippet.name.trim().is_empty() || snippet.body.len() > 1024 * 1024 {
            bail!("invalid snippet name or body size");
        }
        self.conn.execute(
            "INSERT INTO snippets VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET json=excluded.json",
            params![snippet.name, serde_json::to_string(snippet)?],
        )?;
        Ok(())
    }
    pub fn snippets(&self) -> Result<Vec<Snippet>> {
        let mut stmt = self
            .conn
            .prepare("SELECT json FROM snippets ORDER BY name")?;
        stmt.query_map([], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect()
    }
    pub fn delete_snippet(&self, name: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM snippets WHERE name=?1", [name])?;
        Ok(())
    }
    pub fn export(&self) -> Result<Archive> {
        let mut stmt = self.conn.prepare("SELECT * FROM entries ORDER BY id")?;
        let entries = stmt
            .query_map([], Self::row)?
            .map(|e| {
                let e = e?;
                let bundle = self.bundle(e.id)?;
                Ok((e, bundle))
            })
            .collect::<Result<_>>()?;
        Ok(Archive {
            version: 1,
            entries,
            snippets: self.snippets()?,
        })
    }
    pub fn import(&mut self, archive: Archive) -> Result<()> {
        if archive.version != 1 || archive.entries.len() > 100_000 {
            bail!("unsupported or oversized archive");
        }
        for (_, bundle) in &archive.entries {
            bundle.validate()?;
        }
        // Validate before modifying; savepoint makes all imported entries atomic.
        self.conn.execute_batch("SAVEPOINT archive_import")?;
        let result = (|| {
            for (entry, bundle) in archive.entries {
                let base = bundle.identity();
                let prefix = format!("{base}:copy:");
                let identity = if entry.identity.strip_prefix(&prefix).is_some_and(|suffix| {
                    suffix.len() == 32 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
                }) {
                    entry.identity.as_str()
                } else {
                    base.as_str()
                };
                let id = self.capture_identity(
                    &bundle,
                    entry.source.as_deref(),
                    entry.last_used,
                    identity,
                )?;
                self.conn.execute("UPDATE entries SET pinned=MAX(pinned,?2),pin_order=?3,created_at=MIN(created_at,?4),copy_count=MAX(copy_count,?5) WHERE id=?1",params![id,entry.pinned,entry.pin_order,entry.created_at,entry.copy_count])?;
            }
            for snippet in archive.snippets {
                self.save_snippet(&snippet)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.conn.execute_batch("RELEASE archive_import")?,
            Err(e) => {
                self.conn
                    .execute_batch("ROLLBACK TO archive_import; RELEASE archive_import")?;
                return Err(e);
            }
        }
        Ok(())
    }
    pub fn backup(&self, path: &Path) -> Result<()> {
        self.conn.backup("main", path, None)?;
        Ok(())
    }
    /// Recover history and snippets without discarding newer local entries/settings.
    pub fn merge_backup(&mut self, path: &Path) -> Result<()> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version != 1 {
            bail!("Unsupported backup schema");
        }
        let source = Self { conn };
        if source.integrity()? != "ok" {
            bail!("Backup integrity check failed");
        }
        let mut archive = source.export()?;
        let existing: std::collections::HashSet<_> =
            self.snippets()?.into_iter().map(|s| s.name).collect();
        archive
            .snippets
            .retain(|snippet| !existing.contains(&snippet.name));
        for (entry, _) in &mut archive.entries {
            if let Some((last_used, source)) = self
                .conn
                .query_row(
                    "SELECT last_used,source FROM entries WHERE identity=?1",
                    [&entry.identity],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?
                && last_used > entry.last_used
            {
                entry.last_used = last_used;
                entry.source = source;
            }
        }
        self.import(archive)
    }
    pub fn integrity(&self) -> Result<String> {
        Ok(self
            .conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))?)
    }
}

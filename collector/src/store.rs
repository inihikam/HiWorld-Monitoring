//! SQLite store collector (Task D1): hosts, users, metrics_raw,
//! metrics_rollup, events. Schema versioned; operasi idempotent.

use std::path::Path;

use rusqlite::Connection;

use hiworld_core::models::Snapshot;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("serialisasi snapshot gagal: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Satu baris history system-level (untuk query UI/API).
#[derive(Debug, Clone)]
pub struct SystemRow {
    pub timestamp_ms: u64,
    pub cpu_percent: f64,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub mem_percent: f64,
    pub load_1m: f64,
}

/// Satu baris rollup (downsample).
#[derive(Debug, Clone)]
pub struct RollupRow {
    pub bucket_start_ms: u64,
    pub cpu_avg: f64,
    pub cpu_min: f64,
    pub cpu_max: f64,
}

/// Host terdaftar.
#[derive(Debug, Clone)]
pub struct HostRow {
    pub id: i64,
    pub host_id: String,
    pub agent_url: String,
    pub registered_at_ms: u64,
}

/// Satu event untuk API/UI.
pub type EventRow = (String, i64, String, String, String, String);

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Buka DB di path file (produksi).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// In-memory (test).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA_V1)?;
        Ok(Self { conn })
    }

    /// Daftar tabel (test helper — ARCH-AC-001 D1).
    pub fn table_names(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.flatten().collect())
    }

    // ---------- hosts ----------

    /// Register host — idempotent (ARCH-AC-021): host sama → id sama.
    pub fn register_host(&self, host_id: &str, agent_url: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO hosts (host_id, agent_url, registered_at_ms)
             VALUES (?1, ?2, (CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)))
             ON CONFLICT(host_id) DO UPDATE SET agent_url = excluded.agent_url",
            rusqlite::params![host_id, agent_url],
        )?;
        let id: i64 =
            self.conn
                .query_row("SELECT id FROM hosts WHERE host_id = ?1", [host_id], |r| {
                    r.get(0)
                })?;
        Ok(id)
    }

    pub fn list_hosts(&self) -> Result<Vec<HostRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, host_id, agent_url, registered_at_ms FROM hosts ORDER BY host_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(HostRow {
                id: r.get(0)?,
                host_id: r.get(1)?,
                agent_url: r.get(2)?,
                registered_at_ms: r.get(3)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn agent_url(&self, host_id: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT agent_url FROM hosts WHERE host_id = ?1")?;
        let mut rows = stmt.query_map([host_id], |r| r.get::<_, String>(0))?;
        Ok(rows.next().transpose()?)
    }

    /// Hapus host terdaftar (beserta metriknya).
    pub fn delete_host(&self, host_id: &str) -> Result<usize> {
        self.conn
            .execute("DELETE FROM hosts WHERE host_id = ?1", [host_id])?;
        Ok(self
            .conn
            .execute("DELETE FROM metrics_raw WHERE host_id = ?1", [host_id])?)
    }

    // ---------- metrics ----------

    /// Insert snapshot: 1 baris system + N baris proses (dalam 1 transaksi).
    pub fn insert_snapshot(&self, snap: &Snapshot) -> Result<()> {
        let sys = &snap.system;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO metrics_raw
             (host_id, timestamp_ms, interval_ms, cpu_percent, mem_total_bytes,
              mem_used_bytes, mem_percent, swap_used_bytes, load_1m, load_5m, load_15m,
              processes_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![
                snap.host_id,
                snap.timestamp_ms as i64,
                snap.interval_ms,
                sys.cpu_percent.unwrap_or(f64::NAN),
                sys.mem_total_bytes as i64,
                sys.mem_used_bytes as i64,
                sys.mem_percent,
                sys.swap_used_bytes as i64,
                sys.load_avg[0],
                sys.load_avg[1],
                sys.load_avg[2],
                serde_json::to_string(&snap.processes)?,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn query_system_history(
        &self,
        host_id: &str,
        from_ms: u64,
        to_ms: u64,
    ) -> Result<Vec<SystemRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT timestamp_ms, cpu_percent, mem_used_bytes, mem_total_bytes,
                    mem_percent, load_1m
             FROM metrics_raw
             WHERE host_id = ?1 AND timestamp_ms >= ?2 AND timestamp_ms <= ?3
             ORDER BY timestamp_ms",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![host_id, from_ms as i64, to_ms as i64],
            |r| {
                let cpu: f64 = r.get(1)?;
                Ok(SystemRow {
                    timestamp_ms: r.get::<_, i64>(0)? as u64,
                    cpu_percent: if cpu.is_nan() { f64::NAN } else { cpu },
                    mem_used_bytes: r.get::<_, i64>(2)? as u64,
                    mem_total_bytes: r.get::<_, i64>(3)? as u64,
                    mem_percent: r.get(4)?,
                    load_1m: r.get(5)?,
                })
            },
        )?;
        Ok(rows.flatten().collect())
    }

    pub fn latest_snapshot(&self, host_id: &str) -> Result<Option<SystemRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT timestamp_ms, cpu_percent, mem_used_bytes, mem_total_bytes,
                    mem_percent, load_1m
             FROM metrics_raw WHERE host_id = ?1
             ORDER BY timestamp_ms DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map([host_id], |r| {
            let cpu: f64 = r.get(1)?;
            Ok(SystemRow {
                timestamp_ms: r.get::<_, i64>(0)? as u64,
                cpu_percent: cpu,
                mem_used_bytes: r.get::<_, i64>(2)? as u64,
                mem_total_bytes: r.get::<_, i64>(3)? as u64,
                mem_percent: r.get(4)?,
                load_1m: r.get(5)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    /// Proses JSON pada timestamp tertentu (untuk "siapa penyebab spike?").
    pub fn processes_at(&self, host_id: &str, timestamp_ms: u64) -> Result<String> {
        let mut stmt = self.conn.prepare(
            "SELECT processes_json FROM metrics_raw
             WHERE host_id = ?1 AND timestamp_ms = ?2",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![host_id, timestamp_ms as i64], |r| {
            r.get::<_, String>(0)
        })?;
        Ok(rows.next().transpose()?.unwrap_or_else(|| "[]".into()))
    }

    /// Downsample: agregat metrics_raw ke bucket detik → metrics_rollup.
    pub fn downsample(&self, bucket_sec: u64) -> Result<usize> {
        let bucket_ms = (bucket_sec * 1000) as i64;
        let n = self.conn.execute(
            "INSERT INTO metrics_rollup
             (host_id, bucket_start_ms, cpu_avg, cpu_min, cpu_max,
              mem_used_avg, mem_used_max)
             SELECT host_id,
                    (timestamp_ms / ?2) * ?2 AS bucket,
                    AVG(cpu_percent), MIN(cpu_percent), MAX(cpu_percent),
                    AVG(mem_used_bytes), MAX(mem_used_bytes)
             FROM metrics_raw
             GROUP BY host_id, bucket
             ON CONFLICT(host_id, bucket_start_ms) DO UPDATE SET
                cpu_avg = excluded.cpu_avg,
                cpu_min = excluded.cpu_min,
                cpu_max = excluded.cpu_max,
                mem_used_avg = excluded.mem_used_avg,
                mem_used_max = excluded.mem_used_max",
            rusqlite::params![bucket_sec, bucket_ms],
        )?;
        Ok(n)
    }

    pub fn query_rollup(&self, host_id: &str, from_ms: u64, to_ms: u64) -> Result<Vec<RollupRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT bucket_start_ms, cpu_avg, cpu_min, cpu_max
             FROM metrics_rollup
             WHERE host_id = ?1 AND bucket_start_ms >= ?2 AND bucket_start_ms <= ?3
             ORDER BY bucket_start_ms",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![host_id, from_ms as i64, to_ms as i64],
            |r| {
                Ok(RollupRow {
                    bucket_start_ms: r.get::<_, i64>(0)? as u64,
                    cpu_avg: r.get(1)?,
                    cpu_min: r.get(2)?,
                    cpu_max: r.get(3)?,
                })
            },
        )?;
        Ok(rows.flatten().collect())
    }

    /// Hapus baris lebih tua dari cutoff_ms (retensi, ADR-6). Return jumlah baris.
    pub fn purge_older_than(&self, cutoff_ms: u64) -> Result<usize> {
        let mut total = self.conn.execute(
            "DELETE FROM metrics_raw WHERE timestamp_ms < ?1",
            [cutoff_ms as i64],
        )?;
        total += self.conn.execute(
            "DELETE FROM metrics_rollup WHERE bucket_start_ms < ?1",
            [cutoff_ms as i64],
        )?;
        Ok(total)
    }

    /// Insert event (spike, agent_down, dst — dipakai poller & detector).
    pub fn insert_event(
        &self,
        host_id: &str,
        kind: &str,
        severity: &str,
        subject: &str,
        detail: &serde_json::Value,
    ) -> Result<()> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO events (host_id, timestamp_ms, kind, severity, subject, detail_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![host_id, ts, kind, severity, subject, detail.to_string()],
        )?;
        Ok(())
    }

    pub fn list_events(&self, host_id: Option<&str>, limit: usize) -> Result<Vec<EventRow>> {
        let (sql, has_host) = if host_id.is_some() {
            (
                "SELECT host_id, timestamp_ms, kind, severity, subject, detail_json
                 FROM events WHERE host_id = ?1 ORDER BY timestamp_ms DESC LIMIT ?2",
                true,
            )
        } else {
            (
                "SELECT host_id, timestamp_ms, kind, severity, subject, detail_json
                 FROM events ORDER BY timestamp_ms DESC LIMIT ?1",
                false,
            )
        };
        let mut stmt = self.conn.prepare(sql)?;
        let map = |r: &rusqlite::Row| -> rusqlite::Result<_> {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        };
        let rows = if has_host {
            stmt.query_map(rusqlite::params![host_id.unwrap(), limit as i64], map)?
        } else {
            stmt.query_map([limit as i64], map)?
        };
        Ok(rows.flatten().collect())
    }

    // ---------- detector support (Task SD5) ----------

    /// Avg RSS (bytes) satu proses dalam window [from_ms, to_ms] + jumlah sample.
    /// Membaca processes_json per snapshot (JSON di-parsing di Rust — kompatibel
    /// semua build SQLite). Sample dengan rss 0 dikecualikan (anti div-zero).
    /// None = proses tidak ditemukan di window.
    pub fn avg_rss_baseline(
        &self,
        host_id: &str,
        pid: i32,
        from_ms: u64,
        to_ms: u64,
    ) -> Result<Option<(u64, u32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT processes_json FROM metrics_raw
             WHERE host_id = ?1 AND timestamp_ms >= ?2 AND timestamp_ms <= ?3
             ORDER BY timestamp_ms",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![host_id, from_ms as i64, to_ms as i64],
            |r| r.get::<_, String>(0),
        )?;

        let mut total: u64 = 0;
        let mut count: u32 = 0;
        for row in rows {
            let json = row?;
            let procs: Vec<hiworld_core::models::ProcessInfo> =
                serde_json::from_str(&json).map_err(StoreError::Serde)?;
            if let Some(p) = procs.iter().find(|p| p.pid == pid) {
                if p.mem_rss_bytes > 0 {
                    total += p.mem_rss_bytes;
                    count += 1;
                }
            }
        }
        if count == 0 {
            return Ok(None);
        }
        Ok(Some((total / count as u64, count)))
    }

    pub fn get_active_event(&self, key: &str) -> Result<Option<crate::detector::ActiveEvent>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, first_seen_ms, last_seen_ms FROM active_events WHERE key = ?1")?;
        let mut rows = stmt.query_map([key], |r| {
            Ok(crate::detector::ActiveEvent {
                key: r.get(0)?,
                first_seen_ms: r.get::<_, i64>(1)? as u64,
                last_seen_ms: r.get::<_, i64>(2)? as u64,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn insert_active_event(&self, key: &str, now_ms: u64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO active_events (key, first_seen_ms, last_seen_ms)
             VALUES (?1, ?2, ?2)",
            rusqlite::params![key, now_ms as i64],
        )?;
        Ok(())
    }

    pub fn touch_active_event(&self, key: &str, now_ms: u64) -> Result<()> {
        self.conn.execute(
            "UPDATE active_events SET last_seen_ms = ?2 WHERE key = ?1",
            rusqlite::params![key, now_ms as i64],
        )?;
        Ok(())
    }

    pub fn delete_active_event(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM active_events WHERE key = ?1", [key])?;
        Ok(())
    }

    pub fn gc_active_events(&self, older_than_ms: u64) -> Result<usize> {
        Ok(self.conn.execute(
            "DELETE FROM active_events WHERE last_seen_ms < ?1",
            [older_than_ms as i64],
        )?)
    }
}

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS hosts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_id TEXT NOT NULL UNIQUE,
    agent_url TEXT NOT NULL,
    registered_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,        -- bcrypt/argon2, BUKAN plaintext (ARCH-AC-023)
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS metrics_raw (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_id TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    interval_ms INTEGER NOT NULL,
    cpu_percent REAL,                   -- NULL = sample pertama (ADR-1)
    mem_total_bytes INTEGER NOT NULL,
    mem_used_bytes INTEGER NOT NULL,
    mem_percent REAL NOT NULL,
    swap_used_bytes INTEGER NOT NULL,
    load_1m REAL NOT NULL,
    load_5m REAL NOT NULL,
    load_15m REAL NOT NULL,
    processes_json TEXT NOT NULL,       -- top-N proses sebagai JSON
    UNIQUE(host_id, timestamp_ms)
);
CREATE INDEX IF NOT EXISTS idx_raw_host_ts ON metrics_raw(host_id, timestamp_ms);

CREATE TABLE IF NOT EXISTS metrics_rollup (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_id TEXT NOT NULL,
    bucket_start_ms INTEGER NOT NULL,
    cpu_avg REAL, cpu_min REAL, cpu_max REAL,
    mem_used_avg REAL, mem_used_max REAL,
    UNIQUE(host_id, bucket_start_ms)
);
CREATE INDEX IF NOT EXISTS idx_rollup_host_ts ON metrics_rollup(host_id, bucket_start_ms);

CREATE TABLE IF NOT EXISTS active_events (
    key TEXT PRIMARY KEY,
    first_seen_ms INTEGER NOT NULL,
    last_seen_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_active_events_last ON active_events(last_seen_ms);

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    host_id TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    kind TEXT NOT NULL,                 -- spike_cpu | spike_mem | disk_almost_full | agent_down | agent_up
    severity TEXT NOT NULL,             -- info | warning | critical
    subject TEXT NOT NULL,
    detail_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_host_ts ON events(host_id, timestamp_ms);

INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', '2');
"#;

// ---------- wiring trait detector (Opsi B) ----------

impl crate::detector::ActiveEventStore for &Store {
    fn get(&self, key: &str) -> Option<crate::detector::ActiveEvent> {
        (*self).get_active_event(key).ok().flatten()
    }
    fn insert(&self, key: &str, now_ms: u64) {
        let _ = (*self).insert_active_event(key, now_ms);
    }
    fn touch(&self, key: &str, now_ms: u64) {
        let _ = (*self).touch_active_event(key, now_ms);
    }
    fn delete(&self, key: &str) {
        let _ = (*self).delete_active_event(key);
    }
    fn gc(&self, older_than_ms: u64) {
        let _ = (*self).gc_active_events(older_than_ms);
    }
}

impl crate::detector::ActiveEventStore for Store {
    fn get(&self, key: &str) -> Option<crate::detector::ActiveEvent> {
        self.get_active_event(key).ok().flatten()
    }
    fn insert(&self, key: &str, now_ms: u64) {
        let _ = self.insert_active_event(key, now_ms);
    }
    fn touch(&self, key: &str, now_ms: u64) {
        let _ = self.touch_active_event(key, now_ms);
    }
    fn delete(&self, key: &str) {
        let _ = self.delete_active_event(key);
    }
    fn gc(&self, older_than_ms: u64) {
        let _ = self.gc_active_events(older_than_ms);
    }
}

/// BaselineFetcher produksi: baca langsung dari metrics_raw.
/// Memegang referensi ke Store (bukan clone — rusqlite !Sync, dipakai
/// di konteks yang sama dengan poller).
pub struct StoreBaseline<'a> {
    pub store: &'a Store,
    pub window_min: u32,
}

impl crate::detector::BaselineFetcher for StoreBaseline<'_> {
    fn avg_rss(&self, host_id: &str, pid: i32) -> Option<(u64, u32)> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let from = now.saturating_sub(self.window_min as u64 * 60_000);
        self.store
            .avg_rss_baseline(host_id, pid, from, now)
            .ok()
            .flatten()
    }
}

//! Auth UI collector (Task D2): kredensial di SQLite (Q4), bcrypt hash.
//!
//! Bootstrap: bila DB kosong (atau user bootstrap tidak ada) → buat dari
//! plaintext TOML sekali. Password bisa diganti via change_password
//! (butuh password lama). Plaintext TIDAK PERNAH disimpan (ARCH-AC-023).

use std::path::Path;

use rusqlite::Connection;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("password lama salah")]
    WrongOldPassword,
    #[error("password baru tidak boleh kosong")]
    EmptyPassword,
    #[error("password baru terlalu pendek (min 8 karakter)")]
    TooShort,
    #[error("hash error: {0}")]
    Hash(String),
}

impl From<bcrypt::BcryptError> for AuthError {
    fn from(e: bcrypt::BcryptError) -> Self {
        Self::Hash(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AuthError>;

pub struct Auth {
    conn: Connection,
}

const BCRYPT_COST: u32 = 10; // ~100ms di hardware 2026 — cukup untuk login UI

impl Auth {
    /// Buka di file DB (produksi) + bootstrap user awal.
    pub fn open(
        path: impl AsRef<Path>,
        bootstrap_user: &str,
        bootstrap_pass: &str,
    ) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        ensure_users_table(&conn)?;
        let auth = Self { conn };
        auth.bootstrap(bootstrap_user, bootstrap_pass)?;
        Ok(auth)
    }

    /// In-memory (test).
    pub fn bootstrap_in_memory(user: &str, pass: &str) -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        ensure_users_table(&conn)?;
        let auth = Self { conn };
        auth.bootstrap(user, pass)?;
        Ok(auth)
    }

    /// Buat user bila BELUM ada — idempotent, tidak menimpa password existing.
    pub fn bootstrap(&self, user: &str, pass: &str) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM users WHERE username = ?1",
            [user],
            |r| r.get(0),
        )?;
        if exists {
            return Ok(());
        }
        self.create_user(user, pass)
    }

    fn create_user(&self, user: &str, pass: &str) -> Result<()> {
        let hash = bcrypt::hash(pass, BCRYPT_COST)?;
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO users (username, password_hash, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?3)",
            rusqlite::params![user, hash, now],
        )?;
        Ok(())
    }

    /// Verifikasi login. User tidak dikenal → false (tetap lakukan hashing
    /// dummy agar timing tidak membocorkan keberadaan user).
    pub fn verify_login(&self, user: &str, pass: &str) -> Result<bool> {
        let stored: Option<String> = self
            .conn
            .query_row(
                "SELECT password_hash FROM users WHERE username = ?1",
                [user],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e: rusqlite::Error| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(AuthError::Db(other)),
            })?;

        match stored {
            Some(hash) => Ok(bcrypt::verify(pass, &hash)?),
            // hashing dummy dengan cost sama → durasi stabil
            None => {
                let _ = bcrypt::hash(pass, BCRYPT_COST)?;
                Ok(false)
            }
        }
    }

    /// Ganti password: verifikasi password lama, tulis hash baru.
    pub fn change_password(&self, user: &str, old_pass: &str, new_pass: &str) -> Result<()> {
        if new_pass.is_empty() {
            return Err(AuthError::EmptyPassword);
        }
        if new_pass.len() < 8 {
            return Err(AuthError::TooShort);
        }
        if !self.verify_login(user, old_pass)? {
            return Err(AuthError::WrongOldPassword);
        }
        let hash = bcrypt::hash(new_pass, BCRYPT_COST)?;
        self.conn.execute(
            "UPDATE users SET password_hash = ?2, updated_at_ms = ?3
             WHERE username = ?1",
            rusqlite::params![user, hash, now_ms()],
        )?;
        Ok(())
    }

    /// Baca kolom hash mentah (test helper: memastikan bukan plaintext).
    pub fn raw_password_column(&self, user: &str) -> Result<String> {
        self.conn
            .query_row(
                "SELECT password_hash FROM users WHERE username = ?1",
                [user],
                |r| r.get(0),
            )
            .map_err(AuthError::Db)
    }
}

fn ensure_users_table(conn: &Connection) -> std::result::Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );",
    )
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

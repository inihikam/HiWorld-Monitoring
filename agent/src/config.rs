use std::path::Path;

use serde::Deserialize;

/// Konfigurasi agent, di-load dari file TOML (lihat docs/specs/architecture.md §7.1).
#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub sampling: SamplingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub port: u16,
    pub auth_token: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SamplingConfig {
    /// Interval sampling dalam milidetik. Rentang valid: 1000..=60000.
    /// Nilai di luar rentang di-clamp saat validasi.
    pub interval_ms: u32,
    #[serde(default = "default_top_n")]
    pub top_n_processes: usize,
    #[serde(default)]
    pub collect_pss: bool,
}

fn default_top_n() -> usize {
    10
}

impl AgentConfig {
    /// Muat dan parse file konfigurasi.
    ///
    /// Error dirancang eksplisit: menyebut "config" untuk file yang hilang dan
    /// "parse" untuk TOML yang tidak valid (lihat ARCH-AC-003).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let cfg: AgentConfig = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.display().to_string(),
            source,
        })?;
        cfg.validated()
    }

    /// Validasi + clamp nilai di luar rentang (interval 1000..=60000 ms).
    fn validated(mut self) -> Result<Self, ConfigError> {
        self.sampling.interval_ms = self.sampling.interval_ms.clamp(1000, 60_000);
        if self.server.auth_token.is_empty() {
            return Err(ConfigError::EmptyToken);
        }
        Ok(self)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config file not readable: {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("config: auth_token tidak boleh kosong")]
    EmptyToken,
}

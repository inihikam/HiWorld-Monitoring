//! hiworld-core: shared models & /proc parsers for hiworld-monitoring.
//!
//! Crate ini HARUS tetap murni: tanpa tokio/axum/rusqlite/reqwest
//! (ARCH-AC-002). Semua I/O diabstraksi agar testable tanpa /proc asli.

pub mod models;

pub use models::{
    DiskMetrics, EventKind, MonitorEvent, NetMetrics, ProcessInfo, Severity, Snapshot,
    SystemMetrics,
};

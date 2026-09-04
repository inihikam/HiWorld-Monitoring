//! BroadcastHub (Task WS1): N-producer/M-consumer realtime fan-out.
//!
//! Wrapper tipis di atas `tokio::sync::broadcast` (ADR WS-1):
//! - poller tidak tahu jumlah klien (decoupled)
//! - klien lambat → pesan di-skip (Lagged), poller tidak terblokir
//! - tanpa subscriber → send sukses (poller aman saat dashboard tertutup)

use tokio::sync::broadcast;

use hiworld_core::models::Snapshot;

/// Pesan yang di-broadcast ke semua klien WS (JSON: {"type": ..., "data": ...}).
#[derive(Debug, Clone)]
pub enum BroadcastMessage {
    /// Snapshot baru tersimpan (per host, tiap poll).
    Snapshot(Snapshot),
    /// Event baru (spike_cpu/spike_mem/disk_almost_full/agent_down/agent_up).
    Event {
        host_id: String,
        kind: String,
        severity: String,
        subject: String,
        detail: serde_json::Value,
    },
    /// Transisi status host.
    HostStatus { host_id: String, online: bool },
}

#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("tidak ada subscriber aktif")]
    NoSubscribers,
}

/// Hub broadcast — murah di-clone (Arc internal), disimpan di AppState.
#[derive(Clone)]
pub struct BroadcastHub {
    tx: broadcast::Sender<BroadcastMessage>,
}

impl BroadcastHub {
    /// `capacity` = buffer per subscriber sebelum Lagged (WS-5: default 1024).
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(1));
        Self { tx }
    }

    /// Daftar sebagai subscriber baru (dipanggil per koneksi WS).
    pub fn subscribe(&self) -> broadcast::Receiver<BroadcastMessage> {
        self.tx.subscribe()
    }

    /// Broadcast ke semua subscriber aktif. Tanpa subscriber → sukses (Ok).
    pub fn send(&self, msg: BroadcastMessage) {
        // Err(NoReceivers) = tidak ada klien — bukan error bagi poller
        let _ = self.tx.send(msg);
    }

    /// Jumlah subscriber aktif (debug/metrics).
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

//! AlertBridge (Task TA4): filter severity → rate limit → retry backoff.
//! Murni + trait injectable (TelegramHttp + Clock) — 100% unit-testable.

use std::collections::VecDeque;

use crate::telegram::client::TelegramHttp;
use crate::telegram::{Severity, TelegramConfig};

/// Jam abstrak (pola RegistrarClock/Detector).
pub trait Clock {
    fn now_ms(&self) -> u64;

    /// Jeda nyata utk backoff. Default noop — SystemClock override.
    fn sleep_ms(&self, _ms: u64) {}
}

/// Jam nyata produksi.
pub struct SystemClock;
impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Dipanggil dalam spawn_blocking → thread::sleep aman (ADR TA-2).
    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// Backoff tetap (ADR TA-3): 2s → 4s → 8s.
const BACKOFF_MS: [u64; 3] = [2_000, 4_000, 8_000];

pub struct AlertBridge<H: TelegramHttp, C: Clock> {
    cfg: TelegramConfig,
    http: H,
    clock: C,
    /// timestamps pesan yang BERHASIL dikirim dalam window (sliding).
    sent_at: VecDeque<u64>,
}

/// Bridge versi async untuk produksi (poller async, ADR TA-2b).
pub struct AsyncAlertBridge {
    cfg: TelegramConfig,
    client: crate::telegram::client::TelegramClient,
    sent_at: VecDeque<u64>,
}

impl AsyncAlertBridge {
    pub fn new(cfg: TelegramConfig, client: crate::telegram::client::TelegramClient) -> Self {
        Self {
            cfg,
            client,
            sent_at: VecDeque::new(),
        }
    }

    fn admit(&mut self, severity: Severity, kind: &str, host: &str) -> bool {
        if severity < self.cfg.min_severity {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let window_start = now.saturating_sub(60_000);
        while let Some(&front) = self.sent_at.front() {
            if front < window_start {
                self.sent_at.pop_front();
            } else {
                break;
            }
        }
        if self.sent_at.len() as u32 >= self.cfg.max_per_minute {
            tracing::warn!("telegram rate limit: drop event {kind}/{host}");
            return false;
        }
        self.sent_at.push_back(now);
        true
    }

    pub async fn handle_async(
        &mut self,
        severity: Severity,
        kind: &str,
        host: &str,
        _subject: &str,
        text: &str,
    ) -> bool {
        if !self.admit(severity, kind, host) {
            return false;
        }
        for attempt in 0usize.. {
            match self
                .client
                .send_message_async(&self.cfg.chat_id, text)
                .await
            {
                Ok(()) => return true,
                Err(e) => {
                    tracing::warn!("telegram send gagal (percobaan {attempt}): {e}");
                    match crate::telegram::bridge::BACKOFF_MS.get(attempt) {
                        Some(&backoff) => {
                            tokio::time::sleep(std::time::Duration::from_millis(backoff)).await
                        }
                        None => {
                            tracing::error!("telegram give up setelah {} percobaan", attempt + 1);
                            return false;
                        }
                    }
                }
            }
        }
        false
    }
}

impl<H: TelegramHttp, C: Clock> AlertBridge<H, C> {
    pub fn new(cfg: TelegramConfig, http: H, clock: C) -> Self {
        Self {
            cfg,
            http,
            clock,
            sent_at: VecDeque::new(),
        }
    }

    /// Handle sync (unit test + konteks blocking). Return true bila terkirim.
    pub fn handle(
        &mut self,
        severity: Severity,
        kind: &str,
        host: &str,
        _subject: &str,
        text: &str,
    ) -> bool {
        // TA-AC-010/011: severity filter
        if severity < self.cfg.min_severity {
            return false;
        }

        // TA-AC-012: sliding window rate limit
        let now = self.clock.now_ms();
        let window_start = now.saturating_sub(60_000);
        while let Some(&front) = self.sent_at.front() {
            if front < window_start {
                self.sent_at.pop_front();
            } else {
                break;
            }
        }
        if self.sent_at.len() as u32 >= self.cfg.max_per_minute {
            // ADR TA-4: drop (tanpa antre), log oleh pemanggil
            tracing::warn!(
                "telegram rate limit: drop event {kind}/{host} ({} sent in window)",
                self.sent_at.len()
            );
            return false;
        }

        // kirim dengan retry backoff (ADR TA-3): 1 percobaan awal + 3 retry
        // (backoff 2s/4s/8s) = maks 4 panggilan; give up → false
        let mut attempt = 0usize;
        loop {
            match self.http.send_message(&self.cfg.chat_id, text) {
                Ok(()) => {
                    self.sent_at.push_back(now);
                    return true;
                }
                Err(e) => {
                    tracing::warn!("telegram send gagal (percobaan {attempt}): {e}");
                    match BACKOFF_MS.get(attempt) {
                        Some(&backoff) => {
                            self.wait(backoff); // tunggu → coba lagi
                            attempt += 1;
                        }
                        None => {
                            tracing::error!("telegram give up setelah {} percobaan", attempt + 1);
                            return false;
                        }
                    }
                }
            }
        }
    }

    fn wait(&self, ms: u64) {
        self.clock.sleep_ms(ms);
    }
}

//! TurboManager (Task TM2): state machine per-host + anti-flap.
//! Murni + trait TurboClock injectable (TM-5).

use std::collections::HashMap;

use crate::turbo::TurboConfig;

pub trait TurboClock {
    fn now_ms(&self) -> u64;
}

pub struct SystemTurboClock;
impl TurboClock for SystemTurboClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Aksi yang diminta manager ke poller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// POST /config interval turbo
    PushTurbo,
    /// POST /config interval normal
    PopTurbo,
}

#[derive(Debug)]
struct HostState {
    turbo: bool,
    /// timestamp push turbo (ms).
    since_ms: u64,
    /// snapshot normal beruntun (anti-flap, TM-4).
    normal_streak: u32,
}

/// Manager per-host (ADR TM-1). Murni — kirim config ditangani pemanggil.
pub struct TurboManager<C: TurboClock> {
    cfg: TurboConfig,
    clock: C,
    hosts: HashMap<String, HostState>,
}

const NORMAL_STREAK_REQUIRED: u32 = 2;

impl<C: TurboClock> TurboManager<C> {
    pub fn new(cfg: TurboConfig, clock: C) -> Self {
        Self {
            cfg,
            clock,
            hosts: HashMap::new(),
        }
    }

    pub fn is_turbo(&self, host: &str) -> bool {
        self.hosts.get(host).is_some_and(|h| h.turbo)
    }

    /// Event masuk dari detector (TM-AC-004/005/010/011).
    /// Hanya spike_cpu & spike_mem yang memicu (Q-TM1).
    pub fn on_event(&mut self, kind: &str, host: &str) -> Option<Action> {
        if !self.cfg.enabled {
            return None;
        }
        if kind != "spike_cpu" && kind != "spike_mem" {
            return None; // disk / agent_down / agent_up tidak trigger (Q-TM1)
        }

        let now = self.clock.now_ms();
        let st = self.hosts.entry(host.to_string()).or_insert(HostState {
            turbo: false,
            since_ms: 0,
            normal_streak: 0,
        });

        if st.turbo {
            // sudah turbo — tanpa kirim ulang (TM-AC-005); reset streak
            st.normal_streak = 0;
            return None;
        }

        st.turbo = true;
        st.since_ms = now;
        st.normal_streak = 0;
        Some(Action::PushTurbo)
    }

    /// Snapshot tanpa event baru dipanggil oleh poller (ADR TM-2).
    /// Return PopTurbo bila semua syarat anti-flap terpenuhi.
    pub fn on_snapshot(&mut self, host: &str) -> Option<Action> {
        if !self.cfg.enabled {
            return None;
        }
        let st = self.hosts.get_mut(host)?;

        if !st.turbo {
            return None; // idle + normal = tidak ada aksi (TM-AC-008)
        }

        // tetap turbo → streak normal bertambah
        st.normal_streak += 1;

        let age = self.clock.now_ms().saturating_sub(st.since_ms);
        if age >= self.cfg.min_duration_ms && st.normal_streak >= NORMAL_STREAK_REQUIRED {
            st.turbo = false;
            st.normal_streak = 0;
            return Some(Action::PopTurbo);
        }
        None // TM-AC-007: tunggu
    }
}

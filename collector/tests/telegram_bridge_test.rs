//! TDD: AlertBridge (Task TA4, TA-AC-008/010/011/012).
//! Filter severity + rate limit sliding window + retry backoff (FakeClock).

use std::sync::{Arc, Mutex};

use hiworld_collector::telegram::bridge::AlertBridge;
use hiworld_collector::telegram::{Severity, TelegramConfig};

// ---------- fakes ----------

struct FakeHttp {
    calls: Mutex<Vec<String>>,
    /// hasil per call: Ok → sukses; Err(msg) → gagal
    results: Mutex<Vec<Result<(), String>>>,
}

impl FakeHttp {
    fn new(results: Vec<Result<(), String>>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(vec![]),
            results: Mutex::new(results),
        })
    }
    fn fail() -> Arc<Self> {
        // 1 percobaan awal + 3 retry (ADR TA-3)
        Self::new(vec![
            Err("err1".into()),
            Err("err2".into()),
            Err("err3".into()),
            Err("err4".into()),
        ])
    }
}

impl hiworld_collector::telegram::client::TelegramHttp for FakeHttp {
    fn send_message(&self, _chat_id: &str, text: &str) -> Result<(), String> {
        self.calls.lock().unwrap().push(text.to_string());
        let mut r = self.results.lock().unwrap();
        if r.is_empty() {
            Ok(())
        } else {
            r.remove(0)
        }
    }
}

#[derive(Clone)]
struct FakeClock {
    now_ms: Arc<Mutex<u64>>,
}

impl FakeClock {
    fn new() -> Self {
        Self {
            now_ms: Arc::new(Mutex::new(1_000_000)),
        }
    }
    fn advance(&self, ms: u64) {
        *self.now_ms.lock().unwrap() += ms;
    }
}

impl hiworld_collector::telegram::bridge::Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        *self.now_ms.lock().unwrap()
    }

    /// Virtual time: sleep = maju jam (tanpa tunggu nyata).
    fn sleep_ms(&self, ms: u64) {
        *self.now_ms.lock().unwrap() += ms;
    }
}

fn cfg(min_severity: Severity, max_per_minute: u32) -> TelegramConfig {
    TelegramConfig {
        enabled: true,
        bot_token: "tok".into(),
        chat_id: "chat".into(),
        min_severity,
        max_per_minute,
    }
}

const EVENT: &str = "test message";

fn send_via<
    H: hiworld_collector::telegram::client::TelegramHttp,
    C: hiworld_collector::telegram::bridge::Clock,
>(
    bridge: &mut AlertBridge<H, C>,
) -> bool {
    bridge.handle(Severity::Warning, "spike_cpu", "web-01", "nginx", EVENT)
    // handle butuh &mut (sent_at state)
}

// ---------- severity filter (TA-AC-010/011) ----------

#[test]
fn below_threshold_not_sent() {
    let http = FakeHttp::fail(); // kalau terkirim pasti gagal → test deteksi
    let mut bridge = AlertBridge::new(cfg(Severity::Critical, 20), http.clone(), FakeClock::new());
    assert!(!send_via(&mut bridge), "warning < critical → tidak dikirim");
    assert_eq!(http.calls.lock().unwrap().len(), 0);
}

#[test]
fn at_threshold_sent() {
    let http = FakeHttp::new(vec![]);
    let mut bridge = AlertBridge::new(cfg(Severity::Warning, 20), http.clone(), FakeClock::new());
    assert!(send_via(&mut bridge), "warning == warning → dikirim");
    assert_eq!(http.calls.lock().unwrap().len(), 1);
}

#[test]
fn above_threshold_sent() {
    let http = FakeHttp::new(vec![]);
    let mut bridge = AlertBridge::new(cfg(Severity::Warning, 20), http.clone(), FakeClock::new());
    assert!(bridge.handle(Severity::Critical, "spike_cpu", "h", "s", EVENT));
}

// ---------- retry (TA-AC-008) ----------

#[test]
fn retry_backoff_until_success() {
    let http = FakeHttp::new(vec![Err("fail1".into()), Ok(())]);
    let clock = FakeClock::new();
    let mut bridge = AlertBridge::new(cfg(Severity::Warning, 20), http.clone(), clock.clone());
    assert!(send_via(&mut bridge), "sukses di percobaan ke-2");
    assert_eq!(http.calls.lock().unwrap().len(), 2);
    // backoff 2s tercatat antara percobaan
    assert_eq!(*clock.now_ms.lock().unwrap() - 1_000_000, 2_000);
}

#[test]
fn give_up_after_3_and_return_false() {
    let http = FakeHttp::fail();
    let clock = FakeClock::new();
    let mut bridge = AlertBridge::new(cfg(Severity::Warning, 20), http.clone(), clock.clone());
    assert!(!send_via(&mut bridge), "4x gagal → give up");
    assert_eq!(http.calls.lock().unwrap().len(), 4);
    // total backoff: 2s + 4s + 8s = 14s
    assert_eq!(*clock.now_ms.lock().unwrap() - 1_000_000, 14_000);
}

// ---------- rate limit (TA-AC-012) ----------

#[test]
fn rate_limit_drops_excess() {
    let http = FakeHttp::new(vec![]);
    let mut bridge = AlertBridge::new(cfg(Severity::Info, 3), http.clone(), FakeClock::new());
    assert!(send_via(&mut bridge));
    assert!(send_via(&mut bridge));
    assert!(send_via(&mut bridge));
    assert!(!send_via(&mut bridge), "ke-4 dalam window → drop");
    assert_eq!(http.calls.lock().unwrap().len(), 3);
}

#[test]
fn sliding_window_reopens() {
    let http = FakeHttp::new(vec![]);
    let clock = FakeClock::new();
    let mut bridge = AlertBridge::new(cfg(Severity::Info, 2), http.clone(), clock.clone());
    assert!(send_via(&mut bridge));
    assert!(send_via(&mut bridge));
    assert!(!send_via(&mut bridge), "penuh");
    clock.advance(60_001); // window bergulir lewat
    assert!(send_via(&mut bridge), "window baru → boleh lagi");
}

#[test]
fn drop_excess_keeps_sending_older_dropped() {
    // ADR TA-4: drop terlama — test sederhana: setelah drop, pesan berikutnya
    // dalam window tetap drop sampai window buka (tidak antre).
    let http = FakeHttp::new(vec![]);
    let mut bridge = AlertBridge::new(cfg(Severity::Info, 1), http.clone(), FakeClock::new());
    assert!(send_via(&mut bridge));
    assert!(!send_via(&mut bridge));
    assert!(!send_via(&mut bridge), "tetap drop selama window penuh");
    assert_eq!(http.calls.lock().unwrap().len(), 1);
}

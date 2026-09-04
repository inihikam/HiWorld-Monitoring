//! TDD: register_once + retry backoff loop (Task SR3, SR-AC-001..004, SR-AC-008).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use hiworld_agent::registrar::{RegisterAttempt, Registrar, RegistrarClock, RegistrarDeps};

// ---------- fake HTTP (tanpa wiremock — murni unit) ----------

#[derive(Clone)]
struct FakeHttp {
    /// Respons per attempt: Ok(status) | Err(koneksi gagal)
    responses: Arc<Mutex<Vec<Result<u16, ()>>>>,
    /// Request yang diterima (untuk assert body/token)
    requests: Arc<Mutex<Vec<(String, String)>>>, // (url, body)
}

impl FakeHttp {
    fn new(responses: Vec<Result<u16, ()>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            requests: Arc::new(Mutex::new(vec![])),
        }
    }
}

impl hiworld_agent::registrar::RegisterHttp for FakeHttp {
    fn post_register(
        &self,
        url: &str,
        body: &str,
    ) -> Result<u16, hiworld_agent::registrar::RegisterError> {
        self.requests
            .lock()
            .unwrap()
            .push((url.to_string(), body.to_string()));
        let mut r = self.responses.lock().unwrap();
        match r.first() {
            Some(Ok(status)) => {
                let s = *status;
                r.remove(0);
                Ok(s)
            }
            Some(Err(())) => {
                r.remove(0);
                Err(hiworld_agent::registrar::RegisterError::Connect(
                    "simulated".into(),
                ))
            }
            None => Ok(200), // habis → sukses default
        }
    }
}

// ---------- fake clock: catat jeda antar sleep ----------

#[derive(Default)]
struct FakeClock {
    sleeps_ms: Arc<Mutex<Vec<u64>>>,
    now: u64,
}

impl RegistrarClock for FakeClock {
    fn sleep(&mut self, d: Duration) {
        self.sleeps_ms.lock().unwrap().push(d.as_millis() as u64);
        self.now += d.as_millis() as u64;
    }
}

fn deps(
    responses: Vec<Result<u16, ()>>,
) -> (RegistrarDeps<FakeHttp, FakeClock>, Arc<Mutex<Vec<u64>>>) {
    let http = FakeHttp::new(responses);
    let clock = FakeClock::default();
    let sleeps = clock.sleeps_ms.clone();
    (
        RegistrarDeps {
            http,
            clock,
            host_id: "web-01".into(),
            agent_url: "http://10.0.0.1:9100".into(),
            collector_url: "http://collector:8080".into(),
            register_token: "prov-token".into(),
        },
        sleeps,
    )
}

// ---------- SR-AC-001: sukses sekali lalu berhenti ----------

#[test]
fn success_once_then_stop() {
    let (d, sleeps) = deps(vec![Ok(200)]);
    let mut reg = Registrar::new(d);
    let outcome = reg.run_blocking();
    assert!(matches!(outcome, RegisterAttempt::Success));
    assert!(
        sleeps.lock().unwrap().is_empty(),
        "sukses pertama → tanpa retry"
    );
}

// ---------- SR-AC-001: request body & token benar ----------

#[test]
fn request_contains_correct_payload() {
    let (d, _) = deps(vec![Ok(200)]);
    let mut reg = Registrar::new(d);
    let _ = reg.run_blocking();

    let requests = reg.deps.http.requests.lock().unwrap();
    let (url, body) = &requests[0];
    assert_eq!(url, "http://collector:8080/api/agents/register");
    assert!(body.contains("\"host_id\":\"web-01\""), "body: {body}");
    assert!(
        body.contains("\"agent_url\":\"http://10.0.0.1:9100\""),
        "body: {body}"
    );
    assert!(body.contains("\"token\":\"prov-token\""), "body: {body}");
}

// ---------- SR-AC-002/003: gagal → retry backoff 1s→2s→4s→…→cap 60s ----------

#[test]
fn failures_then_success_backoff_schedule() {
    // gagal 4x (2 connect err + 2 HTTP 500), lalu sukses
    let (d, sleeps) = deps(vec![Err(()), Err(()), Ok(500), Ok(500), Ok(200)]);
    let mut reg = Registrar::new(d);
    let outcome = reg.run_blocking();

    assert!(matches!(outcome, RegisterAttempt::Success));
    let sleeps = sleeps.lock().unwrap();
    assert_eq!(
        *sleeps,
        vec![1000, 2000, 4000, 8000],
        "backoff eksponensial dari 1s, x2 tiap gagal"
    );
}

#[test]
fn backoff_caps_at_60s() {
    // gagal banyak → jeda harus berhenti di 60_000
    let responses = vec![Err(()); 10];
    let (d, sleeps) = deps(responses);
    let mut reg = Registrar::new(d);
    let _ = reg.run_blocking_max_attempts(10);

    let sleeps = sleeps.lock().unwrap();
    assert!(
        sleeps.iter().all(|&ms| ms <= 60_000),
        "tidak ada jeda > 60s; dapat {sleeps:?}"
    );
    assert_eq!(*sleeps.last().unwrap(), 60_000, "mencapai cap 60s");
}

// ---------- SR-AC-004: re-register setelah sukses (idempotent) ----------

#[test]
fn rerun_after_success_is_safe() {
    // dua siklus registrar (simulasi restart) — keduanya sukses
    let (d, _) = deps(vec![Ok(200)]);
    let mut reg1 = Registrar::new(d);
    assert!(matches!(reg1.run_blocking(), RegisterAttempt::Success));

    let (d2, _) = deps(vec![Ok(200)]);
    let mut reg2 = Registrar::new(d2);
    assert!(matches!(reg2.run_blocking(), RegisterAttempt::Success));
}

// ---------- SR-AC-002: HTTP 401 juga di-retry (bukan crash) ----------

#[test]
fn unauthorized_is_retried_not_fatal() {
    // 401 dua kali lalu 200 (simulasi collector lama belum terisi provisioning)
    let (d, sleeps) = deps(vec![Ok(401), Ok(401), Ok(200)]);
    let mut reg = Registrar::new(d);
    let outcome = reg.run_blocking();
    assert!(
        matches!(outcome, RegisterAttempt::Success),
        "akhirnya sukses"
    );
    assert_eq!(sleeps.lock().unwrap().len(), 2, "dua retry setelah 401");
}

#[test]
fn max_attempts_exhausted_returns_gave_up() {
    let (d, _) = deps(vec![Err(()); 5]);
    let mut reg = Registrar::new(d);
    let outcome = reg.run_blocking_max_attempts(3);
    assert!(matches!(outcome, RegisterAttempt::GaveUp));
}

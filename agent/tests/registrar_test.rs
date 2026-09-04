//! TDD: host_id provider (Task SR2, SR-AC-006).
//! Trait-based agar fallback chain teruji tanpa /etc/hostname asli.

use hiworld_agent::registrar::{resolve_host_id, HostnameProvider};

/// Provider palsu: return nilai berurutan; None = gagal.
struct FakeProvider {
    results: Vec<Option<String>>,
}

impl HostnameProvider for FakeProvider {
    fn hostname_file(&self) -> Option<String> {
        self.results.first().cloned().flatten()
    }
    fn hostname_cmd(&self) -> Option<String> {
        self.results.get(1).cloned().flatten()
    }
}

#[test]
fn hostname_file_trimmed() {
    let p = FakeProvider {
        results: vec![Some("web-01\n".into()), None],
    };
    assert_eq!(resolve_host_id(&p), "web-01");
}

#[test]
fn fallback_to_hostname_cmd() {
    let p = FakeProvider {
        results: vec![None, Some("  db-02  ".into())],
    };
    assert_eq!(resolve_host_id(&p), "db-02", "cmd result juga di-trim");
}

#[test]
fn fallback_to_unknown_when_both_fail() {
    let p = FakeProvider {
        results: vec![None, None],
    };
    assert_eq!(resolve_host_id(&p), "unknown");
}

#[test]
fn empty_hostname_file_falls_through() {
    // file ada tapi kosong → bukan host_id valid → lanjut ke cmd
    let p = FakeProvider {
        results: vec![Some("   \n".into()), Some("fallback-host".into())],
    };
    assert_eq!(resolve_host_id(&p), "fallback-host");
}

#[test]
fn empty_cmd_result_still_falls_to_unknown() {
    let p = FakeProvider {
        results: vec![None, Some("".into())],
    };
    assert_eq!(resolve_host_id(&p), "unknown");
}

// impl produksi teruji hanya strukturnya (tidak menyentuh /etc/hostname asli)
#[test]
fn system_hostname_provider_exists() {
    // compile-check: SystemHostname implements trait
    fn assert_impl<T: HostnameProvider>() {}
    assert_impl::<hiworld_agent::registrar::SystemHostname>();
}

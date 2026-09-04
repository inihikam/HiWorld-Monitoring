//! Format pesan (Task TA2): render_alert murni — unit-testable penuh.

use crate::telegram::Severity;

/// Emoji per severity (PDD §2.3).
fn emoji(sev: Severity) -> &'static str {
    match sev {
        Severity::Critical => "🚨",
        Severity::Warning => "⚠️",
        Severity::Info => "ℹ️",
    }
}

/// Label kind singkat bahasa Inggris (Q-TA2: alert netral).
fn kind_label(kind: &str) -> &str {
    match kind {
        "spike_cpu" => "CPU spike",
        "spike_mem" => "Memory spike",
        "disk_almost_full" => "Disk almost full",
        "agent_down" => "Agent down",
        "agent_up" => "Agent up",
        _ => kind,
    }
}

const MAX_LEN: usize = 4096; // limit API Telegram

/// Render pesan plain text dari event (TA-AC-004..006).
/// event: (severity_str, kind, host, subject, detail_json)
pub fn render_alert(
    severity: Severity,
    kind: &str,
    host: &str,
    subject: &str,
    detail: &serde_json::Value,
    time_str: &str,
) -> String {
    let mut msg = format!("{} {}\nHost: {}\n", emoji(severity), kind_label(kind), host);

    if !subject.is_empty() {
        let label = match kind {
            "disk_almost_full" => "Mount",
            "agent_down" | "agent_up" => "Agent",
            _ => "Process",
        };
        msg.push_str(&format!("{label}: {subject}\n"));
    }

    // nilai & baseline bila ada (detail fleksibel)
    if let Some(v) = detail.get("value").and_then(|v| v.as_f64()) {
        let unit = match kind {
            "spike_cpu" => "% CPU",
            "spike_mem" => "% RAM",
            "disk_almost_full" => "% used",
            _ => "",
        };
        msg.push_str(&format!("Value: {v:.1}{unit}\n"));
    }
    if let Some(b) = detail.get("baseline") {
        msg.push_str(&format!("Baseline: {b}\n"));
    }

    msg.push_str(&format!("──────────\nhiworld monitoring · {time_str}"));
    truncate_plain(msg)
}

/// Truncate aman ≤4096 char (TA-AC-006) — potong di boundary char, tambah ellipsis.
fn truncate_plain(s: String) -> String {
    if s.chars().count() <= MAX_LEN {
        return s;
    }
    let cut: String = s.chars().take(MAX_LEN - 1).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cpu_spike_critical_contains_all() {
        let msg = render_alert(
            Severity::Critical,
            "spike_cpu",
            "web-01",
            "nginx (pid 1234)",
            &json!({"value": 91.2, "baseline": "45.0%"}),
            "12:03:44",
        );
        assert!(msg.starts_with("🚨"));
        assert!(msg.contains("CPU spike"));
        assert!(msg.contains("Host: web-01"));
        assert!(msg.contains("Process: nginx (pid 1234)"));
        assert!(msg.contains("91.2% CPU"));
        assert!(msg.contains("45.0%"));
        assert!(msg.contains("12:03:44"));
    }

    #[test]
    fn agent_down_warning_emoji() {
        let msg = render_alert(
            Severity::Warning,
            "agent_down",
            "db-01",
            "db-01",
            &json!({}),
            "08:00:00",
        );
        assert!(msg.starts_with("⚠️"));
        assert!(msg.contains("Agent down"));
        assert!(msg.contains("Agent: db-01"));
    }

    #[test]
    fn disk_kind_uses_mount_label() {
        let msg = render_alert(
            Severity::Warning,
            "disk_almost_full",
            "web-01",
            "/data",
            &json!({"value": 92.0}),
            "08:00:00",
        );
        assert!(msg.contains("Mount: /data"));
        assert!(msg.contains("92.0% used"));
    }

    #[test]
    fn special_chars_not_broken() {
        // TA-AC-005: plain text — `<>&` aman
        let msg = render_alert(
            Severity::Warning,
            "spike_cpu",
            "host<>&'",
            "proc<b>&amp;",
            &json!({}),
            "08:00:00",
        );
        assert!(msg.contains("host<>&'"));
        assert!(msg.contains("proc<b>&amp;"));
        // plain text: apa yang dikirim = apa yang tampil (tidak ada parse mode)
        assert!(msg.contains("Process: proc<b>&amp;"));
    }

    #[test]
    fn long_message_truncated_4096() {
        let big = "x".repeat(10_000);
        let msg = render_alert(
            Severity::Info,
            "spike_mem",
            "h",
            &big,
            &json!({}),
            "08:00:00",
        );
        assert!(msg.chars().count() <= 4096);
        assert!(msg.ends_with('…'));
    }

    #[test]
    fn agent_up_info_emoji() {
        let msg = render_alert(
            Severity::Info,
            "agent_up",
            "db-01",
            "db-01",
            &json!({}),
            "08:00:00",
        );
        assert!(msg.starts_with("ℹ️"));
        assert!(msg.contains("Agent up"));
    }
}

//! CI guard untuk ARCH-AC-002: crate `core` harus tetap murni.
//!
//! Test ini gagal bila Cargo.toml core memuat dependency runtime/I/O terlarang.
//! Alasan: parser & model harus 100% unit-testable tanpa runtime async dan
//! tanpa akses resource eksternal — fondasi TDD seluruh project.

use std::path::Path;

const FORBIDDEN: &[&str] = &[
    "tokio", "axum", "rusqlite", "reqwest", "hyper", "warp", "actix", "sqlx", "diesel", "surf",
];

fn core_manifest() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("gagal membaca {}: {e}", path.display()))
}

#[test]
fn core_has_no_runtime_or_io_dependencies() {
    let manifest = core_manifest();

    for dep in FORBIDDEN {
        assert!(
            !manifest.contains(dep),
            "core/Cargo.toml memuat dependency terlarang `{dep}`.\n\
             Prinsip arsitektur (ARCH-AC-002): core harus murni (std + parsing).\n\
             Pindahkan I/O ke agent/collector, atau taruh di belakang trait."
        );
    }
}

#[test]
fn core_dependencies_are_allowlisted() {
    let manifest = core_manifest();
    let deps_section = manifest
        .split("[dependencies]")
        .nth(1)
        .and_then(|rest| rest.split('[').next())
        .unwrap_or("");

    // Allowlist: apa saja yang boleh ada di [dependencies] core.
    // Tambah dengan hati-hati — core harus tetap murni.
    const ALLOWED: &[&str] = &["serde", "serde_json"];

    for line in deps_section.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let name = line.split('=').next().unwrap_or("").trim();
        // Normalisasi: "serde.workspace = true" / "serde = ..." → "serde"
        let name = name.trim_end_matches(".workspace");
        assert!(
            ALLOWED.contains(&name),
            "dependency `{name}` tidak ada di allowlist core.\n\
             Jika benar-benar perlu, tambahkan ke ALLOWED dengan justifikasi \
             bahwa crate itu murni (no runtime/IO)."
        );
    }
}

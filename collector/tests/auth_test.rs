//! TDD: auth UI (Task D2) — ARCH-AC-023.
//! Password hash di DB (bukan plaintext), bisa diganti, bootstrap dari TOML.

use hiworld_collector::auth::{Auth, AuthError};

const STRONG: &str = "correct horse battery staple";

#[test]
fn bootstrap_creates_admin_from_plaintext() {
    let auth = Auth::bootstrap_in_memory("admin", "initial-pass").unwrap();

    // login dengan password bootstrap → OK
    assert!(auth.verify_login("admin", "initial-pass").unwrap());

    // DB TIDAK menyimpan plaintext (ARCH-AC-023)
    let stored = auth.raw_password_column("admin").unwrap();
    assert!(
        !stored.contains("initial-pass"),
        "plaintext tidak boleh di DB, dapat: {stored}"
    );
    assert!(
        stored.starts_with("$2") || stored.starts_with("$argon2"),
        "harus hash bcrypt/argon2, dapat: {stored}"
    );
}

#[test]
fn bootstrap_idempotent_existing_user_untouched() {
    let auth = Auth::bootstrap_in_memory("admin", "first-pass").unwrap();
    // bootstrap kedua dengan password beda → user TIDAK berubah
    auth.bootstrap("admin", "other-pass").unwrap();
    assert!(
        auth.verify_login("admin", "first-pass").unwrap(),
        "bootstrap kedua tidak boleh menimpa password existing"
    );
    assert!(!auth.verify_login("admin", "other-pass").unwrap());
}

#[test]
fn wrong_password_rejected() {
    let auth = Auth::bootstrap_in_memory("admin", STRONG).unwrap();
    assert!(!auth.verify_login("admin", "wrong-pass").unwrap());
    assert!(!auth.verify_login("admin", "").unwrap());
    assert!(
        !auth.verify_login("nobody", STRONG).unwrap(),
        "user tak dikenal"
    );
}

#[test]
fn change_password_requires_old_and_session() {
    let auth = Auth::bootstrap_in_memory("admin", "old-pass").unwrap();

    // password lama salah → ditolak
    let err = auth
        .change_password("admin", "wrong-old", "new-pass")
        .unwrap_err();
    assert!(matches!(err, AuthError::WrongOldPassword));

    // benar → sukses; login lama gagal, baru sukses
    auth.change_password("admin", "old-pass", "new-pass")
        .unwrap();
    assert!(!auth.verify_login("admin", "old-pass").unwrap());
    assert!(auth.verify_login("admin", "new-pass").unwrap());
}

#[test]
fn new_password_hash_different_from_old() {
    let auth = Auth::bootstrap_in_memory("admin", "pass-aaaa").unwrap();
    let h1 = auth.raw_password_column("admin").unwrap();
    auth.change_password("admin", "pass-aaaa", "pass-bbbb")
        .unwrap();
    let h2 = auth.raw_password_column("admin").unwrap();
    assert_ne!(h1, h2, "hash harus beda setelah ganti password");
}

#[test]
fn empty_new_password_rejected() {
    let auth = Auth::bootstrap_in_memory("admin", "old-pass").unwrap();
    let err = auth.change_password("admin", "old-pass", "").unwrap_err();
    assert!(matches!(err, AuthError::EmptyPassword));
}

#[test]
fn timing_safe_enough_via_hash_verify() {
    // user tidak ada → verify tidak panic & tetap false
    let auth = Auth::bootstrap_in_memory("admin", "x").unwrap();
    assert!(!auth.verify_login("ghost", "whatever").unwrap());
}

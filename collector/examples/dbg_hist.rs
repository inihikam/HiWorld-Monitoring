fn main() {
    let conn = rusqlite::Connection::open_with_flags(
        "/var/lib/hiworld/hiworld.db",
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM metrics_raw WHERE host_id='inihikam'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    println!("rows via read-only open: {n}");
}

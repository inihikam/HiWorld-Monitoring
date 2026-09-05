fn main() {
    let conn = rusqlite::Connection::open("/var/lib/hiworld/hiworld.db").unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM metrics_raw", [], |r| r.get(0))
        .unwrap();
    println!("rows via RW open: {n}");
}

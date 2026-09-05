fn main() {
    let conn = rusqlite::Connection::open("/var/lib/hiworld/hiworld.db").unwrap();
    let host = "inihikam";
    let from: u64 = 1788564770461;
    let to: u64 = 1788564830461;
    let mut stmt = conn
        .prepare(
            "SELECT COUNT(*) FROM metrics_raw
             WHERE host_id = ?1 AND timestamp_ms >= ?2 AND timestamp_ms <= ?3",
        )
        .unwrap();
    let n: i64 = stmt
        .query_map(rusqlite::params![host, from as i64, to as i64], |r| r.get(0))
        .unwrap()
        .flatten()
        .next()
        .unwrap();
    println!("count via handler-style query: {n}");
}

p = 'collector/src/store.rs'
s = open(p).read()
old = """             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params!["""
new = """             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(host_id, timestamp_ms) DO UPDATE SET
                interval_ms = excluded.interval_ms,
                cpu_percent = excluded.cpu_percent,
                mem_total_bytes = excluded.mem_total_bytes,
                mem_used_bytes = excluded.mem_used_bytes,
                mem_percent = excluded.mem_percent,
                swap_used_bytes = excluded.swap_used_bytes,
                load_1m = excluded.load_1m,
                load_5m = excluded.load_5m,
                load_15m = excluded.load_15m,
                processes_json = excluded.processes_json",
            rusqlite::params!["""
assert old in s
s = s.replace(old, new)
open(p, 'w').write(s)
print("ok")

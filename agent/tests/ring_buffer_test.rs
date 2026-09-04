//! TDD RED: ring buffer (ARCH-AC-014). Modul belum ada → HARUS gagal dulu.

use hiworld_agent::ring_buffer::RingBuffer;
use std::time::Duration;

use hiworld_core::proc_parser::{CpuStat, CpuTime};

fn dummy_snapshot(ts: u64) -> u64 {
    // RingBuffer v1 generik terhadap tipe item; pakai timestamp u64
    // sebagai item dummy (Snapshot belum bisa dibangun tanpa data penuh —
    // strukturnya diuji, bukan isinya).
    ts
}

#[test]
fn evicts_oldest_when_over_capacity() {
    let mut rb = RingBuffer::new(3, Duration::MAX);
    for ts in [1, 2, 3, 4, 5] {
        rb.push(ts, dummy_snapshot(ts));
    }
    let items = rb.items();
    assert_eq!(items.len(), 3);
    assert_eq!(
        items.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
        vec![3, 4, 5]
    );
}

#[test]
fn evicts_entries_older_than_max_age() {
    // Timestamp produksi = MILISECOND. Interval sampling realistis 1-60 detik.
    // Skenario: interval 200s? tidak realistis — pakai skala detik dengan
    // max_age 600 detik, data ms dengan interval 100s (100_000 ms):
    let mut rb = RingBuffer::new(100, Duration::from_secs(600));
    let ms = |s: u64| s * 1_000;
    // push tiap 100 "detik" (100_000 ms); max_age 600_000 ms:
    // saat newest=600_000 → buang 0 (usia 600_000? tidak, =600_000 tidak >)
    // saat newest=700_000 → 0 dibuang (700_000 > 600_000)
    // saat newest=800_000 → 100_000 dibuang
    // saat newest=900_000 → 200_000 dibuang
    for s in [0, 100, 200, 300, 400, 500, 600, 700, 800, 900] {
        rb.push(ms(s), dummy_snapshot(ms(s)));
    }
    let items = rb.items();
    assert_eq!(
        items.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
        vec![
            ms(300),
            ms(400),
            ms(500),
            ms(600),
            ms(700),
            ms(800),
            ms(900)
        ]
    );
}

#[test]
fn backlog_since_returns_ordered() {
    let mut rb = RingBuffer::new(100, Duration::MAX);
    for ts in [10, 20, 30, 40, 50] {
        rb.push(ts, dummy_snapshot(ts));
    }
    let backlog = rb.backlog_since(25);
    // entry dengan timestamp STRICTLY > 25, urut waktu
    assert_eq!(
        backlog.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
        vec![30, 40, 50]
    );
    // sejak 0 → semua
    assert_eq!(rb.backlog_since(0).len(), 5);
    // sejak 100 → kosong
    assert!(rb.backlog_since(100).is_empty());
}

#[test]
fn empty_buffer_behaves() {
    let rb: RingBuffer<u64> = RingBuffer::new(10, Duration::from_secs(600));
    assert!(rb.items().is_empty());
    assert!(rb.backlog_since(0).is_empty());
    assert!(rb.last_timestamp().is_none());
}

#[test]
fn last_timestamp_tracks_newest() {
    let mut rb = RingBuffer::new(10, Duration::MAX);
    rb.push(5, 5);
    rb.push(9, 9);
    assert_eq!(rb.last_timestamp(), Some(9));
}

// pastikan tipe kompleks juga bisa (dipakai sampler nanti)
#[test]
fn works_with_real_types() {
    let mut rb: RingBuffer<CpuStat> = RingBuffer::new(2, Duration::MAX);
    let st = CpuStat {
        cpu_total: CpuTime {
            user: 0,
            nice: 0,
            system: 0,
            idle: 0,
            iowait: 0,
            irq: 0,
            softirq: 0,
            steal: 0,
        },
        cpus: vec![],
    };
    rb.push(1, st);
    assert_eq!(rb.items().len(), 1);
}

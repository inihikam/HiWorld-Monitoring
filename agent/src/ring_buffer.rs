//! Ring buffer snapshot di RAM (ADR-5): toleransi collector down,
//! sumber backfill (Q2). Batas jumlah entry & batas umur — mana yang
//! kena duluan. Generik terhadap tipe item; timestamp `u64` epoch-ms.

use std::collections::VecDeque;
use std::time::Duration;

#[derive(Debug)]
pub struct RingBuffer<T> {
    max_entries: usize,
    max_age: Duration,
    entries: VecDeque<(u64, T)>,
}

impl<T> RingBuffer<T> {
    /// `max_entries` = batas jumlah, `max_age` = batas umur relatif terhadap
    /// entry terbaru. Entry dibuang bila MELANGGAR salah satu.
    pub fn new(max_entries: usize, max_age: Duration) -> Self {
        Self {
            max_entries: max_entries.max(1),
            max_age,
            entries: VecDeque::new(),
        }
    }

    /// Push entry dengan timestamp epoch-ms (HARUS monoton naik; timestamp
    /// lebih lama dari newest diabaikan — clock mundur tidak boleh merusak order).
    pub fn push(&mut self, timestamp_ms: u64, item: T) {
        if let Some((newest_ts, _)) = self.entries.back() {
            if timestamp_ms <= *newest_ts {
                // timestamp tidak maju: replace item terakhir bila sama,
                // abaikan bila lebih tua (hindari order rusak).
                if timestamp_ms == *newest_ts {
                    if let Some((_, last)) = self.entries.back_mut() {
                        *last = item;
                    }
                }
                self.prune();
                return;
            }
        }
        self.entries.push_back((timestamp_ms, item));
        self.prune();
    }

    fn prune(&mut self) {
        let age_ms = self.max_age.as_millis() as u64;
        // 1) batas umur: relatif terhadap entry TERBARU di buffer.
        //    Dipanggil setiap push, jadi prune incremental berjalan benar:
        //    tiap entry dibuang saat usianya > max_age terhadap newest saat itu.
        if let Some(&(newest_ts, _)) = self.entries.back() {
            while self
                .entries
                .front()
                .is_some_and(|&(ts, _)| newest_ts - ts > age_ms)
            {
                self.entries.pop_front();
            }
        }
        // 2) batas jumlah
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }

    /// Semua entry (urut waktu, oldest → newest).
    pub fn items(&self) -> Vec<(u64, &T)> {
        self.entries.iter().map(|(ts, v)| (*ts, v)).collect()
    }

    /// Entry dengan timestamp strictly > `since_ms`, urut waktu (ADR-5).
    pub fn backlog_since(&self, since_ms: u64) -> Vec<(u64, &T)> {
        self.entries
            .iter()
            .filter(|(ts, _)| *ts > since_ms)
            .map(|(ts, v)| (*ts, v))
            .collect()
    }

    pub fn last_timestamp(&self) -> Option<u64> {
        self.entries.back().map(|(ts, _)| *ts)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_replace_same_timestamp() {
        let mut rb = RingBuffer::new(5, Duration::MAX);
        rb.push(10, 1);
        rb.push(10, 2); // replace
        assert_eq!(rb.len(), 1);
        assert_eq!(rb.items()[0].1, &2);
    }

    #[test]
    fn push_ignore_older_timestamp() {
        let mut rb = RingBuffer::new(5, Duration::MAX);
        rb.push(10, 1);
        rb.push(5, 0); // diabaikan
        assert_eq!(rb.len(), 1);
        assert_eq!(rb.items()[0].1, &1);
    }
}

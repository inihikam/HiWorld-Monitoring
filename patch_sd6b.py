p = 'collector/src/poller.rs'
s = open(p).read()

old_detector_field = """    /// Spike detector (SD6) — None = deteksi nonaktif.
    /// Baseline memegang koneksi store TERPISAH (produksi: file DB sama,
    /// koneksi terpisah — WAL mendukung multi-koneksi).
    detector: Option<
        crate::detector::Detector<Store, PollerClock, crate::poller::StoreBaselineOwned>,
    >,"""
new_detector_field = """    /// Spike detector (SD6) — None = deteksi nonaktif.
    /// Detector stateless-kecuali-config; store & baseline diberikan
    /// per-evaluasi (evaluate menerima &mut store + &baseline).
    detector: Option<crate::detector::Detector<PollerClock>>,"""
s = s.replace(old_detector_field, new_detector_field)

start = s.index("    /// Poller dengan detector aktif (SD6).")
end = s.index("    pub fn store(&self)")
new_with_detector = """    /// Poller dengan detector aktif (SD6).
    pub fn with_detector(
        store: Store,
        agent_token: String,
        interval: Duration,
        cfg: crate::api::DetectorConfig,
        _detector_store: Store,
    ) -> Self {
        let detector = crate::detector::Detector::new(PollerClock, cfg);
        Self {
            store,
            agent_token,
            interval,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
            statuses: HashMap::new(),
            last_seen: HashMap::new(),
            backfill_threshold: interval * 2,
            detector: Some(detector),
        }
    }

"""
s = s[:start] + new_with_detector + s[end:]

old_eval = """        // SD6: evaluasi spike -> insert event yang dihasilkan
        if let Some(detector) = self.detector.as_mut() {
            for ev in detector.evaluate(&snap) {"""
# cari versi dgn panah unicode juga
if old_eval not in s:
    old_eval = old_eval.replace("->", "\u2192")
if old_eval not in s:
    # fallback: cari berdasarkan marker
    import re
    m = re.search(r"        // SD6[^\n]*\n        if let Some\(detector\) = self\.detector\.as_mut\(\) \{\n            for ev in detector\.evaluate\(&snap\) \{", s)
    if m:
        old_eval = m.group(0)
    else:
        raise SystemExit("marker tidak ditemukan")
new_eval = """        // SD6: evaluasi spike; store & baseline diberikan per-evaluasi
        if let Some(detector) = self.detector.as_mut() {
            let baseline = StoreBaselineRef {
                store: &self.store,
                window_min: detector.mem_window_min(),
            };
            for ev in detector.evaluate(&snap, &mut self.store, &baseline) {"""
s = s.replace(old_eval, new_eval)

# StoreBaselineOwned -> StoreBaselineRef
s = s.replace("""/// BaselineFetcher yang MEMILIKI Store (bebas lifetime, dipindah ke Detector).
pub struct StoreBaselineOwned {
    pub store: Store,
    pub window_min: u32,
}

impl crate::detector::BaselineFetcher for StoreBaselineOwned {""", """/// BaselineFetcher yang MEMINJAM Store (parameter evaluasi, bukan dimiliki).
pub struct StoreBaselineRef<'a> {
    pub store: &'a Store,
    pub window_min: u32,
}

impl crate::detector::BaselineFetcher for StoreBaselineRef<'_> {""")
s = s.replace("""        self.store
            .avg_rss_baseline(host_id, pid, from, now)
            .ok()
            .flatten()
    }
}""", """        self.store
            .avg_rss_baseline(host_id, pid, from, now)
            .ok()
            .flatten()
    }
}""")

open(p, 'w').write(s)
print("ok poller")

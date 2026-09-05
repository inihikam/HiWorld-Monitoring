use hiworld_agent::sampler::{Sampler, SystemSource, SystemClock};
use std::time::Duration;

fn main() {
    let src = SystemSource::new();
    let mut clock = SystemClock;
    let mut s = Sampler::new(
        Box::new(src),
        &mut clock,
        Duration::from_millis(10000),
        10,
        false,
    );
    for i in 1..=3 {
        let snap = s.sample_once().unwrap();
        println!("sample {i}: cpu={:?} mem%={:.1}", snap.system.cpu_percent, snap.system.mem_percent);
        std::thread::sleep(Duration::from_millis(1500));
    }
}

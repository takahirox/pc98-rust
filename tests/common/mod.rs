#![allow(dead_code)]
use pc98_rust::{cpu::Cpu, Machine};

/// Compare architectural state while elapsed time continues through retries.
pub fn cpu_state(cpu: &Cpu) -> String {
    let mut c = cpu.clone();
    c.cycles = 0;
    c.completed_instructions = 0;
    c.rep_elements = 0;
    c.approximated_units = 0;
    c.last_timing = Default::default();
    c.timing_writes = 0;
    format!("{c:?}")
}
/// Guest integration waits formerly expressed as 1-MIPS scheduling slots.
/// Keep their intended device time, while bounding every core call separately.
pub fn advance(m: &mut Machine, old_slots: u32) {
    let end = m.bus.cycles + u64::from(old_slots) * 8;
    let mut units = 0;
    while m.bus.cycles < end && m.error.is_none() {
        let remaining = (end - m.bus.cycles).min(80_000) as u32;
        units += m.step_scheduler(remaining, 100_000);
        assert!(
            units <= old_slots.saturating_mul(66).max(100_000),
            "guest time budget stalled"
        );
    }
}

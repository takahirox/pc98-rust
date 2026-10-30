//! Original MIT experiments for #113. Passing controls do not reproduce the
//! private HDI or establish a compatibility correction.
use pc98_rust::{Machine, Profile};
#[path = "../src/bin/headless/observations.rs"]
mod observations;
use observations::{text_summary, Progress};

struct Experiment {
    m: Machine,
    progress: Progress,
    slots: u64,
}
impl Experiment {
    fn step(&mut self, slots: u32) {
        assert_eq!(
            self.m
                .step_with_observer(slots, |c, b| self.progress.observe(c, b)),
            slots
        );
        self.slots += u64::from(slots);
        self.m.bus.audio.clear();
        assert!(self.m.error.is_none(), "{:?}", self.m.error);
        assert!(self.slots <= 8_000_000, "experiment budget exceeded");
    }
    fn until(&mut self, condition: impl Fn(&Machine) -> bool) {
        for _ in 0..6000 {
            if condition(&self.m) {
                return;
            }
            self.step(1000);
        }
        panic!("bounded progress failed: {}", self.m.bus.text());
    }
    fn checkpoint(&self, name: &str) {
        println!(
            "{name}: {}\n{}",
            self.progress.report(self.slots, &self.m.cpu, &self.m.bus),
            text_summary(&self.m.bus)
        );
        // At every checkpoint require real mode, fault-free execution. Do not
        // infer responsiveness from a counter or a changing display alone.
        assert!(!self.m.cpu.protected_mode());
        assert!(!self.m.cpu.paging_enabled() && !self.m.cpu.vm86());
        assert_eq!(self.m.cpu.fault_count, 0);
    }
    fn type_keys(&mut self, keys: &[u8]) {
        for &key in keys {
            self.m.bus.key(key, true);
            self.step(50_000);
            self.m.bus.key(key, false);
            self.step(10_000);
        }
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/fixtures/freedos/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run python3 tools/prepare_hdd_investigation.py")
}

fn handoff(e: &mut Experiment, fat16: bool) {
    let kernel = fixture("hdd-kernel.sys");
    for _ in 0..100_000 {
        // Stop before executing the loaded kernel, just as the baseline does.
        if e.m.cpu.s[1] == 0x60 && e.m.cpu.ip == 0 {
            assert!(
                e.m.bus.mem[0x600..0x600 + kernel.len()] == kernel,
                "loaded kernel differs from pinned source file"
            );
            assert_eq!(
                (e.m.cpu.s[2], e.m.cpu.r[4]),
                if fat16 {
                    (0x3800, 0x2000)
                } else {
                    (0x1e80, 0x100)
                }
            );
            assert_eq!(e.m.bus.read(0x584), 0x80);
            assert_eq!(e.m.bus.read16(0x3fe), 0);
            assert!(!e.m.bus.a20);
            assert!(e.m.bus.text().contains("FreeDOS(98) FAT Kernel GO!"));
            let nulls = (0..2000)
                .filter(|&i| e.m.bus.read16(0xa0000 + i * 2) == 0)
                .count();
            assert_eq!(nulls, 1973);
            e.checkpoint("handoff");
            return;
        }
        e.step(1);
    }
    panic!("no kernel handoff: {}", e.m.bus.text());
}

fn shell(e: &mut Experiment) {
    e.until(|m| m.bus.text().contains("HDD SHELL READY") && m.bus.text().contains("A:\\>"));
    e.checkpoint("shell before input");
    e.type_keys(&[0x19, 0x13, 0x18, 0x2d, 0x12, 0x1c]);
    e.until(|m| m.bus.text().contains("PC98 DOS PROBE OK"));
    e.type_keys(&[0x12, 0x2b, 0x22, 0x18, 0x34, 0x18, 0x24, 0x1c]);
    e.until(|m| {
        let text = m.bus.text();
        text.lines().any(|s| s.trim() == "ok") && text.matches("A:\\>").count() >= 3
    });
    assert!(e.m.cpu.irq_acks > 0);
    assert_eq!((e.m.bus.ide.writes, e.m.bus.fdc.reads), (0, 0));
    e.checkpoint("PROBE/ECHO returned");
}

#[test]
fn fragmented_fat12_and_fat16_load_exact_kernel_and_execute_commands() {
    for name in ["shell", "fragmented12", "fragmented16"] {
        let disk = fixture(&format!("{name}.hdi"));
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.insert(2, disk.clone(), name).unwrap();
        for run in 0..2 {
            m.reset();
            let mut e = Experiment {
                m,
                progress: Progress::default(),
                slots: 0,
            };
            println!("{name} run {run}");
            handoff(&mut e, name == "fragmented16");
            // The FAT16 chain must exercise cylinder 256+, not just a large
            // header on an otherwise low-cylinder file placement.
            if name == "fragmented16" {
                assert!(e.progress.disk_history().any(|call| {
                    let cx = call.split("cx=").nth(1).unwrap();
                    u16::from_str_radix(&cx[..4], 16).unwrap() >= 256
                }));
            }
            shell(&mut e);
            assert!(
                e.m.bus.ide.disk.as_ref().unwrap().bytes == disk,
                "guest changed the HDD"
            );
            m = e.m;
        }
    }
}

#[test]
fn persistent_nul_display_distinguishes_live_keyboard_from_masked_irq_then_recovers() {
    for masked in [false, true] {
        let name = if masked { "maskwait" } else { "wait" };
        let disk = fixture(&format!("{name}.hdi"));
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.insert(2, disk.clone(), name).unwrap();
        for run in 0..2 {
            m.reset();
            let mut e = Experiment {
                m,
                progress: Progress::default(),
                slots: 0,
            };
            println!("{name} run {run}");
            handoff(&mut e, name == "fragmented16");
            // Find the actual executing COM, never host-write its state/PC/IRQ.
            for _ in 0..6_000_000 {
                let base = usize::from(e.m.cpu.s[1]) * 16;
                if e.m.cpu.ip == 0x200 && &e.m.bus.mem[base + 0x300..base + 0x308] == b"H98STATE" {
                    break;
                }
                e.step(1);
            }
            let base = usize::from(e.m.cpu.s[1]) * 16 + 0x300;
            assert_eq!(&e.m.bus.mem[base..base + 8], b"H98STATE");
            assert_eq!(e.m.bus.read(base + 8), 1);
            assert_eq!(e.m.bus.read(base + 9), u8::from(masked));
            e.checkpoint("POSTWAIT ready");
            let irq = e.m.cpu.irq_acks;
            e.step(100_000);
            assert!(e.m.cpu.irq_acks > irq, "IRQ0 must continue during wait");
            assert!(e.m.bus.read16(base + 12) > 0, "guest observed IRQ0 ticks");
            assert!(text_summary(&e.m.bus).contains("literal-3f=0 ank-fallback=1986"));
            e.m.bus.key(0x1c, true);
            e.step(50_000);
            if masked {
                assert_eq!(e.m.bus.read(base + 8), 1, "Enter must remain deferred");
                assert_eq!(e.m.bus.read(base + 17), 0);
                assert_eq!(e.m.bus.pics[0].mask & 2, 2);
                assert_eq!(e.m.bus.pics[0].irr & 2, 2);
                assert!(!e.m.bus.keyboard.fifo.is_empty());
                assert_eq!(e.m.bus.read(0x528), 0);
                assert!(text_summary(&e.m.bus).contains("ank-fallback=1986"));
                e.checkpoint("Enter pending behind IRQ1 mask");
            }
            e.until(|m| m.bus.text().contains("POSTWAIT ENTER PASS"));
            assert_eq!(e.m.bus.read(base + 8), 2);
            assert_eq!(e.m.bus.read(base + 17), 13);
            let ticks = e.m.bus.read16(base + 12);
            assert!(if masked { ticks >= 64 } else { ticks < 64 });
            e.m.bus.key(0x1c, false);
            e.step(10_000);
            assert_eq!(e.m.bus.pics[0].mask & 2, 0);
            e.checkpoint("guest restored IRQ1 and consumed Enter");
            shell(&mut e);
            assert!(
                e.m.bus.ide.disk.as_ref().unwrap().bytes == disk,
                "guest changed the HDD"
            );
            m = e.m;
        }
    }
}

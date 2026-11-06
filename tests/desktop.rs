//! Original MIT guest allocator/XMS controls at the real FreeCOM prompt (#132).
mod common;
use pc98_rust::{Machine, Profile};

fn until(m: &mut Machine, expected: &str) -> bool {
    let mut hma_executed = false;
    for _ in 0..1000 {
        if m.bus.text().contains(expected) {
            return hma_executed;
        }
        assert!(
            !m.bus.text().contains("XMSCHECK FAILED"),
            "{}",
            m.bus.text()
        );
        assert!(!m.bus.text().contains("MEMSIZE FAILED"), "{}", m.bus.text());
        m.step_scheduler_with_observer(80_000, 100_000, |cpu, bus| {
            let linear = usize::from(cpu.s[1]) * 16 + cpu.ip as usize;
            if !cpu.protected_mode() && (0x100000..0x10fff0).contains(&linear) {
                assert!(bus.a20);
                hma_executed = true;
            }
        });
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
    }
    panic!("missing {expected}: {}", m.bus.text());
}
fn command(m: &mut Machine, text: &str) {
    for ch in text.bytes().chain(*b"\r") {
        let scan = if ch == b'\r' {
            0x1c
        } else {
            [(0x10, "qwertyuiop"), (0x1d, "asdfghjkl"), (0x29, "zxcvbnm")]
                .iter()
                .find_map(|(base, keys)| keys.bytes().position(|b| b == ch).map(|i| base + i as u8))
                .expect("supported command letter")
        };
        m.bus.key(scan, true);
        common::advance(m, 50_000);
        m.bus.key(scan, false);
        common::advance(m, 10_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
    }
}
fn image(high: bool) -> Vec<u8> {
    let mut disk = std::fs::read("web/freedos/desktop.bin").expect("prepare_desktop.py");
    if !high {
        // Same FAT16 disk, shell and diagnostic; only disable the manager and
        // restore the exact previous desktop configuration. Leave old fixtures.
        let root = &disk[4096 + 770 * 512..4096 + 802 * 512];
        let entry = root
            .as_chunks::<32>()
            .0
            .iter()
            .find(|e| &e[..11] == b"FDCONFIGSYS")
            .unwrap();
        let cluster = u16::from_le_bytes(entry[26..28].try_into().unwrap()) as usize;
        let size = u32::from_le_bytes(entry[28..32].try_into().unwrap()) as usize;
        let start = 4096 + 802 * 512 + (cluster - 2) * 4096;
        disk[start..start + size].fill(0x1a); // DOS text EOF after configuration
        let low = b"DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n";
        disk[start..start + low.len()].copy_from_slice(low);
    }
    disk.resize(134_221_824, 0);
    disk
}
#[test]
fn desktop_allocator_baseline_and_high_dos_cold_reset_without_leaks() {
    for high in [false, true] {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.insert(2, image(high), "desktop.hdi").unwrap();
        m.set_boot_hdd(true);
        for run in 0..2 {
            m.reset();
            let hma = until(&mut m, "A:\\>");
            assert_eq!(hma, high, "actual instruction execution in HMA");
            assert_eq!(m.bus.a20, high);
            if high {
                assert!(m.bus.text().contains("NEC PC-98 version"));
                assert!(m.bus.text().contains("19836 Bytes in HMA"));
                command(&mut m, "xmscheck");
                until(&mut m, "XMSCHECK HMA/A20/move/invalid/recovery/free OK");
            }
            let (paragraphs, bytes) = if high {
                (38_729, 619_664)
            } else {
                (31_116, 497_856)
            };
            assert_eq!(paragraphs * 16, bytes);
            if high {
                assert!(bytes >= 552_960);
            }
            for _ in 0..2 {
                command(&mut m, "cls");
                command(&mut m, "memsize");
                until(
                    &mut m,
                    &format!(
                        "MEMSIZE paragraphs={paragraphs} bytes={bytes} allocated/freed/restored OK"
                    ),
                );
                until(&mut m, "A:\\>");
            }
            println!("DOS high={high} reset={run}: {paragraphs} paragraphs, {bytes} bytes");
            assert_eq!(m.cpu.fault_count, 0);
            assert!(m.cpu.irq_acks > 0);
            assert_eq!((m.bus.ide.writes, m.bus.fdc.reads), (0, 0));
        }
    }
}

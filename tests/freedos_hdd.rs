//! GPL FreeDOS(98) HDD boot control; original MIT test/helper, Issue #111.
use pc98_rust::{Machine, Profile};

fn image() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/freedos/shell.hdi"
    ))
    .expect("run python3 tools/prepare_freedos_hdd_fixture.py")
}
fn until(m: &mut Machine, condition: impl Fn(&Machine) -> bool) {
    for _ in 0..80 {
        if condition(m) {
            return;
        }
        m.step(100_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
    }
    assert!(
        condition(m),
        "bounded guest progress failed: {}",
        m.bus.text()
    );
}
fn type_keys(m: &mut Machine, scans: &[u8]) {
    for &scan in scans {
        m.bus.key(scan, true);
        m.step(50_000);
        m.bus.key(scan, false);
        m.step(10_000);
        assert!(m.error.is_none(), "{:?}", m.error);
    }
}

#[test]
fn cleared_hdd_loads_exact_kernel_then_runs_shell_commands_cold_and_reset() {
    let disk = image();
    // Independent expected geometry/BPB/FAT values; do not infer them from code
    // executing in the emulator. The pinned builder verifies hashes separately.
    assert_eq!(&disk[4096 + 510..4096 + 512], &[0x55, 0xaa]);
    assert_eq!(&disk[4096 + 512..4096 + 514], &[0xa1, 0x81]);
    let volume = &disk[4096 + 128 * 512..];
    assert_eq!(&volume[11..14], &[0, 2, 4]);
    assert_eq!(&volume[2 * 512..2 * 512 + 3], &[0xf8, 0xff, 0xff]);
    assert!(
        volume[2 * 512..8 * 512] == volume[8 * 512..14 * 512],
        "FAT copies differ"
    );
    let root = &volume[14 * 512..26 * 512];
    assert_eq!(&root[..11], b"KERNEL  SYS");
    assert_eq!(u32::from_le_bytes(root[28..32].try_into().unwrap()), 49938);
    let kernel = &volume[26 * 512..26 * 512 + 49938];
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(2, disk.clone(), "shell.hdi").unwrap();
    for run in 0..2 {
        m.reset();
        let mut handoff = false;
        for slots in 0..100_000 {
            m.step_with_observer(1, |c, b| {
                if c.s[1] == 0x60 && c.ip == 0 {
                    assert!(
                        &b.mem[0x600..0x600 + kernel.len()] == kernel,
                        "loaded kernel differs from pinned file"
                    );
                    assert_eq!((c.s[2], c.r[4]), (0x1e80, 0x100));
                    assert_eq!(b.mem[0x584], 0x80);
                    assert_eq!(b.read16(0x3fe), 0);
                    assert!(!c.protected_mode() && !c.vm86() && !c.paging_enabled());
                    assert!(!b.a20);
                    assert!(
                        b.text().contains("FreeDOS(98) FAT Kernel GO!"),
                        "{}",
                        b.text()
                    );
                    assert_eq!(b.ide.reads, 122); // 1 IPL + 1 table + 2 boot + 12 root + 6 FAT + 100 kernel
                    let null_cells = (0..2000)
                        .filter(|&cell| b.read16(0xa0000 + cell * 2) == 0)
                        .count();
                    println!("handoff text: {null_cells} raw NUL cells use the ANK fallback");
                    assert!(null_cells > 1900);
                    assert!(!pc98_rust::font::ank_supported(0));
                    assert_eq!(b.fdc.reads, 0);
                    handoff = true;
                }
            });
            assert!(m.error.is_none(), "{:?}", m.error);
            if handoff {
                println!("boot {run} handoff at {} slots", slots + 1);
                break;
            }
        }
        assert!(handoff, "never reached kernel entry");
        until(&mut m, |m| {
            m.bus.text().contains("HDD SHELL READY") && m.bus.text().contains("A:\\>")
        });
        assert!(m.bus.text().contains("FreeCom ver 0.85a_DBCS"));
        type_keys(&mut m, &[0x19, 0x13, 0x18, 0x2d, 0x12, 0x1c]); // PROBE
        until(&mut m, |m| m.bus.text().contains("PC98 DOS PROBE OK"));
        // ECHO OK: require its output line and a returned prompt, independently
        // of both the boot greeting and the command's input echo.
        type_keys(&mut m, &[0x12, 0x2b, 0x22, 0x18, 0x34, 0x18, 0x24, 0x1c]);
        until(&mut m, |m| {
            m.bus.text().lines().any(|line| line.trim() == "ok")
        });
        until(&mut m, |m| m.bus.text().matches("A:\\>").count() >= 3);
        assert!(
            m.cpu.retired <= 8_000_000,
            "boot/input exceeded the fixture budget"
        );
        assert_eq!(m.cpu.fault_count, 0);
        assert!(m.cpu.irq_acks > 0);
        assert!(m.bus.ide.reads > 122);
        assert_eq!((m.bus.ide.writes, m.bus.fdc.reads), (0, 0));
        assert!(
            m.bus.ide.disk.as_ref().unwrap().bytes == disk,
            "guest changed the control HDD"
        );
    }
}

use pc98_rust::{bus::Bus, Machine, Profile};

const MODES: [(Profile, u32, u32); 2] = [
    (Profile::Pc9801Vm, 320, 256),
    (Profile::Pc9821ApDos, 256, 200),
];

#[test]
fn horizontal_period_and_duty_follow_elapsed_clocks_on_both_ports() {
    for (profile, line, active) in MODES {
        let mut b = Bus::new(profile);
        // Sample every guest clock across eight complete lines, not just edges.
        for _ in 0..8 {
            let mut blank_clocks = 0;
            for phase in 0..line {
                let expected = if phase >= active { 0x40 } else { 0 };
                for port in [0x60, 0xa0] {
                    assert_eq!(b.input(port), 0x04 | expected, "{profile:?} {phase}");
                }
                blank_clocks += u32::from(expected != 0);
                b.tick(1);
            }
            assert_eq!(blank_clocks, line - active);
        }
    }
}

#[test]
fn time_advances_scan_without_reads_and_reads_do_not_advance_time() {
    for (profile, line, active) in MODES {
        let mut b = Bus::new(profile);
        b.tick(7 * line + active - 1);
        for _ in 0..512 {
            assert_eq!(b.input(0x60), 4);
            assert_eq!(b.input(0xa0), 4);
        }
        assert_eq!(b.cycles, u64::from(7 * line + active - 1));
        b.tick(1);
        for _ in 0..512 {
            assert_eq!(b.input(0x60), 0x44);
            assert_eq!(b.input(0xa0), 0x44);
        }
        assert_eq!(b.cycles, u64::from(7 * line + active));
        b.tick(line - active);
        assert_eq!(b.input(0x60), 4);
        assert_eq!(b.input(0xa0), 4);

        let mut chunked = Bus::new(profile);
        for _ in 0..line {
            chunked.tick(8);
        }
        assert_eq!(b.cycles, chunked.cycles);
        assert_eq!(b.input(0x60), chunked.input(0x60));
        // Horizontal phase keeps running across the existing vertical boundary.
        b.tick(133_333 - b.cycles as u32);
        assert_eq!(b.input(0xa0) & 0x40 != 0, 133_333 % line >= active);
    }
}

#[test]
fn fifo_and_vertical_sync_status_are_preserved_through_horizontal_edges() {
    for (profile, line, active) in MODES {
        let mut b = Bus::new(profile);
        b.gdcs[0].cursor = 0x12345;
        b.output(0x62, 0xe0); // Five readback bytes only on the text GDC.

        // Cover both exact vertical edges, multiple frames, and both horizontal
        // states within vertical sync. No status read consumes the FIFO.
        for clock in 0..2 * 133_333 + 1 {
            let vertical = if clock % 133_333 > 125_000 { 0x20 } else { 0 };
            let horizontal = if clock % line >= active { 0x40 } else { 0 };
            assert_eq!(b.input(0x60), 0x05 | vertical | horizontal);
            assert_eq!(b.input(0xa0), 0x04 | vertical | horizontal);
            b.tick(1);
        }
        for byte in [0x45, 0x23, 1, 0, 0] {
            assert_eq!(b.input(0x62), byte);
        }
        assert_eq!(b.input(0x60) & 0x07, 4);
        b.output(0xa2, 0xe0);
        assert_eq!(b.input(0xa0) & 0x07, 5);
        assert_eq!(b.input(0x60) & 0x07, 4);
        for _ in 0..5 {
            b.input(0xa2);
        }
        assert_eq!(b.input(0xa0) & 0x07, 4);
    }
}

#[test]
fn ap_planar_figd_observes_plane_page_direction_pattern_and_busy() {
    fn gdc(bus: &mut Bus, command: u8, data: &[u8]) {
        bus.output(0xa2, command);
        for &byte in data {
            bus.output(0xa0, byte);
        }
    }

    let mut b = Bus::new(Profile::Pc9821ApDos);
    // Page 1, red plane, word 80, dot 14. The line crosses a byte and
    // word boundary, with one dependent step at each D==0 tie.
    b.output(0xa6, 1);
    gdc(&mut b, 0x49, &[80, 0x80, 0xe0]);
    gdc(&mut b, 0x78, &[0xff, 0xff]);
    gdc(&mut b, 0x23, &[]);
    gdc(&mut b, 0x4c, &[9, 4, 0, 0, 0, 0xfc, 0x3f, 4, 0]);
    gdc(&mut b, 0x6c, &[]);
    let red = 0xb0000;
    for (x, y) in [(14, 2), (15, 3), (16, 3), (17, 4), (18, 4)] {
        assert_ne!(b.read(red + y * 80 + x / 8) & (0x80 >> (x & 7)), 0);
    }
    assert_eq!(b.read(red + 2 * 80 + 1), 0x02);
    assert_eq!(b.read(red + 3 * 80 + 2), 0x80);
    assert_eq!(b.read(red + 4 * 80 + 2), 0x60);
    assert_eq!(b.read(0xa8000 + 2 * 80 + 1), 0);
    b.output(0xa6, 0);
    assert_eq!(b.read(red + 2 * 80 + 1), 0);
    b.output(0xa6, 1);
    assert_eq!(b.display_page, 0);

    b.tick(31);
    for _ in 0..16 {
        assert_eq!(b.input(0x60) & 8, 0);
        assert_eq!(b.input(0xa0) & 8, 8);
    }
    b.tick(1);
    assert_eq!(b.input(0xa0) & 8, 0);

    // FIGD has reset FIGS to a single dot at CURS. XOR with pattern bit 0
    // clears that pixel, while a zero replace bit clears it unconditionally.
    gdc(&mut b, 0x21, &[]);
    gdc(&mut b, 0x6c, &[]);
    assert_eq!(b.read(red + 2 * 80 + 1), 0);
    gdc(&mut b, 0x23, &[]);
    gdc(&mut b, 0x6c, &[]);
    assert_eq!(b.read(red + 2 * 80 + 1), 2);
    gdc(&mut b, 0x78, &[0, 0]);
    gdc(&mut b, 0x20, &[]);
    gdc(&mut b, 0x6c, &[]);
    assert_eq!(b.read(red + 2 * 80 + 1), 0);
    gdc(&mut b, 0, &[]);
    assert_eq!(b.input(0xa0) & 8, 0);
}

#[test]
fn reset_and_display_commands_have_documented_fixed_scan_phase() {
    for (profile, _, active) in MODES {
        let mut m = Machine::new(profile);
        assert_eq!(m.bus.cycles, 0);
        assert_eq!(m.bus.input(0x60), 4);
        m.bus.tick(active);
        // RESET, SYNC, display off/on and PITCH do not retime this fixed mode.
        for port in [0x60, 0xa0] {
            for command in [0x00, 0x0e, 0x0f, 0x0c, 0x0d, 0x47] {
                m.bus.output(port + 2, command);
                m.bus.output(port, 40);
                assert_eq!(m.bus.input(port), 0x44);
            }
        }
        m.reset();
        assert_eq!(m.bus.cycles, 0);
        assert_eq!(m.bus.input(0x60), 4);
        assert_eq!(m.bus.input(0xa0), 4);
        m.bus.tick(active - 1);
        assert_eq!(m.bus.input(0x60), 4);
        m.bus.tick(1);
        assert_eq!(m.bus.input(0xa0), 0x44);
    }
}

#[test]
fn original_dos_guest_observes_both_gdc_transitions_and_returns_to_shell() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/freedos/hblank.hdm"
    ))
    .expect("run python3 tools/prepare_freedos.py before cargo test --test gdc");
    for (profile, _, _) in MODES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "hblank.hdm").unwrap();
        m.reset();
        for _ in 0..160 {
            m.step(100_000);
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("HBLANK PROBE RETURNED") && m.bus.text().contains("A:\\>") {
                break;
            }
        }
        let text = m.bus.text();
        for expected in [
            "TEXT GDC 0060 PASS",
            "BITMAP GDC 00A0 PASS",
            "HBLANK TRANSITION PASS",
            "HBLANK PROBE RETURNED",
            "A:\\>",
        ] {
            assert!(text.contains(expected), "{profile:?}: {expected}: {text}");
        }
        assert!(!text.contains("TIMEOUT"), "{text}");
        assert!(m.bus.gdcs.iter().all(|gdc| gdc.enabled));
    }
}

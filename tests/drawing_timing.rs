use pc98_rust::{cpu::*, firmware, Machine, Profile};

fn request(m: &mut Machine, ch: u8) -> Result<(), String> {
    m.cpu.s[CS] = 0xf000;
    m.cpu.ip = 0x1062; // Actual INT 18 synthetic ROM trap.
    m.cpu.r[AX] = 0x4a5a;
    m.cpu.r[CX] = u16::from(ch) << 8 | 0xab;
    firmware::service(&mut m.cpu, &mut m.bus)
}

#[test]
fn timing_switches_preserve_registers_ram_gdc_and_scan_state() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    assert!(!m.bus.gdcs[1].draw_during_display);
    assert_eq!(m.bus.mem[0x54d] & 8, 0);
    m.bus.mem[0x54d] = 0xf7; // Every unrelated bit must survive.
    m.bus.gdcs[1].enabled = true;
    m.bus.gdcs[1].cursor = 0x12345;
    m.bus.gdcs[1].start = 13;
    m.bus.output(0xa2, 0xe0); // Pending readback must survive too.
    m.bus.tick(125_127); // Active horizontal interval during vertical blank.
    m.cpu.r = [0, 0, 0x2468, 0x1357, 0x8000, 0x579b, 0x3579, 0x468a];
    m.cpu.s = [0x2345, 0xf000, 0x3456, 0x1234];
    m.cpu.upper = [
        0x1234, 0x2345, 0x3456, 0x4567, 0x5678, 0x6789, 0x789a, 0x89ab,
    ];
    m.cpu.extra_segments = [0x5678, 0x9abc];
    m.cpu.flags = 0x647;
    m.cpu.flags_upper = 4;
    for ch in [0x06, 0x16, 0x16, 0x06, 0x06, 0x16] {
        m.cpu.r[AX] = 0x4a5a;
        m.cpu.r[CX] = u16::from(ch) << 8 | 0xab;
        m.cpu.ip = 0x1062;
        let cpu = format!("{:?}", m.cpu);
        let mut ram = m.bus.mem.clone();
        let gdcs = m.bus.gdcs.clone();
        let status = [m.bus.input(0x60), m.bus.input(0xa0)];
        let frame = m.render().to_vec();
        let cycles = m.bus.cycles;
        request(&mut m, ch).unwrap();
        assert_eq!(format!("{:?}", m.cpu), cpu);
        let flash = ch == 0x06;
        ram[0x54d] = if flash { 0xff } else { 0xf7 };
        assert_eq!(m.bus.mem, ram);
        assert_eq!(m.bus.gdcs[1].draw_during_display, flash);
        assert!(!m.bus.gdcs[0].draw_during_display);
        for (g, before) in m.bus.gdcs.iter().zip(&gdcs) {
            assert_eq!(g.enabled, before.enabled);
            assert_eq!(g.command, before.command);
            assert_eq!(g.params, before.params);
            assert_eq!(g.fifo, before.fifo);
            assert_eq!(
                (g.cursor, g.start, g.pitch),
                (before.cursor, before.start, before.pitch)
            );
            assert_eq!(
                (g.cursor_visible, g.cursor_top, g.cursor_bottom),
                (
                    before.cursor_visible,
                    before.cursor_top,
                    before.cursor_bottom
                )
            );
        }
        assert_eq!(m.bus.cycles, cycles);
        assert_eq!([m.bus.input(0x60), m.bus.input(0xa0)], status);
        assert_eq!(m.render(), frame);
    }
}

#[test]
fn graphics_mode_and_display_commands_preserve_timing_and_reset_clears_it() {
    for ch in [0x06, 0x16] {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        request(&mut m, ch).unwrap();
        for ah in [0x42u8, 0x40, 0x41, 0x42] {
            m.cpu.r[AX] = u16::from(ah) << 8;
            m.cpu.r[CX] = 0xc000;
            firmware::service(&mut m.cpu, &mut m.bus).unwrap();
            assert_eq!(m.bus.mem[0x54d] & 8 != 0, ch == 0x06);
            assert_eq!(m.bus.gdcs[1].draw_during_display, ch == 0x06);
        }
        m.reset();
        assert_eq!(m.bus.mem[0x54d] & 8, 0);
        assert!(!m.bus.gdcs[1].draw_during_display);
        assert_eq!(m.bus.cycles, 0);
        assert_eq!(m.bus.input(0xa0), 4);
    }
}

#[test]
fn unsupported_ch_and_vm_requests_fail_without_mutating_state() {
    for profile in [Profile::Pc9821ApDos, Profile::Pc9801Vm] {
        let mut m = Machine::new(profile);
        if profile == Profile::Pc9821ApDos {
            request(&mut m, 0x06).unwrap();
        }
        for ch in 0..=255 {
            if profile == Profile::Pc9821ApDos && [0x06, 0x16].contains(&ch) {
                continue;
            }
            m.cpu.s[CS] = 0xf000;
            m.cpu.ip = 0x1062;
            m.cpu.r[AX] = 0x4a5a;
            m.cpu.r[CX] = u16::from(ch) << 8 | 0xab;
            let cpu = format!("{:?}", m.cpu);
            let ram = m.bus.mem.clone();
            let flash = m.bus.gdcs[1].draw_during_display;
            let error = request(&mut m, ch).unwrap_err();
            assert!(error.contains(if profile == Profile::Pc9821ApDos {
                "unsupported graphics drawing timing"
            } else {
                "unsupported BIOS INT 18 AH=4a"
            }));
            assert_eq!(format!("{:?}", m.cpu), cpu);
            assert_eq!(m.bus.mem, ram);
            assert_eq!(m.bus.gdcs[1].draw_during_display, flash);
            assert_eq!(m.bus.cycles, 0);
        }
    }
}

fn boot(name: &str) -> Machine {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    let image = std::fs::read(format!(
        "{}/fixtures/freedos/{name}.hdm",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run python3 tools/prepare_freedos.py before cargo test --test drawing_timing");
    m.insert(0, image, name).unwrap();
    m.reset();
    m
}
fn until(m: &mut Machine, predicate: impl Fn(&Machine) -> bool) {
    for _ in 0..160 {
        if predicate(m) {
            return;
        }
        m.step(100_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
    }
    panic!("drawing guest exceeded 16 million slots: {}", m.bus.text());
}

#[test]
fn exact_original_control_and_switching_probe_return_through_real_dos() {
    let mut control = boot("drawmode");
    until(&mut control, |m| {
        m.bus.text().contains("PROBE SHELL RETURN") && m.bus.text().contains("A:\\>")
    });
    assert!(control.bus.text().contains("DRAWMODE RETURNED"));
    assert!(control.bus.gdcs[1].draw_during_display);
    assert_ne!(control.bus.mem[0x54d] & 8, 0);

    let mut m = boot("drawtest");
    for stage in 1..=4 {
        until(&mut m, |m| m.bus.mem[0x90000] == stage);
        let flash = stage == 1 || stage == 4;
        assert_eq!(m.bus.mem[0x90000 + usize::from(stage)] & 8 != 0, flash);
        assert_eq!(m.bus.mem[0x54d] & 8 != 0, flash);
        assert_eq!(m.bus.gdcs[1].draw_during_display, flash);
        m.bus.key(0x1c, true);
        m.bus.key(0x1c, false);
    }
    until(&mut m, |m| {
        m.bus.text().contains("PROBE SHELL RETURN") && m.bus.text().contains("A:\\>")
    });
    assert!(
        m.bus.text().contains("DRAW TIMING PASS"),
        "{}",
        m.bus.text()
    );
    assert!(!m.bus.text().contains("FAILED"));
    m.reset();
    assert_eq!(m.bus.mem[0x90000], 0);
    assert_eq!(m.bus.mem[0x54d] & 8, 0);
    assert!(!m.bus.gdcs[1].draw_during_display);
    until(&mut m, |m| m.bus.mem[0x90000] == 1);
    assert!(m.bus.gdcs[1].draw_during_display);
}

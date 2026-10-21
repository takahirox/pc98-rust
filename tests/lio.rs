use pc98_rust::{cpu::*, Machine, Profile};

const PROFILES: [Profile; 2] = [Profile::Pc9801Vm, Profile::Pc9821ApDos];

fn call(m: &mut Machine, entry: usize, work: u16) {
    let off = m.bus.read16(0xf9906 + entry * 4);
    m.bus.write_seg(0, 0x66 * 4, off, true);
    m.bus.write_seg(0, 0x66 * 4 + 2, 0xf990, true);
    m.bus.mem[0x100..0x103].copy_from_slice(&[0xcd, 0x66, 0xf4]);
    m.cpu.s = [0x4567, 0, 0x3000, work];
    m.cpu.r = [
        0xffab, 0x1234, 0x2345, 0x3456, 0x8000, 0x5678, 0x6789, 0x789a,
    ];
    m.cpu.flags = DF | CF | 2;
    m.cpu.ip = 0x100;
    m.cpu.halted = false;
    m.step(3); // alternate guest INT, ROM OUT, IRET
}

#[test]
fn rom_directory_matches_databook_and_is_read_only_and_restored_on_reset() {
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        assert_eq!(&m.bus.mem[0xf9900..0xf9904], &[17, 0, 0, 0]);
        for i in 0..18 {
            let id = match i {
                0..=15 => 0xa0 + i,
                16 => 0xce,
                _ => 0,
            };
            assert_eq!(m.bus.read16(0xf9904 + i * 4), id as u16);
            let off = m.bus.read16(0xf9906 + i * 4) as usize;
            assert!(off >= 76 && off + 4 < 0x66f0);
            assert_eq!(
                &m.bus.mem[0xf9900 + off..0xf9904 + off],
                &[0xe6, 0xf0, 0xcf, 0x90]
            );
            m.bus.write(0xf9900 + off, 0);
            assert_eq!(m.bus.read(0xf9900 + off), 0xe6);
        }
        let directory = m.bus.mem[0xf9900..0xf994c].to_vec();
        m.bus.mem[0xf9900..0xf994c].fill(0); // host corruption is discarded too
        m.reset();
        assert_eq!(&m.bus.mem[0xf9900..0xf994c], directory);
        // Initialization does not seize DOS/user vectors for LIO.
        assert_eq!(m.bus.read16(0x66 * 4 + 2), 0xf000);
        assert_eq!(m.bus.read16(0xa0 * 4 + 2), 0xf000);
    }
}

#[test]
fn ginit_restores_defaults_preserves_frame_gaps_vram_and_repeats() {
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        for work in [0x2000, 0x2000, 0x4000, 0x9ee0] {
            let base = work as usize * 16;
            m.bus.mem[base - 16..base + 0x1210].fill(0xa5);
            m.bus.mem[0x37f70..0x38010].fill(0x5a);
            m.bus.graphics.fill(0x6d);
            m.bus.palette.fill([17, 34, 51]);
            m.bus.analog = true;
            m.bus.pegc = profile == Profile::Pc9821ApDos;
            m.bus.egc.enabled = true;
            m.bus.grcg_mode = 0xff;
            m.bus.display_page = 1;
            m.bus.cpu_page = 1;
            m.bus.gdcs[1].enabled = false;
            m.bus.gdcs[1].start = 8000;
            m.bus.gdcs[1].pitch = 1;
            call(&mut m, 0, work);
            assert!(m.error.is_none(), "{:?}", m.error);
            assert_eq!(m.cpu.byte(4), 0);
            assert_eq!(
                (m.cpu.s[DS], m.cpu.s[SS], m.cpu.r[SP]),
                (work, 0x3000, 0x8000)
            );
            assert_eq!((m.cpu.s[CS], m.cpu.ip), (0, 0x102));
            assert_eq!(m.cpu.flags & (DF | CF), DF | CF);
            // Only the interrupt frame can consume stack space.
            assert!(m.bus.mem[0x37f70..0x37ffa].iter().all(|&x| x == 0x5a));
            assert!(m.bus.mem[0x38000..0x38010].iter().all(|&x| x == 0x5a));
            for range in [
                base - 16..base + 0x620,
                base + 0x6a0..base + 0x1000,
                base + 0x1200..base + 0x1210,
            ] {
                assert!(m.bus.mem[range].iter().all(|&x| x == 0xa5));
            }
            // Original opaque work format: full inclusive 640x200 viewport.
            assert_eq!(
                &m.bus.mem[base + 0x620..base + 0x630],
                &[0, 0, 0, 1, 0, 7, 0, 0, 0, 0, 0, 0, 127, 2, 199, 0]
            );
            assert!(m.bus.mem[base + 0x630..base + 0x6a0]
                .iter()
                .all(|&x| x == 0));
            assert!(m.bus.mem[base + 0x1000..base + 0x1200]
                .iter()
                .all(|&x| x == 0));
            assert_eq!((m.bus.cpu_page, m.bus.display_page), (0, 0));
            assert!(!m.bus.analog && !m.bus.pegc && !m.bus.egc.enabled);
            assert_eq!(m.bus.grcg_mode, 0);
            assert!(m.bus.graphics_200 && m.bus.gdcs[1].enabled);
            assert_eq!((m.bus.gdcs[1].start, m.bus.gdcs[1].pitch), (0, 40));
            assert_eq!(m.bus.read(0x54d), 0x40);
            assert_ne!(m.bus.read(0x54c) & 0x80, 0);
            for (i, rgb) in [
                [0, 0, 0],
                [0, 0, 255],
                [255, 0, 0],
                [255, 0, 255],
                [0, 255, 0],
                [0, 255, 255],
                [255, 255, 0],
                [255, 255, 255],
                [119, 119, 119],
                [0, 0, 170],
                [170, 0, 0],
                [170, 0, 170],
                [0, 170, 0],
                [0, 170, 170],
                [170, 170, 0],
                [170, 170, 170],
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(m.bus.palette[i], rgb);
            }
            // GINIT defines the view; it does not execute GCLS or erase VRAM.
            assert!(m.bus.graphics.iter().all(|&x| x == 0x6d));
        }
        m.reset();
        assert!(!m.bus.graphics_200);
        assert!(m.bus.mem[0x20620..0x206a0].iter().all(|&x| x == 0));
        call(&mut m, 0, 0x2000);
        assert!(m.error.is_none());
        assert!(m.bus.graphics_200);
    }
}

#[test]
fn ginit_renders_first_logical_screen_with_doubled_lines_and_keeps_text_independent() {
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.bus.gdcs[0].enabled = true;
        call(&mut m, 0, 0x2000);
        assert!(m.bus.gdcs[0].enabled && m.bus.text_enabled);
        m.bus.text_enabled = false;
        m.bus.graphics[0] = 0x80; // first row, blue
        m.bus.graphics[0x8000 + 80] = 0x80; // second row, red
        m.bus.graphics[0x10000 + 199 * 80] = 0x80; // final row, green
        m.bus.graphics[16000] = 0xff; // second logical screen must stay invisible
        let pixels = m.render();
        for (y, rgb) in [
            (0, [0, 0, 255, 255]),
            (1, [0, 0, 255, 255]),
            (2, [255, 0, 0, 255]),
            (3, [255, 0, 0, 255]),
            (4, [0, 0, 0, 255]),
            (398, [0, 255, 0, 255]),
            (399, [0, 255, 0, 255]),
        ] {
            assert_eq!(&pixels[y * 640 * 4..y * 640 * 4 + 4], &rgb);
        }
    }
}

#[test]
fn unsupported_entries_invalid_work_and_spoofed_traps_fail_explicitly() {
    for profile in PROFILES {
        for entry in 1..18 {
            let mut m = Machine::new(profile);
            m.bus.mem[0x20000..0x21200].fill(0xa5);
            call(&mut m, entry, 0x2000);
            assert!(m.error.as_ref().unwrap().contains("unsupported Graph LIO"));
            assert!(m.bus.mem[0x20000..0x21200].iter().all(|&x| x == 0xa5));
            assert!(!m.bus.graphics_200);
        }
        for work in [0, 0x9ee1, 0xa800, 0xf990, 0x37f0] {
            let mut m = Machine::new(profile);
            let graphics = m.bus.graphics.clone();
            call(&mut m, 0, work);
            assert!(m
                .error
                .as_ref()
                .unwrap()
                .contains("unsupported GINIT work area"));
            assert_eq!(m.bus.graphics, graphics);
            assert!(!m.bus.graphics_200);
        }
        let mut m = Machine::new(profile);
        m.cpu.s[CS] = 0xf990;
        m.cpu.ip = 0x103; // deliberately unaligned OUT, outside an entry
        assert!(pc98_rust::firmware::service(&mut m.cpu, &mut m.bus).is_err());
    }
}

#[test]
fn original_controls_return_through_real_dos_without_a_second_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/freedos/lio.hdm"
    ))
    .expect("run python3 tools/prepare_freedos.py");
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "lio.hdm").unwrap();
        m.reset();
        let mut reset_entries = 0;
        let mut statuses = Vec::new();
        for _ in 0..16_000_000 {
            if Cpu::phys(m.cpu.s[CS], m.cpu.ip as u16) == 0xffff0 {
                reset_entries += 1;
            }
            if m.cpu.s[CS] == 0xf990 && m.cpu.ip == 0x102 {
                statuses.push(m.cpu.byte(4));
            }
            m.step(1);
            if m.error.is_some() {
                break;
            }
        }
        assert!(m.error.is_none(), "{:?}", m.error);
        let text = m.bus.text();
        for message in [
            "LIO ENTER",
            "LIO RETURNED",
            "PROBE SHELL RETURN",
            "GINIT CONTRACT PASS",
            "A:\\>",
        ] {
            assert!(text.contains(message), "{profile:?}: {text}");
        }
        assert!(!text.contains("FAILED"), "{text}");
        assert_eq!(reset_entries, 1);
        assert_eq!(statuses, [0, 0, 0]);
        assert!(m.bus.fdc.reads > 100);
        let pixels = m.render();
        for (y, rgb) in [
            (300, [0, 0, 255, 255]),
            (301, [0, 0, 255, 255]),
            (302, [255, 0, 0, 255]),
            (303, [255, 0, 0, 255]),
        ] {
            let p = (y * 640 + 632) * 4;
            assert_eq!(&pixels[p..p + 4], &rgb);
        }
    }
}

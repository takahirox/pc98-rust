use pc98_rust::{cpu::*, firmware, font, Machine, Profile};

// Original asymmetric pixels: no ROM, game or third-party glyph data.
fn pattern() -> [u8; 32] {
    std::array::from_fn(|i| {
        if i % 2 == 0 {
            0x80 >> (i / 2 % 8)
        } else {
            (i as u8 * 7) ^ 0x53
        }
    })
}
fn registered_pattern(row: u16, cell: u16) -> [u8; 32] {
    let mut bytes = pattern();
    bytes[0] = row as u8;
    bytes[1] = cell as u8;
    bytes
}
fn service(m: &mut Machine, ah: u8) -> Result<(), String> {
    m.cpu.s[CS] = 0xf000;
    m.cpu.ip = 0x1062; // INT 18 synthetic ROM trap
    m.cpu.set_byte(4, ah);
    firmware::service(&mut m.cpu, &mut m.bus)
}
fn select(m: &mut Machine, code: u16) {
    m.bus.output(0xa1, code as u8);
    m.bus.output(0xa3, (code >> 8) as u8 - 0x20);
}
fn assert_reads(m: &mut Machine, code: u16, expected: &[u8; 32]) {
    select(m, code);
    m.bus.output(0x68, 0x0b);
    for (i, &byte) in expected.iter().enumerate() {
        m.bus
            .output(0xa5, (i / 2) as u8 | if i % 2 == 0 { 32 } else { 0 });
        assert_eq!(m.bus.input(0xa9), byte, "port byte {i}");
        assert_eq!(m.bus.read(0xa4001 + (i / 2) * 2), byte, "window byte {i}");
        assert_eq!(m.bus.read(0xa4fe1 + (i / 2) * 2), byte, "mirrored byte {i}");
        assert_eq!(m.bus.read(0xa4000 + (i / 2) * 2), 255);
    }
    m.cpu.r[BX] = 0x4800;
    m.cpu.r[CX] = 0x1234;
    m.cpu.r[DX] = code;
    m.bus.mem[0x49233..0x49257].fill(0xc7);
    service(m, 0x14).unwrap();
    assert_eq!(&m.bus.mem[0x49236..0x49256], expected);
    assert_eq!(&m.bus.mem[0x49233..0x49236], &[0xc7; 3]);
    assert_eq!(m.bus.mem[0x49256], 0xc7);
}
#[test]
fn bios_user_characters_addressing_endpoints_registers_and_reset() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    let original = pattern();
    // Exercise all 188 slots, including both row endpoints; no aliases.
    for row in 0x76..=0x77u16 {
        for cell in 0x21..=0x7eu16 {
            let code = row << 8 | cell;
            let bytes = registered_pattern(row, cell);
            // Offset wraps at 64 KiB, independently of unrelated DS and ES.
            m.cpu.r = [0x1a69, 0xffef, code, 0x3000, 0x8000, 0x1234, 0x4567, 0x89ab];
            m.cpu.s = [0x2222, 0xf000, 0x3333, 0x1111];
            m.cpu.upper = [0x1234; 8];
            m.cpu.extra_segments = [0x5678, 0x9abc];
            m.cpu.flags_upper = 4;
            m.cpu.flags = 0x643;
            for i in 0..36u16 {
                m.bus
                    .write_seg(0x3000, 0xffeeu16.wrapping_add(i), 0xc7, false);
            }
            for (i, byte) in bytes.iter().enumerate() {
                m.bus.write_seg(
                    0x3000,
                    0xfff1u16.wrapping_add(i as u16),
                    *byte as u16,
                    false,
                );
            }
            let before = m.bus.mem.clone();
            service(&mut m, 0x1a).unwrap();
            assert_eq!(
                m.cpu.r,
                [0x1a69, 0xffef, code, 0x3000, 0x8000, 0x1234, 0x4567, 0x89ab]
            );
            assert_eq!(m.cpu.s, [0x2222, 0xf000, 0x3333, 0x1111]);
            assert_eq!(m.cpu.upper, [0x1234; 8]);
            assert_eq!(m.cpu.extra_segments, [0x5678, 0x9abc]);
            assert_eq!(m.cpu.flags_upper, 4);
            assert_eq!(m.cpu.flags, 0x643);
            assert_eq!(m.bus.mem, before); // Scratch may be destroyed; ours leaves it intact.
            assert_reads(&mut m, code, &bytes);
        }
    }
    // Check again after every slot has been filled, detecting any storage alias.
    for row in 0x76..=0x77u16 {
        for cell in 0x21..=0x7eu16 {
            assert_reads(&mut m, row << 8 | cell, &registered_pattern(row, cell));
        }
    }
    // Neighboring definitions survive replacing a slot, and every reader changes.
    m.cpu.r[BX] = 0x3000;
    m.cpu.r[CX] = 0x80;
    m.cpu.r[DX] = 0x7621;
    m.bus.mem[0x30082..0x300a2].copy_from_slice(&original);
    service(&mut m, 0x1a).unwrap();
    assert_reads(&mut m, 0x7621, &original);
    assert_reads(&mut m, 0x7622, &registered_pattern(0x76, 0x22));
    let replacement: Vec<_> = (0..32).map(|i| font::jis_byte(0x24, 0x4f, i)).collect();
    for (i, &byte) in replacement.iter().enumerate() {
        assert_eq!(m.bus.user_characters.byte(0x24, 0x4f, i), byte);
    }
    m.reset();
    for row in 0x76..=0x77u16 {
        for cell in 0x21..=0x7eu16 {
            assert_reads(&mut m, row << 8 | cell, &[0; 32]);
        }
    }
    for (i, &byte) in replacement.iter().enumerate() {
        assert_eq!(m.bus.user_characters.byte(0x24, 0x4f, i), byte);
    }
    assert_eq!(font::row(b'A', 1), 0x38);
}
#[test]
fn invalid_codes_fail_explicitly_without_mutating_ram_or_replacement_fonts() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    let before = m.bus.mem.clone();
    // BIOS documentation gives a bounding range, but JIS cells are 21..7e.
    // No real-hardware error/status is specified for these invalid requests.
    for code in [
        0x7521, 0x7620, 0x767f, 0x7680, 0x7700, 0x7720, 0x777f, 0x7821, 0xffff, 0x244f, 0x0041,
    ] {
        m.cpu.r[DX] = code;
        assert!(service(&mut m, 0x1a)
            .unwrap_err()
            .contains("registration code"));
        assert_eq!(m.bus.user_characters.byte(0x76, 0x21, 0), 0);
    }
    assert_eq!(m.bus.mem, before);
    for row in 0x76..=0x77 {
        for cell in 0x21..=0x7e {
            for byte in 0..32 {
                assert_eq!(m.bus.user_characters.byte(row, cell, byte), 0);
            }
        }
    }
    for byte in 0..32 {
        assert_eq!(
            m.bus.user_characters.byte(0x24, 0x4f, byte),
            font::jis_byte(0x24, 0x4f, byte)
        );
    }
    let mut vm = Machine::default();
    assert!(service(&mut vm, 0x1a)
        .unwrap_err()
        .contains("unsupported BIOS"));
}

fn boot(name: &str) -> Machine {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(
        0,
        std::fs::read(format!(
            "{}/fixtures/freedos/{name}.hdm",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("run python3 tools/prepare_freedos.py and tools/prepare_font_fixture.py"),
        name,
    )
    .unwrap();
    m.reset();
    m
}
fn until(m: &mut Machine, predicate: impl Fn(&Machine) -> bool) {
    for _ in 0..180 {
        if predicate(m) {
            return;
        }
        m.step(100_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
    }
    panic!("guest did not finish: {}", m.bus.text());
}
fn assert_pixels(m: &mut Machine, bytes: &[u8; 32]) {
    let frame = m.render();
    for y in 0..16 {
        for x in 0..16 {
            let ink = bytes[y * 2 + x / 8] & (128 >> (x % 8)) != 0;
            for (left, color) in [(0, [255, 255, 255]), (32, [0, 0, 255])] {
                let i = ((160 + y) * 640 + left + x) * 4;
                assert_eq!(
                    &frame[i..i + 3],
                    if ink { &color } else { &[0; 3] },
                    "pixel {left}+{x},{y}"
                );
            }
        }
    }
}
#[test]
fn original_control_and_asymmetric_redefinition_run_through_real_dos() {
    let mut control = boot("fontload");
    until(&mut control, |m| m.bus.text().contains("FONTLOAD RETURNED"));
    // The exact control's leading two bytes are scratch, so its final row uses
    // its trailing zero word. This also detects ignoring the whole BIOS call.
    let mut expected = [0x81; 32];
    expected[30..].fill(0);
    assert_reads(&mut control, 0x7621, &expected);

    let mut m = boot("userfont");
    until(&mut m, |m| m.bus.mem[0x90000] == 1);
    assert_eq!(&m.bus.mem[0x90004..0x90024], &pattern());
    assert_eq!(&m.bus.mem[0x90040..0x90060], &pattern());
    assert_pixels(&mut m, &pattern());
    m.bus.key(0x1c, true);
    m.bus.key(0x1c, false);
    until(&mut m, |m| m.bus.mem[0x90000] == 2);
    let second = pattern().map(|v| !v);
    assert_eq!(&m.bus.mem[0x90004..0x90024], &second);
    assert_eq!(&m.bus.mem[0x90040..0x90060], &second);
    assert_pixels(&mut m, &second);
    m.bus.key(0x1c, true);
    m.bus.key(0x1c, false);
    until(&mut m, |m| m.bus.text().contains("USERFONT RETURNED"));
    assert!(!m.bus.text().contains("USERFONT FAILED"));
}

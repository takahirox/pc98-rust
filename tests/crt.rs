use pc98_rust::{cpu::*, firmware, font, Machine, Profile};

fn call(m: &mut Machine, ah: u8, al: u8, dx: u16) -> Result<(), String> {
    m.cpu.s[CS] = 0xf000;
    m.cpu.ip = 0x1062;
    m.cpu.r[AX] = u16::from_be_bytes([ah, al]);
    m.cpu.r[DX] = dx;
    firmware::service(&mut m.cpu, &mut m.bus)
}
fn pixel(m: &mut Machine, x: usize, y: usize) -> [u8; 3] {
    let i = (y * 640 + x) * 4;
    m.render()[i..i + 3].try_into().unwrap()
}
fn cell(m: &mut Machine, n: usize, ch: u16) {
    m.bus.write(0xa0000 + n * 2, ch as u8);
    m.bus.write(0xa0001 + n * 2, (ch >> 8) as u8);
    m.bus.write(0xa2000 + n * 2, 0xe1);
}
#[test]
fn text_row_masks_preserve_doubled_dots_attributes_and_bitmap_overlay() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for wide in [false, true] {
            let mut m = Machine::new(profile);
            m.bus.output(0x68, if wide { 3 } else { 2 });
            m.bus.output(0x68, 1); // Semigraphics: eight 4-by-4 blocks.
            m.bus.gdcs[0].enabled = true;
            m.bus.gdcs[0].pitch = 80;
            m.bus.gdcs[1].enabled = true;
            m.bus.graphics[..0x8000].fill(0xff); // Blue bitmap behind text.
            m.bus.palette[1] = [0, 0, 255];
            m.bus.palette[7] = [255; 3];
            let stride = if wide { 2 } else { 1 };
            let width = if wide { 16 } else { 8 };
            // Normal, reverse, underline, blink, hidden, and blinking cursor.
            for (i, attr) in [0xf1, 0xf5, 0xf9, 0xf3, 0xf0, 0xf3].into_iter().enumerate() {
                cell(&mut m, i * stride, 0x69); // Left, right, right, left blocks.
                m.bus.mem[0xa2000 + i * stride * 2] = attr;
                if wide {
                    // Odd VRAM words must never bleed into doubled characters.
                    cell(&mut m, i * stride + 1, 0);
                    m.bus.mem[0xa2000 + (i * stride + 1) * 2] = 0xe5;
                }
            }
            m.bus.gdcs[0].cursor = (5 * stride) as u32;
            m.bus.gdcs[0].cursor_visible = true;
            m.bus.gdcs[0].cursor_blink = false;
            m.bus.gdcs[0].cursor_top = 14;
            m.bus.gdcs[0].cursor_bottom = 15;
            m.bus.gdcs[0].blink_rate = 1;
            for (frame, masks) in [
                (
                    0,
                    [
                        [0xf0, 0x0f, 0x0f, 0xf0],
                        [0x0f, 0xf0, 0xf0, 0x0f],
                        [0xf0, 0x0f, 0x0f, 0xff],
                        [0xf0, 0x0f, 0x0f, 0xf0],
                        [0; 4],
                        [0xf0, 0x0f, 0x0f, 0x0f],
                    ],
                ),
                (
                    6,
                    [
                        [0xf0, 0x0f, 0x0f, 0xf0],
                        [0x0f, 0xf0, 0xf0, 0x0f],
                        [0xf0, 0x0f, 0x0f, 0xff],
                        [0; 4],
                        [0; 4],
                        [0, 0, 0, 0xff],
                    ],
                ),
            ] {
                m.bus.cycles = frame * 133_333;
                let pixels = m.render();
                for (i, rows) in masks.into_iter().enumerate() {
                    for (y, mask) in [0, 4, 8, 15].into_iter().zip(rows) {
                        for x in 0..width {
                            let dot = if wide { x / 2 } else { x };
                            let expected = if mask & (0x80 >> dot) != 0 {
                                [255, 255, 255, 255]
                            } else {
                                [0, 0, 255, 255]
                            };
                            let at = (y * 640 + i * width + x) * 4;
                            assert_eq!(
                                &pixels[at..at + 4],
                                &expected,
                                "{profile:?} wide={wide} frame={frame} cell={i} x={x} y={y}"
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn japanese_half_width_font_requests_are_rejected_without_touching_buffer_guards() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = Machine::new(profile);
        m.cpu.r[BX] = 0x3000;
        m.cpu.r[CX] = 0;
        // NEC p.48: these Japanese codes require 16 pattern bytes plus scratch.
        // The replacement font does not supply these half-width glyphs.
        for code in (0x2920..=0x297d).chain(0x2a20..=0x2a5f) {
            m.bus.mem[0x30000..0x30032].fill(0xa5);
            let error = call(&mut m, 0x14, 0, code).unwrap_err();
            assert!(error.contains("unsupported CRT half-width font"), "{error}");
            assert_eq!(&m.bus.mem[0x30000..0x30032], &[0xa5; 50]);
        }
    }
}

#[test]
fn two_byte_text_cursor_command_updates_readback_and_rendered_position() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = Machine::new(profile);
        call(&mut m, 0x0c, 0, 0).unwrap();
        call(&mut m, 0x10, 1, 0).unwrap(); // Full-cell, steady cursor.
        call(&mut m, 0x11, 0, 0).unwrap();
        // A previous graphics-sized address must not leak its top bits.
        m.bus.output(0x62, 0x49);
        for v in [0xff, 0xff, 3] {
            m.bus.output(0x60, v);
        }
        m.bus.output(0x62, 0x49);
        m.bus.output(0x60, 0x34);
        m.bus.output(0x60, 0x02);
        assert_eq!(m.bus.gdcs[0].cursor, 0x234);
        m.bus.output(0x62, 0xe0);
        for expected in [0x34, 0x02, 0, 0, 0] {
            assert_eq!(m.bus.input(0x62), expected);
        }
        // Word 0234h is column 4, row 7 at pitch 80.
        cell(&mut m, 0x234, b' ' as u16);
        cell(&mut m, 0, b' ' as u16);
        assert_eq!(pixel(&mut m, 32, 112), [255; 3]);
        assert_eq!(pixel(&mut m, 0, 0), [0; 3]);
        // The third byte remains available to the bitmap GDC's 18-bit address.
        m.bus.output(0xa2, 0x49);
        for v in [0x34, 0x02, 3] {
            m.bus.output(0xa0, v);
        }
        assert_eq!(m.bus.gdcs[1].cursor, 0x30234);
    }
}

#[test]
fn text_initialization_clears_all_words_including_selected_offscreen_area() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = Machine::new(profile);
        call(&mut m, 0x0c, 0, 0).unwrap();
        m.bus.mem[0xa0000..0xa4000].fill(0x11);
        cell(&mut m, 3000, b'Z' as u16);
        call(&mut m, 0x0e, 0, 6000).unwrap();
        call(&mut m, 0x16, 0, 0xe141).unwrap();
        for i in 0..4096 {
            assert_eq!(m.bus.read16(0xa0000 + i * 2), 0x0041, "cell {i}");
            assert_eq!(m.bus.read16(0xa2000 + i * 2), 0x00e1, "attr {i}");
        }
        assert_eq!(m.bus.gdcs[0].text_address(0, 0), Some((3000, 0)));
        assert_eq!(pixel(&mut m, 3, 1), [255; 3]); // A in selected area.
        assert_eq!(pixel(&mut m, 1, 1), [0; 3]); // Z would light this pixel.
    }
}

#[test]
fn normal_mode_set_sense_physical_columns_and_twenty_line_scanout() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = Machine::new(profile);
        assert_eq!(m.bus.mem[0x53c], 0);
        for al in 0..16 {
            call(&mut m, 0x0a, al, 0).unwrap();
            let sense = al & if profile == Profile::Pc9801Vm { 7 } else { 15 };
            assert_eq!(m.bus.mem[0x53c], sense);
            call(&mut m, 0x0b, 0, 0).unwrap();
            assert_eq!(m.cpu.byte(0), sense);
            assert_eq!(
                m.bus.gdcs[0].character_lines,
                if al & 1 == 0 { 16 } else { 20 }
            );
            assert_eq!(m.bus.gdcs[0].pitch, 80);
        }
        call(&mut m, 0x0a, 3, 0).unwrap(); // 40 columns, 20 rows
        call(&mut m, 0x0c, 0, 0).unwrap();
        // Forty columns use even physical words; y=20 starts the next row.
        cell(&mut m, 2, b'A' as u16);
        cell(&mut m, 80, b'A' as u16);
        assert_eq!(pixel(&mut m, 24, 1), [255; 3]);
        assert_eq!(pixel(&mut m, 25, 1), [255; 3]);
        assert_eq!(pixel(&mut m, 4, 21), [255; 3]);
        assert_eq!(pixel(&mut m, 4, 17), [0; 3]);
        // Direct flip-flop write changes scanout, without falsifying the BIOS table.
        m.bus.output(0x68, 2);
        assert_eq!(pixel(&mut m, 24, 1), [0; 3]);
        assert_eq!(m.bus.mem[0x53c], 3);
        call(&mut m, 0x0a, 4, 0).unwrap();
        cell(&mut m, 0, 1);
        m.bus.write(0xa2000, 0xf1);
        assert_eq!(pixel(&mut m, 0, 0), [255; 3]); // simple graphics block
        assert_eq!(pixel(&mut m, 4, 0), [0; 3]);
        call(&mut m, 0x0a, 0, 0).unwrap();
        assert_eq!(pixel(&mut m, 7, 0), [255; 3]); // vertical-line attribute
    }
}
#[test]
fn partitions_partial_updates_direct_pram_pitch_and_physical_wrap() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = Machine::new(profile);
        call(&mut m, 0x0c, 0, 0).unwrap();
        m.cpu.r[BX] = 0x3000;
        m.cpu.r[CX] = 0xfffc;
        for (i, word) in [0xfffu16, 1, 100, 2, 300, 3, 700, 19]
            .into_iter()
            .enumerate()
        {
            m.bus
                .write_seg(0x3000, 0xfffcu16.wrapping_add(i as u16 * 2), word, true);
        }
        call(&mut m, 0x0f, 0, 4).unwrap();
        cell(&mut m, 0xfff, b'A' as u16);
        cell(&mut m, 0, b'B' as u16);
        cell(&mut m, 100, b'C' as u16);
        assert_eq!(m.bus.gdcs[0].text_address(0, 1), Some((0, 0)));
        assert_eq!(m.bus.gdcs[0].text_address(16, 0), Some((100, 0)));
        assert_eq!(m.bus.gdcs[0].text_address(48, 0), Some((300, 0)));
        assert_eq!(m.bus.gdcs[0].text_address(96, 0), Some((700, 0)));
        assert_eq!(pixel(&mut m, 2, 1), [255; 3]);
        assert_eq!(pixel(&mut m, 2, 17), [255; 3]);
        let before = m.bus.gdcs[0].scroll;
        m.bus.write_seg(0x3000, 0xfffc, 200, true);
        call(&mut m, 0x0f, 0, 0x0101).unwrap();
        assert_eq!(&m.bus.gdcs[0].scroll[..4], &before[..4]);
        assert_eq!(&m.bus.gdcs[0].scroll[8..], &before[8..]);
        assert_eq!(m.bus.gdcs[0].text_address(16, 0), Some((200, 0)));
        // Guest-written PITCH and individual parameter-RAM bytes drive scanout too.
        m.bus.output(0x62, 0x47);
        m.bus.output(0x60, 100);
        m.bus.output(0x62, 0x74);
        for v in [40, 1, 0, 2] {
            m.bus.output(0x60, v);
        }
        assert_eq!(m.bus.gdcs[0].text_address(32, 0), Some((396, 0)));
        call(&mut m, 0x0e, 0, 0x1ffe).unwrap();
        assert_eq!(m.bus.gdcs[0].text_address(16, 0), Some((99, 0)));
        assert!(m.bus.gdcs[0].scroll[4..].iter().all(|&v| v == 0));
    }
}
#[test]
fn cursor_controls_share_csrform_visibility_and_exact_guest_frame_blink() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    call(&mut m, 0x1c, 0x40, 0x0e0f).unwrap();
    call(&mut m, 0x0c, 0, 0).unwrap();
    cell(&mut m, 0, b' ' as u16);
    call(&mut m, 0x13, 0, 0).unwrap();
    assert_eq!(pixel(&mut m, 0, 13), [0; 3]);
    assert_eq!(pixel(&mut m, 0, 14), [255; 3]);
    call(&mut m, 0x1e, 0x82, 0x0e0f).unwrap();
    for (frame, expected) in [(0, true), (3, true), (4, false), (7, false), (8, true)] {
        m.bus.cycles = frame * 133_333;
        assert_eq!(
            pixel(&mut m, 0, 14),
            if expected { [255; 3] } else { [0; 3] }
        );
    }
    call(&mut m, 0x12, 0, 0).unwrap();
    call(&mut m, 0x1e, 0, 0x0204).unwrap(); // must not show a hidden cursor
    assert!(!m.bus.gdcs[0].cursor_visible);
    call(&mut m, 0x11, 0, 0).unwrap();
    assert_eq!(pixel(&mut m, 0, 3), [255; 3]);
    assert_eq!(pixel(&mut m, 0, 14), [0; 3]);
    call(&mut m, 0x10, 1, 0xffff).unwrap(); // AL, not CH/CL or DH/DL
    assert_eq!(
        (m.bus.gdcs[0].cursor_top, m.bus.gdcs[0].cursor_bottom),
        (0, 15)
    );
    assert!(!m.bus.gdcs[0].cursor_blink);
    // Hardware CSRFORM is the same state, including row height and blink bits.
    m.bus.output(0x62, 0x4b);
    for v in [0x8f, 0x81, 0x23] {
        m.bus.output(0x60, v);
    }
    assert_eq!(
        (m.bus.gdcs[0].cursor_top, m.bus.gdcs[0].cursor_bottom),
        (1, 4)
    );
    assert_eq!(m.bus.gdcs[0].blink_rate, 14);
    assert!(m.bus.gdcs[0].cursor_blink);
}
#[test]
fn kcg_bios_and_ports_share_reversible_font_storage_and_text_restriction() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    call(&mut m, 0x0c, 0, 0).unwrap();
    m.cpu.r[BX] = 0x3000;
    m.cpu.r[CX] = 0;
    m.bus.mem[0x30002..0x30022].fill(0x80);
    call(&mut m, 0x1a, 0, 0x7621).unwrap();
    cell(&mut m, 0, 0x2156);
    assert_eq!(pixel(&mut m, 0, 0), [255; 3]);
    let graphics = m.bus.graphics.clone();
    for bios in [true, false, true] {
        if bios {
            call(&mut m, 0x1b, 1, 0).unwrap();
        } else {
            m.bus.output(0x68, 0x0b);
        }
        assert_eq!(pixel(&mut m, 0, 0), [0; 3]);
        for (row, col) in [(0x76, 0x21), (0x24, 0x22)] {
            m.bus.output(0xa1, col);
            m.bus.output(0xa3, row - 0x20);
            m.bus.output(0xa5, 32);
            let expected = if row == 0x76 {
                0x80
            } else {
                font::jis_byte(row, col, 0)
            };
            assert_eq!(m.bus.input(0xa9), expected);
            assert_eq!(
                m.bus.read(0xa4001),
                if row == 0x76 {
                    expected
                } else {
                    font::jis_byte(row, col, 1)
                }
            );
        }
        call(&mut m, 0x14, 0, 0x7621).unwrap();
        assert_eq!(&m.bus.mem[0x30002..0x30022], &[0x80; 32]);
        call(&mut m, 0x1b, 0, 0).unwrap();
        assert_eq!(m.bus.read(0xa4001), 255);
        assert_eq!(pixel(&mut m, 0, 0), [255; 3]);
        assert_eq!(m.bus.graphics, graphics);
    }
    // Correct BX:CX plus two scratch bytes for ANK, with independent ES.
    m.cpu.s[ES] = 0x4000;
    m.bus.mem[0x30000..0x30024].fill(0xa5);
    call(&mut m, 0x14, 0, 0x0041).unwrap();
    assert_eq!(&m.bus.mem[0x30000..0x30002], &[0xa5; 2]);
    for i in 0..8 {
        assert_eq!(m.bus.mem[0x30002 + i], font::row(b'A', i as u8 * 2 + 1));
    }
    assert_eq!(m.bus.mem[0x3000a], 0xa5);
}
#[test]
fn bounded_invalid_requests_are_atomic_and_registers_flags_and_graphics_survive() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = Machine::new(profile);
        m.cpu.r = [0, 0x3456, 0, 0x3000, 0x8000, 0x789a, 0xbcde, 0xf012];
        m.cpu.upper = [0x1234; 8];
        m.cpu.flags = 0x647;
        let graphics = m.bus.graphics.clone();
        for (ah, al, dx) in [
            (0x0a, 0, 0),
            (0x0e, 0x69, 0x1ffe),
            (0x10, 1, 0),
            (0x11, 0, 0),
            (0x12, 0, 0),
            (0x13, 0, 100),
            (0x1b, 0, 0),
        ] {
            call(&mut m, ah, al, dx).unwrap();
            assert_eq!(
                m.cpu.r,
                [
                    u16::from_be_bytes([ah, al]),
                    0x3456,
                    dx,
                    0x3000,
                    0x8000,
                    0x789a,
                    0xbcde,
                    0xf012
                ]
            );
            assert_eq!(m.cpu.upper, [0x1234; 8]);
            assert_eq!(m.cpu.flags, 0x647);
        }
        for (ah, al, dx) in [
            (0x0a, 0x80, 0),
            (0x0f, 0, 0),
            (0x0f, 0, 0x0302),
            (0x10, 2, 0),
            (0x1b, 2, 0),
            (0x1d, 79, 0),
            (0x1d, 81, 0),
            (0x1d, 164, 0),
            (0x1e, 0, 0x0f0e),
            (0x1e, 0, 0x0010),
            (0x1c, 0, 0x0010),
        ] {
            let before = m.bus.gdcs[0].clone();
            let mem = m.bus.mem.clone();
            assert!(
                call(&mut m, ah, al, dx).is_err(),
                "{profile:?} {ah:x} {al:x} {dx:x}"
            );
            assert_eq!(m.bus.mem, mem);
            assert_eq!(m.bus.gdcs[0].scroll, before.scroll);
            assert_eq!(m.bus.gdcs[0].pitch, before.pitch);
            assert_eq!(m.bus.gdcs[0].cursor_top, before.cursor_top);
        }
        assert_eq!(m.bus.graphics, graphics);
        if profile == Profile::Pc9801Vm {
            for (ah, al, dx) in [(0x1b, 1, 0), (0x1c, 0, 15), (0x1d, 80, 0), (0x1e, 0, 15)] {
                assert!(call(&mut m, ah, al, dx).is_err());
            }
        } else {
            for width in (80..=162).step_by(2) {
                call(&mut m, 0x1d, width, 0).unwrap();
                assert_eq!(
                    m.bus.gdcs[0].text_address(16, 0).unwrap().0,
                    (4095 + width as usize) & 4095
                );
            }
            call(&mut m, 0x1c, 0xec, 0x0407).unwrap();
            assert_eq!(m.cpu.r[AX], 0x1cec);
            assert_eq!(m.cpu.r[DX], 0x0407);
            assert_eq!(m.bus.mem[0x53c], 8);
            assert_eq!(m.bus.gdcs[0].pitch, 80);
        }
        m.reset();
        assert_eq!(m.bus.mem[0x53c], 0);
        assert_eq!(m.bus.gdcs[0].pitch, 80);
        assert_eq!(m.bus.gdcs[0].character_lines, 16);
        assert!(!m.bus.gdcs[0].cursor_visible);
        assert_eq!(m.bus.gdcs[0].text_address(399, 0), Some((1920, 15)));
    }
}

fn boot_probe(name: &str) -> Machine {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(
        0,
        std::fs::read(format!("fixtures/crt/{name}.hdm"))
            .expect("run python3 tools/prepare_crt_fixture.py"),
        "crt.hdm",
    )
    .unwrap();
    m
}
fn stage(m: &mut Machine, expected: u8) {
    for _ in 0..200 {
        if m.bus.mem[0x8001] == expected {
            return;
        }
        m.step(1000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
        assert_ne!(
            m.bus.mem[0x8001],
            255,
            "probe failed at {:04x}:{:04x}: {}",
            m.cpu.s[CS],
            m.cpu.ip,
            m.bus.text()
        );
    }
    panic!("probe stage {expected} timed out: {}", m.bus.text());
}
fn enter(m: &mut Machine) {
    m.bus.key(0x1c, true);
    m.bus.key(0x1c, false);
}
#[test]
fn original_guest_combined_crt_family_real_rom_path_pixels_guards_disk_and_reset() {
    let mut m = boot_probe("crt");
    let original = m.bus.disks[0].as_ref().unwrap().bytes.clone();
    for _ in 0..2 {
        stage(&mut m, 1);
        assert_eq!((m.bus.gdcs[0].pitch, m.bus.gdcs[0].cursor), (100, 4));
        assert_eq!(m.bus.gdcs[0].text_address(16, 0), Some((500, 0)));
        assert_eq!(m.bus.gdcs[0].text_address(32, 0), Some((600, 0)));
        assert_eq!(pixel(&mut m, 0, 0), [255; 3]); // original asymmetric user glyph
        assert_eq!(pixel(&mut m, 2, 17), [255; 3]); // A from second area
        assert_eq!(pixel(&mut m, 2, 33), [255; 3]); // B reached by pitch 100
        assert_eq!(pixel(&mut m, 32, 14), [255; 3]);
        assert_eq!(pixel(&mut m, 32, 13), [0; 3]);
        enter(&mut m);
        stage(&mut m, 2);
        assert_eq!(pixel(&mut m, 0, 0), [0; 3]);
        assert_eq!(pixel(&mut m, 2, 17), [255; 3]);
        assert_eq!(pixel(&mut m, 32, 14), [255; 3]);
        enter(&mut m);
        stage(&mut m, 3);
        // Only guest execution advances time. Observe both edges over a bounded
        // twelve-frame interval while blocked inside the real keyboard ROM.
        let mut states = [false; 2];
        for _ in 0..40 {
            let expected = (m.bus.cycles / 133_333 / 4) & 1 == 0;
            let value = pixel(&mut m, 32, 14);
            assert_eq!(value, if expected { [255; 3] } else { [0; 3] });
            states[usize::from(expected)] = true;
            m.step(5000);
            m.bus.audio.clear();
        }
        assert_eq!(states, [true, true]);
        enter(&mut m);
        stage(&mut m, 4);
        assert_eq!(m.bus.mem[0x53c], 3);
        assert_eq!(m.bus.gdcs[0].character_lines, 20);
        assert_eq!(pixel(&mut m, 0, 0), [255; 3]);
        assert_eq!(pixel(&mut m, 1, 0), [255; 3]);
        assert_eq!(pixel(&mut m, 0, 16), [0; 3]);
        enter(&mut m);
        stage(&mut m, 0x5a);
        assert!(m.bus.text().contains("CRT FAMILY PASS"));
        let result = &m.bus.mem[0x8000..0x8080];
        let pattern: [u8; 32] = std::array::from_fn(|i| {
            if i % 2 == 0 {
                0x80 >> (i / 2 % 8)
            } else {
                (i as u8 * 7) ^ 0x53
            }
        });
        let mut expected = [0xa5; 128];
        expected[1] = 0x5a;
        expected[4] = font::jis_byte(0x24, 0x22, 0);
        expected[10..18].copy_from_slice(&[0x38, 0x44, 0x44, 0x7c, 0x44, 0x44, 0x44, 0]);
        expected[34..66].copy_from_slice(&pattern);
        expected[68..100].copy_from_slice(&pattern);
        assert_eq!(result, &expected);
        for i in [
            0, 2, 3, 5, 6, 7, 8, 9, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33,
            66, 67, 100, 127,
        ] {
            assert_eq!(result[i], 0xa5, "guard {i}");
        }
        assert_eq!(&result[34..66], &result[68..100]);
        assert_eq!(m.bus.mem[0x53c], 0);
        assert_eq!(m.bus.gdcs[0].pitch, 80);
        assert!(!m.bus.gdcs[0].cursor_visible);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes.clone(), original);
        m.reset();
        assert_eq!(m.bus.mem[0x8001], 0);
        assert_eq!(m.bus.mem[0x53c], 0);
        assert_eq!(m.bus.read(0xa4001), 255);
    }
    for ah in [0x0f, 0x1b, 0x1c, 0x1d, 0x1e] {
        let mut entry = boot_probe(&format!("first-{ah:02x}"));
        stage(&mut entry, 1);
    }
}

#[test]
fn crt_initialization_keeps_unrelated_devices_and_bad_second_area_is_atomic() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.bus.graphics[1234] = 0x96;
    m.bus.palette[5] = [1, 2, 3];
    m.bus.gdcs[1].enabled = true;
    m.bus.gdcs[1].start = 123;
    m.bus.gdcs[1].pitch = 37;
    cell(&mut m, 120, 0x2156);
    m.bus.user_characters.define(0x7621, [0x81; 32]);
    call(&mut m, 0x13, 0, 240).unwrap();
    call(&mut m, 0x1c, 0x60, 0x0407).unwrap();
    assert_eq!(m.bus.read16(0xa0000 + 240), 0x2156);
    assert_eq!(m.bus.read(0xa2000 + 240), 0xe1);
    assert_eq!(m.bus.user_characters.byte(0x76, 0x21, 0), 0x81);
    assert_eq!(m.bus.graphics[1234], 0x96);
    assert_eq!(m.bus.palette[5], [1, 2, 3]);
    assert!(m.bus.gdcs[1].enabled);
    assert_eq!((m.bus.gdcs[1].start, m.bus.gdcs[1].pitch), (123, 37));
    assert_eq!(m.bus.gdcs[0].cursor, 120);
    assert_eq!(m.bus.mem[0x53c], 8);
    let before = m.bus.gdcs[0].scroll;
    m.cpu.r[BX] = 0x3000;
    m.cpu.r[CX] = 0;
    for (i, word) in [200, 2, 4096, 1].into_iter().enumerate() {
        m.bus.write_seg(0x3000, i as u16 * 2, word, true);
    }
    assert!(call(&mut m, 0x0f, 0, 2)
        .unwrap_err()
        .contains("display-area entry"));
    assert_eq!(m.bus.gdcs[0].scroll, before);
    for bad_rows in [0, 26, u16::MAX] {
        m.bus.write_seg(0x3000, 4, 300, true);
        m.bus.write_seg(0x3000, 6, bad_rows, true);
        assert!(call(&mut m, 0x0f, 0, 2).is_err());
        assert_eq!(m.bus.gdcs[0].scroll, before);
    }
    // Explicit firmware initialization restores its own CRT baseline even
    // when called on a bus previously programmed through hardware ports.
    m.bus.output(0x68, 3);
    m.bus.output(0x68, 1);
    m.bus.output(0x68, 0x0e);
    firmware::initialize(&mut m.bus);
    assert_eq!(m.bus.mem[0x53c], 0);
    assert!(m.bus.text_enabled);
    assert_eq!(m.bus.read(0xa4001), 255);
    assert_eq!(
        (m.bus.gdcs[0].pitch, m.bus.gdcs[0].character_lines),
        (80, 16)
    );
    assert_eq!(m.bus.gdcs[0].text_address(399, 79), Some((1999, 15)));
    assert!(!m.bus.gdcs[0].cursor_visible);
    assert_eq!(m.bus.graphics[1234], 0x96);
    assert_eq!(m.bus.palette[5], [1, 2, 3]);
    assert_eq!(m.bus.user_characters.byte(0x76, 0x21, 0), 0x81);
}

use pc98_rust::{video::SnapshotRenderer, Machine, Profile};

#[test]
fn planar_pixels_match_bit_reference_at_pages_wraps_and_doubled_rows() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut machine = Machine::new(profile);
        for (start, page, doubled, columns40, analog) in [
            (0, 0, false, false, false),
            (0x3fff, 1, true, true, true),
            (0x3ffe, 0, false, false, true),
            (42, 1, true, true, false),
        ] {
            let b = &mut machine.bus;
            b.gdcs[0].enabled = false;
            b.gdcs[1].enabled = true;
            b.gdcs[1].start = start;
            b.display_page = page;
            b.graphics_200 = doubled;
            b.analog = analog && profile == Profile::Pc9821ApDos;
            b.output(0x68, u8::from(columns40));
            for (i, byte) in b.graphics.iter_mut().enumerate() {
                *byte = (i ^ (i >> 3) ^ (i >> 15)) as u8;
            }
            for (i, rgb) in b.palette.iter_mut().enumerate() {
                *rgb = [i as u8 * 13, i as u8 * 7, i as u8 * 3];
            }
            let mut pixels = vec![0; 640 * 400 * 4];
            b.frame(&mut pixels);
            let planes = if b.analog { profile.planes() } else { 3 };
            for y in 0..400 {
                for x in 0..640 {
                    let line = if doubled { y / 2 } else { y };
                    let a = ((start as usize * 2 + line * 80 + x / 8) & 0x7fff)
                        + usize::from(page) * profile.planes() * 0x8000;
                    let mut color = 0;
                    for plane in 0..planes {
                        color |= usize::from((b.graphics[a + plane * 0x8000] >> (7 - x % 8)) & 1)
                            << plane;
                    }
                    let at = (y * 640 + x) * 4;
                    assert_eq!(&pixels[at..at + 3], &b.palette[color]);
                    assert_eq!(pixels[at + 3], 255);
                }
            }
        }
    }
}

#[test]
fn full_snapshots_match_inline_scanout_after_mode_palette_page_glyph_and_blink_changes() {
    let mut renderer = SnapshotRenderer::default();
    let mut snapshot = Vec::new();
    let mut pixels = vec![0; 640 * 400 * 4];
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut machine = Machine::new(profile);
        for mode in 0..24 {
            let b = &mut machine.bus;
            for (i, byte) in b.graphics.iter_mut().enumerate() {
                *byte = (i ^ (i >> 5) ^ (i >> 16) ^ mode as usize) as u8;
            }
            for i in 0..4096 {
                b.mem[0xa0000 + i * 2] = if i % 4 == 0 {
                    0x56
                } else {
                    (i % 94 + 0x21) as u8
                };
                b.mem[0xa0001 + i * 2] = if i % 3 == 0 { 0 } else { 0x21 };
                b.mem[0xa2000 + i * 2] = i as u8;
            }
            b.user_characters.define(0x7621, [0x5a; 32]);
            for (i, rgb) in b.pegc_palette.iter_mut().enumerate() {
                *rgb = [i as u8, i as u8 ^ 7, 255 - i as u8];
            }
            b.cycles = 133_333 * mode * 13;
            b.gdcs[0].enabled = mode % 7 != 0;
            b.gdcs[1].enabled = mode % 5 != 0;
            b.gdcs[0].pitch = if mode % 3 == 0 { 0 } else { 80 };
            b.gdcs[0].start = 4090 + mode as u32;
            b.gdcs[0].cursor = (mode * 5) as u32;
            b.gdcs[0].cursor_visible = true;
            b.gdcs[0].character_lines = (mode % 18) as u8;
            b.gdcs[0].scroll = [
                0xf0, 0x0f, 0x30, 0x84, 0, 0, 0x80, 0x14, 0, 0, 0, 0, 0, 0, 0, 0,
            ];
            b.gdcs[1].start = mode as u32 * 101;
            b.gdcs[1].pitch = 80;
            b.display_page = (mode % 2) as u8;
            b.graphics_200 = mode % 3 == 0;
            b.analog = profile == Profile::Pc9821ApDos && mode % 2 == 0;
            b.pegc = profile == Profile::Pc9821ApDos && mode % 4 == 0;
            b.palette[(mode % 16) as usize] = [0x33, mode as u8, 0xaa];
            b.pegc_palette[(mode % 256) as usize] = [0xee, 0xff, mode as u8];
            // Include private 40-column/semigraphics/dot-access flags through ports.
            b.output(0x68, if mode % 2 == 0 { 0x01 } else { 0x00 });
            b.output(0x68, if mode % 3 == 0 { 0x03 } else { 0x02 });
            b.output(0x68, if mode % 4 == 0 { 0x0b } else { 0x0a });
            b.video().snapshot(&mut snapshot);
            let expected = machine.render().to_vec();
            // Rendering a retained snapshot must never observe subsequent writes.
            machine.bus.graphics.fill(mode as u8);
            machine.bus.mem[0xa0000..0xa4000].fill(0);
            machine.bus.user_characters.define(0x7621, [0xff; 32]);
            assert!(renderer.render(&snapshot, &mut pixels));
            assert_eq!(pixels, expected, "{profile:?} mode {mode}");
        }
    }
}

#[test]
fn malformed_snapshots_are_rejected_before_reading_vram_or_touching_pixels() {
    let mut renderer = SnapshotRenderer::default();
    let mut snapshot = Vec::new();
    Machine::default().bus.video().snapshot(&mut snapshot);
    let mut pixels = vec![0x55; 640 * 400 * 4];
    for len in [0, 4, 127, snapshot.len() - 1] {
        assert!(!renderer.render(&snapshot[..len], &mut pixels));
    }
    for (offset, value) in [(0, 0), (3, 2), (4, 2), (5, 128), (5, 32), (6, 255)] {
        let mut bad = snapshot.clone();
        bad[offset] = value;
        assert!(!renderer.render(&bad, &mut pixels));
    }
    assert!(pixels.iter().all(|v| *v == 0x55));
}

#[test]
fn retained_scanout_tracks_blink_boundaries_and_restores_arbitrary_output_buffers() {
    let mut renderer = SnapshotRenderer::default();
    let mut machine = Machine::new(Profile::Pc9821ApDos);
    let mut snapshot = Vec::new();
    let mut pixels = vec![0; 640 * 400 * 4];
    machine.bus.gdcs[0].enabled = true;
    machine.bus.gdcs[0].cursor_visible = true;
    machine.bus.gdcs[0].cursor_blink = true;
    machine.bus.gdcs[0].blink_rate = 1;
    machine.bus.mem[0xa0000] = b'A';
    machine.bus.mem[0xa2000] = 0xf3; // Visible blinking text.
    for frame in [0, 0, 1, 5, 6, 7, 8, 8, 9, 15, 16] {
        machine.bus.cycles = frame * 133_333;
        machine.bus.video().snapshot(&mut snapshot);
        pixels.fill(0x55); // The caller need not retain the previous pixels.
        assert!(renderer.render(&snapshot, &mut pixels));
        assert_eq!(pixels, machine.render(), "frame {frame}");
    }
    // A failed render must not poison the last valid cached result.
    let valid = snapshot.clone();
    snapshot[3] ^= 1;
    pixels.fill(0xaa);
    assert!(!renderer.render(&snapshot, &mut pixels));
    assert!(pixels.iter().all(|&v| v == 0xaa));
    assert!(renderer.render(&valid, &mut pixels));
    assert_eq!(pixels, machine.render());
}

#[test]
fn interleaved_snapshots_do_not_change_guest_execution_devices_or_audio() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut plain = Machine::new(profile);
        let mut captured = Machine::new(profile);
        let mut snapshot = Vec::new();
        for _ in 0..20 {
            plain.step_scheduler(80_000, 100_000);
            captured.bus.video().snapshot(&mut snapshot);
            captured.step_scheduler(80_000, 100_000);
        }
        assert_eq!(plain.cpu.ip, captured.cpu.ip);
        assert_eq!(plain.cpu.flags, captured.cpu.flags);
        assert_eq!(plain.bus.cycles, captured.bus.cycles);
        assert_eq!(plain.bus.mem, captured.bus.mem);
        assert_eq!(plain.bus.graphics, captured.bus.graphics);
        assert_eq!(plain.bus.audio, captured.bus.audio);
        assert_eq!(plain.bus.beep_edges, captured.bus.beep_edges);
        assert_eq!(plain.bus.sound_irq_edges, captured.bus.sound_irq_edges);
    }
}

#[test]
fn owned_snapshots_recycle_input_storage_without_changing_pixels_or_invalidating_cache() {
    let mut renderer = SnapshotRenderer::default();
    let mut machine = Machine::new(Profile::Pc9821ApDos);
    let mut transfer = Vec::new();
    for n in 0..8 {
        machine.bus.cycles = n * 133_333;
        if n % 3 == 0 {
            machine.bus.graphics[n as usize] ^= 0xff;
        }
        machine.bus.video().snapshot(&mut transfer);
        let expected = machine.render().to_vec();
        assert_eq!(renderer.frame_owned(&mut transfer).unwrap(), expected);
        let mut unchanged = Vec::new();
        machine.bus.video().snapshot(&mut unchanged);
        assert_eq!(renderer.frame(&unchanged).unwrap(), expected);
        assert!(renderer.frame_owned(&mut vec![0; 12]).is_none());
        assert_eq!(renderer.frame(&unchanged).unwrap(), expected);
    }
}

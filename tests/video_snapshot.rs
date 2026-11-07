use pc98_rust::{video::SnapshotRenderer, Machine, Profile};

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

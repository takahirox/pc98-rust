use pc98_rust::{bus::Bus, font::jis_byte, sound::Opn, Machine, Profile};

fn machine(name: &str) -> Machine {
    let path = format!(
        "{}/fixtures/dos-apps/{name}.hdm",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(
        0,
        std::fs::read(path).expect("run python3 tools/prepare_dos_apps.py before cargo test"),
        "test.hdm",
    )
    .unwrap();
    m.reset();
    m
}
fn advance(m: &mut Machine, slices: usize) -> f64 {
    let mut energy = 0.0;
    for _ in 0..slices {
        m.step(100_000);
        assert!(m.error.is_none(), "{:?}", m.error);
        energy += m
            .bus
            .audio
            .iter()
            .map(|s| f64::from(*s).powi(2))
            .sum::<f64>();
        m.bus.audio.clear();
    }
    energy
}
fn key(m: &mut Machine, scan: u8) {
    m.bus.key(scan, true);
    advance(m, 1);
    m.bus.key(scan, false);
    advance(m, 20);
}
fn pixel(frame: &[u8], x: usize, y: usize) -> &[u8] {
    &frame[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3]
}
fn frame_hash(m: &mut Machine) -> u64 {
    m.render().iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
fn glyph(m: &mut Machine, jis: [u8; 2], x: usize, y: usize) {
    let frame = m.render();
    for row in 0..16 {
        for col in 0..16 {
            let ink = jis_byte(jis[0], jis[1], row * 2 + col / 8) & (128 >> (col & 7)) != 0;
            assert_eq!(
                pixel(frame, x + col, y + row) == [255, 255, 255],
                ink,
                "glyph pixel {col},{row}"
            );
        }
    }
}
fn fat_file(image: &[u8], name: &[u8; 11]) -> Vec<u8> {
    let entry = image[5 * 1024..11 * 1024]
        .as_chunks::<32>()
        .0
        .iter()
        .find(|e| &e[..11] == name)
        .expect("guest saved file");
    let size = u32::from_le_bytes(entry[28..32].try_into().unwrap()) as usize;
    let mut cluster = u16::from_le_bytes(entry[26..28].try_into().unwrap()) as usize;
    let mut result = Vec::new();
    for _ in 0..1232 {
        if cluster >= 0xff8 {
            break;
        }
        assert!((2..1232).contains(&cluster));
        result.extend_from_slice(&image[(9 + cluster) * 1024..(10 + cluster) * 1024]);
        let off = 1024 + cluster * 3 / 2;
        cluster = (u16::from_le_bytes(image[off..off + 2].try_into().unwrap()) as usize
            >> if cluster & 1 != 0 { 4 } else { 0 })
            & 0xfff;
    }
    result.truncate(size);
    result
}

#[test]
fn independent_adv98_japanese_game_input_save_and_reload() {
    let mut m = machine("adv98");
    advance(&mut m, 160);
    // Independent engine's title menu begins with は; exact replacement glyph,
    // read by guest code through CG and painted by the guest into planar VRAM.
    glyph(&mut m, [0x24, 0x4f], 160, 317);
    assert!(m.cpu.retired > 8_000_000 && m.bus.fdc.reads > 150);
    let title = frame_hash(&mut m);
    key(&mut m, 0x1c); // Start game.
    assert_ne!(title, frame_hash(&mut m));
    for col in 0..16 {
        let expected = m.bus.palette[(col + 2) & 15];
        assert_eq!(
            pixel(m.render(), col * 40 + 8, 100),
            expected,
            "four-plane background column {col}"
        );
    }
    let dialogue = frame_hash(&mut m);
    key(&mut m, 0x1c); // Scenario choices.
    assert_ne!(dialogue, frame_hash(&mut m));
    key(&mut m, 0x3c); // Choose 話す (Talk) instead of 見る (Look).
    let selected = frame_hash(&mut m);
    key(&mut m, 0x1c);
    assert_ne!(selected, frame_hash(&mut m));
    key(&mut m, 0); // Esc: system menu.
    key(&mut m, 0x1c); // Save menu.
    key(&mut m, 0x1c); // Slot 1.
    let bytes = m.bus.disks[0].as_ref().unwrap().bytes.clone();
    let saved = fat_file(&bytes, b"SAVE1   DAT");
    assert_eq!(&saved[..8], b"ADV98SAV");
    assert!(m.bus.fdc.writes > 0);
    // Export/reimport is an actual disk round trip. Then use the title Load menu.
    let mut restored = Machine::new(Profile::Pc9821ApDos);
    restored.insert(0, bytes, "saved.hdm").unwrap();
    restored.reset();
    advance(&mut restored, 160);
    key(&mut restored, 0x3c);
    key(&mut restored, 0x1c);
    key(&mut restored, 0x1c);
    let again = fat_file(
        &restored.bus.disks[0].as_ref().unwrap().bytes,
        b"SAVE1   DAT",
    );
    assert_eq!(again, saved);
    assert_ne!(title, frame_hash(&mut restored));
    assert!(m.bus.unknown_ports.is_empty());
}

#[test]
fn independent_video_decodes_four_planes_and_returns_to_dos() {
    let mut m = machine("98videop");
    let mut colors = std::collections::BTreeSet::new();
    for _ in 0..5000 {
        m.step(10_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
        if m.bus.analog && m.bus.gdcs[1].enabled && !m.bus.gdcs[0].enabled {
            let rgb: [u8; 3] = pixel(m.render(), 0, 0).try_into().unwrap();
            colors.insert(rgb);
        }
        if m.bus.text().contains("VIDEO RETURNED") {
            break;
        }
    }
    assert!(m.bus.text().contains("VIDEO RETURNED"), "{}", m.bus.text());
    assert!(
        (colors.contains(&[0, 255, 255]) || colors.contains(&[255, 119, 119]))
            && colors.iter().any(|rgb| rgb.contains(&119)),
        "{colors:?}"
    );
    assert!(!m.bus.beep);
    assert!(
        m.bus.pcm.writes > 0,
        "independent video player must use 86 PCM"
    );
}

#[test]
fn independent_fm_routines_execute_under_dos_without_beep() {
    let mut m = machine("fm98");
    let mut energy = 0.0;
    for _ in 0..160 {
        energy += advance(&mut m, 1);
        if m.bus.text().contains("INDEPENDENT FM ROUTINES OK") {
            break;
        }
    }
    assert!(m.bus.text().contains("INDEPENDENT FM ROUTINES OK"));
    assert!(m.bus.opn.writes >= 40 && m.bus.opn.key_edges >= 16);
    assert!(!m.bus.beep && m.bus.beep_edges == 0);
    assert!(energy > 1.0, "FM energy {energy}");
    assert!(m.bus.unknown_ports.is_empty());
}

#[test]
fn profile_memory_a20_reset_cg_and_fourth_plane_are_isolated() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    assert_eq!(m.bus.mem[0x401], 24);
    m.bus.write(0, 0x12);
    m.bus.write(0x100000, 0x34);
    assert_eq!(m.bus.read(0), 0x34);
    m.bus.output(0xf2, 0);
    m.bus.write(0x100000, 0x56);
    assert_eq!(m.bus.read(0), 0x34);
    assert_eq!(m.bus.read(0x100000), 0x56);
    m.bus.write(0x3fffff, 0xa5);
    assert_eq!(m.bus.read(0x3fffff), 0xa5);
    m.bus.write(0x400000, 0);
    assert_eq!(m.bus.read(0x400000), 255);
    m.bus.output(0xf6, 3);
    assert_eq!(m.bus.input(0xf2), 1);
    m.bus.output(0x68, 0xb);
    m.bus.output(0xa3, 4);
    m.bus.output(0xa1, 0x22);
    for byte in 0..32 {
        assert_eq!(m.bus.read(0xa4000 + byte), jis_byte(0x24, 0x22, byte));
    }
    m.bus.output(0xa5, 0x20);
    assert_eq!(m.bus.input(0xa9), jis_byte(0x24, 0x22, 0));
    m.bus.output(0xa5, 0);
    assert_eq!(m.bus.input(0xa9), jis_byte(0x24, 0x22, 1));
    m.bus.output(0x6a, 1);
    m.bus.output(0xa8, 8);
    m.bus.output(0xac, 7);
    m.bus.gdcs[1].enabled = true;
    m.bus.gdcs[0].enabled = false;
    m.bus.write(0xe0000, 128);
    assert_eq!(pixel(m.render(), 0, 0), [119, 0, 0]);
    m.bus.output(0xa6, 1);
    assert_eq!(m.bus.read(0xe0000), 0);
    m.bus.write(0xe0000, 64);
    m.bus.output(0xa4, 1);
    assert_eq!(pixel(m.render(), 1, 0), [119, 0, 0]);
    m.reset();
    assert_eq!(m.bus.profile, Profile::Pc9821ApDos);
    assert!(!m.bus.a20);
    let mut vm = Bus::default();
    vm.write(0xe0000, 128);
    assert_eq!(vm.read(0xe0000), 255);
    vm.output(0xa3, 4);
    assert!(vm.unknown_ports.contains(&0xa3));
}

#[test]
fn japanese_text_vram_halves_and_overlay_use_text_colors() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.bus.gdcs[0].enabled = true;
    m.bus.gdcs[0].cursor_visible = false;
    m.bus.write_seg(0xa000, 0, 0x2204, true); // JIS 2422, あ left half
    m.bus.write_seg(0xa000, 2, 0x2284, true); // same glyph, right half
    m.bus.palette[7] = [255, 0, 0]; // bitmap palette must not recolor text
    let frame = m.render();
    for y in 0..16 {
        for x in 0..16 {
            let ink = jis_byte(0x24, 0x22, y * 2 + x / 8) & (128 >> (x & 7)) != 0;
            assert_eq!(
                pixel(frame, x, y),
                if ink { [255, 255, 255] } else { [0, 0, 0] }
            );
        }
    }
}

#[test]
fn ap_common_real_mode_flags_radix_and_v30_escape_are_distinct() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu.s[1] = 0;
    m.cpu.ip = 0x600;
    m.bus.mem[0x600..0x607].copy_from_slice(&[0xb0, 31, 0xd4, 16, 0x0f, 0x10, 0xc0]);
    m.step(2);
    assert_eq!(m.cpu.r[0], 0x010f);
    assert_eq!(m.cpu.flags & 0xf000, 0);
    m.cpu.r[4] = 0x8000;
    m.bus.mem[6 * 4..6 * 4 + 4].copy_from_slice(&[0, 7, 0, 0]);
    m.step(1);
    assert!(m.error.is_none());
    assert_eq!(m.cpu.ip, 0x700); // #UD, not the V30 TEST1 extension.
    assert_eq!(m.bus.read16(0x7ffa), 0x604); // Restart at 0F, not past it.
    m.reset();
    assert_eq!(m.cpu.flags, 2);
}

fn fm_write(fm: &mut Opn, a: u8, v: u8) {
    fm.address = a;
    fm.write(v);
}

#[test]
fn opn_algorithm_two_second_operator_contributes_to_pcm() {
    fn tone(second_total_level: u8) -> Vec<f32> {
        let mut fm = Opn::default();
        for slot in [0, 4, 8, 12] {
            fm_write(&mut fm, 0x30 + slot, 1);
            fm_write(&mut fm, 0x40 + slot, 24);
            fm_write(&mut fm, 0x50 + slot, 31);
            fm_write(&mut fm, 0x80 + slot, 15);
        }
        // Logical operator 2 occupies register offset 8 (physical order 1,3,2,4).
        fm_write(&mut fm, 0x48, second_total_level);
        for (a, v) in [(0xb0, 2), (0xa4, 0x22), (0xa0, 0x69), (0x28, 0xf0)] {
            fm_write(&mut fm, a, v);
        }
        (0..4800).map(|_| fm.sample()).collect()
    }

    assert!(
        tone(0) != tone(127),
        "algorithm 2 must route operator 2 through operator 3 to the output"
    );
}

#[test]
fn opn_timers_irq_busy_fm_pitch_and_deterministic_pcm() {
    let mut fm = Opn::default();
    assert_eq!(fm.sample(), 0.0);
    fm_write(&mut fm, 0x24, 255);
    fm_write(&mut fm, 0x25, 3);
    fm_write(&mut fm, 0x27, 5);
    assert_ne!(fm.status() & 128, 0);
    fm.tick(144);
    assert_eq!(fm.status() & 1, 0);
    fm.tick(1);
    assert!(fm.irq());
    fm_write(&mut fm, 0x27, 0x10);
    assert!(!fm.irq());
    let mut b = Bus::new(Profile::Pc9821ApDos);
    b.output(0x188, 0x24);
    b.output(0x18a, 255);
    b.output(0x188, 0x25);
    b.output(0x18a, 3);
    b.output(0x188, 0x27);
    b.output(0x18a, 5);
    b.tick(145);
    assert_ne!(b.pics[1].irr & 16, 0);
    fn tone() -> Vec<f32> {
        let mut fm = Opn::default();
        for slot in [0, 4, 8, 12] {
            fm_write(&mut fm, 0x30 + slot, 1);
            fm_write(&mut fm, 0x40 + slot, 24);
            fm_write(&mut fm, 0x50 + slot, 31);
            fm_write(&mut fm, 0x80 + slot, 15);
        }
        for (a, v) in [(0xb0, 7), (0xa4, 0x22), (0xa0, 0x69), (0x28, 0xf0)] {
            fm_write(&mut fm, a, v);
        }
        let data = (0..4800).map(|_| fm.sample()).collect::<Vec<_>>();
        fm_write(&mut fm, 0x28, 0);
        for _ in 0..48000 {
            fm.sample();
        }
        assert!(fm.sample().abs() < 0.0001);
        data
    }
    let samples = tone();
    assert_eq!(samples, tone());
    let crossings = samples
        .windows(2)
        .filter(|s| s[0] <= 0.0 && s[1] > 0.0)
        .count();
    assert!((25..=27).contains(&crossings), "C4 crossings {crossings}");
    assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
}

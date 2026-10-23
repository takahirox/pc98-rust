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

// Independent PCM oracle: default prescale /144, 20-bit phase accumulator,
// 48 kHz output. Interpolated rising zero crossings avoid FFT bin rounding.
fn ch3_hz(fnum: u16, block: u8) -> f64 {
    (3_993_600.0 / 144.0) * (f64::from(fnum) / 1_048_576.0) * 2f64.powi(block.into())
}
fn ch3_pitch(samples: &[f32], expected: f64) {
    assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    if expected == 0.0 {
        assert!(samples.iter().all(|&s| s == 0.0));
        return;
    }
    let crossings: Vec<f64> = samples
        .windows(2)
        .enumerate()
        .filter(|(_, s)| s[0] <= 0.0 && s[1] > 0.0)
        .map(|(i, s)| i as f64 - f64::from(s[0]) / f64::from(s[1] - s[0]))
        .collect();
    assert!(crossings.len() >= 3, "not enough periods: {crossings:?}");
    let measured =
        48_000.0 * (crossings.len() - 1) as f64 / (crossings.last().unwrap() - crossings[0]);
    let tolerance = (expected * 0.0002).max(0.03);
    assert!(
        (measured - expected).abs() <= tolerance,
        "PCM pitch {measured:.6} Hz, expected {expected:.6} +/- {tolerance:.6} Hz"
    );
}
fn ch3_pair(fm: &mut Opn, low: u8, fnum: u16, block: u8) {
    fm_write(fm, low + 4, (block << 3) | (fnum >> 8) as u8);
    fm_write(fm, low, fnum as u8);
}
fn ch3_voice(ch: usize) -> Opn {
    let mut fm = Opn::default();
    fm_write(&mut fm, 0x29, 0x80);
    let bank = ch / 3 * 256;
    for slot in [0, 8, 4, 12] {
        for (reg, value) in [(0x30, 1), (0x40, 0), (0x50, 31), (0x80, 15)] {
            let addr = bank + reg + ch % 3 + slot;
            if bank == 0 {
                fm_write(&mut fm, addr as u8, value);
            } else {
                fm.bank_address = addr as u8;
                fm.write_bank(value);
            }
        }
    }
    if bank == 0 {
        fm_write(&mut fm, 0xb0 + ch as u8, 7);
    } else {
        fm.bank_address = 0xb0 + (ch % 3) as u8;
        fm.write_bank(7);
    }
    fm
}
fn ch3_pcm(fm: &mut Opn) -> Vec<f32> {
    (0..24_000).map(|_| fm.sample()).collect()
}

#[test]
fn opn_ch3_reproduction_independent_operator_one_pcm() {
    for mode in [0x40, 0x80] {
        let mut fm = ch3_voice(2);
        ch3_pair(&mut fm, 0xa2, 0x600, 4);
        ch3_pair(&mut fm, 0xa9, 0x200, 4);
        fm_write(&mut fm, 0x27, mode); // Timer A and B stopped, including 80h.
        fm_write(&mut fm, 0x28, 0x12);
        ch3_pitch(&ch3_pcm(&mut fm), ch3_hz(0x200, 4));
    }
}

#[test]
fn opn_ch3_each_operator_normal_and_both_special_encodings() {
    let pairs = [
        (0xa9, 0x123, 3),
        (0xaa, 0x456, 4),
        (0xa8, 0x789, 2),
        (0xa2, 0x5ab, 5),
    ];
    for mode in [0, 0x40, 0x80] {
        for slot in 0..4 {
            let mut fm = ch3_voice(2);
            for (reg, fnum, block) in pairs {
                ch3_pair(&mut fm, reg, fnum, block);
            }
            fm_write(&mut fm, 0x27, mode);
            fm_write(&mut fm, 0x28, 2 | (0x10 << slot));
            let (_, fnum, block) = pairs[if mode == 0 { 3 } else { slot }];
            ch3_pitch(&ch3_pcm(&mut fm), ch3_hz(fnum, block));
            assert_eq!(fm.status() & 3, 0);
            assert_eq!(fm.key_edges, 1);
        }
    }
}

#[test]
fn opn_ch3_fnumber_boundaries_zero_blocks_and_reserved_bits() {
    for mode in [0, 0x40, 0x80] {
        for (slot, reg) in [0xa9, 0xaa, 0xa8, 0xa2].into_iter().enumerate() {
            for (fnum, block) in [
                (0, 4),
                (0xff, 4),
                (0x100, 4),
                (0x3ff, 4),
                (0x400, 4),
                (0x7ff, 4),
                (0x240, 0),
                (0x240, 1),
                (0x240, 2),
                (0x240, 3),
                (0x240, 4),
                (0x240, 5),
                (0x240, 6),
                (0x240, 7),
            ] {
                let mut fm = ch3_voice(2);
                ch3_pair(&mut fm, 0xa2, 0x611, 2);
                let selected = if mode == 0 { 0xa2 } else { reg };
                ch3_pair(&mut fm, selected, fnum, block);
                // D7:D6 are outside BLOCK/FNUM and must not affect pitch.
                fm_write(&mut fm, selected + 4, 0xc0 | block << 3 | (fnum >> 8) as u8);
                fm_write(&mut fm, selected, fnum as u8);
                fm_write(&mut fm, 0x27, mode);
                fm_write(&mut fm, 0x28, 2 | (0x10 << slot));
                ch3_pitch(&ch3_pcm(&mut fm), ch3_hz(fnum, block));
            }
        }
    }
}

#[test]
fn opn_ch3_keyed_mode_changes_preserve_phase_envelope_and_frequencies() {
    for mode in [0x40, 0x80] {
        for (slot, reg) in [0xa9, 0xaa, 0xa8, 0xa2].into_iter().enumerate() {
            let mut fm = ch3_voice(2);
            ch3_pair(&mut fm, 0xa2, 0x623, 4);
            if slot != 3 {
                ch3_pair(&mut fm, reg, 0x345, 3);
            }
            fm_write(&mut fm, 0x72 + [0, 8, 4, 12][slot], 8);
            fm_write(&mut fm, 0x28, 2 | (0x10 << slot));
            let programmed = fm.registers;
            let mut phase = 0.0;
            let mut elapsed = 0i32;
            for control in [0, mode, 0, mode] {
                fm_write(&mut fm, 0x27, control);
                assert_eq!(fm.key_edges, 1, "mode change must not key operators");
                let hz = if control != 0 && slot != 3 {
                    ch3_hz(0x345, 3)
                } else {
                    ch3_hz(0x623, 4)
                };
                for _ in 0..4800 {
                    // Sum the expected phase increments across mode boundaries;
                    // sustain rate 8 decays continuously after attack/decay.
                    phase = (phase + hz / 48_000.0).fract();
                    let level = (1.0f64 - 16.0 / 768_000.0).powi((elapsed - 1).max(0));
                    let expected = 0.08 * level * (phase * std::f64::consts::TAU).sin();
                    assert!((f64::from(fm.sample()) - expected).abs() < 0.0000003,
                        "slot {slot}, mode {control:02x}, sample {elapsed}: phase/envelope discontinuity");
                    elapsed += 1;
                }
                assert_eq!(&fm.registers[0xa0..=0xae], &programmed[0xa0..=0xae]);
            }
            assert_eq!(fm.key_edges, 1);
        }
    }
}

#[test]
fn opn_ch3_selection_preserves_operator_multiple_and_detune() {
    for mode in [0x40, 0x80] {
        for (slot, reg) in [0xa9, 0xaa, 0xa8, 0xa2].into_iter().enumerate() {
            for (dtmul, multiple, detune) in [(0, 0.5, 0.0), (2, 2.0, 0.0), (0x71, 1.0, -0.3)] {
                let mut fm = ch3_voice(2);
                ch3_pair(&mut fm, reg, 0x421, 4);
                fm_write(&mut fm, 0x32 + [0, 8, 4, 12][slot], dtmul);
                fm_write(&mut fm, 0x27, mode);
                fm_write(&mut fm, 0x28, 2 | (0x10 << slot));
                ch3_pitch(&ch3_pcm(&mut fm), ch3_hz(0x421, 4) * multiple + detune);
            }
        }
    }
}

#[test]
fn opn_ch3_other_channels_and_upper_bank_are_isolated() {
    for ch in 0..6 {
        let mut reference = ch3_voice(ch);
        let mut changed = ch3_voice(ch);
        for fm in [&mut reference, &mut changed] {
            if ch < 3 {
                ch3_pair(fm, 0xa0 + ch as u8, 0x523, 4);
            } else {
                for (reg, value) in [(0xa4 + ch % 3, 0x25), (0xa0 + ch % 3, 0x23)] {
                    fm.bank_address = reg as u8;
                    fm.write_bank(value);
                }
            }
            fm_write(
                fm,
                0x28,
                0xf0 | (ch % 3) as u8 | if ch >= 3 { 4 } else { 0 },
            );
        }
        for control in [0, 0x40, 0x80, 0] {
            if ch != 2 {
                fm_write(&mut changed, 0x27, control);
                for reg in [0xa8, 0xa9, 0xaa] {
                    ch3_pair(&mut changed, reg, 0x712, 7);
                }
            } else {
                // Bank-one extra pairs and mode control must never select or
                // overwrite channel 3's bank-zero frequency source.
                changed.bank_address = 0x27;
                changed.write_bank(control);
            }
            for reg in [0xa8, 0xa9, 0xaa, 0xac, 0xad, 0xae] {
                changed.bank_address = reg;
                changed.write_bank(0xff);
            }
            assert_eq!(ch3_pcm(&mut changed), ch3_pcm(&mut reference));
        }
    }
    // Bank-one writes also cannot change an already selected special pitch.
    let mut fm = ch3_voice(2);
    ch3_pair(&mut fm, 0xa9, 0x345, 3);
    fm_write(&mut fm, 0x27, 0x40);
    fm_write(&mut fm, 0x28, 0x12);
    for reg in [0x27, 0xa2, 0xa6, 0xa9, 0xad] {
        fm.bank_address = reg;
        fm.write_bank(0xff);
    }
    ch3_pitch(&ch3_pcm(&mut fm), ch3_hz(0x345, 3));
}

#[test]
fn opn_ch3_board_masks_instances_and_machine_reset() {
    fn port_write(bus: &mut Bus, bank: bool, reg: u8, value: u8) {
        bus.output(if bank { 0x18c } else { 0x188 }, reg);
        bus.output(if bank { 0x18e } else { 0x18a }, value);
    }
    let mut m = Machine::new(Profile::Pc9821ApDos);
    let mut other = Machine::new(Profile::Pc9821ApDos);
    m.bus.opn = ch3_voice(2);
    ch3_pair(&mut m.bus.opn, 0xa9, 0x345, 3);
    port_write(&mut m.bus, false, 0x27, 0x40);
    port_write(&mut m.bus, false, 0x28, 0x12);
    assert_eq!(other.bus.opn.registers[0x27], 0);
    assert_eq!(other.bus.opn.key_edges, 0);
    assert_eq!(other.bus.opn.sample(), 0.0);
    other.bus.opn = ch3_voice(2);
    ch3_pair(&mut other.bus.opn, 0xa2, 0x765, 2);
    ch3_pair(&mut other.bus.opn, 0xa9, 0x111, 6);
    fm_write(&mut other.bus.opn, 0x27, 0);
    fm_write(&mut other.bus.opn, 0x28, 0x12);
    let writes = m.bus.opn.writes;
    // Extended bank disabled at reset; whole FM mask bit 1 gates both ports.
    port_write(&mut m.bus, true, 0xa9, 0xff);
    assert_eq!(m.bus.opn.writes, writes);
    m.bus.output(0xa460, 3);
    port_write(&mut m.bus, false, 0x27, 0);
    port_write(&mut m.bus, true, 0xa9, 0xff);
    assert_eq!(m.bus.opn.writes, writes);
    m.bus.output(0xa460, 1);
    port_write(&mut m.bus, true, 0xa9, 0xff);
    assert_eq!(m.bus.opn.registers[0x1a9], 0xff);
    ch3_pitch(&ch3_pcm(&mut m.bus.opn), ch3_hz(0x345, 3));
    fm_write(&mut m.bus.opn, 0x27, 0x80);
    ch3_pitch(&ch3_pcm(&mut other.bus.opn), ch3_hz(0x765, 2));
    ch3_pitch(&ch3_pcm(&mut m.bus.opn), ch3_hz(0x345, 3));
    let mut vm = Bus::default();
    port_write(&mut vm, false, 0x27, 0x40);
    port_write(&mut vm, false, 0x28, 0x12);
    assert_eq!(vm.opn.writes, 0); // VM sound exposure remains out of scope.
    m.bus.tick(100_000);
    assert!(!m.bus.audio.is_empty());
    m.reset();
    assert_eq!(m.bus.opn.registers[0x27], 0);
    assert_eq!(&m.bus.opn.registers[0xa0..=0xae], &[0; 15]);
    assert_eq!(m.bus.opn.key_edges, 0);
    assert_eq!(m.bus.opn.sample(), 0.0);
    assert!(m.bus.audio.is_empty());
    assert_eq!(m.bus.pcm.mask, 0);
    assert_eq!(other.bus.opn.key_edges, 1);
    assert_eq!(other.bus.opn.registers[0x27], 0);
    ch3_pitch(&ch3_pcm(&mut other.bus.opn), ch3_hz(0x765, 2));
}

#[test]
fn opn_ch3_mode_bits_preserve_timer_load_reset_and_irq() {
    let mut normal = Opn::default();
    let mut special = Opn::default();
    for fm in [&mut normal, &mut special] {
        for (reg, value) in [(0x24, 255), (0x25, 3), (0x26, 255), (0x27, 15)] {
            fm_write(fm, reg, value);
        }
    }
    // Changes while loaded must preserve accumulated clocks, status and IRQ.
    for (cycles, mode, control) in [
        (100, 0x40, 15),
        (44, 0x80, 15),
        (1, 0x40, 15),
        (1000, 0x80, 31),
        (1200, 0x40, 15),
        (1, 0x80, 63),
        (3000, 0, 12),
        (100, 0x40, 15),
        (45, 0x80, 15),
    ] {
        normal.tick(cycles);
        special.tick(cycles);
        assert_eq!(special.status(), normal.status());
        assert_eq!(special.irq(), normal.irq());
        if cycles == 1 && control == 15 {
            assert_eq!(
                normal.status() & 1,
                1,
                "mode change must not reload Timer A"
            );
        }
        fm_write(&mut normal, 0x27, control);
        fm_write(&mut special, 0x27, control | mode);
        assert_eq!(special.status(), normal.status());
        assert_eq!(special.irq(), normal.irq());
        assert_eq!(
            special.key_edges, 0,
            "CSM automatic keying is outside this subset"
        );
    }
}

#[test]
fn opn_ch3_guest_pcm_stages_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/opn_ch3/opn_ch3.hdm"
    ))
    .expect("run python3 tools/prepare_opn_ch3_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "opn_ch3.hdm").unwrap();
    for run in 0..2 {
        m.reset();
        assert_eq!(m.bus.opn.key_edges, 0);
        assert_eq!(m.bus.opn.registers[0x27], 0);
        let mut pcm: [Vec<f32>; 16] = std::array::from_fn(|_| Vec::new());
        let mut slots = 0;
        while slots < 2_000_000 {
            let before = m.bus.read(0x8001);
            slots += m.step(1000);
            assert!(m.error.is_none(), "{:?}", m.error);
            let after = m.bus.read(0x8001);
            assert!(
                m.bus.audio.len() <= 48,
                "drain far below the 4096 sample queue cap"
            );
            assert!(m.bus.audio.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            if before == after && (1..=16).contains(&after) {
                pcm[(after - 1) as usize].extend_from_slice(&m.bus.audio);
                assert_eq!(m.bus.opn.key_edges, 1 + 2 * u64::from((after - 1) / 4));
                assert_eq!(m.bus.opn.registers[0x27] & 3, 0, "timers stopped");
            }
            m.bus.audio.clear();
            let text = m.bus.text();
            assert!(!text.contains("FAIL"), "{text}");
            if text.contains("OPN CH3 PASS") {
                break;
            }
        }
        assert!(slots <= 2_000_000);
        assert!(m.bus.text().contains("OPN CH3 PASS"), "{}", m.bus.text());
        let pairs = [(0x123, 3), (0x456, 4), (0x789, 2), (0x5ab, 5)];
        for (stage, data) in pcm.iter().enumerate() {
            assert!(
                data.len() >= 3500,
                "stage {}: {} samples",
                stage + 1,
                data.len()
            );
            let (fnum, block) = pairs[if matches!(stage % 4, 1 | 2) {
                stage / 4
            } else {
                3
            }];
            ch3_pitch(data, ch3_hz(fnum, block));
        }
        let mut expected = [0xa5; 64];
        expected[1] = 0xff;
        expected[16..32].fill(1);
        let actual: Vec<_> = (0..64).map(|i| m.bus.read(0x8000 + i)).collect();
        assert_eq!(actual, expected);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.fdc.writes, 0);
        assert_eq!(m.bus.opn.key_edges, 8);
        assert_eq!(m.bus.beep_edges, 0);
        println!(
            "OPN CH3 boot {run}: {slots} slots, stage samples {:?}",
            pcm.each_ref().map(Vec::len)
        );
    }
}

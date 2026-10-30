use pc98_rust::{bus::Bus, cpu::*, Machine, Profile};

#[test]
fn independent_debug_writes_egc_and_256_color_pegc_through_dos() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(
        0,
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/dos-apps/pegc.hdm"
        ))
        .expect("run python3 tools/prepare_device_fixture.py"),
        "pegc.hdm",
    )
    .unwrap();
    m.reset();
    for _ in 0..220 {
        m.step(100_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
        if m.bus.text().contains("PEGC UTILITY COMPLETE") {
            break;
        }
    }
    assert!(
        m.bus.text().contains("PEGC UTILITY COMPLETE"),
        "{}",
        m.bus.text()
    );
    assert!(m.bus.pegc);
    assert!(m.bus.text().contains("EXTENDED MEMORY OK"));
    assert_eq!(&m.bus.mem[0x200000..0x200010], b"PC98 INT1F RAM!!");
    for plane in 0..4 {
        assert_eq!(
            &m.bus.mem[0x90000 + plane * 16..0x90010 + plane * 16],
            &[if plane < 3 { 255 } else { 0 }; 16]
        );
    }
    for i in 0..256 {
        assert_eq!(m.bus.graphics[i * 128], i as u8);
        assert_eq!(
            m.bus.pegc_palette[i],
            [i as u8, 255 - i as u8, i as u8 ^ 85]
        );
    }
    for bank in 1..16 {
        assert!(m.bus.graphics[bank * 0x8000..(bank + 1) * 0x8000]
            .iter()
            .all(|&v| v == bank as u8));
    }
    let palette = m.bus.pegc_palette;
    let frame = m.render();
    let mut seen = std::collections::BTreeSet::new();
    for y in 0..50 {
        for x in 0..640 {
            let color = (y * 640 + x) / 128;
            let expected = [color as u8, 255 - color as u8, color as u8 ^ 85];
            assert_eq!(palette[color], expected);
            assert_eq!(&frame[(y * 640 + x) * 4..(y * 640 + x) * 4 + 3], &expected);
            seen.insert(expected);
        }
    }
    assert!(seen.len() > 240);
    m.bus.output(0xa4, 1);
    assert_eq!(&m.render()[..3], &palette[8]);
    m.reset();
    assert!(!m.bus.pegc);
    assert!(m.bus.graphics.iter().all(|&v| v == 0));
}

fn copy_call(m: &mut Machine, src: u32, dst: u32, len: u16) -> bool {
    copy_call_attributes(m, src, dst, len, 0, 0xffff, 0x92)
}
fn copy_call_attributes(
    m: &mut Machine,
    src: u32,
    dst: u32,
    len: u16,
    flags: u8,
    limit: u16,
    access: u8,
) -> bool {
    m.cpu.s[CS] = 0;
    m.cpu.ip = 0x600;
    m.cpu.s[SS] = 0;
    m.cpu.r[SP] = 0x8000;
    m.cpu.halted = false;
    m.bus.mem[0x600..0x605].copy_from_slice(&[0xb4, 0x90, 0xcd, 0x1f, 0xf4]);
    m.cpu.s[ES] = 0;
    m.cpu.r[BX] = 0x9000;
    m.cpu.r[CX] = len;
    m.cpu.r[SI] = 0;
    m.cpu.r[DI] = 0;
    for (p, base) in [(0x9010, src), (0x9018, dst)] {
        m.bus.mem[p..p + 8].copy_from_slice(&[
            limit as u8,
            (limit >> 8) as u8,
            base as u8,
            (base >> 8) as u8,
            (base >> 16) as u8,
            access,
            flags,
            (base >> 24) as u8,
        ]);
    }
    m.step(8);
    assert!(m.error.is_none(), "{:?}", m.error);
    m.cpu.flags & CF == 0
}

#[test]
fn int1f_copy_uses_extended_ram_preserves_a20_and_rejects_holes_atomically() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.bus.mem[0x100000..0x100010].copy_from_slice(b"EXTENDED RAM OK!");
    assert!(copy_call(&mut m, 0x100000, 0x3ffff0, 16));
    assert_eq!(&m.bus.mem[0x3ffff0..], b"EXTENDED RAM OK!");
    assert!(!m.bus.a20);
    for (src, dst, n) in [
        (0x100000, 0x3ffff0, 17),
        (0x9fff0, 0x100000, 32),
        (0xf0000, 0x100000, 16),
        (0x100000, 0xa8000, 16),
        (0x100000, 0x400000, 1),
        (0xfffffff0, 0x100000, 32),
    ] {
        let before = m.bus.mem[0x100000..].to_vec();
        assert!(!copy_call(&mut m, src, dst, n));
        assert!(m.bus.mem[0x100000..] == before);
        assert!(!m.bus.a20);
    }
    m.bus.a20 = true;
    assert!(copy_call(&mut m, 0x100000, 0x800, 16));
    assert!(m.bus.a20);
    let mut vm = Machine::default();
    assert!(!copy_call(&mut vm, 0x800, 0x900, 16));
    m.reset();
    assert!(!m.bus.a20);
    assert_eq!(m.bus.mem[0x401], 24);
}

#[test]
fn int1f_copy_accepts_386_limit_bits_and_granularity_but_rejects_invalid_descriptors() {
    for (flags, limit, access, success) in [
        (0x0f, 0xffff, 0x93, true),  // 20-bit byte limit, used by public FDXMS
        (0xcf, 0xffff, 0x93, true),  // 4 KiB granularity, 32-bit data
        (0x80, 0, 0x92, true),       // inclusive effective limit FFFh
        (0x10, 0xffff, 0x92, true),  // software AVL bit
        (0x20, 0xffff, 0x92, false), // reserved 486 descriptor bit
        (0, 0, 0x92, false),         // inclusive byte limit too short
        (0, 0xffff, 0x9a, false),    // executable
        (0, 0xffff, 0x12, false),    // not present
        (0, 0xffff, 0x90, false),    // read-only destination
        (0, 0xffff, 0x96, false),    // expand-down outside supported contract
    ] {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.bus.mem[0x100000..0x100010].copy_from_slice(b"386 COPY LIMITS!");
        m.bus.mem[0x200000..0x200010].fill(0xa5);
        assert_eq!(
            copy_call_attributes(&mut m, 0x100000, 0x200000, 16, flags, limit, access),
            success,
            "flags={flags:02x} access={access:02x}"
        );
        assert_eq!(
            &m.bus.mem[0x200000..0x200010],
            if success {
                &b"386 COPY LIMITS!"[..]
            } else {
                &[0xa5; 16]
            }
        );
        assert!(!m.bus.a20);
    }
}

#[test]
fn mouse_latching_buttons_ppi_bsr_periodic_irq_and_reset() {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    b.mouse.motion(-17, 35, 3);
    b.output(0x7fdd, 0x90);
    assert_eq!(b.input(0x7fd9), 0x4f);
    b.mouse.motion(4, 5, 0);
    for (control, nibble) in [(0x90, 15), (0xb0, 14), (0xd0, 3), (0xf0, 2)] {
        b.output(0x7fdd, control);
        assert_eq!(b.input(0x7fd9) & 15, nibble);
    }
    b.output(0x7fdd, 0);
    b.output(0x7fdf, 15); // BSR sets HC, latches next movement
    assert_eq!(b.input(0x7fd9) & 15, 4);
    b.output(0x7fdf, 14);
    b.output(0x7fdf, 8); // clear HC and IRQ mask
    b.output(0xbfdb, 3);
    b.tick(533_333);
    assert_eq!(b.pics[1].irr & 32, 0);
    b.tick(1);
    assert_ne!(b.pics[1].irr & 32, 0);
}

#[test]
fn pegc_bank_windows_palette_pages_and_rom_holes_stay_bounded() {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    b.output(0x6a, 0x21);
    assert!(!b.pegc); // graphics mode cannot change while locked
    b.output(0x6a, 7);
    b.output(0x6a, 0x21);
    for window in 0..2 {
        for bank in 0..16 {
            b.write(0xe0004 + window * 2, bank);
            b.write(0xa8000 + window * 0x8000, bank + window as u8 * 16);
            assert_eq!(b.graphics[bank as usize * 0x8000], bank + window as u8 * 16);
            assert_eq!(b.read(0xa8000 + window * 0x8000), bank + window as u8 * 16);
        }
    }
    b.write(0xe0004, 255);
    assert_eq!(b.read(0xe0004), 15);
    assert_eq!(b.read(0xe0005), 0);
    b.output(0xa8, 255);
    b.output(0xac, 123);
    b.output(0xaa, 45);
    b.output(0xae, 67);
    assert_eq!(b.pegc_palette[255], [123, 45, 67]);
    assert_eq!(b.input(0xac), 123);
    b.write(0xb8000, 1);
    assert_eq!(b.read(0xb8000), 255);
    b.mem[0xf0000] = 0x5a;
    b.write(0xf0000, 0);
    assert_eq!(b.read(0xf0000), 0x5a);
    b.output(0x6a, 0x20);
    assert!(!b.pegc);
    b.output(0x6a, 6);
    b.output(0x6a, 0x21);
    assert!(!b.pegc);
    let mut vm = Bus::default();
    vm.output(0x6a, 7);
    vm.output(0x6a, 0x21);
    assert!(!vm.pegc);
}

#[test]
fn egc_copy_invert_xor_masks_pattern_and_read_plane() {
    let mut e = pc98_rust::egc::Egc::default();
    e.registers[2] = 0x20f0 | 0x800; // source-copy ROP, selected read rather than compare
    assert_eq!(e.read([0x5a, 0xa5, 0x3c, 0xc3], 0), 0x5a);
    assert_eq!(e.write([0; 4], 0, 0), [0x5a, 0xa5, 0x3c, 0xc3]);
    e.registers[2] = 0x280f;
    assert_eq!(e.write([0; 4], 0, 0), [0xa5, 0x5a, 0xc3, 0x3c]);
    e.registers[2] = 0x283c;
    assert_eq!(e.write([255; 4], 0, 0), [0xa5, 0x5a, 0xc3, 0x3c]);
    e.registers[4] = 0x00ff;
    assert_eq!(e.write([0x17; 4], 0, 0), [0x17; 4]);
    e.registers[4] = 0;
    e.registers[0] = 0xfffe;
    e.registers[2] = 0x2c00;
    assert_eq!(e.write([255; 4], 0, 0), [0, 255, 255, 255]);
}

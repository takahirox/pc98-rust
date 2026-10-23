use pc98_rust::{bus::Bus, cpu::*, devices::Pic, disk::Disk, firmware, Machine};
fn execute(code: &[u8], count: usize) -> (Cpu, Bus) {
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x100,
        ..Cpu::default()
    };
    c.r[SP] = 0x8000;
    let mut b = Bus::default();
    b.mem[0x100..0x100 + code.len()].copy_from_slice(code);
    for _ in 0..count {
        c.step(&mut b).unwrap()
    }
    (c, b)
}
fn run(m: &mut Machine, n: u32) {
    m.step(n);
    assert!(m.error.is_none(), "{:?}", m.error)
}
fn boot(image: &[u8]) -> Machine {
    let mut m = Machine::default();
    m.insert(0, image.to_vec(), "test.hdm").unwrap();
    m.reset();
    run(&mut m, 100_000);
    m
}
fn press(m: &mut Machine, scan: u8) {
    m.bus.key(scan, true);
    run(m, 60_000);
    m.bus.key(scan, false);
    run(m, 1000);
}
#[test]
fn arithmetic_flags() {
    let (c, _) = execute(&[0xb0, 0x7f, 0x04, 1], 2);
    assert_eq!(c.byte(0), 128);
    assert_eq!(c.flags & (SF | OF | AF | CF | ZF | PF), SF | OF | AF);
    let (c, _) = execute(&[0xb0, 0xff, 0x04, 1], 2);
    assert_eq!(c.flags & (CF | ZF | PF | AF | OF), CF | ZF | PF | AF);
    let (c, _) = execute(&[0xb8, 0, 0x80, 0x2d, 1, 0], 2);
    assert_eq!(c.r[AX], 0x7fff);
    assert_eq!(c.flags & (CF | OF | AF | SF), OF | AF);
    let (c, _) = execute(&[0xf9, 0xb0, 0xff, 0x14, 0, 0x1c, 0], 4);
    assert_eq!(c.byte(0), 255);
    assert_eq!(c.flags & (CF | SF | PF | AF | OF), CF | SF | PF | AF);
    let (c, _) = execute(&[0xf9, 0xb8, 0xff, 0xff, 0x40, 0x48], 4);
    assert_eq!(c.r[AX], 65535);
    assert_ne!(c.flags & CF, 0);
}
#[test]
fn modrm_segments_wrap_and_rom() {
    let mut b = Bus::default();
    b.write_seg(0xffff, 0x10, 0xabcdu16, true);
    assert_eq!(b.read16(0), 0xabcd);
    b.write_seg(0x100, 0xffff, 0x1234, true);
    assert_eq!(b.read(0x10fff), 0x34);
    assert_eq!(b.read(0x1000), 0x12);
    b.mem[0xf0000] = 0x55;
    b.write(0xf0000, 1);
    assert_eq!(b.read(0xf0000), 0x55);
    b.write(0xd0000, 1);
    assert_eq!(b.read(0xd0000), 255);
    let (c, b) = execute(
        &[
            0xbb, 0x00, 0x20, 0xbe, 3, 0, 0xc7, 0x40, 0xfe, 0x34, 0x12, 0x8b, 0x50, 0xfe,
        ],
        4,
    );
    assert_eq!(c.r[DX], 0x1234);
    assert_eq!(b.read16(0x2001), 0x1234);
    let (c, b) = execute(
        &[
            0xb8, 0x00, 0x20, 0x8e, 0xd0, 0xbd, 0x10, 0, 0xc7, 0x46, 0, 0x67, 0x45,
        ],
        4,
    );
    assert_eq!(b.read16(0x20010), 0x4567);
    assert_eq!(c.s[SS], 0x2000);
}
#[test]
fn strings_are_budgeted_and_directional() {
    let mut m = Machine::default();
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x100;
    m.bus.mem[0x100..0x110].copy_from_slice(&[
        0xbf, 0, 0x20, 0xb9, 0xff, 0xff, 0xb0, 0x61, 0xf3, 0xaa, 0xfd, 0xb9, 2, 0, 0xf3, 0xaa,
    ]);
    run(&mut m, 20);
    assert_eq!(m.cpu.r[CX], 65518);
    assert_eq!(m.bus.read(0x2000), b'a');
    assert_eq!(m.bus.read(0x2010), b'a');
    assert_eq!(m.cpu.ip, 0x108);
    let (c, b) = execute(
        &[
            0xbf, 1, 0x20, 0xb9, 2, 0, 0xb8, 0x34, 0x12, 0xfd, 0xf3, 0xab,
        ],
        6,
    );
    assert_eq!(c.r[DI], 0x1ffd);
    assert_eq!(b.read16(0x2001), 0x1234);
    assert_eq!(b.read16(0x1fff), 0x1234);
}
#[test]
fn stack_call_ret_interrupt_iret() {
    let (c, _) = execute(&[0xb8, 1, 0, 0xe8, 2, 0, 0xf4, 0x90, 0x40, 0xc3], 5);
    assert_eq!(c.r[AX], 2);
    assert_eq!(c.r[SP], 0x8000);
    assert!(c.halted);
    let mut c = Cpu::default();
    let mut b = Bus::default();
    c.s = [0; 4];
    c.ip = 0x100;
    c.r[SP] = 0x8000;
    c.flags |= IF;
    b.mem[0x100..0x103].copy_from_slice(&[0xcd, 0x30, 0xf4]);
    b.write_seg(0, 0xc0, 0x200, true);
    b.write_seg(0, 0xc2, 0, true);
    b.mem[0x200] = 0xcf;
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x200);
    assert_eq!(c.flags & IF, 0);
    assert_eq!(b.read16(0x7ffa), 0x102);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x102);
    assert_ne!(c.flags & IF, 0);
}
#[test]
fn v30_adjust_bit_and_shift_behavior() {
    let (c, _) = execute(&[0xb8, 0, 1, 0xd5, 0x10], 2);
    assert_eq!(c.r[AX], 10);
    let (c, _) = execute(&[0xb0, 83, 0xd4, 16], 2);
    assert_eq!(c.r[AX], 0x0803);
    let (c, _) = execute(&[0xb8, 0, 0, 0x0f, 0x1c, 0xc0, 3, 0x0f, 0x18, 0xc0, 3], 3);
    assert_eq!(c.byte(0), 8);
    assert_eq!(c.flags & ZF, 0);
    let (c, _) = execute(&[0xb8, 1, 0, 0xb1, 64, 0xd3, 0xe8], 3);
    assert_eq!(c.r[AX], 0);
    let (c, _) = execute(&[0xb0, 0x81, 0xd0, 0xe0], 2);
    assert_eq!(c.byte(0), 2);
    assert_eq!(c.flags & (CF | OF), CF | OF);
}
#[test]
fn v30_bit_instructions_flags_and_operands() {
    // NEC V20/V30 User's Manual, Section 12: TEST1 defines ZF and clears
    // CF/OF (SF/AF/PF are undefined); CLR1/SET1/NOT1 preserve all flags.
    for ext in 0x10..=0x1f {
        let word = ext & 1 != 0;
        for memory in [false, true] {
            for index in [3, 0xff] {
                let mask = 1u16 << (index & if word { 15 } else { 7 });
                for bit_set in [false, true] {
                    let value = if bit_set { 0xffff } else { 0xffff ^ mask };
                    for flags in [0xf002, 0xf002 | CF | PF | AF | ZF | SF | TF | IF | DF | OF] {
                        let mut c = Cpu {
                            s: [0; 4],
                            ip: 0x100,
                            flags,
                            ..Cpu::default()
                        };
                        c.r[AX] = value;
                        c.set_byte(CX, index);
                        let mut b = Bus::default();
                        b.write_seg(0, 0x2000, value, true);
                        let mut code = vec![0x0f, ext, if memory { 0x06 } else { 0xc0 }];
                        if memory {
                            code.extend_from_slice(&[0x00, 0x20]); // DS:[2000h]
                        }
                        if ext & 8 != 0 {
                            code.push(index);
                        }
                        b.mem[0x100..0x100 + code.len()].copy_from_slice(&code);
                        c.step(&mut b).unwrap();
                        let operation = (ext >> 1) & 3;
                        let expected = match operation {
                            0 => value,
                            1 => value & !mask,
                            2 => value | mask,
                            _ => value ^ mask,
                        };
                        let actual = if memory { b.read16(0x2000) } else { c.r[AX] };
                        let context = format!(
                            "ext={ext:02x}, memory={memory}, index={index}, value={value:04x}, flags={flags:04x}"
                        );
                        assert_eq!(actual, expected, "operand: {context}");
                        if operation == 0 {
                            // Do not constrain the undefined arithmetic flags.
                            let defined = !(SF | AF | PF);
                            let expected_flags =
                                (flags & !(CF | OF | ZF)) | if bit_set { 0 } else { ZF };
                            assert_eq!(c.flags & defined, expected_flags & defined, "{context}");
                        } else {
                            assert_eq!(c.flags, flags, "{context}");
                        }
                        assert_eq!(c.ip, 0x100 + code.len() as u32, "{context}");
                        assert_eq!(c.byte(CX), index, "CL: {context}");
                    }
                }
            }
        }
    }
}
#[test]
fn multiply_divide_and_rep_compare() {
    let (c, _) = execute(&[0xb8, 0x00, 1, 0xbb, 3, 0, 0xf7, 0xe3, 0xf7, 0xf3], 4);
    assert_eq!(c.r[AX], 256);
    assert_eq!(c.r[DX], 0);
    let (c, _) = execute(&[0xb0, 0xfe, 0xb3, 3, 0xf6, 0xeb, 0xf6, 0xfb], 4);
    assert_eq!(c.byte(0), 0xfe);
    assert_eq!(c.byte(4), 0);
}
#[test]
fn pic_masks_priority_eoi_and_cascade() {
    let mut p = Pic::new(8);
    p.request(2);
    assert_eq!(p.ack(), None);
    p.mask = 0;
    p.request(1);
    assert_eq!(p.ack(), Some(9));
    assert_eq!(p.ack(), None);
    p.write(false, 0x20);
    assert_eq!(p.ack(), Some(10));
    p.write(false, 0x0b);
    assert_eq!(p.read(false), 4);
    p.write(false, 0x20);
    assert_eq!(p.read(false), 0);
    let mut b = Bus::default();
    b.pics[0].mask = 0x7f;
    b.pics[1].mask = 0xf7;
    b.pics[1].request(3);
    assert_eq!(b.irq(), Some(0x13));
    assert_eq!(b.pics[0].isr, 128);
    assert_eq!(b.pics[1].isr, 8);
    assert_eq!(b.irq(), None);
    b.output(8, 0x20);
    b.output(0, 0x20);
    assert_eq!(b.pics[1].isr, 0);
}
#[test]
fn pit_keyboard_and_beeper() {
    let mut b = Bus::default();
    b.output(2, 0xfe);
    b.output(0x77, 0x36);
    assert_eq!(b.irq(), Some(8)); // programming-induced rise, separate from periods
    b.output(0, 0x20);
    b.output(0x71, 0xe8);
    b.output(0x71, 3);
    b.tick(4000);
    assert_eq!(b.pics[0].irr & 1, 0);
    b.output(0x77, 0);
    let lo = b.input(0x71);
    b.tick(500);
    let hi = b.input(0x71);
    // 998 PIT clocks: load, 500 high clocks, then 497 low decrements by two.
    // Intel mode 3 therefore exposes count 6; the latch survives the next rise.
    assert_eq!(u16::from_le_bytes([lo, hi]), 6);
    assert_ne!(b.pics[0].irr & 1, 0);
    b.output(0x3fdf, 0x76);
    b.output(0x3fdb, 0xcd);
    b.output(0x3fdb, 7);
    assert_eq!(b.pit.beep_hz(), 999);
    b.output(0x37, 6);
    assert!(b.beep);
    b.output(0x37, 7);
    assert!(!b.beep);
    assert_eq!(b.beep_edges, 2);
    b.key(0x1c, true);
    assert_ne!(b.pics[0].irr & 2, 0);
    assert_ne!(b.input(0x43) & 2, 0);
    assert_eq!(b.input(0x41), 0x1c);
    b.key(0x1c, false);
    assert_eq!(b.input(0x41), 0x9c);
    b.key(0x70, true);
    b.keyboard.service();
    b.key(0x1d, true);
    assert_eq!(b.keyboard.service(), Some(0x1d41));
    b.key(0x70, false);
    b.keyboard.service();
    b.key(0x1c, true);
    assert_eq!(b.keyboard.service(), Some(0x1c0d));
}
#[test]
fn bios_keyboard_returns_punctuation_and_keypad_characters() {
    let mut m = boot(include_bytes!("../web/media/hello98.tfd"));
    // Guest reads INT 18h keys and stores the returned scan/ASCII words in RAM.
    let code = [
        0x31, 0xc0, 0x8e, 0xc0, 0xbf, 0x00, 0x06, 0xb4, 0, 0xcd, 0x18, 0xab, 0xeb, 0xf9,
    ];
    m.bus.mem[0x1000..0x1000 + code.len()].copy_from_slice(&code);
    m.cpu.s[CS] = 0;
    m.cpu.ip = 0x1000;
    m.cpu.halted = false;
    run(&mut m, 1000);
    let mut address = 0x600;
    for shifted in [false, true] {
        if shifted {
            m.bus.key(0x70, true);
            run(&mut m, 1000);
        }
        let keys = [
            (0x0c, if shifted { b'~' } else { b'^' }),
            (0x0d, if shifted { b'|' } else { b'\\' }),
            (0x33, if shifted { b'_' } else { b'\\' }),
        ]
        .into_iter()
        .chain((0x40..=0x50).zip(b"-/789*456+123=0,.".iter().copied()));
        for (scan, ch) in keys {
            press(&mut m, scan);
            assert_eq!(
                m.bus.read16(address),
                ((scan as u16) << 8) | ch as u16,
                "scan={scan:02x}, shifted={shifted}"
            );
            address += 2;
            assert_eq!(m.cpu.r[DI] as usize, address);
        }
    }
}
#[test]
fn gdc_planes_pages_palette_grcg_and_text() {
    let mut b = Bus::default();
    b.output(0xa2, 0x0d);
    b.output(0xa2, 0x47);
    b.output(0xa0, 40);
    assert_eq!(b.gdcs[1].pitch, 40);
    b.write(0xa8000, 128);
    let mut frame = vec![0; 640 * 400 * 4];
    b.frame(&mut frame);
    assert_eq!(&frame[..4], &[0, 0, 255, 255]);
    b.output(0xa6, 1);
    assert_eq!(b.read(0xa8000), 0);
    b.write(0xb0000, 128);
    b.output(0xa4, 1);
    b.frame(&mut frame);
    assert_eq!(&frame[..4], &[255, 0, 0, 255]);
    b.output(0x7c, 0x80);
    for tile in [0, 255, 0, 0] {
        b.output(0x7e, tile)
    }
    b.write(0xa8000, 0);
    assert_eq!(b.read(0xb0000), 255);
    assert_eq!(b.read(0xa8000), 0);
    b.output(0x7c, 0xc0);
    for tile in [255, 0, 0, 0] {
        b.output(0x7e, tile)
    }
    b.write(0xa8000, 15);
    assert_eq!(b.read(0xa8000), 15);
    assert_eq!(b.read(0xb0000), 240);
    b.output(0xa8, 0x41);
    assert_eq!(b.palette[3], [0, 0, 255]);
    assert_eq!(b.palette[7], [255, 0, 0]);
    assert_eq!(b.input(0xa8), 0x41);
    b.output(0xa8, 0x77);
    b.output(0x6a, 1);
    b.output(0xa8, 1);
    b.output(0xaa, 8);
    assert_eq!(b.palette[1][1], 136);
    b.output(0x62, 0x0d);
    b.write(0xa0000, b'A');
    b.write(0xa2000, 0xe1);
    b.frame(&mut frame);
    assert!(frame[..16 * 640 * 4]
        .as_chunks::<4>()
        .0
        .contains(&[255, 255, 255, 255]));
    b.output(0x62, 0x49);
    for v in [2, 1, 0] {
        b.output(0x60, v)
    }
    b.output(0x62, 0xe0);
    assert_ne!(b.input(0x60) & 1, 0);
    assert_eq!(b.input(0x62), 2);
    assert_eq!(b.input(0x62), 1);
}
#[test]
fn fdc_dma_read_write_seek_and_errors() {
    let mut b = Bus::default();
    let mut media = vec![0; 1261568];
    media[8192] = 0x42;
    b.disks[0] = Some(Disk::parse(media, "a.hdm").unwrap());
    assert!(firmware::disk_transfer(
        &mut b,
        0,
        [0, 1, 1, 3],
        1024,
        0x2000,
        false
    ));
    assert_eq!(b.read(0x2000), 0x42);
    assert_eq!(b.dma.count[2], 65535);
    assert_ne!(b.dma.status & 4, 0);
    assert_ne!(b.pics[1].irr & 8, 0);
    b.write(0x2000, 0x71);
    assert!(firmware::disk_transfer(
        &mut b,
        0,
        [0, 1, 1, 3],
        1024,
        0x2000,
        true
    ));
    assert_eq!(b.disks[0].as_ref().unwrap().bytes[8192], 0x71);
    assert!(firmware::disk_transfer(
        &mut b,
        0,
        [2, 0, 1, 3],
        1024,
        0x3000,
        false
    ));
    assert_eq!(b.fdc.cylinder[0], 2);
    assert!(!firmware::disk_transfer(
        &mut b,
        0,
        [2, 0, 9, 3],
        1024,
        0x3000,
        false
    ));
    // Raw guest-issued commands must honor the FDC's DMA-enable control.
    b.output(0x94, 8);
    for v in [0x46, 0, 2, 0, 1, 3, 1, 0x1b, 255] {
        b.output(0x92, v)
    }
    b.tick(160);
    assert_eq!(b.input(0x92) & 0x40, 0x40);
    assert_eq!(b.input(0x92), 0x10);
    for _ in 0..5 {
        b.input(0x92);
    }
    b.disks[0].as_mut().unwrap().protected = true;
    assert!(!firmware::disk_transfer(
        &mut b,
        0,
        [2, 0, 1, 3],
        1024,
        0x2000,
        true
    ));
}
#[test]
fn independent_hello_boot_executes_and_draws() {
    let mut m = boot(include_bytes!("../web/media/hello98.tfd"));
    assert!(m.bus.text().starts_with("Hello world!!"));
    assert_eq!(m.bus.fdc.reads, 1);
    assert!(m.cpu.retired > 150);
    assert!(m.bus.beep);
    assert_eq!(m.bus.pit.beep_hz(), 97);
    assert_eq!(m.bus.beep_edges, 1);
    assert!(m
        .render()
        .as_chunks::<4>()
        .0
        .contains(&[255, 255, 255, 255]));
    assert_eq!(m.bus.read(0xa0000), b'H');
}
#[test]
fn guest_game_playthrough_and_media_export() {
    let mut m = boot(include_bytes!("../web/media/startrail.hdm"));
    let base = 0x21400;
    assert_eq!(m.bus.read(base + 8), 1);
    let before = m.render().to_vec();
    for _ in 0..3 {
        press(&mut m, 0x3c);
    }
    assert_eq!(m.bus.read(base + 10), 1);
    assert_eq!(m.bus.beep_edges, 2);
    assert_ne!(m.render(), before);
    for _ in 0..2 {
        press(&mut m, 0x3d)
    }
    for _ in 0..4 {
        press(&mut m, 0x3c)
    }
    for _ in 0..3 {
        press(&mut m, 0x3d)
    }
    for _ in 0..2 {
        press(&mut m, 0x3c)
    }
    for _ in 0..3 {
        press(&mut m, 0x3a)
    }
    assert_eq!(m.bus.read(base + 10), 3);
    assert_eq!(m.bus.read(base + 14), 1);
    assert!(m.bus.text().contains("YOU WIN!"));
    let disk = m.bus.disks[0].as_ref().unwrap();
    assert_eq!(disk.bytes[7 * 1024 + 10], 3);
    assert!(m.bus.fdc.writes >= 17);
    let exported = disk.bytes.clone();
    let mut restored = boot(&exported);
    assert_eq!(restored.bus.read(0x21800 + 10), 3);
    assert_eq!(restored.bus.disks[0].as_ref().unwrap().bytes, exported);
    restored.reset();
    run(&mut restored, 100_000);
    assert_eq!(restored.bus.read(base + 10), 0);
}
#[test]
fn corrupt_media_and_instruction_fail_safely() {
    assert!(Disk::parse(vec![0; 42], "x.hdm").is_err());
    for n in 0..700 {
        assert!(Disk::parse(vec![0; n], "x.d88").is_err())
    }
    let mut m = Machine::default();
    m.cpu.s[CS] = 0;
    m.cpu.ip = 0x100;
    m.bus.mem[0x100] = 0x63;
    run_error(&mut m);
    assert!(m.error.as_ref().unwrap().contains("unsupported opcode"));
    assert_eq!(m.step(1000), 0);
}
fn run_error(m: &mut Machine) {
    assert_eq!(m.step(1000), 1);
    assert!(m.error.is_some());
}
#[test]
fn d88_chrn_roundtrip_and_write_protection() {
    let mut image = vec![0; 0x2b0 + 16 + 1024];
    let len = image.len();
    image[0x1b] = 0x20;
    image[0x1c..0x20].copy_from_slice(&(len as u32).to_le_bytes());
    image[0x20..0x24].copy_from_slice(&0x2b0u32.to_le_bytes());
    let p = 0x2b0;
    image[p + 2] = 1;
    image[p + 3] = 3;
    image[p + 4] = 1;
    image[p + 14..p + 16].copy_from_slice(&1024u16.to_le_bytes());
    image[p + 16] = 77;
    let disk = Disk::parse(image.clone(), "a.d88").unwrap();
    assert_eq!(disk.sector(0, 0, 1, 3).unwrap().len, 1024);
    let mut b = Bus::default();
    b.disks[0] = Some(disk);
    assert!(firmware::disk_transfer(
        &mut b,
        0,
        [0, 0, 1, 3],
        1024,
        0x1000,
        false
    ));
    assert_eq!(b.read(0x1000), 77);
    b.write(0x1000, 42);
    assert!(firmware::disk_transfer(
        &mut b,
        0,
        [0, 0, 1, 3],
        1024,
        0x1000,
        true
    ));
    let exported = b.disks[0].as_ref().unwrap().bytes.clone();
    let parsed = Disk::parse(exported.clone(), "a.d88").unwrap();
    assert_eq!(parsed.bytes[p + 16], 42);
    image[0x1a] = 16;
    assert!(Disk::parse(image.clone(), "a.d88").unwrap().protected);
    image[p + 14] = 1;
    assert!(Disk::parse(image, "a.d88").is_err());
}

#[test]
fn d88_sector_255_bios_read_completes_or_reports_missing_next_sector() {
    for (bytes, expected_carry) in [(1024u16, 0), (2048, CF)] {
        let mut image = vec![0; 0x2b0 + 2 * (16 + 1024)];
        let len = image.len() as u32;
        image[0x1b] = 0x20;
        image[0x1c..0x20].copy_from_slice(&len.to_le_bytes());
        image[0x20..0x24].copy_from_slice(&0x2b0u32.to_le_bytes());
        for (i, r) in [1, 255].into_iter().enumerate() {
            let p = 0x2b0 + i * (16 + 1024);
            image[p + 2] = r;
            image[p + 3] = 3;
            image[p + 4] = 2;
            image[p + 14..p + 16].copy_from_slice(&1024u16.to_le_bytes());
        }
        // Boot code reads R=255 through INT 1Bh, then saves FLAGS and a marker.
        let code = [
            0xfa,
            0x31,
            0xc0,
            0x8e,
            0xc0,
            0xbd,
            0,
            0x20,
            0xbb,
            bytes as u8,
            (bytes >> 8) as u8,
            0xb9,
            0,
            3,
            0xba,
            255,
            0,
            0xb8,
            0x90,
            0x56,
            0xcd,
            0x1b,
            0x9c,
            0x58,
            0xa3,
            0,
            6,
            0xc6,
            0x06,
            2,
            6,
            0xa5,
            0xf4,
        ];
        image[0x2c0..0x2c0 + code.len()].copy_from_slice(&code);
        image[0x2b0 + 16 + 1024 + 16..].fill(0x5a);
        let mut m = Machine::default();
        m.insert(0, image, "sector255.d88").unwrap();
        m.reset();
        run(&mut m, 100_000);
        assert!(m.cpu.halted);
        assert_eq!(m.bus.read(0x602), 0xa5, "guest must return from BIOS");
        assert_eq!(m.bus.read16(0x600) & CF, expected_carry);
        assert!(m.bus.mem[0x2000..0x2400].iter().all(|&b| b == 0x5a));
        assert_eq!(m.bus.fdc.reads, 2);
    }
}

#[test]
fn all_byte_add_sub_inputs_match_mathematical_flags() {
    let mut b = Bus::default();
    let mut c = Cpu {
        s: [0; 4],
        ..Cpu::default()
    };
    for (opcode, subtract) in [(0x00u8, false), (0x28, true)] {
        b.mem[0x100..0x102].copy_from_slice(&[opcode, 0xd8]); // ADD/SUB AL,BL
        for a in 0..=255u16 {
            for rhs in 0..=255u16 {
                c.r[AX] = a;
                c.r[BX] = rhs;
                c.ip = 0x100;
                c.flags = 0xf002;
                c.step(&mut b).unwrap();
                let mathematical = if subtract {
                    a as i32 - rhs as i32
                } else {
                    a as i32 + rhs as i32
                };
                let result = mathematical as u8;
                let signed = if subtract {
                    (a as u8 as i8 as i16) - (rhs as u8 as i8 as i16)
                } else {
                    (a as u8 as i8 as i16) + (rhs as u8 as i8 as i16)
                };
                let cf = if subtract { a < rhs } else { a + rhs > 255 };
                let af = if subtract {
                    a & 15 < rhs & 15
                } else {
                    (a & 15) + (rhs & 15) > 15
                };
                let expected = (u16::from(cf) * CF)
                    | (u16::from(result == 0) * ZF)
                    | (u16::from(result >= 128) * SF)
                    | (u16::from(!(-128..=127).contains(&signed)) * OF)
                    | (u16::from(af) * AF)
                    | (u16::from(result.count_ones() & 1 == 0) * PF);
                assert_eq!(c.byte(0), result, "a={a} rhs={rhs}");
                assert_eq!(
                    c.flags & (CF | ZF | SF | OF | AF | PF),
                    expected,
                    "a={a} rhs={rhs} subtract={subtract}"
                );
            }
        }
    }
}

#[test]
fn sti_delays_irq_for_exactly_one_instruction_and_halt_wakes() {
    let mut b = Bus::default();
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x100,
        ..Cpu::default()
    };
    c.r[SP] = 0x8000;
    b.mem[0x100..0x104].copy_from_slice(&[0xfb, 0x90, 0xf4, 0x90]);
    b.write_seg(0, 9 * 4, 0x200, true);
    b.write_seg(0, 9 * 4 + 2, 0, true);
    b.mem[0x200..0x203].copy_from_slice(&[0xe6, 0, 0xcf]); // EOI value supplied in AL
    c.set_byte(0, 0x20);
    b.pics[0].mask = 0xfd;
    b.pics[0].request(1);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x101);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x102);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x202);
    assert_eq!(b.pics[0].isr, 0);
    c.step(&mut b).unwrap();
    c.step(&mut b).unwrap();
    assert!(c.halted);
    b.pics[0].request(1);
    c.step(&mut b).unwrap();
    assert!(!c.halted);
    assert_eq!(c.ip, 0x202);
}

#[test]
fn clocked_audio_contains_square_wave_and_silence_with_bounded_queue() {
    let mut b = Bus::default();
    b.output(0x77, 0x76);
    b.output(0x73, 0xcd);
    b.output(0x73, 7);
    b.output(0x37, 6);
    for _ in 0..5000 {
        b.tick(8)
    }
    assert!(b.audio.iter().any(|&s| s > 0.0));
    assert!(b.audio.iter().any(|&s| s < 0.0));
    let crossings = b
        .audio
        .windows(2)
        .filter(|p| p[0].is_sign_negative() != p[1].is_sign_negative())
        .count();
    assert!(
        (8..=11).contains(&crossings),
        "5ms at ~1kHz: {crossings} crossings"
    );
    b.audio.clear();
    b.output(0x37, 7);
    for _ in 0..5000 {
        b.tick(8)
    }
    assert!(b.audio.iter().all(|&s| s == 0.0));
    for _ in 0..100_000 {
        b.tick(8)
    }
    assert_eq!(b.audio.len(), 4096);
}

#[test]
fn no_disk_boot_fails_visibly_and_reset_preserves_drive_two() {
    let mut m = Machine::default();
    m.step(1000);
    assert!(m.error.as_ref().unwrap().contains("Boot failed"));
    m.insert(
        0,
        include_bytes!("../web/media/hello98.tfd").to_vec(),
        "hello.tfd",
    )
    .unwrap();
    m.insert(1, vec![0; 1261568], "blank.hdm").unwrap();
    m.reset();
    run(&mut m, 100_000);
    assert_eq!(m.bus.mem[0x55c] & 3, 3);
    assert!(m.bus.disks[1].is_some());
}

#[test]
fn device_fifos_remain_bounded_under_unserviced_guest_commands() {
    let mut b = Bus::default();
    for _ in 0..2000 {
        b.output(0x62, 0xe0);
        b.output(0x41, 0x9d);
        b.fdc.seek_result(0x20, 4);
        b.fdc.seek_result(0x21, 5);
        b.key(0x1d, true);
        b.keyboard.service();
    }
    assert!(b.gdcs[0].fifo.len() <= 16);
    assert!(b.keyboard.fifo.len() <= 64);
    assert!(b.keyboard.bios.len() <= 16);
    assert_eq!(b.fdc.sense.len(), 2);
}

#[test]
fn v30_bound_minimal_valid_reproduction() {
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x1000,
        ..Cpu::default()
    };
    c.r[SP] = 0x9000;
    c.r[AX] = 5;
    let mut b = Bus::default();
    b.mem[0x1000..0x1004].copy_from_slice(&[0x62, 0x06, 0x00, 0x20]);
    b.write_seg(0, 0x2000, 0, true);
    b.write_seg(0, 0x2002, 10, true);
    let saved = c.clone();
    let memory = b.mem.clone();
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x1004);
    assert_eq!(c.r, saved.r);
    assert_eq!(c.s, saved.s);
    assert_eq!(c.flags, saved.flags);
    assert_eq!(b.mem, memory);
}

fn v30_bound_cpu(code: &[u8]) -> (Cpu, Bus) {
    let mut c = Cpu {
        s: [0x2000, 0x1000, 0x3000, 0x4000],
        ip: 0x1000,
        flags: 0xf002 | CF | PF | AF | ZF | SF | TF | IF | DF | OF,
        ..Cpu::default()
    };
    c.r = [5, 0x1234, 0x5678, 0x2000, 0x9000, 0x2100, 3, 7];
    let mut b = Bus::default();
    b.mem[0x11000..0x11000 + code.len()].copy_from_slice(code);
    b.write_seg(0, 0x14, 0x500, true);
    b.write_seg(0, 0x16, 0x5000, true);
    // INC byte [DS:6000]; IRET. Caller follows BOUND with NOP.
    b.mem[0x50500..0x50505].copy_from_slice(&[0xfe, 0x06, 0x00, 0x60, 0xcf]);
    (c, b)
}

#[test]
fn v30_bound_signed_ranges_and_every_index_register() {
    let cases: &[(i16, i16, &[i16])] = &[
        (0, 10, &[-1, 0, 1, 5, 10, 11]),
        (-10, -2, &[-11, -10, -6, -2, -1]),
        (-2, 2, &[-3, -2, -1, 0, 2, 3]),
        (i16::MIN, i16::MAX, &[i16::MIN, -1, 0, i16::MAX]),
        (i16::MIN, i16::MIN, &[i16::MIN, i16::MIN + 1]),
        (i16::MAX, i16::MAX, &[i16::MAX - 1, i16::MAX]),
        (5, 5, &[4, 5, 6]),
        (10, -10, &[-11, -10, 0, 10, 11]),
    ];
    for reg in 0..8 {
        for &(low, high, values) in cases {
            for &value in values {
                let (mut c, mut b) = v30_bound_cpu(&[0x62, 0x06 | (reg << 3), 0, 0x20]);
                c.r[reg as usize] = value as u16;
                b.write_seg(c.s[DS], 0x2000, low as u16, true);
                b.write_seg(c.s[DS], 0x2002, high as u16, true);
                let saved = c.clone();
                let mut expected = b.mem.clone();
                c.step(&mut b).unwrap();
                let outside = value < low || value > high;
                let mut registers = saved.r;
                if outside {
                    registers[SP] = saved.r[SP].wrapping_sub(6);
                    for (i, word) in [0x1004, saved.s[CS], saved.flags].iter().enumerate() {
                        for (j, byte) in word.to_le_bytes().iter().enumerate() {
                            let off = saved.r[SP].wrapping_sub(6).wrapping_add((2 * i + j) as u16);
                            expected[saved.s[SS] as usize * 16 + off as usize] = *byte;
                        }
                    }
                    assert_eq!((c.s[CS], c.ip), (0x5000, 0x500));
                    assert_eq!(c.flags, saved.flags & !(IF | TF));
                } else {
                    assert_eq!((c.s[CS], c.ip), (saved.s[CS], 0x1004));
                    assert_eq!(c.flags, saved.flags);
                }
                assert_eq!(c.r, registers, "register {reg} value {value}");
                assert_eq!(b.mem, expected);
            }
        }
    }
}

#[test]
fn v30_bound_addressing_segments_unaligned_and_wrapping() {
    // All 16-bit EA combinations, signed disp8, disp16 and direct addressing.
    let cases: &[(u8, &[u8], u16, usize)] = &[
        (0x06, &[1, 0x22], 0x2201, DS),
        (0, &[], 0x2003, DS),
        (1, &[], 0x2007, DS),
        (2, &[], 0x2103, SS),
        (3, &[], 0x2107, SS),
        (4, &[], 3, DS),
        (5, &[], 7, DS),
        (7, &[], 0x2000, DS),
        (0x46, &[0], 0x2100, SS),
        (0x40, &[0xfe], 0x2001, DS),
        (0x83, &[0xfa, 0xff], 0x2101, SS),
        (0x87, &[0xff, 0xdf], 0xffff, DS), // BX + dfff wraps; words cross ffff
        (0x80, &[0xff, 0xdf], 2, DS),      // BX + SI + dfff wraps
    ];
    for &(modrm, disp, off, default_seg) in cases {
        for prefix in [
            None,
            Some((0x26, ES)),
            Some((0x2e, CS)),
            Some((0x36, SS)),
            Some((0x3e, DS)),
        ] {
            let mut code = Vec::new();
            if let Some((byte, _)) = prefix {
                code.push(byte);
            }
            code.extend([0x62, modrm]);
            code.extend(disp);
            let (mut c, mut b) = v30_bound_cpu(&code);
            let selected = prefix.map_or(default_seg, |(_, seg)| seg);
            for seg in [ES, CS, SS, DS] {
                // Wrong segments contain a reversed range; only the selected one succeeds.
                b.write_seg(
                    c.s[seg],
                    off,
                    if seg == selected { 0xfffb } else { 10 },
                    true,
                );
                b.write_seg(
                    c.s[seg],
                    off.wrapping_add(2),
                    if seg == selected { 5 } else { 0 },
                    true,
                );
            }
            let saved = c.clone();
            let memory = b.mem.clone();
            c.step(&mut b).unwrap();
            assert_eq!(c.ip, 0x1000 + code.len() as u32);
            assert_eq!(c.r, saved.r);
            assert_eq!(c.s, saved.s);
            assert_eq!(c.flags, saved.flags);
            assert_eq!(b.mem, memory);
        }
    }
}

#[test]
fn v30_bound_vector5_exact_frame_iret_and_forward_progress() {
    for value in [0xfffau16, 11] {
        for interrupts in [0, IF] {
            let (mut c, mut b) = v30_bound_cpu(&[0x26, 0x62, 0x06, 1, 0x20, 0x90]);
            c.flags = (c.flags & !IF) | interrupts;
            c.r[AX] = value;
            b.write_seg(c.s[ES], 0x2001, 0xfffb, true);
            b.write_seg(c.s[ES], 0x2003, 10, true);
            let saved = c.clone();
            let mut expected = b.mem.clone();
            let frame = 0x38ffa;
            let bytes: Vec<u8> = [0x1005u16, 0x1000, saved.flags]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect();
            expected[frame..frame + 6].copy_from_slice(&bytes);
            c.step(&mut b).unwrap();
            assert_eq!((c.s[CS], c.ip, c.r[SP]), (0x5000, 0x500, 0x8ffa));
            assert_eq!(&b.mem[frame..frame + 6], bytes);
            assert_eq!(c.flags, saved.flags & !(IF | TF));
            assert_eq!(b.mem, expected);
            c.step(&mut b).unwrap(); // guest counter
            expected[0x46000] = 1;
            c.step(&mut b).unwrap(); // unmodified guest IRET
            assert_eq!((c.s[CS], c.ip), (0x1000, 0x1005));
            assert_eq!(c.r, saved.r);
            assert_eq!(c.flags, saved.flags);
            c.step(&mut b).unwrap(); // forward progress after BOUND, no retry
            assert_eq!(c.ip, 0x1006);
            assert_eq!(b.mem, expected);
        }
    }
}

#[test]
fn v30_bound_register_operand_has_explicit_diagnostic() {
    let (mut c, mut b) = v30_bound_cpu(&[0x62, 0xc0]);
    let memory = b.mem.clone();
    assert_eq!(
        c.step(&mut b).unwrap_err(),
        "invalid V30 BOUND: memory operand required"
    );
    assert_eq!(b.mem, memory);
}

#[test]
fn v30_bound_original_boot_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/v30_bound/v30_bound.hdm"
    ))
    .expect("run python3 tools/prepare_v30_bound_fixture.py");
    let manifest = include_str!("../fixtures/v30-bound-manifest.json");
    let hex = manifest
        .split_once("\"result_hex\": \"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap();
    let expected: Vec<u8> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(expected.len(), 32);
    for profile in [pc98_rust::Profile::Pc9801Vm] {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "v30_bound.hdm").unwrap();
        for run in 0..2 {
            if run != 0 {
                m.reset();
            }
            let mut slots = 0;
            while slots < 2_000_000 {
                slots += m.step(1000);
                assert!(m.error.is_none(), "{:?}", m.error);
                let text = m.bus.text();
                assert!(
                    !text.contains("FAIL"),
                    "{profile:?} boot {run}: {slots} slots; {text}; {:02x?}",
                    &m.bus.mem[0x8000..0x8020]
                );
                if text.contains("V30 BOUND PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(m.bus.text().contains("V30 BOUND PASS"), "{}", m.bus.text());
            assert_eq!(&m.bus.mem[0x8000..0x8020], expected.as_slice());
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

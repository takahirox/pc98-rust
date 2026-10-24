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
fn v30_repc_minimal_reproduction() {
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x100,
        flags: 2,
        ..Cpu::default()
    };
    let mut b = Bus::default();
    b.mem[0x100..0x103].copy_from_slice(&[0x65, 0xa6, 0x90]);
    b.mem[0x4000..0x4003].copy_from_slice(&[1, 5, 0]);
    b.mem[0x5000..0x5003].copy_from_slice(&[2, 4, 0]);
    c.r[SI] = 0x4000;
    c.r[DI] = 0x5000;
    c.r[CX] = 3;
    c.step(&mut b).unwrap();
    assert_eq!(
        (c.ip, c.r[CX], c.r[SI], c.r[DI]),
        (0x100, 2, 0x4001, 0x5001)
    );
    assert_ne!(c.flags & CF, 0);
    c.step(&mut b).unwrap();
    assert_eq!(
        (c.ip, c.r[CX], c.r[SI], c.r[DI]),
        (0x102, 1, 0x4002, 0x5002)
    );
    assert_eq!(c.flags & (CF | ZF), 0);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x103);
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

fn v30_repc_cpu(code: &[u8]) -> (Cpu, Bus) {
    let mut c = Cpu {
        s: [0x3000, 0, 0x2000, 0x1000],
        ip: 0x100,
        ..Cpu::default()
    };
    c.r = [5, 0, 0x1234, 0x5678, 0x8000, 0x4321, 0x4005, 0x5005];
    let mut b = Bus::default();
    b.mem[0x100..0x100 + code.len()].copy_from_slice(code);
    (c, b)
}

#[test]
fn v30_repc_prefix_opcode_count_flags_and_direction_matrix() {
    // 5-6 => -1 (CF/PF/AF/SF); 5-4 => 1; 5-5 => 0 (PF/ZF).
    // Equality continues REPNC, while inequality continues REPC: ZF cannot
    // accidentally be used as the repeat condition. All arithmetic flags defined.
    for prefix in [0x64, 0x65] {
        for op in [0xa6, 0xa7, 0xae, 0xaf] {
            for incoming_cf in [0, CF] {
                for backward in [false, true] {
                    for count in [0, 1, 3, 0xffff] {
                        for early in [false, true] {
                            let (mut c, mut b) = v30_repc_cpu(&[prefix, op, 0x90]);
                            c.r[CX] = count;
                            c.flags = 0xf002
                                | IF
                                | TF
                                | PF
                                | AF
                                | ZF
                                | SF
                                | OF
                                | incoming_cf
                                | if backward { DF } else { 0 };
                            let initial = c.clone();
                            let word = op & 1 != 0;
                            let delta = if word { 2u16 } else { 1 };
                            let delta = if backward {
                                0u16.wrapping_sub(delta)
                            } else {
                                delta
                            };
                            let dest = match (prefix, early) {
                                (0x65, true) => [6, 4, 5],
                                (0x64, true) => [5, 4, 6],
                                (0x65, false) => [6; 3],
                                _ => [5; 3],
                            };
                            for i in 0..3u16 {
                                b.write_seg(
                                    c.s[DS],
                                    c.r[SI].wrapping_add(delta.wrapping_mul(i)),
                                    5,
                                    word,
                                );
                                b.write_seg(
                                    c.s[ES],
                                    c.r[DI].wrapping_add(delta.wrapping_mul(i)),
                                    dest[i as usize],
                                    word,
                                );
                            }
                            let memory = b.mem.clone();
                            let limit = if early && prefix == 0x65 { 2 } else { 3 };
                            let steps = usize::from(count).min(limit).max(1);
                            for i in 0..steps {
                                c.step(&mut b).unwrap();
                                let done = if count == 0 { 0 } else { i as u16 + 1 };
                                let rhs = dest[done.saturating_sub(1) as usize];
                                let arithmetic = match rhs {
                                    6 => CF | PF | AF | SF,
                                    5 => ZF | PF,
                                    _ => 0,
                                };
                                let expected_flags = if count == 0 {
                                    initial.flags
                                } else {
                                    (initial.flags & !(CF | PF | AF | ZF | SF | OF)) | arithmetic
                                };
                                assert_eq!(
                                    c.flags, expected_flags,
                                    "{prefix:02x} {op:02x} count={count} step={i}"
                                );
                                let mut registers = initial.r;
                                registers[CX] -= done;
                                if op < 0xae {
                                    registers[SI] =
                                        registers[SI].wrapping_add(delta.wrapping_mul(done));
                                }
                                registers[DI] =
                                    registers[DI].wrapping_add(delta.wrapping_mul(done));
                                assert_eq!(c.r, registers);
                                assert_eq!(c.s, initial.s);
                                let continues = done < count && (rhs == 6) == (prefix == 0x65);
                                assert_eq!(c.ip, if continues { 0x100 } else { 0x102 });
                            }
                            assert_eq!(b.mem, memory, "operands and every guard remain unchanged");
                            if c.ip == 0x102 {
                                c.step(&mut b).unwrap();
                                assert_eq!(c.ip, 0x103);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn v30_repc_word_overflow_flags_and_offset_wrapping_with_segment_orders() {
    for prefix in [0x64, 0x65] {
        for op in [0xa6, 0xa7, 0xae, 0xaf] {
            for segment_first in [false, true] {
                for backward in [false, true] {
                    let code = if segment_first {
                        [0x36, prefix, op, 0x90]
                    } else {
                        [prefix, 0x36, op, 0x90]
                    };
                    let (mut c, mut b) = v30_repc_cpu(&code);
                    let word = op & 1 != 0;
                    let left = if word { 0x8000 } else { 0x80 };
                    c.r[AX] = left;
                    c.r[CX] = 2;
                    c.r[SI] = if backward { 0 } else { 0xffff };
                    c.r[DI] = c.r[SI];
                    c.flags = 0xf002 | IF | if backward { DF } else { 0 };
                    let initial = c.clone();
                    let stride = if word { 2u16 } else { 1 };
                    let stride = if backward {
                        0u16.wrapping_sub(stride)
                    } else {
                        stride
                    };
                    for i in 0..2u16 {
                        let offset = c.r[SI].wrapping_add(stride.wrapping_mul(i));
                        b.write_seg(c.s[SS], offset, left, word); // SS override must select source
                        b.write_seg(c.s[ES], offset, 1, word); // destination always ES
                    }
                    let memory = b.mem.clone();
                    c.step(&mut b).unwrap();
                    assert_eq!(c.flags, initial.flags | OF | AF | if word { PF } else { 0 });
                    assert_eq!(c.r[CX], 1);
                    assert_eq!(c.r[DI], initial.r[DI].wrapping_add(stride));
                    assert_eq!(
                        c.r[SI],
                        initial.r[SI].wrapping_add(if op < 0xae { stride } else { 0 })
                    );
                    assert_eq!(c.ip, if prefix == 0x64 { 0x100 } else { 0x103 });
                    if prefix == 0x64 {
                        c.step(&mut b).unwrap();
                        assert_eq!(c.r[CX], 0);
                        assert_eq!(c.ip, 0x103);
                        assert_eq!(c.flags, initial.flags | OF | AF | if word { PF } else { 0 });
                        assert_eq!(c.r[DI], initial.r[DI].wrapping_add(stride.wrapping_mul(2)));
                    }
                    assert_eq!(c.r[AX], left);
                    assert_eq!(c.s, initial.s);
                    assert_eq!(b.mem, memory);
                    c.step(&mut b).unwrap();
                    assert_eq!(c.ip, 0x104);
                }
            }
        }
    }
}

#[test]
fn v30_repc_full_count_exhaustion_is_budgeted() {
    for prefix in [0x64, 0x65] {
        let (mut c, mut b) = v30_repc_cpu(&[prefix, 0xa6, 0x90]);
        c.r[SI] = 0;
        c.r[DI] = 0;
        c.r[CX] = 0xffff;
        b.mem[0x10000..0x20000].fill(0);
        b.mem[0x30000..0x40000].fill(if prefix == 0x65 { 1 } else { 0 });
        let memory = b.mem.clone();
        for done in 1..=0xffffu16 {
            c.step(&mut b).unwrap();
            assert_eq!((c.r[SI], c.r[DI], c.r[CX]), (done, done, 0xffff - done));
            assert_eq!(c.ip, if done == 0xffff { 0x102 } else { 0x100 });
            assert_eq!(
                c.flags & (CF | PF | AF | ZF | SF | OF),
                if prefix == 0x65 {
                    CF | PF | AF | SF
                } else {
                    ZF | PF
                }
            );
        }
        assert_eq!(b.mem, memory);
    }
}

#[test]
fn v30_repc_pic_irq_between_elements_and_guest_iret() {
    for prefix in [0x64, 0x65] {
        // Two segment/repeat prefix orders restart at the first prefix.
        for code in [[0x36, prefix, 0xa6, 0x90], [prefix, 0x36, 0xa6, 0x90]] {
            let (mut c, mut b) = v30_repc_cpu(&code);
            c.flags = 0xf002 | IF;
            c.r[CX] = 3;
            b.mem[0x24005..0x24008].copy_from_slice(&[1, 2, 3]);
            b.mem[0x35005..0x35008].copy_from_slice(if prefix == 0x65 {
                &[2, 3, 3]
            } else {
                &[1, 2, 4]
            });
            b.write_seg(0, 9 * 4, 0x200, true);
            b.write_seg(0, 9 * 4 + 2, 0, true);
            // Guest handler preserves AX and FLAGS, counts entry, sends EOI, IRET.
            b.mem[0x200..0x20c].copy_from_slice(&[
                0x50, 0xfe, 0x06, 0x00, 0x60, 0xb0, 0x20, 0xe6, 0, 0x58, 0xcf, 0x90,
            ]);
            b.pics[0].mask = 0xfd;
            c.step(&mut b).unwrap();
            assert_eq!(
                (c.ip, c.r[CX], c.r[SI], c.r[DI]),
                (0x100, 2, 0x4006, 0x5006)
            );
            let flags = c.flags;
            b.pics[0].request(1);
            c.step(&mut b).unwrap(); // interrupt delivery + PUSH AX, no second comparison
            assert_eq!(c.ip, 0x201);
            assert_eq!(c.r[CX], 2);
            assert_eq!(b.read_seg(0x2000, 0x7ffa, true), 0x100);
            assert_eq!(b.read_seg(0x2000, 0x7ffc, true), 0);
            assert_eq!(b.read_seg(0x2000, 0x7ffe, true), flags);
            for _ in 0..5 {
                c.step(&mut b).unwrap();
            }
            assert_eq!(c.ip, 0x100);
            assert_eq!(c.flags, flags);
            assert_eq!(c.r[SP], 0x8000);
            assert_eq!(b.mem[0x16000], 1); // DS counter, exactly one entry
            assert_eq!(b.pics[0].isr, 0);
            c.step(&mut b).unwrap();
            assert_eq!(
                (c.ip, c.r[CX], c.r[SI], c.r[DI]),
                (0x100, 1, 0x4007, 0x5007)
            );
            c.step(&mut b).unwrap();
            assert_eq!(
                (c.ip, c.r[CX], c.r[SI], c.r[DI]),
                (0x103, 0, 0x4008, 0x5008)
            );
            assert_eq!(c.flags & (CF | ZF), if prefix == 0x65 { ZF } else { CF });
            c.step(&mut b).unwrap();
            assert_eq!(c.ip, 0x104);
            assert_eq!(&b.mem[0x24005..0x24008], &[1, 2, 3]);
            assert_eq!(
                &b.mem[0x35005..0x35008],
                if prefix == 0x65 {
                    &[2, 3, 3]
                } else {
                    &[1, 2, 4]
                }
            );
        }
    }
}

#[test]
fn v30_repc_excluded_forms_are_emulator_diagnostics() {
    for code in [
        vec![0x64, 0xa4],
        vec![0x65, 0xaa],
        vec![0x65, 0x90],
        vec![0x64, 0x65, 0xa6],
        vec![0xf3, 0x64, 0xa6],
        vec![0x65, 0xf2, 0xaf],
        vec![0xf2, 0x65, 0xa7],
        vec![0x64, 0xf3, 0xae],
    ] {
        let (mut c, mut b) = v30_repc_cpu(&code);
        let registers = c.r;
        let flags = c.flags;
        let memory = b.mem.clone();
        let error = c.step(&mut b).unwrap_err();
        assert!(
            error.contains("unsupported V30") && error.contains("emulator limitation"),
            "{error}"
        );
        assert_eq!(c.r, registers);
        assert_eq!(c.flags, flags);
        assert_eq!(b.mem, memory); // no invented guest exception or stack write
    }
}

#[test]
fn v30_repc_original_boot_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/v30_repc/v30_repc.hdm"
    ))
    .expect("run python3 tools/prepare_v30_repc_fixture.py");
    let manifest = include_str!("../fixtures/v30-repc-manifest.json");
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
    assert_eq!(expected.len(), 96);
    for profile in [pc98_rust::Profile::Pc9801Vm] {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "v30_repc.hdm").unwrap();
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
                    &m.bus.mem[0x8000..0x8060]
                );
                if text.contains("V30 REPC PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(m.bus.text().contains("V30 REPC PASS"), "{}", m.bus.text());
            assert_eq!(&m.bus.mem[0x8000..0x8060], expected.as_slice());
            for (key, address) in [("source_hex", 0x1ff00), ("destination_hex", 0x9000)] {
                let marker = format!("\"{key}\": \"");
                let hex = manifest
                    .split_once(&marker)
                    .unwrap()
                    .1
                    .split('"')
                    .next()
                    .unwrap();
                let bytes: Vec<u8> = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect();
                assert_eq!(&m.bus.mem[address..address + bytes.len()], bytes.as_slice());
            }
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

fn v30_nibble_mem_reproduction(ext: u8, memory: u8, al: u16) {
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x100,
        flags: 0xffd7,
        ..Cpu::default()
    };
    c.r[AX] = 0x5a83;
    let mut b = Bus::default();
    b.mem[0x100..0x106].copy_from_slice(&[0x0f, ext, 6, 0, 0x20, 0x90]);
    b.mem[0x2000] = 0x12;
    c.step(&mut b).unwrap();
    assert_eq!(b.read(0x2000), memory);
    assert_eq!(c.r[AX], 0x5a00 | al);
    assert_eq!(c.flags, 0xffd7);
    assert_eq!(c.ip, 0x105);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x106);
}

#[test]
fn v30_nibble_mem_minimal_reproduction_rol4() {
    v30_nibble_mem_reproduction(0x28, 0x23, 0x81);
}
#[test]
fn v30_nibble_mem_minimal_reproduction_ror4() {
    v30_nibble_mem_reproduction(0x2a, 0x31, 0x82);
}

// Independent oracle: rotate the three hexadecimal digits [memory high,
// memory low, AL low] as digits, rather than repeating production bit formulas.
fn v30_nibble_mem_digits(ext: u8, d: u8, al: u8) -> (u8, u8) {
    let digits = [d / 16, d % 16, al % 16];
    let out = if ext == 0x28 {
        [digits[1], digits[2], digits[0]]
    } else {
        [digits[2], digits[0], digits[1]]
    };
    (out[0] * 16 + out[1], out[2])
}

#[test]
fn v30_nibble_mem_exhaustive_digits_and_upper_nibble_policy() {
    let mut b = Bus::default();
    b.mem[0x100..0x106].copy_from_slice(&[0x0f, 0x28, 6, 0, 0x20, 0x90]);
    for ext in [0x28, 0x2a] {
        b.mem[0x101] = ext;
        for d in 0..=255u8 {
            for low in 0..16 {
                for high in [0, 0x30, 0x80, 0xf0] {
                    let al = high | low;
                    let mut c = Cpu {
                        s: [0; 4],
                        ip: 0x100,
                        r: [
                            0x5a00 | al as u16,
                            0x1234,
                            0x2345,
                            0x3456,
                            0x4567,
                            0x5678,
                            0x6789,
                            0x789a,
                        ],
                        upper: [0x9876; 8],
                        flags: if d & 1 == 0 { 0xf002 } else { 0xffd7 },
                        ..Cpu::default()
                    };
                    let before = c.clone();
                    b.mem[0x1fff..0x2002].copy_from_slice(&[0xa5, d, 0x5a]);
                    let (memory, accumulator) = v30_nibble_mem_digits(ext, d, al);
                    c.step(&mut b).unwrap();
                    assert_eq!(b.read(0x2000), memory);
                    assert_eq!(c.byte(AX) & 15, accumulator); // Hardware-defined.
                    assert_eq!(c.byte(AX) & 0xf0, high); // Emulator policy only.
                    assert_eq!(c.byte(4), 0x5a);
                    assert_eq!(&c.r[1..], &before.r[1..]);
                    assert_eq!(c.upper, before.upper);
                    assert_eq!(c.s, before.s);
                    assert_eq!(c.flags, before.flags);
                    assert_eq!(b.read(0x1fff), 0xa5);
                    assert_eq!(b.read(0x2001), 0x5a);
                    assert_eq!(c.ip, 0x105);
                    c.step(&mut b).unwrap();
                    assert_eq!(c.ip, 0x106);
                }
            }
        }
    }
}

fn v30_nibble_mem_address_case(
    ext: u8,
    modrm: u8,
    displacement: &[u8],
    prefix: Option<u8>,
    segments: [u16; 4],
    offset: u16,
    segment: usize,
) {
    let mut c = Cpu {
        s: segments,
        ip: 0x100,
        r: [0x5a8d, 0x1234, 0x2345, 0xfff0, 0x8000, 0x3000, 0x30, 0x50],
        upper: [0x9876; 8],
        flags: 0xffd7,
        ..Cpu::default()
    };
    let mut code = Vec::new();
    code.extend(prefix);
    code.extend([0x0f, ext, modrm]);
    code.extend(displacement);
    let mut b = Bus::default();
    let pc = segments[CS] as usize * 16 + 0x100;
    b.mem[pc..pc + code.len()].copy_from_slice(&code);
    b.mem[pc + code.len()] = 0x90;
    let address = (segments[segment] as usize * 16 + offset as usize) & 0xfffff;
    b.mem[(address + 0xfffff) & 0xfffff] = 0xa5;
    b.mem[address] = 0x12;
    b.mem[(address + 1) & 0xfffff] = 0x5a;
    // Whole backing RAM comparison detects writes anywhere outside the operand.
    let mut expected_ram = b.mem.clone();
    let (memory, low) = v30_nibble_mem_digits(ext, 0x12, 0x8d);
    expected_ram[address] = memory;
    let before = c.clone();
    c.step(&mut b).unwrap();
    assert_eq!(b.mem, expected_ram);
    assert_eq!(c.r[AX], 0x5a80 | low as u16);
    assert_eq!(&c.r[1..], &before.r[1..]);
    assert_eq!(c.upper, before.upper);
    assert_eq!(c.s, before.s);
    assert_eq!(c.flags, before.flags);
    assert_eq!(c.ip, 0x100 + code.len() as u32);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x101 + code.len() as u32);
}

#[test]
fn v30_nibble_mem_addressing_segments_and_wraps() {
    let segments = [0x1000, 0x2000, 0x3000, 0x4000];
    // Expected effective offsets from the explicit register values above.
    let bases = [0x20u16, 0x40, 0x3030, 0x3050, 0x30, 0x50, 0x3000, 0xfff0];
    for ext in [0x28, 0x2a] {
        for mode in 0..3u8 {
            for rm in 0..8u8 {
                let (disp, offset) = match mode {
                    0 if rm == 6 => (vec![0xff, 0xff], 0xffff),
                    0 => (vec![], bases[rm as usize]),
                    1 => (vec![0x80], bases[rm as usize].wrapping_sub(128)),
                    _ => (vec![0x34, 0xf0], bases[rm as usize].wrapping_add(0xf034)),
                };
                let default = if rm == 2 || rm == 3 || (rm == 6 && mode != 0) {
                    SS
                } else {
                    DS
                };
                for (prefix, segment) in [
                    (None, default),
                    (Some(0x26), ES),
                    (Some(0x2e), CS),
                    (Some(0x36), SS),
                    (Some(0x3e), DS),
                ] {
                    v30_nibble_mem_address_case(
                        ext,
                        mode * 64 + rm,
                        &disp,
                        prefix,
                        segments,
                        offset,
                        segment,
                    );
                }
            }
        }
        // FFFF:0020 wraps the VM 20-bit physical address to 00010h.
        v30_nibble_mem_address_case(
            ext,
            6,
            &[0x20, 0],
            None,
            [0x1000, 0x2000, 0x3000, 0xffff],
            0x20,
            DS,
        );
    }
}

#[test]
fn v30_nibble_mem_bus_rom_and_planar_vram() {
    for ext in [0x28, 0x2a] {
        for (seg, address, writable) in [(0xf000, 0xf0123, false), (0xa800, 0xa8123, true)] {
            let mut c = Cpu {
                s: [0, 0, 0, seg],
                ip: 0x100,
                flags: 0xffd7,
                ..Cpu::default()
            };
            c.r[AX] = 0x5a83;
            let mut b = Bus::default();
            b.mem[0x100..0x106].copy_from_slice(&[0x0f, ext, 6, 0x23, 1, 0x90]);
            assert_eq!(b.grcg_mode, 0);
            // VRAM backing RAM deliberately differs from the Bus-visible plane.
            b.mem[address] = if writable { 0x96 } else { 0x12 };
            if writable {
                b.write(address - 1, 0xa5);
                b.write(address, 0x12);
                b.write(address + 1, 0x5a);
            }
            let ram = b.mem.clone();
            let mut graphics = b.graphics.clone();
            let (memory, low) = v30_nibble_mem_digits(ext, 0x12, 3);
            if writable {
                graphics[0x123] = memory;
            }
            c.step(&mut b).unwrap();
            assert_eq!(b.read(address), if writable { memory } else { 0x12 });
            assert_eq!(b.mem, ram);
            assert_eq!(b.graphics, graphics);
            assert_eq!(c.r[AX], 0x5a80 | low as u16);
            assert_eq!(c.flags, 0xffd7);
            assert_eq!(c.ip, 0x105);
            c.step(&mut b).unwrap();
            assert_eq!(c.ip, 0x106);
        }
    }
}

#[test]
fn v30_nibble_mem_excluded_encodings_are_unsupported_without_writes() {
    let mut b = Bus::default();
    for ext in [0x28, 0x2a] {
        for modrm in 0..=255u8 {
            if modrm < 0xc0 && modrm & 0x38 == 0 {
                continue;
            }
            let mut c = Cpu {
                s: [0; 4],
                ip: 0x100,
                r: [0x2000; 8],
                flags: 0xffd7,
                ..Cpu::default()
            };
            b.mem[0x100..0x105].copy_from_slice(&[0x0f, ext, modrm, 0, 0x20]);
            let ram = b.mem.clone();
            let before = c.clone();
            let error = c.step(&mut b).unwrap_err();
            assert!(error.contains("unsupported V30 nibble rotation"), "{error}");
            assert!(error.contains("emulator limitation"));
            assert_eq!(c.r, before.r);
            assert_eq!(c.upper, before.upper);
            assert_eq!(c.s, before.s);
            assert_eq!(c.flags, before.flags);
            assert_eq!(b.mem, ram);
        }
    }
}

#[test]
fn v30_nibble_mem_original_boot_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/v30_nibble_mem/v30_nibble_mem.hdm"
    ))
    .expect("run python3 tools/prepare_v30_nibble_mem_fixture.py");
    let manifest = include_str!("../fixtures/v30-nibble-mem-manifest.json");
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
    assert_eq!(expected.len(), 36);
    for profile in [pc98_rust::Profile::Pc9801Vm] {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "v30_nibble_mem.hdm").unwrap();
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
                    &m.bus.mem[0x8000..0x8024]
                );
                if text.contains("V30 NIBBLE MEM PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(
                m.bus.text().contains("V30 NIBBLE MEM PASS"),
                "{}",
                m.bus.text()
            );
            assert_eq!(&m.bus.mem[0x8000..0x8024], expected.as_slice());
            assert_eq!(&m.bus.mem[0x9000..0x9003], &[0xa5, 0xea, 0x5a]);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

// A real PIC request, never a direct Cpu::interrupt call.
fn segment_irq_cpu(producer: &[u8], following: &[u8]) -> (Cpu, Bus) {
    let mut c = Cpu {
        s: [0x1100, 0x1000, 0x2200, 0x3300],
        ip: 0x100,
        flags: 0xf002 | IF | CF | DF | OF | AF | SF | PF | ZF,
        ..Cpu::default()
    };
    c.r[AX] = 0x4400;
    c.r[SP] = 0x8000;
    c.r[BP] = 0x4001;
    let mut b = Bus::default();
    let code = [producer, following, &[0x41, 0x90]].concat(); // resumed INC CX
    b.mem[0x10100..0x10100 + code.len()].copy_from_slice(&code);
    b.write_seg(0, 9 * 4, 0x200, true);
    b.write_seg(0, 9 * 4 + 2, 0, true);
    // NOP; PUSH AX; INC byte CS:[6000]; MOV AL,20; OUT 0,AL; POP AX; IRET
    b.mem[0x200..0x20d].copy_from_slice(&[
        0x90, 0x50, 0x2e, 0xfe, 0x06, 0, 0x60, 0xb0, 0x20, 0xe6, 0, 0x58, 0xcf,
    ]);
    b.pics[0].mask = 0xfd;
    (c, b)
}

fn segment_irq_pending(c: &mut Cpu, b: &mut Bus, resume: u32) {
    b.pics[0].request(1);
    c.step(b).unwrap();
    assert_eq!((c.s[CS], c.ip), (0x1000, resume));
    assert_eq!((b.pics[0].irr, b.pics[0].isr), (2, 0));
    assert_ne!(c.flags & IF, 0);
}

fn segment_irq_deliver_and_resume(c: &mut Cpu, b: &mut Bus) {
    let (resume, ss, sp, flags, ax) = (c.ip, c.s[SS], c.r[SP], c.flags, c.r[AX]);
    let base = (ss as usize) << 4;
    b.mem[base + sp as usize - 9] = 0xa5;
    b.mem[base + sp as usize] = 0x5a;
    c.step(b).unwrap(); // acknowledgement and handler NOP
    assert_eq!((c.s[CS], c.ip, c.r[SP]), (0, 0x201, sp - 6));
    assert_eq!(c.flags & IF, 0);
    assert_eq!((b.pics[0].irr, b.pics[0].isr), (0, 2));
    assert_eq!(b.read_seg(ss, sp - 6, true), resume as u16);
    assert_eq!(b.read_seg(ss, sp - 4, true), 0x1000);
    assert_eq!(b.read_seg(ss, sp - 2, true), flags);
    for _ in 0..6 {
        c.step(b).unwrap();
    }
    assert_eq!(
        (c.s[CS], c.ip, c.r[SP], c.flags),
        (0x1000, resume, sp, flags)
    );
    assert_eq!(c.r[AX], ax);
    assert_eq!((b.pics[0].irr, b.pics[0].isr, b.mem[0x6000]), (0, 0, 1));
    assert_eq!(
        (b.mem[base + sp as usize - 9], b.mem[base + sp as usize]),
        (0xa5, 0x5a)
    );
    c.step(b).unwrap();
    assert_eq!(c.r[CX], 1);
    assert_eq!(b.mem[0x6000], 1);
}

#[test]
fn v30_segment_irq_minimal_mov_ds_reproduction() {
    let (mut c, mut b) = segment_irq_cpu(&[0x8e, 0xd8], &[0x43]);
    let flags = c.flags;
    c.step(&mut b).unwrap();
    assert_eq!((c.s[DS], c.flags), (0x4400, flags));
    segment_irq_pending(&mut c, &mut b, 0x103);
    assert_eq!(c.r[BX], 1);
    segment_irq_deliver_and_resume(&mut c, &mut b);
}

#[test]
fn v30_segment_irq_all_documented_mov_operands_and_pops() {
    for op in [0x8c, 0x8e] {
        for seg in [ES, CS, SS, DS] {
            if op == 0x8e && seg == CS {
                continue;
            }
            // register, default DS, default SS, overridden ES/DS/CS, odd addresses
            for form in 0..6 {
                let mut code = Vec::new();
                if form >= 3 {
                    code.push([0x26, 0x3e, 0x2e][form - 3]);
                }
                code.extend_from_slice(&[
                    op,
                    (seg as u8) << 3
                        | if form == 0 {
                            0xc0
                        } else if form == 1 || form == 5 {
                            6
                        } else {
                            0x46
                        },
                ]);
                if form == 1 || form == 5 {
                    code.extend_from_slice(&[1, 0x40]);
                } else if form >= 2 {
                    code.push(0);
                }
                let (mut c, mut b) = segment_irq_cpu(&code, &[0x26, 0x43]);
                let old_segments = c.s;
                let memory_segment = match form {
                    2 => SS,
                    3 => ES,
                    5 => CS,
                    _ => DS,
                };
                let address = ((c.s[memory_segment] as usize) << 4) + 0x4001;
                b.mem[address - 1..address + 3].copy_from_slice(&[0xa5, 0, 0x44, 0x5a]);
                let flags = c.flags;
                c.step(&mut b).unwrap();
                assert_eq!(c.flags, flags, "op={op:x} seg={seg} form={form}");
                let transferred = if op == 0x8e {
                    0x4400
                } else {
                    old_segments[seg]
                };
                if op == 0x8e {
                    assert_eq!(c.s[seg], transferred);
                    assert_eq!(c.r[AX], 0x4400);
                } else if form == 0 {
                    assert_eq!(c.r[AX], transferred);
                    assert_eq!(c.s, old_segments);
                } else {
                    assert_eq!(b.read16(address), transferred);
                    assert_eq!(c.s, old_segments);
                }
                let expected_memory = if op == 0x8c && form != 0 {
                    transferred
                } else {
                    0x4400
                };
                assert_eq!(
                    &b.mem[address - 1..address + 3],
                    &[
                        0xa5,
                        expected_memory as u8,
                        (expected_memory >> 8) as u8,
                        0x5a
                    ]
                );
                segment_irq_pending(&mut c, &mut b, 0x100 + code.len() as u32 + 2);
                assert_eq!(c.r[BX], 1);
                segment_irq_deliver_and_resume(&mut c, &mut b);
            }
        }
    }
    for seg in [ES, SS, DS] {
        let (mut c, mut b) = segment_irq_cpu(&[0x07 | ((seg as u8) << 3)], &[0x43]);
        b.mem[0x2a000 - 1..0x2a000 + 3].copy_from_slice(&[0xa5, 0, 0x44, 0x5a]);
        let flags = c.flags;
        c.step(&mut b).unwrap();
        assert_eq!((c.s[seg], c.r[SP], c.flags), (0x4400, 0x8002, flags));
        assert_eq!(&b.mem[0x29fff..0x2a003], &[0xa5, 0, 0x44, 0x5a]);
        segment_irq_pending(&mut c, &mut b, 0x102);
        assert_eq!(c.r[BX], 1);
        segment_irq_deliver_and_resume(&mut c, &mut b);
    }
}

#[test]
fn v30_segment_irq_changed_ss_protects_stack_pointer_setup() {
    for producer in [&[0x8e, 0xd0][..], &[0x17][..]] {
        let (mut c, mut b) = segment_irq_cpu(producer, &[0xbc, 0, 0x70]);
        b.write_seg(0x2200, 0x8000, 0x4400, true);
        let flags = c.flags;
        c.step(&mut b).unwrap();
        segment_irq_pending(&mut c, &mut b, 0x100 + producer.len() as u32 + 3);
        assert_eq!((c.s[SS], c.r[SP], c.flags), (0x4400, 0x7000, flags));
        segment_irq_deliver_and_resume(&mut c, &mut b);
    }
}

#[test]
fn v30_segment_irq_consecutive_producers_renew_and_prefix_is_one_instruction() {
    let code = [0x8e, 0xd8, 0x8e, 0xc0, 0x8c, 0xd8, 0x07];
    let (mut c, mut b) = segment_irq_cpu(&code, &[0x26, 0x2e, 0x43]);
    b.write_seg(0x2200, 0x8000, 0x4400, true);
    let flags = c.flags;
    c.step(&mut b).unwrap();
    b.pics[0].request(1);
    for ip in [0x104, 0x106, 0x107, 0x10a] {
        c.step(&mut b).unwrap();
        assert_eq!(
            (c.s[CS], c.ip, b.pics[0].irr, b.pics[0].isr),
            (0x1000, ip, 2, 0)
        );
        if ip != 0x10a {
            assert_eq!(c.flags, flags);
        }
    }
    assert_eq!(c.r[BX], 1);
    segment_irq_deliver_and_resume(&mut c, &mut b);
}

#[test]
fn v30_segment_irq_if_clear_masking_and_nonproducer_controls() {
    for if_clear in [false, true] {
        let (mut c, mut b) = segment_irq_cpu(&[0x8e, 0xd8], &[0x90, 0x90]);
        if if_clear {
            c.flags &= !IF;
        } else {
            b.pics[0].mask = 0xff;
        }
        c.step(&mut b).unwrap();
        let flags = c.flags;
        b.pics[0].request(1);
        for _ in 0..2 {
            c.step(&mut b).unwrap();
        }
        assert_eq!(
            (c.ip, c.flags, b.pics[0].irr, b.pics[0].isr),
            (0x104, flags, 2, 0)
        );
        c.flags |= IF;
        b.pics[0].mask = 0xfd;
        segment_irq_deliver_and_resume(&mut c, &mut b);
    }
    for code in [
        vec![0x89, 0xc2],
        vec![0xb8, 0, 0x44],
        vec![0x06],
        vec![0x0e],
        vec![0x16],
        vec![0x1e],
    ] {
        let (mut c, mut b) = segment_irq_cpu(&code, &[0x43]);
        let flags = c.flags;
        c.step(&mut b).unwrap();
        assert_eq!(c.flags, flags);
        b.pics[0].request(1);
        c.step(&mut b).unwrap();
        assert_eq!((c.s[CS], c.ip, c.r[BX]), (0, 0x201, 0));
        assert_eq!(
            b.read_seg(c.s[SS], c.r[SP], true),
            0x100 + code.len() as u16
        );
        assert_eq!((b.pics[0].irr, b.pics[0].isr), (0, 2));
    }
}

#[test]
fn v30_segment_irq_reset_discards_pending_inhibition() {
    let (mut c, mut b) = segment_irq_cpu(&[0x8e, 0xd8], &[0x43]);
    c.step(&mut b).unwrap();
    b.pics[0].request(1);
    let mut m = Machine {
        cpu: c,
        bus: b,
        ..Machine::default()
    };
    m.reset();
    assert_eq!(m.cpu.inhibit, 0);
    assert_eq!((m.bus.pics[0].irr, m.bus.pics[0].isr), (0, 0));
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x100;
    m.cpu.r[SP] = 0x8000;
    m.cpu.flags |= IF;
    m.bus.mem[0x100] = 0x43;
    m.bus.write_seg(0, 9 * 4, 0x200, true);
    m.bus.write_seg(0, 9 * 4 + 2, 0, true);
    m.bus.mem[0x200] = 0x90;
    m.bus.pics[0].mask = 0xfd;
    m.bus.pics[0].request(1);
    m.cpu.step(&mut m.bus).unwrap();
    assert_eq!((m.cpu.ip, m.cpu.r[BX], m.bus.pics[0].isr), (0x201, 0, 2));
    assert_eq!(m.bus.read16(0x7ffa), 0x100);
}

#[test]
fn v30_segment_irq_original_boot_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/v30_segment_irq/v30_segment_irq.hdm"
    ))
    .expect("run python3 tools/prepare_v30_segment_irq_fixture.py");
    let manifest = include_str!("../fixtures/v30-segment-irq-manifest.json");
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
    assert_eq!(expected.len(), 48);
    for profile in [pc98_rust::Profile::Pc9801Vm] {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "v30_segment_irq.hdm").unwrap();
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
                    &m.bus.mem[0x8000..0x8030]
                );
                if text.contains("V30 SEGMENT IRQ PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(
                m.bus.text().contains("V30 SEGMENT IRQ PASS"),
                "{}",
                m.bus.text()
            );
            assert_eq!(&m.bus.mem[0x8000..0x8030], expected.as_slice());
            assert_eq!(&m.bus.mem[0x9000..0x9004], &[0xa5, 0xc0, 0x1f, 0x5a]);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

#[test]
fn v30_add4s_minimal_reproduction() {
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x100,
        flags: CF | ZF | 2,
        ..Cpu::default()
    };
    c.r[SI] = 0x2000;
    c.r[DI] = 0x3000;
    c.r[CX] = 2;
    let mut b = Bus::default();
    b.mem[0x100..0x103].copy_from_slice(&[0x0f, 0x20, 0x90]);
    b.mem[0x2000] = 0x09;
    b.mem[0x3000] = 0x01;
    c.step(&mut b).unwrap();
    assert_eq!(b.read(0x3000), 0x10);
    assert_eq!(c.flags & (CF | ZF), 0);
    assert_eq!(c.ip, 0x102);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x103);
}

// Independent base-100 oracle: no whole-string host integer and no nibble
// carry/adjustment formula shared with production.
fn v30_add4s_oracle(source: &[u8], destination: &[u8]) -> (Vec<u8>, u16) {
    let mut carry = 0u16;
    let result: Vec<u8> = source
        .iter()
        .zip(destination)
        .map(|(&a, &b)| {
            let sum = u16::from(a / 16 * 10 + a % 16) + u16::from(b / 16 * 10 + b % 16) + carry;
            carry = sum / 100;
            let value = (sum % 100) as u8;
            value / 10 * 16 + value % 10
        })
        .collect();
    let flags = if carry != 0 { CF } else { 0 }
        | if result.iter().all(|&b| b == 0) {
            ZF
        } else {
            0
        };
    (result, flags)
}

fn v30_add4s_cpu(cl: u8, flags: u16) -> Cpu {
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x100,
        r: [
            0x1234,
            0xab00 | cl as u16,
            0x3456,
            0x4567,
            0x8000,
            0x6789,
            0x2000,
            0x3000,
        ],
        upper: [0x9876; 8],
        flags,
        ..Cpu::default()
    };
    c.r[CX] |= (cl as u16).wrapping_mul(47) << 8;
    c
}

fn v30_add4s_assert_state(c: &mut Cpu, b: &mut Bus, before: &Cpu, end: u32, flags: u16) {
    assert_eq!(c.r, before.r);
    assert_eq!(c.upper, before.upper);
    assert_eq!(c.s, before.s);
    assert_eq!(c.flags & (CF | ZF), flags);
    // Control/reserved bits must survive; undefined status bits are excluded.
    assert_eq!(
        c.flags & !(CF | ZF | AF | OF | PF | SF),
        before.flags & !(CF | ZF | AF | OF | PF | SF)
    );
    assert_eq!(c.ip, end);
    c.step(b).unwrap();
    assert_eq!(c.ip, end + 1);
}

#[test]
fn v30_add4s_exhaustive_one_byte_pairs() {
    let mut b = Bus::default();
    b.mem[0x100..0x103].copy_from_slice(&[0x0f, 0x20, 0x90]);
    for a in 0..100u8 {
        for d in 0..100u8 {
            let source = a / 10 * 16 + a % 10;
            let destination = d / 10 * 16 + d % 10;
            for incoming in [0, CF] {
                let initial = 0xf002
                    | incoming
                    | if (a + d) & 1 == 0 {
                        AF | OF | SF | PF | DF | IF | ZF
                    } else {
                        0
                    };
                let mut c = v30_add4s_cpu(2, initial);
                c.r[CX] = ((a as u16 ^ d as u16) << 8) | 2;
                let before = c.clone();
                b.mem[0x1fff..0x2002].copy_from_slice(&[0xa5, source, 0x5a]);
                b.mem[0x2fff..0x3002].copy_from_slice(&[0xa5, destination, 0x5a]);
                let (expected, flags) = v30_add4s_oracle(&[source], &[destination]);
                c.step(&mut b).unwrap();
                assert_eq!(&b.mem[0x2fff..0x3002], &[0xa5, expected[0], 0x5a]);
                assert_eq!(&b.mem[0x1fff..0x2002], &[0xa5, source, 0x5a]);
                v30_add4s_assert_state(&mut c, &mut b, &before, 0x102, flags);
            }
        }
    }
}

fn v30_add4s_ram_case(cl: u8, source: &[u8], destination: &[u8], literal: Option<(&[u8], u16)>) {
    let (expected, flags) = v30_add4s_oracle(source, destination);
    if let Some((bytes, defined)) = literal {
        assert_eq!(expected, bytes);
        assert_eq!(flags, defined);
    }
    for df in [0, DF] {
        let mut c = v30_add4s_cpu(cl, 0xfad7 | df);
        let before = c.clone();
        let mut b = Bus::default();
        b.mem[0x100..0x103].copy_from_slice(&[0x0f, 0x20, 0x90]);
        let n = source.len();
        assert_eq!(n, (cl as usize).div_ceil(2));
        b.mem[0x2000..0x2000 + n].copy_from_slice(source);
        b.mem[0x2fff] = 0xa5;
        b.mem[0x3000..0x3000 + n].copy_from_slice(destination);
        b.mem[0x3000 + n] = 0x5a;
        c.step(&mut b).unwrap();
        assert_eq!(&b.mem[0x3000..0x3000 + n], expected.as_slice(), "CL={cl}");
        assert_eq!(&b.mem[0x2000..0x2000 + n], source);
        assert_eq!((b.read(0x2fff), b.read(0x3000 + n)), (0xa5, 0x5a));
        v30_add4s_assert_state(&mut c, &mut b, &before, 0x102, flags);
    }
}

#[test]
fn v30_add4s_all_documented_counts_and_carry_patterns() {
    for cl in 1..=254u8 {
        let n = (cl as usize).div_ceil(2);
        let source: Vec<u8> = (0..n)
            .map(|i| ((i * 37 + cl as usize) % 100) as u8)
            .map(|v| v / 10 * 16 + v % 10)
            .collect();
        let dest: Vec<u8> = (0..n)
            .map(|i| ((i * 61 + 19) % 100) as u8)
            .map(|v| v / 10 * 16 + v % 10)
            .collect();
        v30_add4s_ram_case(cl, &source, &dest, None);
        let mut one = vec![0; n];
        one[0] = 1;
        v30_add4s_ram_case(cl, &one, &vec![0x99; n], None); // Full carry chain.
        let mut terminated = vec![0x99; n];
        terminated[n / 2] = 0x48;
        v30_add4s_ram_case(cl, &one, &terminated, None);
        v30_add4s_ram_case(cl, &vec![0; n], &one, None); // Last byte zero, result nonzero.
        v30_add4s_ram_case(cl, &vec![0; n], &vec![0; n], None);
    }
}

#[test]
fn v30_add4s_literal_rounded_width_and_whole_result_zero() {
    v30_add4s_ram_case(4, &[0x01, 0], &[0x99, 0x99], Some((&[0, 0], CF | ZF)));
    v30_add4s_ram_case(3, &[0x01, 0], &[0x99, 0x09], Some((&[0, 0x10], 0)));
    v30_add4s_ram_case(3, &[0x01, 0x80], &[0x99, 0x19], Some((&[0, 0], CF | ZF)));
    v30_add4s_ram_case(4, &[1, 0], &[0, 0], Some((&[1, 0], 0)));
}

#[test]
fn v30_add4s_source_overrides_fixed_destination_and_aliasing() {
    for (prefix, segment) in [
        (None, DS),
        (Some(0x26), ES),
        (Some(0x2e), CS),
        (Some(0x36), SS),
        (Some(0x3e), DS),
    ] {
        for df in [0, DF] {
            for alias in [false, true] {
                let mut c = v30_add4s_cpu(3, 0xfad7 | df);
                c.s = [0x1100, 0x2200, 0x3300, 0x4400];
                if alias {
                    c.s[ES] = c.s[segment];
                    c.r[DI] = c.r[SI];
                }
                let mut b = Bus::default();
                // Distinct candidate sources and destinations expose wrong selection.
                for (i, &s) in c.s.iter().enumerate() {
                    b.write_seg(s, c.r[SI], 0x1111 * (i as u16 + 1), true);
                    if !alias {
                        b.write_seg(s, c.r[DI], 0x1212 * (i as u16 + 1), true);
                    }
                }
                let src = c.s[segment] as usize * 16 + c.r[SI] as usize;
                let dst = c.s[ES] as usize * 16 + c.r[DI] as usize;
                b.write(dst - 1, 0xa5);
                b.write(dst + 2, 0x5a);
                let source = [b.read(src), b.read(src + 1)];
                let destination = [b.read(dst), b.read(dst + 1)];
                let (expected, flags) = v30_add4s_oracle(&source, &destination);
                let mut code: Vec<u8> = prefix.into_iter().collect();
                code.extend([0x0f, 0x20, 0x90]);
                let start = c.s[CS] as usize * 16 + 0x100;
                b.mem[start..start + code.len()].copy_from_slice(&code);
                let ram = b.mem.clone();
                let before = c.clone();
                c.step(&mut b).unwrap();
                assert_eq!(&b.mem[dst..dst + 2], expected.as_slice());
                // Every other byte includes all alternative destinations, source and guards.
                assert_eq!(&b.mem[..dst], &ram[..dst]);
                assert_eq!(&b.mem[dst + 2..], &ram[dst + 2..]);
                v30_add4s_assert_state(
                    &mut c,
                    &mut b,
                    &before,
                    0x102 + u32::from(prefix.is_some()),
                    flags,
                );
            }
        }
    }
}

#[test]
fn v30_add4s_offset_and_physical_wraps() {
    for (source_segment, dest_segment, si, di) in [
        (0x1000, 0x4000, 0xffff, 0x2000),
        (0x1000, 0x4000, 0x2000, 0xffff),
        (0x1000, 0x4000, 0xffff, 0xffff),
        (0xffff, 0x4000, 0x000f, 0x2000),
        (0x1000, 0xffff, 0x2000, 0x000f),
        (0xffff, 0xffff, 0x000f, 0x002f),
    ] {
        let mut c = v30_add4s_cpu(4, 0xfed7);
        c.s[DS] = source_segment;
        c.s[ES] = dest_segment;
        c.r[SI] = si;
        c.r[DI] = di;
        let addr = |s: u16, off: u16, delta: u16| {
            (s as usize * 16 + off.wrapping_add(delta) as usize) & 0xfffff
        };
        let mut b = Bus::default();
        b.mem[0x100..0x103].copy_from_slice(&[0x0f, 0x20, 0x90]);
        // Seed ROM through fixture setup; CPU must read through the bus.
        for (i, value) in [0x99, 0x01].into_iter().enumerate() {
            b.mem[addr(source_segment, si, i as u16)] = value;
        }
        for (i, value) in [0x01, 0x02].into_iter().enumerate() {
            b.mem[addr(dest_segment, di, i as u16)] = value;
        }
        let left = addr(dest_segment, di, u16::MAX);
        let right = addr(dest_segment, di, 2);
        b.mem[left] = 0xa5;
        b.mem[right] = 0x5a;
        let before = c.clone();
        let ram = b.mem.clone();
        c.step(&mut b).unwrap();
        // First destination byte may be ROM (fffff), whose write is ignored.
        for (i, expected) in [0x00, 0x04].into_iter().enumerate() {
            let a = addr(dest_segment, di, i as u16);
            assert_eq!(b.read(a), if a >= 0xf0000 { ram[a] } else { expected });
        }
        assert_eq!((b.read(left), b.read(right)), (0xa5, 0x5a));
        for i in 0..2 {
            let a = addr(source_segment, si, i);
            assert_eq!(b.read(a), ram[a]);
        }
        v30_add4s_assert_state(&mut c, &mut b, &before, 0x102, 0);
    }
}

#[test]
fn v30_add4s_rom_and_planar_vram_use_bus_mapping() {
    for (source, destination) in [
        (0xf000, 0x3000),
        (0x2000, 0xf000),
        (0xa800, 0xb000),
        (0xb800, 0x3000),
        (0x2000, 0xb800),
    ] {
        let mut c = v30_add4s_cpu(4, 0xfed7);
        c.s[DS] = source;
        c.s[ES] = destination;
        c.r[SI] = 0x10;
        c.r[DI] = 0x20;
        let mut b = Bus::default();
        assert_eq!(b.grcg_mode, 0);
        assert!(!b.egc.enabled && !b.pegc);
        b.mem[0x100..0x103].copy_from_slice(&[0x0f, 0x20, 0x90]);
        let src = source as usize * 16 + 0x10;
        let dst = destination as usize * 16 + 0x20;
        for (base, data) in [(src, [0x99, 0x01]), (dst, [0x01, 0x02])] {
            for (i, value) in data.into_iter().enumerate() {
                if base >= 0xf0000 {
                    b.mem[base + i] = value;
                } else {
                    b.write(base + i, value);
                }
            }
        }
        if destination >= 0xf000 {
            b.mem[dst - 1] = 0xa5;
            b.mem[dst + 2] = 0x5a;
        } else {
            b.write(dst - 1, 0xa5);
            b.write(dst + 2, 0x5a);
        }
        // Poison unused backing RAM for VRAM; raw access must fail these assertions.
        for base in [src, dst] {
            if (0xa8000..0xc0000).contains(&base) {
                b.mem[base..base + 2].fill(0x77);
            }
        }
        let ram = b.mem.clone();
        let graphics = b.graphics.clone();
        let before = c.clone();
        c.step(&mut b).unwrap();
        assert_eq!(
            (b.read(dst), b.read(dst + 1)),
            if destination >= 0xf000 {
                (1, 2)
            } else {
                (0, 4)
            }
        );
        assert_eq!((b.read(src), b.read(src + 1)), (0x99, 1));
        assert_eq!((b.read(dst - 1), b.read(dst + 2)), (0xa5, 0x5a));
        if (0xa8000..0xc0000).contains(&dst) {
            assert_eq!(b.mem, ram);
            let offset = dst - 0xa8000;
            assert_eq!(&b.graphics[..offset], &graphics[..offset]);
            assert_eq!(&b.graphics[offset + 2..], &graphics[offset + 2..]);
        } else {
            assert_eq!(b.graphics, graphics);
        }
        if destination >= 0xf000 {
            assert_eq!(b.mem, ram);
        }
        v30_add4s_assert_state(&mut c, &mut b, &before, 0x102, 0);
    }
}

#[test]
fn v30_add4s_undefined_status_flag_preservation_policy() {
    // Emulator policy only: NEC leaves AF/OF/PF/SF undefined for ADD4S.
    for policy in [0, AF | OF | PF | SF, AF | PF, OF | SF] {
        let mut c = v30_add4s_cpu(2, 2 | policy);
        let mut b = Bus::default();
        b.mem[0x100..0x102].copy_from_slice(&[0x0f, 0x20]);
        b.mem[0x2000] = 0x99;
        b.mem[0x3000] = 1;
        c.step(&mut b).unwrap();
        assert_eq!(c.flags & (AF | OF | PF | SF), policy);
    }
}

#[test]
fn v30_add4s_original_boot_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/v30_add4s/v30_add4s.hdm"
    ))
    .expect("run python3 tools/prepare_v30_add4s_fixture.py");
    let manifest = include_str!("../fixtures/v30-add4s-manifest.json");
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
        m.insert(0, image.clone(), "v30_add4s.hdm").unwrap();
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
                if text.contains("V30 ADD4S PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(m.bus.text().contains("V30 ADD4S PASS"), "{}", m.bus.text());
            assert_eq!(&m.bus.mem[0x8000..0x8020], expected.as_slice());
            assert_eq!(
                &m.bus.mem[0x9000..0x9014],
                &[0xa5, 0, 0, 0x5a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xa5, 1, 0, 0x5a]
            );
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

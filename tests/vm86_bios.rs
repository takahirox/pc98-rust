// Original MIT synthetic BIOS/VM86 regression. Only repository-authored media.
use pc98_rust::{bus::Bus, cpu::*, Machine, Profile};

#[test]
fn original_vm86_bios_guest_cold_reset() {
    let image = std::fs::read("fixtures/vm86_bios/vm86_bios.hdm")
        .expect("run tools/prepare_vm86_bios_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "vm86_bios.hdm").unwrap();
    for run in 0..2 {
        if run != 0 {
            m.reset();
        }
        let mut injected = [false; 3];
        let mut seen = [false; 3];
        for _ in 0..2000 {
            m.step_with_observer(1000, |c, b| {
                if c.vm86() {
                    assert_eq!(c.cpl, 3);
                    seen[b.mem[0x8000] as usize] = true;
                }
            });
            assert!(m.error.is_none(), "{:?} {:?}", m.error, m.cpu);
            let phase = m.bus.mem[0x8000] as usize;
            if phase < 3 && m.bus.mem[0x8003] == 1 && !injected[phase] {
                m.bus.key(0x1d, true);
                injected[phase] = true;
            }
            let text = m.bus.text();
            assert!(!text.contains("VM86 BIOS FAIL"), "{text} {:?}", m.cpu);
            if text.contains("VM86 BIOS PASS") {
                break;
            }
        }
        assert!(
            m.bus.text().contains("VM86 BIOS PASS"),
            "{} {:?}",
            m.bus.text(),
            m.cpu
        );
        assert_eq!(seen, [true; 3]);
        assert_eq!(injected, [true; 3]);
        assert_eq!(&m.bus.mem[0x8000..0x8004], &[3, 0, 4, 0]);
        assert_eq!(m.cpu.fault_count, 4 + get(&m.bus, 0x8008) as u64);
        assert_eq!(m.bus.fdc.writes, 0);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
    }
}

fn put(b: &mut Bus, a: usize, v: u32) {
    b.mem[a..a + 4].copy_from_slice(&v.to_le_bytes());
}
fn get(b: &Bus, a: usize) -> u32 {
    u32::from_le_bytes(b.mem[a..a + 4].try_into().unwrap())
}
fn desc(b: &mut Bus, selector: usize, access: u8) {
    b.mem[0x5000 + selector..0x5008 + selector]
        .copy_from_slice(&[255, 255, 0, 0, 0, access, 0xcf, 0]);
}
fn gate(b: &mut Bus, n: usize, access: u8) {
    b.mem[0x6000 + n * 8..0x6008 + n * 8].copy_from_slice(&[0, 0x10, 8, 0, 0, access, 1, 0]);
}
fn setup(code: &[u8], pg: bool, flags: u32) -> (Cpu, Bus) {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    pc98_rust::firmware::initialize(&mut b);
    b.a20 = true;
    desc(&mut b, 8, 0x9b);
    desc(&mut b, 16, 0x93);
    for n in [1, 3, 4, 6, 8, 10, 11, 12, 13, 14, 0x30] {
        gate(&mut b, n, 0xee);
    }
    b.mem[0x10000] = 0xcf;
    b.mem[0x11000] = 0xf4;
    for (i, v) in [
        0, 0x2000, flags, 0xabcd7000, 0x3000, 0x4000, 0x5000, 0x6000, 0x7000,
    ]
    .into_iter()
    .enumerate()
    {
        put(&mut b, 0x18000 + i * 4, v);
    }
    // A real cached, busy 32-bit TSS; I/O map covers all ports and sentinel.
    put(&mut b, 0x8004, 0x18000);
    put(&mut b, 0x8008, 16);
    b.mem[0x8066..0x8068].copy_from_slice(&128u16.to_le_bytes());
    b.mem[0xa080] = 255;
    let mut c = Cpu {
        cr0: 0x60010011 | if pg { 0x80000000 } else { 0 },
        cr3: 0x1000,
        s: [16, 8, 16, 16],
        ip: 0x10000,
        flags: 2,
        gdtr_base: 0x5000,
        gdtr_limit: 31,
        idtr_base: 0x6000,
        idtr_limit: 0x7ff,
        tr: 24,
        tr_cache: Some(SegmentCache {
            base: 0x8000,
            limit: 0x2080,
            access: 0x8b,
            attributes: 0,
        }),
        ..Cpu::default()
    };
    for s in 0..6 {
        c.segment_caches[s] = Some(SegmentCache {
            base: 0,
            limit: u32::MAX,
            access: if s == CS { 0x9b } else { 0x93 },
            attributes: 0xc0,
        });
    }
    c.set_reg32(SP, 0x18000);
    if pg {
        put(&mut b, 0x1000, 0x2007);
        for i in 0..1024 {
            put(&mut b, 0x2000 + i * 4, (i as u32 * 4096) | 7);
        }
        for (page, physical) in [(0x20, 0x300000), (0x50, 0x310000), (0x37, 0x320000)] {
            put(&mut b, 0x2000 + page * 4, physical | 7);
        }
    }
    let a = if pg { 0x300000 } else { 0x20000 };
    b.mem[a..a + code.len()].copy_from_slice(code);
    (c, b)
}
fn enter(c: &mut Cpu, b: &mut Bus) {
    c.step(b).unwrap();
    assert!(c.vm86());
    assert_eq!(c.cpl, 3);
    assert_eq!(c.cr0 & 1, 1);
}

fn request(int: u8, ah: u8, pg: bool) -> (Cpu, Bus) {
    let (mut c, mut b) = setup(&[], pg, 0x23002);
    enter(&mut c, &mut b);
    c.s[CS] = 0xf000;
    c.segment_caches[CS].as_mut().unwrap().base = 0xf0000;
    c.ip = 0x1000 + int as u32 * 4;
    c.set_byte(4, ah);
    b.pics[0].mask = 255;
    let stack = if pg { 0x320000 } else { 0x37000 };
    b.mem[stack..stack + 6].copy_from_slice(&[0, 0, 0, 0x20, 3, 0x36]);
    (c, b)
}
fn resume_after_fault(c: &mut Cpu, b: &mut Bus) {
    // Original monitor code: flush CR3 after host test repairs a PTE, discard
    // #PF error code, IRETD to retry precisely the original OUT instruction.
    b.mem[0x11000..0x1100c].copy_from_slice(&[
        0x50, 0x0f, 0x20, 0xd8, 0x0f, 0x22, 0xd8, 0x58, 0x83, 0xc4, 4, 0xcf,
    ]);
    for _ in 0..6 {
        c.step(b).unwrap();
    }
    assert!(c.vm86());
    c.step(b).unwrap();
}
#[test]
fn timer_permissions_restart_at_out_without_ticks_or_early_eoi() {
    for permission in [0, 3, 5, 7] {
        let (mut c, mut b) = request(8, 0, true);
        put(&mut b, 0x2000, 0x120000 | permission);
        b.mem[0x12058a..0x12058c].copy_from_slice(&41u16.to_le_bytes());
        b.mem[0x58a..0x58c].copy_from_slice(&99u16.to_le_bytes());
        b.pics[0].isr = 1;
        c.step(&mut b).unwrap();
        if permission != 7 {
            assert_eq!(c.cr2, 0x58a);
            assert_eq!(
                c.last_fault,
                Some((14, if permission == 0 { 6 } else { 7 }))
            );
            assert_eq!(get(&b, 0x17fdc), 0x1020); // saved restart EIP
            assert_eq!(b.read16(0x12058a), 41);
            assert_eq!(b.pics[0].isr, 1);
            assert_eq!(get(&b, 0x2000) & 0x40, 0);
            put(&mut b, 0x2000, 0x120007);
            resume_after_fault(&mut c, &mut b);
        }
        assert!(c.vm86());
        assert_eq!(c.ip, 0x1022);
        assert_eq!(c.cpl, 3);
        assert_eq!(b.read16(0x12058a), 42);
        assert_eq!(b.read16(0x58a), 99);
        assert_eq!(b.pics[0].isr, 0);
    }
}
#[test]
fn keyboard_fault_does_not_consume_scan_or_mutate_ring_and_retry_consumes_once() {
    let (mut c, mut b) = request(9, 0, true);
    put(&mut b, 0x2000, 0x120005);
    b.key(0x1d, true);
    b.pics[0].isr = 2;
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((14, 7)));
    assert_eq!(c.cr2, 0x502);
    assert_eq!(b.keyboard.fifo.len(), 1);
    assert!(b.keyboard.bios.is_empty());
    assert_eq!(b.pics[0].isr, 2);
    put(&mut b, 0x2000, 0x120007);
    resume_after_fault(&mut c, &mut b);
    assert!(b.keyboard.fifo.is_empty());
    assert_eq!(b.keyboard.bios.front(), Some(&0x1d61));
    assert_eq!(b.read16(0x120502), 0x1d61);
    assert_eq!(b.read(0x12052d), 0x20);
    assert_eq!(b.read(0x52d), 0);
    assert_eq!(b.pics[0].isr, 0);
}
#[test]
fn saved_carry_uses_translated_stack_and_denial_precedes_disk_reset() {
    let (mut c, mut b) = request(0x1b, 0, true);
    c.set_byte(0, 0x90);
    let image = std::fs::read("fixtures/vm86_bios/vm86_bios.hdm").unwrap();
    b.disks[0] = Some(pc98_rust::disk::Disk::parse(image, "bios.hdm").unwrap());
    b.fdc.cylinder[0] = 9;
    put(&mut b, 0x2000 + 0x37 * 4, 0x320005);
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((14, 7)));
    assert_eq!(c.cr2, 0x37004);
    assert_eq!(b.fdc.cylinder[0], 9);
    assert_eq!(b.read16(0x320004), 0x3603);
    put(&mut b, 0x2000 + 0x37 * 4, 0x320007);
    resume_after_fault(&mut c, &mut b);
    assert_eq!(b.fdc.cylinder[0], 0);
    assert_eq!(b.read16(0x320004), 0x3602);
    c.step(&mut b).unwrap(); // actual VM86 IRET
    assert_eq!(c.flags, 0x3602);
    assert_eq!(c.flags_upper & 2, 2);
    assert_eq!(c.reg32(SP), 0xabcd7006);
}

#[test]
fn floppy_scatter_dma_preflights_the_entire_request_before_controller_activity() {
    let (mut c, mut b) = request(0x1b, 0x56, true);
    let image = std::fs::read("fixtures/vm86_bios/vm86_bios.hdm").unwrap();
    b.disks[0] = Some(pc98_rust::disk::Disk::parse(image.clone(), "bios.hdm").unwrap());
    c.set_byte(0, 0x90);
    c.r[CX] = 0x0300;
    c.r[DX] = 1;
    c.r[BX] = 2048;
    c.r[BP] = 0xff0;
    put(&mut b, 0x2100, 0x130007); // ES=4000, first 16 bytes
    put(&mut b, 0x2104, 0x150005); // remaining bytes: denied write
    b.mem[0x130ff0..0x131000].fill(0xa5);
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((14, 7)));
    assert_eq!(c.cr2, 0x41000);
    assert_eq!(&b.mem[0x130ff0..0x131000], &[0xa5; 16]);
    assert_eq!(b.fdc.reads, 0);
    assert_eq!(b.fdc.writes, 0);
    put(&mut b, 0x2104, 0x150007);
    resume_after_fault(&mut c, &mut b);
    assert_eq!(c.byte(4), 0);
    assert_eq!(b.read16(0x320004) & CF, 0);
    assert_eq!(b.fdc.reads, 2);
    assert_eq!(&b.mem[0x130ff0..0x131000], &image[..16]);
    assert_eq!(&b.mem[0x150000..0x1507f0], &image[16..2048]);
    // A write reads the same scatter buffer with user read permissions.
    b.mem[0x130ff0..0x131000].fill(0x5a);
    b.mem[0x150000..0x1507f0].fill(0x69);
    c.ip = 0x106c;
    c.set_byte(4, 0x55);
    c.step(&mut b).unwrap();
    assert_eq!(c.byte(4), 0);
    let disk = b.disks[0].as_ref().unwrap();
    assert_eq!(&disk.bytes[..16], &[0x5a; 16]);
    assert_eq!(&disk.bytes[16..2048], &[0x69; 2032]);
    assert_eq!(b.fdc.writes, 2);
}

#[test]
fn ide_pio_translates_noncontiguous_buffers_for_reads_and_writes() {
    let (mut c, mut b) = request(0x1b, 6, true);
    let mut image = vec![0; 32 + 1024];
    for (offset, value) in [(8, 32u32), (12, 1024), (16, 512), (20, 2), (24, 1), (28, 1)] {
        image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (i, byte) in image[32..].iter_mut().enumerate() {
        *byte = (i % 251) as u8;
    }
    b.ide.mount(Some(
        pc98_rust::hdd::HardDisk::parse(image.clone()).unwrap(),
    ));
    c.set_byte(0, 0);
    c.r[CX] = 0;
    c.r[DX] = 0;
    c.r[BX] = 1024;
    c.r[BP] = 0xff0;
    put(&mut b, 0x2100, 0x130007);
    put(&mut b, 0x2104, 0x150003); // supervisor-only second page
    c.step(&mut b).unwrap();
    assert_eq!(c.cr2, 0x41000);
    assert_eq!(c.last_fault, Some((14, 7)));
    assert_eq!(b.ide.reads, 0);
    assert_eq!(b.ide.writes, 0);
    put(&mut b, 0x2104, 0x150007);
    resume_after_fault(&mut c, &mut b);
    assert_eq!(c.byte(4), 0);
    assert_eq!(&b.mem[0x130ff0..0x131000], &image[32..48]);
    assert_eq!(&b.mem[0x150000..0x1503f0], &image[48..]);
    b.mem[0x130ff0..0x131000].fill(0x5a);
    b.mem[0x150000..0x1503f0].fill(0x69);
    c.ip = 0x106c;
    c.set_byte(4, 5);
    c.step(&mut b).unwrap();
    assert_eq!(c.byte(4), 0);
    let disk = b.ide.disk.as_ref().unwrap();
    assert_eq!(&disk.bytes[32..48], &[0x5a; 16]);
    assert_eq!(&disk.bytes[48..], &[0x69; 1008]);
}

#[test]
fn rtc_wraps_segment_offsets_and_font_registration_preflights_before_definition() {
    let (mut c, mut b) = request(0x1c, 0, true);
    c.r[BX] = 0xfffe;
    put(&mut b, 0x2100, 0x130007);
    put(&mut b, 0x213c, 0x150007);
    c.step(&mut b).unwrap();
    let clock = b.rtc.snapshot();
    assert_eq!(&b.mem[0x150ffe..0x151000], &clock[..2]);
    assert_eq!(&b.mem[0x130000..0x130004], &clock[2..]);
    // BX is a raw segment for JIS registration, not a protected selector.
    c.ip = 0x1060;
    c.set_byte(4, 0x1a);
    c.r[BX] = 0x5000;
    c.r[CX] = 0xff0;
    c.r[DX] = 0x7621;
    put(&mut b, 0x2140, 0x130007);
    put(&mut b, 0x2144, 0x150003);
    b.mem[0x130ff2..0x131000].fill(0x5a);
    b.mem[0x150000..0x150012].fill(0x69);
    let original = b.user_characters.byte(0x76, 0x21, 0);
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((14, 5)));
    assert_eq!(c.cr2, 0x51000);
    assert_eq!(b.user_characters.byte(0x76, 0x21, 0), original);
    put(&mut b, 0x2144, 0x150007);
    resume_after_fault(&mut c, &mut b);
    assert_eq!(b.user_characters.byte(0x76, 0x21, 0), 0x5a);
    assert_eq!(b.user_characters.byte(0x76, 0x21, 31), 0x69);
}

#[test]
fn firmware_trap_io_permission_and_unsupported_modes_remain_explicit() {
    let (mut c, mut b) = request(8, 0, true);
    b.mem[0x8080 + 0xf0 / 8] = 1;
    let ticks = b.read16(0x58a);
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((13, 0)));
    assert_eq!(b.read16(0x58a), ticks);
    for (segment, int, expected) in [(0xf000, 0x1c, "INT 1C"), (0xf990, 0, "Graph LIO")] {
        let (mut c, mut b) = request(int, 0xff, true);
        c.s[CS] = segment;
        c.segment_caches[CS].as_mut().unwrap().base = (segment as u32) << 4;
        if segment == 0xf990 {
            c.ip = 0x104;
        }
        let error = c.step(&mut b).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu.cr0 |= 1;
    assert!(pc98_rust::firmware::service(&mut m.cpu, &mut m.bus)
        .unwrap_err()
        .contains("protected mode"));
}

#[test]
fn denied_font_source_preserves_mmio_read_latches() {
    let (mut c, mut b) = request(0x18, 0x1a, true);
    c.r[BX] = 0x5000;
    c.r[CX] = 0xff0;
    c.r[DX] = 0x7621;
    put(&mut b, 0x2140, 0xa8007);
    put(&mut b, 0x2144, 0);
    b.egc.enabled = true;
    b.grcg_mode = 0x80;
    let egc = b.egc.clone();
    let graphics = b.graphics.clone();
    c.step(&mut b).unwrap();
    assert_eq!(c.cr2, 0x51000);
    assert_eq!(c.last_fault, Some((14, 4)));
    assert_eq!(b.egc, egc);
    assert_eq!(b.graphics, graphics);
}

#[test]
fn saved_flags_word_respects_vm86_stack_limit_before_disk_side_effects() {
    let (mut c, mut b) = request(0x1b, 0, true);
    c.r[SP] = 0xfffb; // FLAGS word would cross SS:FFFF
    c.set_byte(0, 0x90);
    b.fdc.cylinder[0] = 9;
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((12, 0)));
    assert_eq!(b.fdc.cylinder[0], 9);
}

#[test]
fn expired_callback_survives_fault_and_preserves_vm86_cpl_and_iret() {
    let (mut c, mut b) = request(0x1c, 2, true);
    c.r[CX] = 1;
    c.r[BX] = 0x200;
    c.step(&mut b).unwrap(); // arm ES:0200
    b.tick(80_000);
    c.ip = 0x1020;
    put(&mut b, 0x2000, 0x120005);
    c.paging = Paging::default(); // test setup: force a fresh walk
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((14, 7)));
    put(&mut b, 0x2000, 0x120007);
    resume_after_fault(&mut c, &mut b);
    assert!(c.vm86());
    assert_eq!(c.cpl, 3);
    assert_eq!((c.s[CS], c.ip), (0x4000, 0x200));
    assert_eq!(c.segment_cache(CS).base, 0x40000);
    b.mem[0x40200] = 0xcf;
    c.step(&mut b).unwrap();
    assert!(c.vm86());
    assert_eq!(c.cpl, 3);
    assert_eq!((c.s[CS], c.ip), (0x2000, 0));
    assert_eq!(c.reg32(SP), 0xabcd7006);
}

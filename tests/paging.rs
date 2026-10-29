// Original byte-level paging and exception probes. SPDX-License-Identifier: MIT
use pc98_rust::{bus::Bus, cpu::*, Machine, Profile};

fn put32(b: &mut Bus, a: usize, value: u32) {
    b.mem[a..a + 4].copy_from_slice(&value.to_le_bytes());
}
fn get32(b: &Bus, a: usize) -> u32 {
    u32::from_le_bytes(b.mem[a..a + 4].try_into().unwrap())
}
fn gate(b: &mut Bus, base: usize, vector: usize, selector: u16) {
    b.mem[base + vector * 8..base + vector * 8 + 8].copy_from_slice(&[
        0,
        0x10,
        selector as u8,
        (selector >> 8) as u8,
        0,
        0x8e,
        0x40,
        0,
    ]);
}
fn setup(code: &[u8]) -> (Cpu, Bus) {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    b.a20 = true;
    for i in 0..1024 {
        put32(&mut b, 0x2000 + i * 4, ((i as u32) * 4096) | 3);
    }
    for (a, v) in [
        (0x1000, 0x2003),
        (0x1004, 0x4003),
        (0x1008, 0x5003),
        (0x100c, 0x6003),
        (0x4000, 0x30003),
        (0x4004, 0x31003),
        (0x5000, 0x50003),
        (0x5004, 0x60003),
        (0x6000, 0x70003),
    ] {
        put32(&mut b, a, v);
    }
    let mut c = Cpu {
        cr0: 0xe0010011,
        cr3: 0x1000,
        s: [16, 8, 16, 16],
        ip: 0x400000,
        flags: 2,
        gdtr_base: 0x10000,
        gdtr_limit: 0x1fff,
        idtr_base: 0x13000,
        idtr_limit: 0x7ff,
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
    for (selector, access) in [(8, 0x9b), (16, 0x93), (24, 0x9a)] {
        b.mem[0x10000 + selector..0x10008 + selector]
            .copy_from_slice(&[255, 255, 0, 0, 0, access, 0xcf, 0]);
    }
    for n in [6, 8, 11, 12, 13, 14] {
        gate(&mut b, 0x13000, n, 8);
    }
    b.mem[0x30000..0x30000 + code.len()].copy_from_slice(code);
    b.mem[0x31000] = 0xf4;
    b.mem[0x70000..0x71000].fill(0xa5);
    c.set_reg32(SP, 0xc00ff0);
    (c, b)
}
fn frame(c: &Cpu, b: &Bus, vector: u8, code: u32, ip: u32) {
    assert_eq!(c.last_fault, Some((vector, code)));
    assert_eq!(c.ip, 0x401000);
    let sp = 0x70000 + (c.reg32(SP) as usize & 4095);
    let error_bytes = if vector == 6 { 0 } else { 4 };
    if error_bytes != 0 {
        assert_eq!(get32(b, sp), code);
    }
    assert_eq!(get32(b, sp + error_bytes), ip);
    assert_eq!(get32(b, sp + error_bytes + 4), 8);
    assert_eq!(get32(b, sp + error_bytes + 8), 0x10002);
}

#[test]
fn original_boot_guest_nonidentity_code_data_stack_fault_retry_and_pg_cycle_cold_reset() {
    let image =
        std::fs::read("fixtures/paging/paging.hdm").expect("run tools/prepare_paging_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "paging.hdm").unwrap();
    let mut expected = [0xa5; 64];
    expected[1] = 1;
    for (off, value) in [
        (4, 0xe0010011u32),
        (8, 0x10018),
        (12, 0x403000),
        (16, 0x11223344),
        (20, 0xcafebabe),
        (24, 0x60010011),
    ] {
        expected[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }
    for run in 0..2 {
        if run != 0 {
            m.reset();
        }
        let mut paged32 = false;
        let mut paged16 = false;
        m.step_with_observer(100_000, |c, _| {
            if c.paging_enabled() && c.segment_cache(CS).base.wrapping_add(c.ip) >= 0x400000 {
                if c.segment_cache(CS).big() {
                    paged32 = true;
                } else {
                    paged16 = true;
                }
            }
        });
        assert!(m.error.is_none(), "{:?}", m.error);
        assert!(m.bus.text().contains("PAGING PASS"), "{}", m.bus.text());
        assert!(!m.bus.text().contains("FAIL"));
        assert!(paged16 && paged32);
        assert_eq!(&m.bus.mem[0x8000..0x8040], &expected);
        for address in [0x102000, 0x102ffc] {
            assert_eq!(get32(&m.bus, address), 0xa5a5a5a5);
        }
        assert_eq!(
            (m.cpu.cr0, m.cpu.cr2, m.cpu.cr3),
            (0x60010011, 0x403000, 0x10018)
        );
        assert_eq!(m.cpu.reg32(SP), 0x7c00);
        assert_eq!(m.cpu.last_fault, Some((14, 2)));
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
    }
}

#[test]
fn guest_handler_repairs_mapping_and_iretd_retries_first_prefix_once() {
    let (mut c, mut b) = setup(&[0x67, 0x66, 0xa1, 0, 0]); // 16-bit moffs word, DS.base supplies high linear address
    c.segment_caches[DS].as_mut().unwrap().base = 0x800000;
    put32(&mut b, 0x5000, 0);
    c.set_reg32(AX, 0x1234ffff);
    c.step(&mut b).unwrap();
    frame(&c, &b, 14, 0, 0x400000);
    assert_eq!(c.cr2, 0x800000);
    assert_eq!(c.reg32(AX), 0x1234ffff);
    // Absolute memory uses flat ES override; handler updates the *physical*
    // table through an identity-mapped page and returns to the first prefix.
    let handler = [
        0x26, 0xc7, 0x05, 0x00, 0x50, 0, 0, 0x03, 0, 0x05, 0, 0x83, 0xc4, 4, 0xcf,
    ];
    b.mem[0x31000..0x31000 + handler.len()].copy_from_slice(&handler);
    b.mem[0x50000..0x50002].copy_from_slice(&[0x78, 0x56]);
    for _ in 0..3 {
        c.step(&mut b).unwrap();
    }
    assert_eq!((c.ip, c.reg32(SP)), (0x400000, 0xc00ff0));
    c.step(&mut b).unwrap();
    assert_eq!((c.reg32(AX), c.ip), (0x12345678, 0x400005));
    assert_eq!(get32(&b, 0x5000) & 0x60, 0x20);
}

#[test]
fn cross_page_read_write_and_rmw_faults_preflight_all_bytes_and_latches() {
    for code in [
        vec![0xa1, 0xfe, 0x0f, 0x80, 0],
        vec![0xa3, 0xfe, 0x0f, 0x80, 0],
        vec![0x83, 0x05, 0xfe, 0x0f, 0x80, 0, 1],
    ] {
        let write = code[0] != 0xa1;
        let (mut c, mut b) = setup(&code);
        c.set_reg32(AX, 0x12345678);
        b.mem[0x50ffe..0x51000].copy_from_slice(&[0x12, 0x34]);
        b.mem[0x60000..0x60002].copy_from_slice(&[0x56, 0x78]);
        c.step(&mut b).unwrap();
        assert_eq!(get32(&b, 0x5000) & 0x60, if write { 0x60 } else { 0x20 });
        if !write {
            assert_eq!(c.reg32(AX), 0x78563412);
        } else if code[0] == 0xa3 {
            assert_eq!(&b.mem[0x50ffe..0x51000], &[0x78, 0x56]);
            assert_eq!(&b.mem[0x60000..0x60002], &[0x34, 0x12]);
        }
        let (mut c, mut b) = setup(&code);
        c.set_reg32(AX, 0x12345678);
        put32(&mut b, 0x5000, 0xa8003);
        put32(&mut b, 0x5004, 0);
        b.egc.enabled = true;
        b.grcg_mode = 0x80;
        let egc = b.egc.clone();
        let graphics = b.graphics.clone();
        c.step(&mut b).unwrap();
        frame(&c, &b, 14, if write { 2 } else { 0 }, 0x400000);
        assert_eq!(c.cr2, 0x801000);
        assert_eq!(c.reg32(AX), 0x12345678);
        assert_eq!(b.egc, egc);
        assert_eq!(b.graphics, graphics);
        assert_eq!(get32(&b, 0x5000) & 0x40, 0); // no partial dirty on rejected span
    }
}

#[test]
fn wp_permission_fault_retry_invlpg_and_no_read_side_effects() {
    for wp in [false, true] {
        let (mut c, mut b) = setup(&[0x83, 0x05, 0, 0, 0x80, 0, 1]);
        if !wp {
            c.cr0 &= !0x10000;
        }
        put32(&mut b, 0x5000, 0xa8001);
        b.egc.enabled = true;
        b.grcg_mode = 0x80;
        let egc = b.egc.clone();
        let graphics = b.graphics.clone();
        c.step(&mut b).unwrap();
        if wp {
            frame(&c, &b, 14, 3, 0x400000);
            assert_eq!(c.cr2, 0x800000);
            assert_eq!(b.egc, egc);
            assert_eq!(b.graphics, graphics);
            assert_eq!(get32(&b, 0x5000) & 0x40, 0);
        } else {
            assert_eq!(c.ip, 0x400007);
            assert_ne!(get32(&b, 0x5000) & 0x40, 0);
        }
    }
}

#[test]
fn instruction_fetch_faults_restart_at_original_opcode_even_mid_immediate() {
    for prefix_bytes in [0, 1, 2, 3, 4] {
        let (mut c, mut b) = setup(&[]);
        let start = 0x401000 - prefix_bytes;
        c.ip = start;
        let instruction = [0x66, 0xb8, 0x34, 0x12];
        for (i, byte) in instruction.iter().enumerate() {
            b.mem[0x30000 + (start as usize & 4095) + i] = *byte;
        }
        // Handler code uses 401000, so a successful event transfer does not
        // require the handler instruction's page until its next fetch.
        put32(&mut b, 0x4004, 0);
        c.step(&mut b).unwrap();
        if prefix_bytes == 4 {
            assert_eq!(c.ip, 0x401000);
        } else {
            frame(&c, &b, 14, 0, start);
            assert_eq!(c.cr2, 0x401000);
        }
    }
}

#[test]
fn descriptor_load_busy_update_and_faults_use_supervisor_translations() {
    let (mut c, mut b) = setup(&[0x8e, 0xd8]); // MOV DS,AX, remote GDT descriptor page
    c.r[AX] = 0x1008;
    put32(&mut b, 0x2000 + 0x11 * 4, 0);
    c.step(&mut b).unwrap();
    frame(&c, &b, 14, 0, 0x400000);
    assert_eq!(c.cr2, 0x11008);
    assert_eq!(c.s[DS], 16);
    let (mut c, mut b) = setup(&[0x0f, 0x00, 0xd8]); // LTR AX
    c.r[AX] = 0x20;
    // TSS base is a linear high address; LTR does not read TSS storage.
    b.mem[0x10020..0x10028].copy_from_slice(&[103, 0, 0, 0, 0x80, 0x89, 0, 0]);
    c.step(&mut b).unwrap();
    assert_eq!(c.tr, 0x20);
    assert_eq!(c.tr_cache.unwrap().base, 0x800000);
    assert_eq!(b.mem[0x10025], 0x8b);
    let (mut c, mut b) = setup(&[0x0f, 0x00, 0xd8]);
    c.r[AX] = 0x20;
    b.mem[0x10020..0x10028].copy_from_slice(&[103, 0, 0, 0, 0x80, 0x89, 0, 0]);
    put32(&mut b, 0x2000 + 0x10 * 4, 0x10001);
    c.step(&mut b).unwrap();
    frame(&c, &b, 14, 3, 0x400000);
    assert_eq!(c.cr2, 0x10025); // exact busy access-byte write, after descriptor validation
    assert_eq!(c.tr, 0);
    assert_eq!(b.mem[0x10025], 0x89);
}

#[test]
fn late_descriptor_write_fault_rolls_back_stack_graphics_and_egc_latches() {
    let mut results = Vec::new();
    // Both operations fault on the same GDT access byte. Far CALL has already
    // pushed its return address when the accessed-bit write faults. Its final
    // graphics/frame/latches must equal the direct-write fault's state.
    for code in [
        vec![0x9a, 0, 0, 0x40, 0, 24, 0],
        vec![0xc6, 0x05, 0x1d, 0, 1, 0, 0x9b],
    ] {
        let (mut c, mut b) = setup(&code);
        put32(&mut b, 0x2000 + 0x10 * 4, 0x10001);
        put32(&mut b, 0x6000, 0xa8003);
        b.egc.enabled = true;
        b.grcg_mode = 0x80;
        b.egc.registers[0] = 0xfff0;
        b.egc.registers[2] = 0x1000;
        c.step(&mut b).unwrap();
        assert_eq!(c.last_fault, Some((14, 3)));
        assert_eq!(c.cr2, 0x1001d);
        assert_eq!(c.reg32(SP), 0xc00fe0);
        assert_eq!(c.s[CS], 8);
        results.push((b.graphics, b.egc));
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn idt_stack_and_nested_faults_follow_486_double_fault_matrix_and_are_bounded() {
    // Contributory #GP followed by #PF is delivered serially, unlike PF->GP.
    let (mut c, mut b) = setup(&[0x8e, 0xd8]);
    c.r[AX] = 0xffff;
    c.idtr_base = 0x11f97;
    gate(&mut b, 0x11f97, 14, 8);
    put32(&mut b, 0x2000 + 0x11 * 4, 0);
    c.step(&mut b).unwrap();
    frame(&c, &b, 14, 0, 0x400000);
    assert_eq!(c.cr2, 0x11fff);
    // A software INT entry crosses onto an absent page, but #PF's gate remains
    // accessible. Save the original INT instruction, not its consumed next IP.
    let (mut c, mut b) = setup(&[0xcd, 0x30]);
    c.idtr_base = 0x11f00;
    gate(&mut b, 0x11f00, 14, 8);
    put32(&mut b, 0x2000 + 0x12 * 4, 0);
    c.step(&mut b).unwrap();
    frame(&c, &b, 14, 0, 0x400000);
    assert_eq!(c.cr2, 0x12080);
    // #PF followed by #PF while accessing its gate becomes #DF(0).
    let (mut c, mut b) = setup(&[0xa1, 0, 0, 0x80, 0]);
    c.idtr_base = 0x11fa0;
    gate(&mut b, 0x11fa0, 8, 8);
    put32(&mut b, 0x2000 + 0x12 * 4, 0);
    put32(&mut b, 0x5000, 0);
    c.step(&mut b).unwrap();
    frame(&c, &b, 8, 0, 0x400000);
    assert_eq!(c.cr2, 0x12010);
    // #PF followed by a contributory #GP also becomes #DF, not another #GP.
    let (mut c, mut b) = setup(&[0xa1, 0, 0, 0x80, 0]);
    put32(&mut b, 0x5000, 0);
    gate(&mut b, 0x13000, 14, 0);
    c.step(&mut b).unwrap();
    frame(&c, &b, 8, 0, 0x400000);
    // Faulting exception stack cannot deliver #PF or #DF. No partial frame.
    let (mut c, mut b) = setup(&[0xa1, 0, 0, 0x80, 0]);
    put32(&mut b, 0x5000, 0);
    put32(&mut b, 0x6000, 0);
    let stack = b.mem[0x70000..0x71000].to_vec();
    assert!(c
        .step(&mut b)
        .unwrap_err()
        .contains("triple fault/shutdown"));
    assert!(c.shutdown);
    assert_eq!(c.cr2, 0xc00fec);
    assert_eq!(&b.mem[0x70000..0x71000], stack);
    let state = format!("{c:?}");
    c.step(&mut b).unwrap();
    assert_eq!(format!("{c:?}"), state);
}

#[test]
fn invlpg_encoding_privilege_nonpresent_address_and_no_operand_access() {
    for (code, vector) in [
        (vec![0x0f, 0x01, 0xf8], 6),
        (vec![0xf0, 0x0f, 0x01, 0x3d, 0, 0, 0x80, 0], 6),
    ] {
        let (mut c, mut b) = setup(&code);
        c.step(&mut b).unwrap();
        frame(&c, &b, vector, 0, 0x400000);
    }
    let (mut c, mut b) = setup(&[0x0f, 0x01, 0x3d, 0, 0, 0x80, 0]);
    put32(&mut b, 0x5000, 0);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x400007);
    assert_eq!(c.cr2, 0);
    assert_eq!(get32(&b, 0x5000), 0);
    for cpl in 1..=3 {
        let (mut c, mut b) = setup(&[0x0f, 0x01, 0x3d, 0, 0, 0x80, 0]);
        c.cpl = cpl;
        c.s[CS] |= cpl as u16;
        c.segment_caches[CS].as_mut().unwrap().access |= cpl << 5;
        b.mem[0x1000d] |= cpl << 5;
        // User code/stack fixtures only; this does not enable general CPL3 execution.
        for address in [0x1004, 0x4000, 0x100c, 0x6000] {
            let value = get32(&b, address) | 4;
            put32(&mut b, address, value);
        }
        c.step(&mut b).unwrap();
        assert_eq!(c.last_fault, Some((13, 0)));
        assert_eq!(c.cr2, 0);
    }
}

#[test]
fn cr3_different_directory_same_value_reload_and_pg_toggle_flush_stale_pages() {
    // Execute control instructions at an identity address so disabling PG keeps
    // the instruction stream valid; the data page remains nonidentity.
    let code = [
        0xa1, 0, 0, 0x80, 0, 0xa1, 0, 0, 0x80, 0, 0x0f, 0x22, 0xd8, 0xa1, 0, 0, 0x80, 0, 0x0f,
        0x22, 0xc0, 0x0f, 0x22, 0xc0, 0xa1, 0, 0, 0x80, 0,
    ];
    let (mut c, mut b) = setup(&[]);
    c.ip = 0x32000;
    b.mem[0x32000..0x32000 + code.len()].copy_from_slice(&code);
    put32(&mut b, 0x50000, 0x11112222);
    put32(&mut b, 0x60000, 0x33334444);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x11112222);
    put32(&mut b, 0x5000, 0x60003);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x11112222);
    // A new directory selects a different physical page table.
    b.mem.copy_within(0x1000..0x2000, 0x8000);
    put32(&mut b, 0x8008, 0x9003);
    put32(&mut b, 0x9000, 0x60003);
    let caches = c.segment_caches;
    c.set_reg32(AX, 0x8018);
    c.step(&mut b).unwrap();
    assert_eq!(c.cr3, 0x8018);
    assert_eq!(c.segment_caches, caches);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x33334444);
    c.set_reg32(AX, 0x60010011);
    c.step(&mut b).unwrap();
    assert!(!c.paging_enabled());
    assert_eq!(c.segment_caches, caches);
    put32(&mut b, 0x9000, 0x50003);
    c.set_reg32(AX, 0xe0010011);
    c.step(&mut b).unwrap();
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x11112222);
}

#[test]
fn paged_16bit_stack_offsets_cross_frames_independently_of_operand_size() {
    let (mut c, mut b) = setup(&[0x50, 0x5b]); // 32-bit PUSH EAX / POP EBX with SS.B=0
    c.segment_caches[SS] = Some(SegmentCache {
        base: 0xc00000,
        limit: 0xffff,
        access: 0x93,
        attributes: 0,
    });
    put32(&mut b, 0x6004, 0x71003);
    c.set_reg32(SP, 0xabcd1002);
    c.set_reg32(AX, 0x12345678);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(SP), 0xabcd0ffe);
    assert_eq!(&b.mem[0x70ffe..0x71000], &[0x78, 0x56]);
    assert_eq!(&b.mem[0x71000..0x71002], &[0x34, 0x12]);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(SP), 0xabcd1002);
    assert_eq!(c.reg32(BX), 0x12345678);
}

#[test]
fn nonidentity_gdt_idt_ldt_descriptor_reads_and_updates_are_linear_supervisor_accesses() {
    // MOV DS,AX; INT 30. Virtual GDT/IDT pages have different physical bases.
    let (mut c, mut b) = setup(&[0x8e, 0xd8, 0xcd, 0x30]);
    put32(&mut b, 0x5400, 0x10003);
    put32(&mut b, 0x5404, 0x13003);
    c.gdtr_base = 0x900000;
    c.idtr_base = 0x901000;
    c.r[AX] = 16;
    b.mem[0x10015] = 0x92;
    gate(&mut b, 0x13000, 0x30, 8);
    c.step(&mut b).unwrap();
    assert_eq!(b.mem[0x10015], 0x93);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x401000);
    assert_eq!(get32(&b, 0x70fe4), 0x400004);
    let (mut c, mut b) = setup(&[0x8e, 0xd8]);
    put32(&mut b, 0x5408, 0x20003);
    c.ldtr = 0x20;
    c.ldtr_cache = Some(SegmentCache {
        base: 0x902000,
        limit: 15,
        access: 0x82,
        attributes: 0,
    });
    b.mem[0x20008..0x20010].copy_from_slice(&[255, 255, 0, 0, 0x80, 0x92, 0xcf, 0]);
    c.r[AX] = 12;
    c.step(&mut b).unwrap();
    assert_eq!(c.s[DS], 12);
    assert_eq!(c.segment_caches[DS].unwrap().base, 0x800000);
    assert_eq!(b.mem[0x2000d], 0x93);
}

#[test]
fn wp_changes_are_checked_on_tlb_hits_and_permission_repairs_require_invalidation() {
    let code = [0xa1, 0, 0x20, 0x80, 0, 0xa3, 0, 0x20, 0x80, 0];
    let (mut c, mut b) = setup(&code);
    c.cr0 &= !0x10000;
    put32(&mut b, 0x5008, 0x50001);
    c.step(&mut b).unwrap(); // cache read-only mapping under WP=0
    c.cr0 |= 0x10000;
    c.step(&mut b).unwrap();
    frame(&c, &b, 14, 3, 0x400005);
    assert_eq!(c.cr2, 0x802000);
    // Repair RW in guest memory but omit invalidation: cached protection wins.
    put32(&mut b, 0x5008, 0x50003);
    b.mem[0x31000..0x31004].copy_from_slice(&[0x83, 0xc4, 4, 0xcf]);
    for _ in 0..2 {
        c.step(&mut b).unwrap();
    }
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((14, 3)));
    // Guest now invalidates the page, then returns to retry the same store.
    let handler = [0x0f, 0x01, 0x3d, 0, 0x20, 0x80, 0, 0x83, 0xc4, 4, 0xcf];
    b.mem[0x31000..0x31000 + handler.len()].copy_from_slice(&handler);
    for _ in 0..4 {
        c.step(&mut b).unwrap();
    }
    assert_eq!(c.ip, 0x40000a);
    assert_eq!(get32(&b, 0x5008) & 0x60, 0x60);
}

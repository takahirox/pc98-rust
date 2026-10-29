// Wholly original machine-code/descriptor probes. SPDX-License-Identifier: MIT
use pc98_rust::{bus::Bus, cpu::*, Machine, Profile};

fn raw(base: u32, limit: u32, access: u8, attributes: u8) -> [u8; 8] {
    [
        limit as u8,
        (limit >> 8) as u8,
        base as u8,
        (base >> 8) as u8,
        (base >> 16) as u8,
        access,
        ((limit >> 16) as u8 & 15) | attributes,
        (base >> 24) as u8,
    ]
}
fn descriptor(b: &mut Bus, selector: usize, base: u32, limit: u32, access: u8, attr: u8) {
    b.mem[0x5000 + selector..0x5008 + selector].copy_from_slice(&raw(base, limit, access, attr));
}
fn gate(b: &mut Bus, vector: usize, selector: u16, offset: u32, access: u8) {
    b.mem[0x6000 + vector * 8..0x6008 + vector * 8].copy_from_slice(&[
        offset as u8,
        (offset >> 8) as u8,
        selector as u8,
        (selector >> 8) as u8,
        0,
        access,
        (offset >> 16) as u8,
        (offset >> 24) as u8,
    ]);
}
fn protected(code: &[u8], big: bool, stack_big: bool) -> (Cpu, Bus) {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    descriptor(
        &mut b,
        8,
        0x10000,
        0x3ffff,
        0x9b,
        if big { 0x40 } else { 0 },
    );
    descriptor(
        &mut b,
        16,
        0,
        0x3ffff,
        0x93,
        if stack_big { 0x40 } else { 0 },
    );
    descriptor(&mut b, 24, 0x20000, 0x2ffff, 0x93, 0x40);
    b.mem[0x11000..0x11000 + code.len()].copy_from_slice(code);
    let mut c = Cpu {
        cr0: 0x60000011,
        s: [24, 8, 16, 24],
        extra_segments: [24; 2],
        ip: 0x1000,
        gdtr_base: 0x5000,
        gdtr_limit: 0x7f,
        idtr_base: 0x6000,
        idtr_limit: 0x7ff,
        flags: 2,
        ..Cpu::default()
    };
    for (s, selector) in [(CS, 8), (SS, 16), (DS, 24), (ES, 24), (4, 24), (5, 24)] {
        c.segment_caches[s] = Some(SegmentCache::parse(
            b.mem[0x5000 + selector..0x5008 + selector]
                .try_into()
                .unwrap(),
        ));
    }
    c.set_reg32(SP, 0x18000);
    for n in [6, 8, 11, 12, 13] {
        gate(&mut b, n, 8, 0x2000, 0x8e);
    }
    b.mem[0x12000] = 0xf4;
    (c, b)
}
fn fault_frame(c: &Cpu, b: &Bus, vector: u8, error: u32) {
    assert_eq!(c.last_fault, Some((vector, error)));
    assert_eq!((c.s[CS], c.ip), (8, 0x2000));
    let sp = if c.segment_cache(SS).big() {
        c.reg32(SP)
    } else {
        c.r[SP] as u32
    } as usize;
    assert_eq!(&b.mem[sp..sp + 4], &error.to_le_bytes());
    assert_eq!(&b.mem[sp + 4..sp + 8], &0x1000u32.to_le_bytes());
    assert_eq!(&b.mem[sp + 8..sp + 12], &8u32.to_le_bytes());
    assert_eq!(b.mem[sp + 14] & 1, 1); // RF saved for restartable fault
}

#[test]
fn descriptor_parsing_granularity_base_and_default_size() {
    let d = SegmentCache::parse(raw(0xfedcba98, 0xabcde, 0x97, 0xc0));
    assert_eq!(
        (d.base, d.limit, d.access, d.big()),
        (0xfedcba98, 0xabcdefff, 0x97, true)
    );
    let d = SegmentCache::parse(raw(0x12345678, 0x12345, 0x92, 0));
    assert_eq!((d.base, d.limit, d.big()), (0x12345678, 0x12345, false));
}

#[test]
fn entering_pe_keeps_real_segment_cache_until_far_jump_and_atomic_pg_failure() {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    let mut c = Cpu {
        s: [0x3000, 0x1000, 0x4000, 0x2000],
        ip: 0,
        flags: 2,
        ..Cpu::default()
    };
    c.set_reg32(AX, 0x60000011);
    b.mem[0x10000..0x1000a].copy_from_slice(&[0x0f, 0x22, 0xc0, 0xb0, 0x55, 0xea, 0, 1, 8, 0]);
    c.gdtr_base = 0x5000;
    c.gdtr_limit = 15;
    descriptor(&mut b, 8, 0x90000, 0xffff, 0x9a, 0);
    c.step(&mut b).unwrap();
    assert!(c.protected_mode());
    assert_eq!(c.segment_cache(CS).base, 0x10000);
    assert_eq!(c.segment_cache(DS).base, 0x20000);
    c.step(&mut b).unwrap();
    assert_eq!(c.byte(0), 0x55);
    c.step(&mut b).unwrap();
    assert_eq!(
        (c.s[CS], c.ip, c.segment_cache(CS).base),
        (8, 0x100, 0x90000)
    );
    assert_eq!(b.mem[0x500d], 0x9b); // accessed bit set only on successful load
    b.mem[0x90100..0x90103].copy_from_slice(&[0x0f, 0x22, 0xc0]);
    c.set_reg32(AX, 0xe0000011);
    let saved = format!("{c:?}");
    let memory = b.mem.clone();
    assert!(c
        .step(&mut b)
        .unwrap_err()
        .contains("unsupported paging transition"));
    assert_eq!(format!("{c:?}"), saved);
    assert_eq!(b.mem, memory);
}

#[test]
fn code_defaults_overrides_full_offsets_and_ss_b_are_independent() {
    for big in [false, true] {
        for stack_big in [false, true] {
            // Encode 32-bit immediate, 32-bit moffs, then a 16-bit push, irrespective of D.
            let mut code = vec![];
            if !big {
                code.push(0x66)
            }
            code.extend([0xb8, 0x78, 0x56, 0x34, 0x12]);
            if !big {
                code.extend([0x66, 0x67])
            }
            code.extend([0xa3, 0x01, 0x00, 0x01, 0x00]);
            if big {
                code.push(0x66)
            }
            code.extend([0x50]);
            let (mut c, mut b) = protected(&code, big, stack_big);
            for _ in 0..3 {
                c.step(&mut b).unwrap()
            }
            assert_eq!(c.reg32(AX), 0x12345678);
            assert_eq!(&b.mem[0x30001..0x30005], &[0x78, 0x56, 0x34, 0x12]);
            assert_eq!(c.reg32(SP), 0x17ffe);
            let p = if stack_big { 0x17ffe } else { 0x7ffe };
            assert_eq!(&b.mem[p..p + 2], &[0x78, 0x56]);
        }
    }
    let (mut c, mut b) = protected(
        &[0x66, 0x66, 0xb8, 0x34, 0x12, 0x67, 0x67, 0xa0, 0x01, 0x00],
        true,
        true,
    );
    b.mem[0x20001] = 0x77;
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x1234);
    c.step(&mut b).unwrap();
    assert_eq!(c.byte(0), 0x77);
}

#[test]
fn every_data_segment_uses_cache_and_validates_type_rpl_present_null_and_limits() {
    for seg in [ES, SS, DS, 4, 5] {
        for (access, rpl, vector, error, valid) in [
            (0x92, 0, 0, 0, true),
            (0xf2, 0, 0, 0, seg != SS),
            (0x92, 3, 13, 0x20, false),
            (0x12, 0, if seg == SS { 12 } else { 11 }, 0x20, false),
            (0x9a, 0, 13, 0x20, seg != SS),
            (0x98, 0, 13, 0x20, false),
            (0x90, 0, 13, 0x20, seg != SS),
            (0x82, 0, 13, 0x20, false),
        ] {
            let (mut c, mut b) = protected(&[0x8e, 0xc0 | ((seg as u8) << 3)], true, true);
            descriptor(&mut b, 32, 0x30000, 0x1fff, access, 0);
            c.r[AX] = 32 | rpl;
            let old = c.segment_cache(seg);
            let memory = b.mem.clone();
            c.step(&mut b).unwrap();
            if valid {
                assert_eq!(c.segment_cache(seg).base, 0x30000);
                assert_eq!(b.mem[0x5025], access | 1);
            } else {
                let expected_vector = if vector == 0 { 13 } else { vector };
                let expected_error = if error == 0 { 32 } else { error };
                fault_frame(&c, &b, expected_vector, expected_error);
                assert_eq!(c.segment_cache(seg), old);
                let mut expected = memory;
                expected[0x17ff0..0x18000].copy_from_slice(&b.mem[0x17ff0..0x18000]);
                assert_eq!(b.mem, expected);
            }
        }
    }
    for prefix in [0x26, 0x3e, 0x64, 0x65] {
        let (mut c, mut b) = protected(&[prefix, 0xa3, 0xff, 0xff, 2, 0], true, true);
        c.set_reg32(AX, 0xdeadbeef);
        let memory = b.mem.clone();
        c.step(&mut b).unwrap();
        fault_frame(&c, &b, 13, 0);
        assert_eq!(&b.mem[0x4ffff..0x50004], &memory[0x4ffff..0x50004]);
    }
    let (mut c, mut b) = protected(&[0x8e, 0xd8, 0xa0, 0, 0, 0, 0], true, true);
    c.r[AX] = 0;
    c.step(&mut b).unwrap();
    assert_eq!(c.s[DS], 0);
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((13, 0)));
}

#[test]
fn expand_down_readonly_and_fetch_limits_fault_before_writes_or_io() {
    for (off, bytes_ok) in [
        (0x1000, false),
        (0x1001, true),
        (0xfffc, true),
        (0xfffe, false),
    ] {
        let mut code = vec![0xa3];
        code.extend((off as u32).to_le_bytes());
        let (mut c, mut b) = protected(&code, true, true);
        let d = SegmentCache::parse(raw(0x20000, 0x1000, 0x97, 0));
        c.segment_caches[DS] = Some(d);
        c.set_reg32(AX, 0x12345678);
        c.step(&mut b).unwrap();
        assert_eq!(c.last_fault.is_none(), bytes_ok);
    }
    // INS must validate destination writability before consuming a device byte.
    let (mut c, mut b) = protected(&[0x6c], true, true);
    c.segment_caches[ES].as_mut().unwrap().access = 0x91;
    c.r[DX] = 0x60;
    c.set_reg32(DI, 0x100);
    let before = b.unknown_ports.clone();
    c.step(&mut b).unwrap();
    fault_frame(&c, &b, 13, 0);
    assert_eq!(b.unknown_ports, before);
    // Truncated 32-bit immediate faults at the first-prefix EIP.
    let (mut c, mut b) = protected(&[0xb8, 1, 2, 3, 4], true, true);
    c.segment_caches[CS].as_mut().unwrap().limit = 0x1002;
    // Handler needs another valid CS descriptor so #GP itself remains deliverable.
    descriptor(&mut b, 32, 0x10000, 0xffff, 0x9b, 0x40);
    gate(&mut b, 13, 32, 0x2000, 0x8e);
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((13, 0)));
    assert_eq!(c.reg32(AX), 0);
    assert_eq!(b.read16(0x17ff4), 0x1000);
}

#[test]
fn gates_frame_width_if_tf_and_guest_iret_with_16_and_32_bit_stacks() {
    for gate32 in [false, true] {
        for trap in [false, true] {
            for stack_big in [false, true] {
                let (mut c, mut b) = protected(&[0xcd, 0x30], true, stack_big);
                c.flags = 2 | IF | TF;
                // A 16-bit gate may enter 32-bit code; explicit 66 IRET matches its frame.
                gate(
                    &mut b,
                    0x30,
                    8,
                    0x2000,
                    0x80 | if gate32 { 14 } else { 6 } | u8::from(trap),
                );
                b.mem[0x12000..0x12002].copy_from_slice(if gate32 {
                    &[0xcf, 0x90]
                } else {
                    &[0x66, 0xcf]
                });
                c.step(&mut b).unwrap();
                assert_eq!(c.flags & IF, if trap { IF } else { 0 });
                assert_eq!(c.flags & TF, 0);
                let sp = if stack_big { 0x18000 } else { 0x8000 } - if gate32 { 12 } else { 6 };
                assert_eq!(c.r[SP], sp as u16);
                assert_eq!(b.read16(sp), 0x1002);
                assert_eq!(b.read16(sp + if gate32 { 4 } else { 2 }), 8);
                c.step(&mut b).unwrap();
                // IRET restores TF; its saved input TF was clear, so no immediate #DB.
                assert_eq!(
                    (c.s[CS], c.ip, c.reg32(SP), c.flags),
                    (8, 0x1002, 0x18000, 2 | IF | TF)
                );
            }
        }
    }
}

#[test]
fn rejected_far_call_and_interrupt_preflight_entire_stack_without_partial_mutation() {
    for code in [&[0x9a, 0x00, 0x20, 0, 0, 8, 0][..], &[0xcd, 0x30][..]] {
        let (mut c, mut b) = protected(code, true, true);
        gate(&mut b, 0x30, 8, 0x2000, 0x8e);
        c.segment_caches[SS].as_mut().unwrap().limit = 0x17ffb;
        // Both the requested frame and #SS/#DF fail; no partial first push.
        let memory = b.mem.clone();
        let sp = c.reg32(SP);
        let error = c.step(&mut b).unwrap_err();
        assert!(
            error.contains("triple fault") || error.contains("shutdown"),
            "{error}"
        );
        assert!(c.shutdown);
        assert_eq!(c.reg32(SP), sp);
        assert_eq!(b.mem, memory);
    }
}

#[test]
fn unsupported_task_privilege_vm86_and_ldt_diagnostics_are_bounded() {
    let (mut c, mut b) = protected(&[0x0f, 0x00, 0xd8], true, true); // LTR AX
    let before = b.mem.clone();
    let saved = format!("{c:?}");
    let error = c.step(&mut b).unwrap_err();
    assert!(error.contains("TSS/task"));
    assert!(error.contains("PE=1 PG=0"));
    assert_eq!(b.mem, before);
    assert_eq!(format!("{c:?}"), saved);
    let (mut arpl, mut arpl_bus) = protected(&[0x63, 0xc0], true, true);
    assert!(arpl
        .step(&mut arpl_bus)
        .unwrap_err()
        .contains("ARPL privilege"));
    let (mut c, mut b) = protected(&[0xcd, 0x30], true, true);
    gate(&mut b, 0x30, 8, 0, 0x85);
    assert!(c.step(&mut b).unwrap_err().contains("task gate"));
    let (mut c, mut b) = protected(&[0xcf], true, true);
    for (i, v) in [0x1000u32, 11, 2].into_iter().enumerate() {
        b.mem[0x18000 + i * 4..0x18004 + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    assert!(c.step(&mut b).unwrap_err().contains("privilege return"));
    let (mut c, mut b) = protected(&[0xcf], true, true);
    for (i, v) in [0x1000u32, 8, 0x20002].into_iter().enumerate() {
        b.mem[0x18000 + i * 4..0x18004 + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    assert!(c.step(&mut b).unwrap_err().contains("virtual-8086"));
    let (mut c, mut b) = protected(&[0x8e, 0xd8], true, true);
    c.r[AX] = 4;
    c.step(&mut b).unwrap();
    fault_frame(&c, &b, 13, 4);
}

#[test]
fn original_boot_round_trip_same_core_cold_and_after_reset() {
    let image = std::fs::read("fixtures/protected_mode/protected_mode.hdm")
        .expect("run python3 tools/prepare_protected_mode_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "protected_mode.hdm").unwrap();
    for run in 0..2 {
        if run == 1 {
            m.reset()
        }
        let mut entered16 = false;
        let mut entered32 = false;
        let mut irq = false;
        let mut fault = false;
        for _ in 0..20 {
            m.step_with_observer(100_000, |c, b| {
                if c.protected_mode() {
                    entered16 |= !c.segment_cache(CS).big();
                    entered32 |= c.segment_cache(CS).big() && c.ip > 0xffff;
                    irq |= b.pics[0].isr & 1 != 0;
                    fault |= c.last_fault == Some((13, 0x40));
                }
            });
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("PROTECTED MODE") {
                break;
            }
        }
        assert!(
            m.bus.text().contains("PROTECTED MODE PASS"),
            "{}",
            m.bus.text()
        );
        assert!(entered16 && entered32 && irq && fault);
        assert!(!m.cpu.protected_mode());
        assert_eq!(m.cpu.cr0, 0x60000010);
        assert_eq!((m.cpu.idtr_base, m.cpu.idtr_limit), (0, 0x3ff));
        assert_ne!(m.cpu.gdtr_base, 0);
        assert_eq!(m.cpu.gdtr_limit, 0x3f);
        assert_eq!(
            (
                m.cpu.s[CS],
                m.cpu.segment_cache(CS).base,
                m.cpu.segment_cache(CS).big()
            ),
            (0x1fc0, 0x1fc00, false)
        );
        assert!(!m.cpu.segment_cache(SS).big());
        assert_eq!(m.cpu.segment_cache(SS).base, 0);
        assert_eq!(m.cpu.r[SP], 0x7c00);
        let mut expected = [0xa5; 64];
        expected[1..5].copy_from_slice(&[0x16, 1, 1, 1]);
        expected[9..13].fill(0x32);
        expected[17] = 0x67;
        expected[25..28].copy_from_slice(&[0x55, 0x4e, 0x46]);
        expected[33] = 0x52;
        assert_eq!(&m.bus.mem[0x8000..0x8040], &expected);
        assert_eq!(
            &m.bus.mem[0x210000..0x210006],
            &[0xa5, 0xcd, 0xab, 0x34, 0x12, 0xa5]
        );
        assert_eq!((m.bus.mem[0x18fe0], m.bus.mem[0x19000]), (0xa5, 0xa5));
        assert_eq!(m.bus.pics[0].isr, 0);
        assert!(m.bus.unknown_ports.is_empty());
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
    }
}

#[test]
fn code_selector_validation_and_conforming_direct_transfer() {
    for (selector, access, limit, offset, vector, error) in [
        (0, 0x9a, 0xffff, 0x2000, 13, 0),
        (0x20, 0x92, 0xffff, 0x2000, 13, 0x20),
        (0x20, 0xfa, 0xffff, 0x2000, 13, 0x20),
        (0x23, 0x9a, 0xffff, 0x2000, 13, 0x20),
        (0x20, 0x1a, 0xffff, 0x2000, 11, 0x20),
        (0x80, 0x9a, 0xffff, 0x2000, 13, 0x80),
        (0x20, 0x9a, 0x1fff, 0x2000, 13, 0),
    ] {
        let mut code = vec![0xea];
        code.extend((offset as u32).to_le_bytes());
        code.extend((selector as u16).to_le_bytes());
        let (mut c, mut b) = protected(&code, true, true);
        if selector < 0x80 {
            descriptor(&mut b, 0x20, 0x10000, limit, access, 0x40);
        }
        c.step(&mut b).unwrap();
        fault_frame(&c, &b, vector, error);
    }
    // A conforming direct jump ignores RPL but retains the original CPL.
    let (mut c, mut b) = protected(&[0xea, 0, 0x20, 0, 0, 0x23, 0], true, true);
    descriptor(&mut b, 0x20, 0x20000, 0xffff, 0x9e, 0x40);
    c.step(&mut b).unwrap();
    assert_eq!(
        (c.cpl, c.s[CS], c.ip, c.segment_cache(CS).base),
        (0, 0x20, 0x2000, 0x20000)
    );
}

#[test]
fn lldt_sldt_cached_ldt_and_a20_descriptor_bus_mapping() {
    let (mut c, mut b) = protected(
        &[0x0f, 0x00, 0xd0, 0x0f, 0x00, 0xc3, 0x8e, 0xd8, 0x8e, 0xe8],
        true,
        true,
    );
    descriptor(&mut b, 0x20, 0x7000, 15, 0x82, 0);
    b.mem[0x7000..0x7008].copy_from_slice(&raw(0x30000, 0xffff, 0x92, 0));
    c.r[AX] = 0x20;
    c.step(&mut b).unwrap();
    assert_eq!(c.ldtr, 0x20);
    // Later GDT edits do not change the hidden LDTR cache.
    descriptor(&mut b, 0x20, 0x7500, 0, 0x82, 0);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(BX), 0x20);
    c.r[AX] = 4;
    c.step(&mut b).unwrap();
    assert_eq!(c.segment_cache(DS).base, 0x30000);
    assert_eq!(b.mem[0x7005], 0x93);
    c.step(&mut b).unwrap();
    assert_eq!(c.segment_cache(5).base, 0x30000);
    for a20 in [false, true] {
        let (mut c, mut b) = protected(&[0x8e, 0xd8, 0xa0, 1, 0, 0, 0], true, true);
        c.gdtr_base = 0x105000;
        b.a20 = a20;
        c.r[AX] = 24;
        b.mem[0x105018..0x105020].copy_from_slice(&raw(0x30000, 0xffff, 0x92, 0));
        b.mem[0x20001] = 0x22;
        b.mem[0x30001] = 0x33;
        c.step(&mut b).unwrap();
        c.step(&mut b).unwrap();
        assert_eq!(c.byte(0), if a20 { 0x33 } else { 0x22 });
    }
    // Bad LLDT must preserve the old hidden table and leave descriptor bytes alone.
    for (selector, access, vector, error) in [
        (0x20, 0x02, 11, 0x20),
        (0x20, 0x92, 13, 0x20),
        (0x24, 0x82, 13, 0x24),
    ] {
        let (mut c, mut b) = protected(&[0x0f, 0x00, 0xd0], true, true);
        descriptor(&mut b, 32, 0x7000, 7, access, 0);
        c.r[AX] = selector;
        c.step(&mut b).unwrap();
        fault_frame(&c, &b, vector, error);
        assert_eq!(c.ldtr_cache, None);
    }
}

#[test]
fn idt_bounds_present_target_checks_and_contributory_double_fault() {
    for (access, selector, vector, error) in [
        (0x0e, 8, 11, 0x182),
        (0x80, 8, 13, 0x182),
        (0x8e, 0x80, 13, 0x80),
        (0x8e, 16, 13, 16),
    ] {
        let (mut c, mut b) = protected(&[0xcd, 0x30], true, true);
        gate(&mut b, 0x30, selector, 0x2000, access);
        c.step(&mut b).unwrap();
        fault_frame(&c, &b, vector, error);
    }
    let (mut c, mut b) = protected(&[0xcd, 0x30], true, true);
    c.idtr_limit = 0x186;
    c.step(&mut b).unwrap();
    fault_frame(&c, &b, 13, 0x182);
    // #GP during #GP delivery becomes #DF with zero error code, at the original EIP.
    let (mut c, mut b) = protected(&[0x8e, 0xd8], true, true);
    c.r[AX] = 0x80;
    gate(&mut b, 13, 8, 0x2000, 0x0e);
    c.step(&mut b).unwrap();
    fault_frame(&c, &b, 8, 0);
    // AAM zero radix must use fault delivery (not software INT semantics).
    let (mut c, mut b) = protected(&[0xd4, 0], true, true);
    gate(&mut b, 0, 8, 0x2000, 0x8e);
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((0, 0)));
    assert_eq!(c.reg32(SP), 0x17ff4);
    assert_eq!(b.read16(0x17ff4), 0x1000);
    assert_eq!(b.mem[0x17ffe] & 1, 1);
    // A benign #UD delivery failure instead enters #NP, with the IDT selector error.
    let (mut c, mut b) = protected(&[0x0f, 0x0b], true, true);
    gate(&mut b, 6, 8, 0x2000, 0x0e);
    c.step(&mut b).unwrap();
    fault_frame(&c, &b, 11, 0x32);
}

#[test]
fn protected_system_memory_operands_x87_and_lmsw_use_descriptors() {
    // SGDT stores beyond a 64 KiB offset using the cached DS base and real bus.
    let (mut c, mut b) = protected(
        &[
            0x0f, 0x01, 0x05, 1, 0, 1, 0, 0x0f, 0x01, 0x25, 9, 0, 1, 0, 0xd9, 0x3d, 17, 0, 1, 0,
        ],
        true,
        true,
    );
    for _ in 0..3 {
        c.step(&mut b).unwrap();
    }
    assert_eq!(&b.mem[0x30001..0x30007], &[0x7f, 0, 0, 0x50, 0, 0]);
    assert_eq!(b.read16(0x30009), 0x11);
    assert_eq!(b.read16(0x30011), 0x37f);
    // LMSW can set PE, but cannot clear it; MOV CR0 is required to leave.
    let mut b = Bus::new(Profile::Pc9821ApDos);
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x1000,
        ..Cpu::default()
    };
    b.mem[0x1000..0x1006].copy_from_slice(&[0x0f, 0x01, 0xf0, 0x0f, 0x01, 0xf0]);
    c.r[AX] = 1;
    c.step(&mut b).unwrap();
    assert_eq!(c.cr0 & 0x1f, 0x11);
    c.r[AX] = 0;
    c.step(&mut b).unwrap();
    assert!(c.protected_mode());
}

#[test]
fn external_idt_failure_has_ext_bit_and_iret_restores_rf() {
    let (mut c, mut b) = protected(&[0x90], true, true);
    gate(&mut b, 0x30, 8, 0x2000, 0x0e);
    c.interrupt(&mut b, 0x30).unwrap();
    fault_frame(&c, &b, 11, 0x183);
    // Return from a recoverable fault: discard its code, then IRETD restores RF.
    b.mem[0x12000..0x12004].copy_from_slice(&[0x83, 0xc4, 4, 0xcf]);
    c.step(&mut b).unwrap();
    c.step(&mut b).unwrap();
    assert_eq!((c.s[CS], c.ip, c.flags_upper & 1), (8, 0x1000, 1));
    c.step(&mut b).unwrap();
    assert_eq!((c.ip, c.flags_upper & 1), (0x1001, 0));
}

#[test]
fn enter_frame_chain_uses_operand_size_independently_of_stack_size() {
    for operand32 in [false, true] {
        for stack_big in [false, true] {
            // CS.D=1; 66 selects BP/word entries, 67 must not affect ENTER.
            for address_override in [false, true] {
                for old_bp in [0x18100u32, 0x20002] {
                    let mut code = vec![];
                    if !operand32 {
                        code.push(0x66);
                    }
                    if address_override {
                        code.push(0x67);
                    }
                    code.extend([0xc8, 0x10, 0, 3]); // two chain entries, then frame pointer
                    let (mut c, mut b) = protected(&code, true, stack_big);
                    c.set_reg32(BP, old_bp);
                    let bytes = if operand32 { 4 } else { 2 };
                    let mask = if operand32 { u32::MAX } else { 0xffff };
                    let stack_top = if stack_big { 0x18000 } else { 0x8000 };
                    let frame = 0x18000 - bytes;
                    // Distinct entries above/below 64 KiB detect wrong width selection.
                    b.mem[0x7fe0..0x8001].fill(0xa5);
                    b.mem[0x17fe0..0x18001].fill(0xa5);
                    for (i, value) in [(1, 0x12345678u32), (2, 0x87654321)] {
                        let offset = (old_bp.wrapping_sub(bytes * i) & mask) as usize;
                        b.mem[offset..offset + bytes as usize]
                            .copy_from_slice(&value.to_le_bytes()[..bytes as usize]);
                    }
                    let mut expected = b.mem.clone();
                    for (i, value) in [old_bp, 0x12345678, 0x87654321, frame]
                        .into_iter()
                        .enumerate()
                    {
                        let offset = (stack_top - bytes * (i as u32 + 1)) as usize;
                        expected[offset..offset + bytes as usize]
                            .copy_from_slice(&value.to_le_bytes()[..bytes as usize]);
                    }
                    c.step(&mut b).unwrap();
                    assert_eq!(c.last_fault, None);
                    assert_eq!(c.reg32(SP), 0x18000 - bytes * 4 - 0x10);
                    assert_eq!(
                        c.reg32(BP),
                        if operand32 {
                            frame
                        } else {
                            (old_bp & 0xffff0000) | (frame & 0xffff)
                        }
                    );
                    assert!(b.mem == expected, "ENTER frame/guards differ: operand32={operand32} SS.B={stack_big} EBP={old_bp:#x}");
                }
            }
        }
    }
}

#[test]
fn protected_enter_leave_pop_esp_and_segment_reload_paths() {
    for level in [0u8, 2] {
        let (mut c, mut b) = protected(&[0xc8, 0x10, 0, level, 0xc9], true, true);
        c.set_reg32(BP, 0x18100);
        b.mem[0x180fc..0x18100].copy_from_slice(&0x12345678u32.to_le_bytes());
        c.step(&mut b).unwrap();
        assert_eq!(c.reg32(BP), 0x17ffc);
        assert_eq!(c.reg32(SP), 0x18000 - 4 * (1 + level as u32) - 0x10);
        c.step(&mut b).unwrap();
        assert_eq!((c.reg32(SP), c.reg32(BP)), (0x18000, 0x18100));
    }
    let (mut c, mut b) = protected(&[0x8f, 0x04, 0x24], true, true); // POP [ESP], using incremented ESP
    b.mem[0x18000..0x18004].copy_from_slice(&0x12345678u32.to_le_bytes());
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0x18004..0x18008], &0x12345678u32.to_le_bytes());
    // LFS loads both full offset and hidden segment state, then PUSH/POP GS.
    let (mut c, mut b) = protected(
        &[0x0f, 0xb4, 0x05, 0x00, 0x10, 0, 0, 0x0f, 0xa0, 0x0f, 0xa9],
        true,
        true,
    );
    descriptor(&mut b, 0x20, 0x35000, 0xffff, 0x92, 0);
    b.mem[0x21000..0x21006].copy_from_slice(&[0x78, 0x56, 0x34, 0x12, 0x20, 0]);
    c.step(&mut b).unwrap();
    assert_eq!(
        (c.reg32(AX), c.segment_cache(4).base),
        (0x12345678, 0x35000)
    );
    c.step(&mut b).unwrap();
    c.step(&mut b).unwrap();
    assert_eq!(
        (c.extra_segments[1], c.segment_cache(5).base),
        (0x20, 0x35000)
    );
}

#[test]
fn clearing_pe_and_real_segment_reloads_retain_hidden_limits_and_data_base() {
    let (mut c, mut b) = protected(
        &[
            0x0f, 0x22, 0xc0, 0xa1, 1, 0, 1, 0, 0xea, 0, 0x10, 0, 0, 0, 0x20,
        ],
        true,
        true,
    );
    c.set_reg32(AX, 0x60000010);
    b.mem[0x30001..0x30005].copy_from_slice(&0x12345678u32.to_le_bytes());
    b.mem[0x40001..0x40005].copy_from_slice(&0xdeadbeefu32.to_le_bytes());
    b.mem[0x21000..0x2100c]
        .copy_from_slice(&[0xb8, 0, 0x30, 0x8e, 0xd8, 0x66, 0x67, 0xa1, 1, 0, 1, 0]);
    c.step(&mut b).unwrap();
    assert!(!c.protected_mode());
    assert!(c.segment_cache(CS).big());
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x12345678);
    c.step(&mut b).unwrap();
    assert_eq!(
        (c.segment_cache(CS).base, c.segment_cache(CS).big()),
        (0x20000, false)
    );
    assert_eq!(c.segment_cache(CS).limit, 0x3ffff);
    c.step(&mut b).unwrap();
    c.step(&mut b).unwrap();
    assert_eq!(
        (c.segment_cache(DS).base, c.segment_cache(DS).limit),
        (0x30000, 0x2ffff)
    );
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0xdeadbeef);
}

#[test]
fn synthetic_bios_timer_after_protected_return_reloads_real_cs_cache() {
    let image = std::fs::read("fixtures/protected_mode/protected_mode.hdm").unwrap();
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image, "protected_mode.hdm").unwrap();
    m.step(10000);
    assert!(m.error.is_none());
    assert!(m.bus.text().contains("PROTECTED MODE PASS"));
    // Execute a new original real-mode BIOS caller with the retained post-PE caches.
    m.cpu.halted = false;
    m.cpu.ip = 0x1000;
    m.bus.mem[0x20c00..0x20c12].copy_from_slice(&[
        0x8c, 0xc8, 0x8e, 0xc0, // MOV AX,CS; MOV ES,AX
        0xbb, 0, 0x20, 0xb9, 1, 0, 0xb8, 0, 2, // BX=2000, CX=1, AH=02
        0xcd, 0x1c, 0xfb, 0xeb, 0xfe, // INT 1C; STI; JMP $
    ]);
    m.bus.mem[0x21c00..0x21c06].copy_from_slice(&[0x2e, 0xfe, 0x06, 0x10, 0x20, 0xcf]);
    m.bus.mem[0x21c10] = 0;
    m.bus.pics[0].mask = 0xfe;
    for _ in 0..4 {
        m.step(10000);
        assert!(m.error.is_none(), "{:?}", m.error);
    }
    assert_eq!(m.bus.mem[0x21c10], 1);
    assert_eq!(
        (m.cpu.s[CS], m.cpu.segment_cache(CS).base),
        (0x1fc0, 0x1fc00)
    );
}

#[test]
fn full_width_fetch_wrap_and_indirect_far_pointer_overflow_are_bounded() {
    let (mut c, mut b) = protected(&[], true, true);
    c.segment_caches[CS] = Some(SegmentCache {
        base: 0x10003,
        limit: u32::MAX,
        access: 0x9b,
        attributes: 0xc0,
    });
    c.ip = 0xffff_fffd;
    c.set_reg32(AX, 0xe0000011);
    b.mem[0x10000..0x10003].copy_from_slice(&[0x0f, 0x22, 0xc0]);
    let error = c.step(&mut b).unwrap_err();
    assert!(error.contains("CS:IP=0008:fffffffd"), "{error}");
    assert_eq!(c.ip, 0xffff_fffd);
    let (mut c, mut b) = protected(&[0xff, 0x2d, 0xfc, 0xff, 0xff, 0xff], true, true);
    c.segment_caches[DS] = Some(SegmentCache {
        base: 0,
        limit: u32::MAX,
        access: 0x93,
        attributes: 0xc0,
    });
    c.step(&mut b).unwrap();
    fault_frame(&c, &b, 13, 0);
}

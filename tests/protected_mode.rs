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
fn entering_pe_keeps_real_segment_cache_until_far_jump_and_paging_enable() {
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
    let cache = c.segment_caches;
    let memory = b.mem.clone();
    c.step(&mut b).unwrap();
    assert_eq!(c.cr0, 0xe0000011);
    assert!(c.paging_enabled());
    assert_eq!(c.segment_caches, cache);
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
    c.set_reg32(AX, 0x60000051); // undefined bit 6: retain a rejected-write diagnostic
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

#[test]
fn cmpxchg_requires_writable_destination_for_both_comparison_outcomes() {
    for bytes in [1usize, 2, 4] {
        for locked in [false, true] {
            for equal in [false, true] {
                for writable in [false, true] {
                    let mut code = vec![];
                    if bytes == 2 {
                        code.push(0x66);
                    }
                    if locked {
                        code.push(0xf0);
                    }
                    code.extend([0x0f, if bytes == 1 { 0xb0 } else { 0xb1 }, 0x0d, 0, 0, 0, 0]);
                    let (mut c, mut b) = protected(&code, true, true);
                    c.segment_caches[DS].as_mut().unwrap().access =
                        if writable { 0x93 } else { 0x91 };
                    let initial = 0x87654321u32;
                    let accumulator = if equal { initial } else { 0x12345678 };
                    c.set_reg32(AX, accumulator);
                    c.set_reg32(CX, 0xaabbccdd);
                    c.flags = 2 | CF;
                    b.mem[0x1ffff..0x20005].fill(0xa5);
                    b.mem[0x20000..0x20000 + bytes]
                        .copy_from_slice(&initial.to_le_bytes()[..bytes]);
                    c.step(&mut b).unwrap();
                    let mask = u32::MAX >> ((4 - bytes) * 8);
                    let mut expected = initial.to_le_bytes();
                    if writable {
                        assert_eq!(c.last_fault, None);
                        assert_eq!(c.ip, 0x1000 + code.len() as u32);
                        assert_eq!(c.flags & ZF != 0, equal);
                        assert_eq!(
                            c.reg32(AX),
                            if equal {
                                accumulator
                            } else {
                                (accumulator & !mask) | (initial & mask)
                            }
                        );
                        if equal {
                            expected = 0xaabbccddu32.to_le_bytes();
                        }
                    } else {
                        fault_frame(&c, &b, 13, 0);
                        assert_eq!(c.reg32(AX), accumulator);
                        assert_eq!(b.mem[0x17ffc], (2 | CF) as u8);
                    }
                    assert_eq!(c.reg32(CX), 0xaabbccdd);
                    assert_eq!(&b.mem[0x20000..0x20000 + bytes], &expected[..bytes]);
                    assert_eq!((b.mem[0x1ffff], b.mem[0x20000 + bytes]), (0xa5, 0xa5));
                }
            }
        }
    }
}

#[test]
fn cmpxchg_mismatch_performs_unchanged_value_bus_writeback() {
    for equal in [false, true] {
        // The EGC's inverted CPU-source ROP exposes the byte actually written.
        let (mut c, mut b) = protected(&[0x0f, 0xb0, 0x0d, 0, 0, 0, 0], true, true);
        c.segment_caches[DS].as_mut().unwrap().base = 0xa8000;
        c.set_reg32(AX, if equal { 0x12 } else { 0xff });
        c.set_reg32(CX, 0x34);
        b.egc.enabled = true;
        b.grcg_mode = 0x80;
        b.egc.registers[2] = 0x2c0f;
        b.graphics[0] = 0x12;
        c.step(&mut b).unwrap();
        assert_eq!(c.last_fault, None);
        assert_eq!(c.flags & ZF != 0, equal);
        assert_eq!(c.byte(AX), 0x12);
        for plane in 0..4 {
            assert_eq!(b.graphics[plane * 0x8000], if equal { 0xcb } else { 0xed });
            assert_eq!(b.graphics[plane * 0x8000 + 1], 0);
        }
    }
}

#[test]
fn rejected_memory_read_modify_write_preserves_egc_source_and_pattern_latches() {
    // Each encoding addresses DS:0 in 32-bit address size. Cover every shared
    // modifying path, both bit addressing forms, and byte/word/dword spans.
    let cases: &[(&[u8], u8, &[u8], bool)] = &[
        (&[0x00], 0x0d, &[], false), // ADD byte [0],CL
        (&[0x01], 0x0d, &[], true),  // ADD [0],ECX
        (&[0x80], 0x05, &[1], false),
        (&[0x81], 0x05, &[1, 0, 0, 0], true),
        (&[0x83], 0x05, &[1], true),
        (&[0x86], 0x0d, &[], false), // XCHG
        (&[0x87], 0x0d, &[], true),
        (&[0xc0], 0x25, &[1], false), // SHL
        (&[0xc1], 0x25, &[1], true),
        (&[0xd0], 0x25, &[], false),
        (&[0xd1], 0x25, &[], true),
        (&[0xd2], 0x25, &[], false),
        (&[0xd3], 0x25, &[], true),
        (&[0xf6], 0x15, &[], false), // NOT
        (&[0xf7], 0x15, &[], true),
        (&[0xf6], 0x1d, &[], false), // NEG
        (&[0xf7], 0x1d, &[], true),
        (&[0xfe], 0x05, &[], false), // INC
        (&[0xff], 0x05, &[], true),
        (&[0xfe], 0x0d, &[], false), // DEC
        (&[0xff], 0x0d, &[], true),
        (&[0x0f, 0xab], 0x0d, &[], true), // BTS/BTR/BTC
        (&[0x0f, 0xb3], 0x0d, &[], true),
        (&[0x0f, 0xbb], 0x0d, &[], true),
        (&[0x0f, 0xba], 0x2d, &[1], true),
        (&[0x0f, 0xba], 0x35, &[1], true),
        (&[0x0f, 0xba], 0x3d, &[1], true),
        (&[0x0f, 0xa4], 0x0d, &[1], true), // SHLD/SHRD
        (&[0x0f, 0xa5], 0x0d, &[], true),
        (&[0x0f, 0xac], 0x0d, &[1], true),
        (&[0x0f, 0xad], 0x0d, &[], true),
        (&[0x0f, 0xb0], 0x0d, &[], false), // CMPXCHG mismatch
        (&[0x0f, 0xb1], 0x0d, &[], true),
        (&[0x0f, 0xc0], 0x0d, &[], false), // XADD
        (&[0x0f, 0xc1], 0x0d, &[], true),
    ];
    for &(opcode, modrm, immediate, wide) in cases {
        for word in [false, true] {
            if word && !wide {
                continue;
            }
            let mut code = vec![];
            if word {
                code.push(0x66);
            }
            code.extend(opcode);
            code.extend([modrm, 0, 0, 0, 0]);
            code.extend(if word && opcode == [0x81] {
                &immediate[..2]
            } else {
                immediate
            });
            let (mut c, mut b) = protected(&code, true, true);
            let ds = c.segment_caches[DS].as_mut().unwrap();
            ds.base = 0xa8000;
            ds.access = 0x91; // Readable, present, read-only data.
            c.set_reg32(AX, 0x12345678);
            c.set_reg32(CX, 1);
            let registers = c.r;
            b.egc.enabled = true;
            b.grcg_mode = 0x80;
            b.egc.registers[2] = 0x29f0; // source-copy ROP; reads also load pattern
            for plane in 0..4 {
                b.graphics[plane * 0x8000..plane * 0x8000 + 4].copy_from_slice(&[
                    0x12 + plane as u8,
                    0x34,
                    0x56,
                    0x78,
                ]);
            }
            let graphics = b.graphics.clone();
            c.step(&mut b).unwrap();
            fault_frame(&c, &b, 13, 0);
            for r in [AX, CX, DX, BX, BP, SI, DI] {
                assert_eq!(c.r[r], registers[r], "opcode={code:02x?}");
            }
            assert_eq!(b.graphics, graphics, "opcode={code:02x?}");
            // Expose both latch lanes through real bus writes, without reading
            // VRAM first. Rejected operand reads would copy nonzero data here.
            for (rop, offset) in [(0x28f0, 0x10), (0x3000, 0x12)] {
                b.egc.registers[2] = rop;
                b.write(0xa8000 + offset, 0xff);
                b.write(0xa8001 + offset, 0xff);
                for plane in 0..4 {
                    assert_eq!(
                        &b.graphics[plane * 0x8000 + offset..plane * 0x8000 + offset + 2],
                        &[0, 0],
                        "opcode={code:02x?} ROP={rop:04x} plane={plane}"
                    );
                }
            }
        }
    }
}

#[test]
fn rejected_movs_and_cmps_preserve_egc_source_and_pattern_latches() {
    for opcode in [0xa4, 0xa6] {
        for bytes in [1usize, 2, 4] {
            for failure in ["destination access", "destination limit", "source limit"] {
                for repeat in [false, true] {
                    let mut code = vec![];
                    if repeat {
                        code.push(0xf3);
                    }
                    if bytes == 2 {
                        code.push(0x66);
                    }
                    code.push(opcode | u8::from(bytes != 1));
                    let (mut c, mut b) = protected(&code, true, true);
                    c.segment_caches[DS].as_mut().unwrap().base = 0xa8000;
                    match failure {
                        "destination access" if opcode == 0xa4 => {
                            c.segment_caches[ES].as_mut().unwrap().access = 0x91;
                        }
                        "destination access" => {
                            c.segment_caches[ES].as_mut().unwrap().access = 0;
                        }
                        _ => {
                            let segment = if failure == "source limit" { DS } else { ES };
                            // Start inside the segment but let the final byte exceed
                            // its limit (or start outside for the byte instruction).
                            let offset = if bytes == 1 { 1 } else { 0 };
                            c.set_reg32(if segment == DS { SI } else { DI }, offset);
                            c.segment_caches[segment].as_mut().unwrap().limit =
                                bytes.saturating_sub(2) as u32;
                        }
                    }
                    c.set_reg32(CX, 2);
                    c.flags = 2 | CF;
                    let registers = [c.reg32(SI), c.reg32(DI), c.reg32(CX), c.reg32(AX)];
                    b.egc.enabled = true;
                    b.grcg_mode = 0x80;
                    b.egc.registers[2] = 0x29f0; // reads load source and pattern
                    for plane in 0..4 {
                        b.graphics[plane * 0x8000..plane * 0x8000 + 4].copy_from_slice(&[
                            0x12 + plane as u8,
                            0x34,
                            0x56,
                            0x78,
                        ]);
                    }
                    let graphics = b.graphics.clone();
                    let memory = b.mem.clone();
                    c.step(&mut b).unwrap();
                    fault_frame(&c, &b, 13, 0);
                    assert_eq!(
                        [c.reg32(SI), c.reg32(DI), c.reg32(CX), c.reg32(AX)],
                        registers
                    );
                    assert_eq!(b.mem[0x17ffc], (2 | CF) as u8);
                    assert_eq!(b.graphics, graphics);
                    assert_eq!(&b.mem[0x20000..0x50000], &memory[0x20000..0x50000]);
                    // Observe both latch lanes through writes, without another
                    // VRAM read that could overwrite evidence of a rejected read.
                    for (rop, offset) in [(0x28f0, 0x10), (0x3000, 0x12)] {
                        b.egc.registers[2] = rop;
                        b.write(0xa8000 + offset, 0xff);
                        b.write(0xa8001 + offset, 0xff);
                        for plane in 0..4 {
                            assert_eq!(
                                &b.graphics[plane * 0x8000 + offset..plane * 0x8000 + offset + 2],
                                &[0, 0],
                                "opcode={code:02x?} failure={failure} ROP={rop:04x} plane={plane}"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn seed_stack_operand_egc(b: &mut Bus) {
    b.egc.enabled = true;
    b.grcg_mode = 0x80;
    b.egc.registers[2] = 0x29f0; // reads load both source and pattern latches
    for plane in 0..4 {
        b.graphics[plane * 0x8000..plane * 0x8000 + 4].copy_from_slice(&[
            0x12 + plane as u8,
            0x34,
            0x56,
            0x78,
        ]);
    }
}

fn assert_stack_operand_egc_latches_unchanged(b: &mut Bus) {
    // Observe source and pattern through bus writes without reading VRAM first.
    // Both lanes of every plane must still contain their original zero values.
    for (rop, offset) in [(0x28f0, 0x10), (0x3000, 0x12)] {
        b.egc.registers[2] = rop;
        b.write(0xa8000 + offset, 0xff);
        b.write(0xa8001 + offset, 0xff);
        for plane in 0..4 {
            assert_eq!(
                &b.graphics[plane * 0x8000 + offset..plane * 0x8000 + offset + 2],
                &[0, 0],
                "ROP={rop:04x} plane={plane}"
            );
        }
    }
}

#[test]
fn rejected_retf_preflights_both_slots_before_stack_device_reads() {
    for big in [false, true] {
        for bytes in [2u32, 4] {
            for stack_big in [false, true] {
                for op in [0xca, 0xcb] {
                    let mut code = vec![];
                    if big != (bytes == 4) {
                        code.push(0x66);
                    }
                    code.push(op);
                    if op == 0xca {
                        code.extend([0x10, 0]);
                    }
                    let (mut c, mut b) = protected(&code, big, stack_big);
                    let sp = if stack_big { 0xa8000 } else { 0x8000 };
                    c.set_reg32(SP, sp);
                    let ss = c.segment_caches[SS].as_mut().unwrap();
                    ss.base = if stack_big { 0 } else { 0xa0000 };
                    // The offset slot fits in EGC VRAM; the selector slot does not.
                    ss.limit = sp + bytes - 1;
                    seed_stack_operand_egc(&mut b);
                    let registers = std::array::from_fn::<_, 8, _>(|r| c.reg32(r));
                    let memory = b.mem.clone();
                    let graphics = b.graphics.clone();
                    let flags = c.flags;
                    c.step(&mut b).unwrap();
                    assert_eq!(c.last_fault, Some((12, 0)));
                    assert_eq!((c.s[CS], c.ip), (8, 0x2000));
                    assert_eq!(c.flags, flags);
                    for (r, value) in registers.into_iter().enumerate() {
                        assert_eq!(c.reg32(r), if r == SP { value - 16 } else { value });
                    }
                    // Only the #SS frame is pushed, below VRAM in unmapped A7FF0.
                    assert_eq!(b.mem, memory);
                    assert_eq!(b.graphics, graphics);
                    assert_stack_operand_egc_latches_unchanged(&mut b);
                }
            }
        }
    }
}

#[test]
fn truncated_test_immediate_faults_before_operand_device_reads() {
    for big in [false, true] {
        for bytes in [1usize, 2, 4] {
            for available in 0..bytes {
                let mut code = vec![];
                if bytes != 1 && big != (bytes == 4) {
                    code.push(0x66);
                }
                if !big {
                    code.push(0x67);
                }
                code.extend([if bytes == 1 { 0xf6 } else { 0xf7 }, 0x05, 0, 0, 0, 0]);
                let immediate_start = code.len();
                code.extend_from_slice(&1u32.to_le_bytes()[..bytes]);
                let (mut c, mut b) = protected(&code, big, true);
                c.segment_caches[CS].as_mut().unwrap().limit =
                    0x1000 + (immediate_start + available) as u32 - 1;
                c.segment_caches[DS].as_mut().unwrap().base = 0xa8000;
                seed_stack_operand_egc(&mut b);
                let registers = std::array::from_fn::<_, 8, _>(|r| c.reg32(r));
                let memory = b.mem.clone();
                let graphics = b.graphics.clone();
                let flags = c.flags;
                c.step(&mut b).unwrap();
                fault_frame(&c, &b, 13, 0);
                assert_eq!(c.flags, flags);
                for (r, value) in registers.into_iter().enumerate() {
                    assert_eq!(c.reg32(r), if r == SP { value - 16 } else { value });
                }
                let sp = c.reg32(SP) as usize;
                // Only the expected exception frame may change RAM.
                assert_eq!(&b.mem[..sp], &memory[..sp]);
                assert_eq!(&b.mem[sp + 16..], &memory[sp + 16..]);
                assert_eq!(b.graphics, graphics);
                assert_stack_operand_egc_latches_unchanged(&mut b);
            }
        }
    }
}

#[test]
fn rejected_pop_memory_preflights_destination_before_stack_device_read() {
    for bytes in [2u32, 4] {
        for failure in ["read-only", "limit", "post-increment ESP limit"] {
            let mut code = vec![];
            if bytes == 2 {
                code.push(0x66);
            }
            if failure == "post-increment ESP limit" {
                code.extend([0x3e, 0x8f, 0x04, 0x24]); // POP DS:[ESP]
            } else {
                code.extend([0x8f, 0x05, 0, 0, 0, 0]); // POP DS:[0]
            }
            let (mut c, mut b) = protected(&code, true, true);
            c.set_reg32(SP, 0xa8000);
            c.segment_caches[SS].as_mut().unwrap().limit = 0xaffff;
            let ds = c.segment_caches[DS].as_mut().unwrap();
            match failure {
                "read-only" => ds.access = 0x91,
                "limit" => ds.limit = bytes - 2,
                _ => {
                    ds.base = 0;
                    // The old ESP fits; the incremented destination does not.
                    ds.limit = 0xa8000 + bytes - 1;
                }
            }
            seed_stack_operand_egc(&mut b);
            let registers = std::array::from_fn::<_, 8, _>(|r| c.reg32(r));
            let memory = b.mem.clone();
            let graphics = b.graphics.clone();
            c.step(&mut b).unwrap();
            assert_eq!(c.last_fault, Some((13, 0)));
            assert_eq!((c.s[CS], c.ip), (8, 0x2000));
            for (r, value) in registers.into_iter().enumerate() {
                assert_eq!(c.reg32(r), if r == SP { value - 16 } else { value });
            }
            // The exception frame is below VRAM in the unmapped A7FF0 range;
            // bus writes there leave RAM and graphics unchanged.
            assert_eq!(b.mem, memory);
            assert_eq!(b.graphics, graphics);
            assert_stack_operand_egc_latches_unchanged(&mut b);
        }
    }
}

#[test]
fn rejected_memory_push_and_near_call_preflight_stack_before_device_read() {
    for bytes in [2u32, 4] {
        for stack_big in [false, true] {
            for modrm in [0x15, 0x35] {
                // CALL DS:[0], PUSH DS:[0]
                for failure in ["limit", "read-only"] {
                    let mut code = vec![];
                    if bytes == 2 {
                        code.push(0x66);
                    }
                    code.extend([0xff, modrm, 0, 0, 0, 0]);
                    let (mut c, mut b) = protected(&code, true, stack_big);
                    c.segment_caches[DS].as_mut().unwrap().base = 0xa8000;
                    let sp = if stack_big { 0x18000 } else { 0x8000 };
                    let ss = c.segment_caches[SS].as_mut().unwrap();
                    if failure == "limit" {
                        // First byte is valid, final byte exceeds the limit.
                        ss.limit = sp - 2;
                    } else {
                        ss.access = 0x91;
                    }
                    seed_stack_operand_egc(&mut b);
                    let registers = std::array::from_fn::<_, 8, _>(|r| c.reg32(r));
                    let memory = b.mem.clone();
                    let graphics = b.graphics.clone();
                    let flags = c.flags;
                    let error = c.step(&mut b).unwrap_err();
                    assert!(error.contains("shutdown"), "{error}");
                    assert!(c.shutdown); // #SS and #DF frames also cannot fit.
                    assert_eq!(c.ip, 0x1000);
                    assert_eq!(c.flags, flags);
                    for (r, value) in registers.into_iter().enumerate() {
                        assert_eq!(c.reg32(r), value);
                    }
                    assert_eq!(b.mem, memory);
                    assert_eq!(b.graphics, graphics);
                    assert_stack_operand_egc_latches_unchanged(&mut b);
                }
            }
        }
    }
}

#[test]
fn truncated_imul_immediates_fault_before_operand_device_reads() {
    for big in [false, true] {
        for bytes in [2usize, 4] {
            for op in [0x69, 0x6b] {
                let immediate_bytes = if op == 0x69 { bytes } else { 1 };
                for available in 0..immediate_bytes {
                    let mut code = vec![];
                    if big != (bytes == 4) {
                        code.push(0x66);
                    }
                    if !big {
                        code.push(0x67);
                    }
                    code.extend([op, 0x05, 0, 0, 0, 0]); // IMUL E/AX, DS:[0], imm
                    let immediate_start = code.len();
                    code.extend_from_slice(&1u32.to_le_bytes()[..immediate_bytes]);
                    let (mut c, mut b) = protected(&code, big, true);
                    c.segment_caches[CS].as_mut().unwrap().limit =
                        0x1000 + (immediate_start + available) as u32 - 1;
                    c.segment_caches[DS].as_mut().unwrap().base = 0xa8000;
                    c.set_reg32(AX, 0x12345678);
                    c.flags |= CF | OF;
                    seed_stack_operand_egc(&mut b);
                    let saved = c.clone();
                    let memory = b.mem.clone();
                    let graphics = b.graphics.clone();
                    c.step(&mut b).unwrap();
                    fault_frame(&c, &b, 13, 0);
                    assert_eq!(c.flags, saved.flags);
                    assert_eq!(c.cr0, saved.cr0);
                    assert_eq!(c.s, saved.s);
                    // Fault delivery reloads CS from the GDT; other caches stay put.
                    for seg in [ES, SS, DS, 4, 5] {
                        assert_eq!(c.segment_caches[seg], saved.segment_caches[seg]);
                    }
                    for r in 0..8 {
                        assert_eq!(
                            c.reg32(r),
                            if r == SP {
                                saved.reg32(r) - 16
                            } else {
                                saved.reg32(r)
                            }
                        );
                    }
                    let sp = c.reg32(SP) as usize;
                    assert_eq!(&b.mem[..sp], &memory[..sp]);
                    assert_eq!(&b.mem[sp + 16..], &memory[sp + 16..]);
                    assert_eq!(b.graphics, graphics);
                    assert_stack_operand_egc_latches_unchanged(&mut b);
                }
            }
        }
    }
}

#[test]
fn rejected_indirect_far_call_preflights_both_slots_before_device_pointer_reads() {
    for big in [false, true] {
        for bytes in [2u32, 4] {
            for stack_big in [false, true] {
                for failure in ["first slot", "second slot", "read-only"] {
                    let mut code = vec![];
                    if big != (bytes == 4) {
                        code.push(0x66);
                    }
                    if !big {
                        code.push(0x67);
                    }
                    code.extend([0xff, 0x1d, 0, 0, 0, 0]); // CALL FAR DS:[0]
                    let (mut c, mut b) = protected(&code, big, stack_big);
                    c.segment_caches[DS].as_mut().unwrap().base = 0xa8000;
                    let sp = if stack_big { 0x18000 } else { 0x8000 };
                    let ss = c.segment_caches[SS].as_mut().unwrap();
                    match failure {
                        "first slot" => ss.limit = sp - bytes - 1,
                        "second slot" => {
                            ss.access = 0x97; // expand down: only the first push fits
                            ss.limit = sp - bytes * 2;
                        }
                        _ => ss.access = 0x91,
                    }
                    seed_stack_operand_egc(&mut b);
                    // A valid same-ring pointer, so only the stack rejects CALL.
                    b.graphics[..bytes as usize]
                        .copy_from_slice(&0x2000u32.to_le_bytes()[..bytes as usize]);
                    b.graphics[bytes as usize..bytes as usize + 2]
                        .copy_from_slice(&8u16.to_le_bytes());
                    let saved = c.clone();
                    let memory = b.mem.clone();
                    let graphics = b.graphics.clone();
                    let error = c.step(&mut b).unwrap_err();
                    assert!(error.contains("shutdown"), "{error}");
                    assert!(c.shutdown); // Neither #SS nor #DF frames fit.
                    assert_eq!(c.ip, saved.ip);
                    assert_eq!(c.flags, saved.flags);
                    assert_eq!(c.cr0, saved.cr0);
                    assert_eq!(c.s, saved.s);
                    assert_eq!(c.segment_caches, saved.segment_caches);
                    for r in 0..8 {
                        assert_eq!(c.reg32(r), saved.reg32(r));
                    }
                    assert_eq!(b.mem, memory);
                    assert_eq!(b.graphics, graphics);
                    assert_stack_operand_egc_latches_unchanged(&mut b);
                }
            }
        }
    }
}

#[test]
fn memory_push_and_near_call_read_operand_before_changing_esp() {
    for bytes in [2u32, 4] {
        for stack_big in [false, true] {
            for modrm in [0x14, 0x34] {
                // CALL [ESP], PUSH [ESP]
                let mut code = vec![];
                if bytes == 2 {
                    code.push(0x66);
                }
                code.extend([0xff, modrm, 0x24]);
                let (mut c, mut b) = protected(&code, true, stack_big);
                b.mem[0x18000..0x18004].copy_from_slice(&0x2000u32.to_le_bytes());
                let sp = if stack_big { 0x18000 } else { 0x8000 } - bytes as usize;
                b.mem[sp - 1..sp + bytes as usize].fill(0xa5);
                c.step(&mut b).unwrap();
                assert_eq!(c.last_fault, None);
                assert_eq!(c.reg32(SP), 0x18000 - bytes);
                let ret = 0x1000 + code.len() as u32;
                let value = if modrm == 0x14 { ret } else { 0x2000 };
                assert_eq!(
                    &b.mem[sp..sp + bytes as usize],
                    &value.to_le_bytes()[..bytes as usize]
                );
                assert_eq!(b.mem[sp - 1], 0xa5);
                assert_eq!(c.ip, if modrm == 0x14 { 0x2000 } else { ret });
            }
        }
    }
}

#[test]
fn ltr_accepts_both_tss_types_and_caches_metadata_without_touching_tss_or_flags() {
    for (access, limit) in [(0x81, 43), (0xe9, 103), (0x81, 0), (0x89, 102)] {
        let (mut c, mut b) = protected(&[0x0f, 0x00, 0xd8], true, true);
        // Size is checked only when a task switch actually consumes the TSS.
        // Neither DPL nor source RPL limits this privileged descriptor load.
        descriptor(&mut b, 32, 0x12345678, limit, access, 0);
        c.set_reg32(AX, 0xabcd0023);
        c.flags = 2 | CF | OF | DF;
        let regs = c.r;
        let upper = c.upper;
        let flags = c.flags;
        let mut memory = b.mem.clone();
        memory[0x5025] |= 2;
        c.step(&mut b).unwrap();
        assert_eq!(c.last_fault, None);
        assert_eq!((c.tr, c.r, c.upper, c.flags), (0x23, regs, upper, flags));
        let d = c.tr_cache.unwrap();
        assert_eq!((d.base, d.limit, d.access), (0x12345678, limit, access | 2));
        assert_eq!(b.mem, memory); // No TSS read/write, not even an installed base required.
    }
    // Page-granular limits use the same descriptor parser as other tables.
    let (mut c, mut b) = protected(&[0x0f, 0x00, 0xd8], true, true);
    descriptor(&mut b, 32, 0x210001, 1, 0x89, 0x80);
    c.r[AX] = 32;
    c.step(&mut b).unwrap();
    assert_eq!(c.tr_cache.unwrap().limit, 0x1fff);
}

#[test]
fn str_and_ltr_register_width_is_always_word_across_defaults_and_prefixes() {
    for big in [false, true] {
        for prefix in [&[][..], &[0x66][..], &[0x66, 0x66, 0x67, 0x67][..]] {
            for reg in 0..8 {
                let mut code = prefix.to_vec();
                code.extend([0x0f, 0x00, 0xd8 | reg]); // LTR r16
                code.extend(prefix);
                code.extend([0x0f, 0x00, 0xc8 | reg]); // STR r16
                let (mut c, mut b) = protected(&code, big, true);
                descriptor(&mut b, 32, 0x30000, 103, 0x89, 0);
                c.set_reg32(reg as usize, 0x98760023);
                let flags = c.flags;
                c.step(&mut b).unwrap();
                assert_eq!(c.tr, 0x23);
                // Cache and visible TR survive guest edits, including busy clear.
                let cache = c.tr_cache;
                b.mem[0x5020..0x5028].fill(0);
                c.set_reg32(reg as usize, 0xbeefffff);
                let mut expected = c.r;
                expected[reg as usize] = 0x23;
                c.step(&mut b).unwrap();
                assert_eq!(c.r, expected);
                assert_eq!(c.reg32(reg as usize), 0xbeef0023);
                assert_eq!(c.flags, flags);
                assert_eq!(c.tr_cache, cache);
            }
        }
    }
}

#[test]
fn task_register_memory_forms_address_override_segment_override_and_word_guards() {
    for big in [false, true] {
        for address32 in [false, true] {
            for seg in [DS, SS, 4, 5] {
                let prefix = [0x3e, 0x36, 0x64, 0x65]
                    [[DS, SS, 4, 5].iter().position(|s| *s == seg).unwrap()];
                let mut code = vec![prefix, 0x66];
                if big != address32 {
                    code.push(0x67);
                }
                code.extend([0x0f, 0x00, if address32 { 0x1d } else { 0x1e }]);
                if address32 {
                    code.extend(0x10101u32.to_le_bytes())
                } else {
                    code.extend(0x101u16.to_le_bytes())
                }
                let mut store = code.clone();
                let m = store.iter().position(|v| *v == 0x0f).unwrap() + 2;
                store[m] -= 0x10; // /3 -> /1; identical effective address.
                code.extend(store);
                let (mut c, mut b) = protected(&code, big, true);
                descriptor(&mut b, 32, 0x31000, 43, 0x81, 0);
                let off = if address32 { 0x10101 } else { 0x101 };
                let addr = (c.segment_cache(seg).base + off) as usize;
                b.mem[addr - 1..addr + 3].copy_from_slice(&[0xa5, 0x20, 0, 0xa5]);
                c.step(&mut b).unwrap();
                assert_eq!(c.tr, 0x20);
                b.mem[addr..addr + 2].fill(0xff);
                c.step(&mut b).unwrap();
                assert_eq!(&b.mem[addr - 1..addr + 3], &[0xa5, 0x20, 0, 0xa5]);
                assert_eq!(c.last_fault, None);
            }
        }
    }
}

#[test]
fn ltr_selector_type_presence_bounds_fault_codes_and_retry_are_atomic() {
    for (selector, access, table_limit, vector, error) in [
        (0, 0x89, 0x7f, 13, 0),
        (3, 0x89, 0x7f, 13, 0),
        (4, 0x89, 0x7f, 13, 4),
        (0x27, 0x89, 0x7f, 13, 0x24),
        (0x23, 0x89, 0x26, 13, 0x20),
        (0x80, 0x89, 0x7f, 13, 0x80),
        (0x23, 0x83, 0x7f, 13, 0x20),
        (0x23, 0x8b, 0x7f, 13, 0x20),
        (0x23, 0x99, 0x7f, 13, 0x20),
        (0x23, 0x82, 0x7f, 13, 0x20),
        (0x23, 0x85, 0x7f, 13, 0x20),
        (0x23, 0x80, 0x7f, 13, 0x20),
        (0x23, 0x01, 0x7f, 11, 0x20),
        (0x23, 0x69, 0x7f, 11, 0x20),
        (0x23, 0x0b, 0x7f, 13, 0x20), // type/busy check precedes P
    ] {
        let (mut c, mut b) = protected(&[0x66, 0x0f, 0x00, 0xd8], true, true);
        descriptor(&mut b, 32, 0x30000, 103, access, 0);
        c.gdtr_limit = table_limit;
        c.set_reg32(AX, 0xbeef0000 | selector);
        c.tr = 0x38;
        c.tr_cache = Some(SegmentCache::parse(raw(0x32000, 43, 0x83, 0)));
        let old = c.tr_cache;
        let mut memory = b.mem.clone();
        c.step(&mut b).unwrap();
        fault_frame(&c, &b, vector, error);
        assert_eq!((c.tr, c.tr_cache), (0x38, old));
        assert_eq!(c.reg32(AX), 0xbeef0000 | selector);
        memory[0x17ff0..0x18000].copy_from_slice(&b.mem[0x17ff0..0x18000]);
        assert_eq!(b.mem, memory);
    }
    // A guest #NP handler repairs the descriptor, drops its error word and
    // IRETD retries at the first prefix, rather than the emulator skipping LTR.
    let (mut c, mut b) = protected(&[0x66, 0x0f, 0x00, 0xd8], true, true);
    descriptor(&mut b, 32, 0x31000, 103, 0x09, 0);
    c.r[AX] = 0x23;
    // DS has base 20000h: patch the descriptor at linear 5025h through FS=flat SS.
    c.segment_caches[4] = c.segment_caches[SS];
    b.mem[0x12000..0x1200c].copy_from_slice(&[
        0x64, 0x80, 0x0d, 0x25, 0x50, 0, 0, 0x80, // OR byte [FS:5025],80h
        0x83, 0xc4, 4, 0xcf, // ADD ESP,4; IRETD
    ]);
    c.step(&mut b).unwrap();
    fault_frame(&c, &b, 11, 0x20);
    for _ in 0..4 {
        c.step(&mut b).unwrap();
    }
    assert_eq!((c.tr, c.ip, c.reg32(SP)), (0x23, 0x1004, 0x18000));
    assert_eq!(b.mem[0x5025], 0x8b);
}

#[test]
fn task_register_segment_preflight_prevents_partial_words_and_descriptor_updates() {
    for store in [false, true] {
        for seg in [DS, SS] {
            for access in [0x93, 0x91, 0] {
                if seg == SS && access != 0x93 {
                    continue; // A read-only/null SS cannot deliver the fault frame either.
                }
                let code = [
                    if seg == SS { 0x36 } else { 0x3e },
                    0x0f,
                    0x00,
                    if store { 0x0d } else { 0x1d },
                    0xff,
                    0xff,
                    0,
                    0,
                ];
                let (mut c, mut b) = protected(&code, true, true);
                c.tr = 32;
                c.set_reg32(SP, 0x8000);
                c.segment_caches[seg] = Some(SegmentCache::parse(raw(0x30000, 0xffff, access, 0)));
                descriptor(&mut b, 32, 0x31000, 103, 0x89, 0);
                b.mem[0x3ffff] = 0x20;
                let before = b.mem.clone();
                c.step(&mut b).unwrap();
                assert_eq!(c.last_fault, Some((if seg == SS { 12 } else { 13 }, 0)));
                assert_eq!(&b.mem[0x3ffff..0x40003], &before[0x3ffff..0x40003]);
                assert_eq!(b.mem[0x5025], 0x89);
                assert_eq!((c.tr, c.tr_cache), (32, None));
            }
        }
    }
    // Write permission is checked even when the whole STR word fits.
    let (mut c, mut b) = protected(&[0x0f, 0x00, 0x0d, 0, 1, 0, 0], true, true);
    c.segment_caches[DS].as_mut().unwrap().access = 0x91;
    let old = b.read16(0x20100);
    c.step(&mut b).unwrap();
    fault_frame(&c, &b, 13, 0);
    assert_eq!(b.read16(0x20100), old);
}

#[test]
fn ltr_non_ram_tables_and_sources_stop_before_device_reads_or_busy_writes() {
    for base in [0xa2fda, 0xa7fda, 0xeffda, 0x3fffda, 0xffff_ffda] {
        let (mut c, mut b) = protected(&[0x0f, 0x00, 0xd8], true, true);
        c.r[AX] = 0x23;
        c.gdtr_base = base;
        let saved = format!("{c:?}");
        let mem = b.mem.clone();
        seed_stack_operand_egc(&mut b);
        let graphics = b.graphics.clone();
        let error = c.step(&mut b).unwrap_err();
        assert!(error.contains("LTR descriptor bus access"), "{error}");
        assert!(error.contains("opcode=0f 00 modrm=d8"));
        assert!(error.contains("TR=0000 TSS=None"));
        assert_eq!(format!("{c:?}"), saved);
        assert_eq!(b.mem, mem);
        // Seeded graphics are unchanged and no EGC read latch was consumed.
        assert_eq!(b.graphics, graphics);
        assert_stack_operand_egc_latches_unchanged(&mut b);
    }
    let (mut c, mut b) = protected(&[0x0f, 0x00, 0x1d, 0, 0, 0, 0], true, true);
    c.segment_caches[DS].as_mut().unwrap().base = 0xa8000;
    seed_stack_operand_egc(&mut b);
    let memory = b.mem.clone();
    let saved = format!("{c:?}");
    assert!(c
        .step(&mut b)
        .unwrap_err()
        .contains("LTR source bus access"));
    assert_eq!(format!("{c:?}"), saved);
    assert_eq!(b.mem, memory);
    assert_stack_operand_egc_latches_unchanged(&mut b);
}

#[test]
fn task_register_reset_mode_and_lock_invalid_encoding_boundaries() {
    for code in [
        vec![0xf0, 0x0f, 0x00, 0xd8],
        vec![0xf0, 0x0f, 0x00, 0xc8],
        vec![0x0f, 0x00, 0xf0],
        vec![0x0f, 0x00, 0xf8],
    ] {
        let (mut c, mut b) = protected(&code, true, true);
        c.tr = 0x20;
        c.step(&mut b).unwrap();
        assert_eq!(c.last_fault, Some((6, 0)));
        assert_eq!(b.read16(0x17ff4), 0x1000); // #UD has no error code
        assert_eq!(c.tr, 0x20);
    }
    // Nonzero CPL execution remains outside the core's supported subset.
    let (mut c, mut b) = protected(&[0x0f, 0x00, 0xd8], true, true);
    c.cpl = 3;
    assert!(c.step(&mut b).unwrap_err().contains("only ring 0"));
    for m in [0xc8, 0xd8] {
        let mut c = Cpu {
            s: [0, 0x1000, 0, 0],
            flags: 2,
            ..Cpu::default()
        };
        let mut b = Bus::new(Profile::Pc9821ApDos);
        b.mem[0x10000..0x10003].copy_from_slice(&[0x0f, 0x00, m]);
        c.step(&mut b).unwrap();
        assert_eq!((c.s[CS], c.ip), (0, 0)); // real-mode #UD vector
        assert_eq!((c.tr, c.tr_cache), (0, None));
    }
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu.tr = 0x20;
    m.cpu.tr_cache = Some(SegmentCache::parse(raw(0x9000, 103, 0x8b, 0)));
    m.reset();
    assert_eq!((m.cpu.tr, m.cpu.tr_cache), (0, None));
}

#[test]
fn loaded_tr_does_not_enable_hardware_tasks_privilege_switches_or_nt_return() {
    for (code, access, name) in [
        (
            vec![0x9a, 0, 0, 0, 0, 0x20, 0],
            0x89,
            "hardware task switch",
        ),
        (
            vec![0xea, 0, 0, 0, 0, 0x20, 0],
            0x81,
            "hardware task switch",
        ),
        (vec![0x9a, 0, 0, 0, 0, 0x20, 0], 0x85, "task gate transfer"),
        (
            vec![0x9a, 0, 0, 0, 0, 0x20, 0],
            0x8c,
            "call gate/privilege-stack transfer",
        ),
        (vec![0xcf], 0x89, "IRET.NT hardware task return"),
        (vec![0xcd, 0x30], 0x89, "IDT task gate"),
    ] {
        let mut setup = vec![0x0f, 0x00, 0xd8];
        setup.extend(code);
        let (mut c, mut b) = protected(&setup, true, true);
        descriptor(&mut b, 0x28, 0x32000, 103, 0x89, 0);
        c.r[AX] = 0x28;
        c.step(&mut b).unwrap();
        descriptor(&mut b, 0x20, 0x31000, 103, access, 0);
        gate(&mut b, 0x30, 0x20, 0, 0x85);
        if name.contains("IRET.NT") {
            c.flags |= 0x4000;
            c.set_reg32(SP, 0xffff_ffff); // NT must stop before reading any normal return frame.
        }
        let saved = format!("{c:?}");
        let memory = b.mem.clone();
        let error = c.step(&mut b).unwrap_err();
        assert!(error.contains(name), "{error}");
        assert!(error.contains("TR=0028 TSS=Some"), "{error}");
        assert_eq!(format!("{c:?}"), saved);
        assert_eq!(b.mem, memory);
    }
}

#[test]
fn original_task_register_boot_guest_cold_reset_results_guards_and_cache() {
    let image = std::fs::read("fixtures/task_register/task_register.hdm")
        .expect("run python3 tools/prepare_task_register_fixture.py");
    let expected = [
        0xa5, 0x28, 0, 0x16, 0xa5, 0x28, 0, 0xa5, 0xa5, 0x33, 0, 0x33, 0, 0xa5, 0xa5, 0x33, 0,
        0x32, 0xa5, 0x30, 0, 0x30, 0, 0xa5, 0xa5, 0x30, 0, 0x46,
    ];
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "task_register.hdm").unwrap();
    for run in 0..2 {
        if run != 0 {
            m.reset();
        }
        assert_eq!((m.cpu.tr, m.cpu.tr_cache), (0, None));
        m.step(100_000);
        assert!(m.error.is_none(), "{:?}", m.error);
        assert!(
            m.bus.text().contains("TASK REGISTER PASS"),
            "{}",
            m.bus.text()
        );
        assert!(!m.bus.text().contains("FAIL"));
        assert_eq!(&m.bus.mem[0x8000..0x801c], &expected);
        assert_eq!(&m.bus.mem[0x801c..0x8040], &[0xa5; 36]);
        assert_eq!(&m.bus.mem[0x8100..0x8268], &[0; 0x168]);
        assert_eq!(m.cpu.tr, 0x30);
        let d = m.cpu.tr_cache.unwrap();
        assert_eq!((d.base, d.limit, d.access), (0x8200, 103, 0xeb));
        assert!(!m.cpu.protected_mode());
        assert!(m.bus.unknown_ports.is_empty());
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
    }
}

#[test]
fn task_register_gdt_and_memory_operands_use_pc98_a20_bus_decode() {
    for a20 in [false, true] {
        let (mut c, mut b) = protected(
            &[0x0f, 0x00, 0x1d, 0, 1, 0, 0, 0x0f, 0x00, 0xc8],
            true,
            true,
        );
        descriptor(&mut b, 32, 0xfedcba98, 103, 0x89, 0);
        let table = b.mem[0x5000..0x5080].to_vec();
        b.mem[0x105000..0x105080].copy_from_slice(&table);
        // Distinguish high RAM from its A20 alias.
        b.mem[0x105025] = 0x81;
        c.gdtr_base = 0x105000;
        c.gdtr_limit = 0x27; // exact final descriptor byte fits
        c.segment_caches[DS].as_mut().unwrap().base = 0x120000;
        b.mem[0x20100..0x20102].copy_from_slice(&0x23u16.to_le_bytes());
        b.mem[0x120100..0x120102].copy_from_slice(&0x20u16.to_le_bytes());
        b.a20 = a20;
        c.step(&mut b).unwrap();
        assert_eq!(c.tr, if a20 { 0x20 } else { 0x23 });
        assert_eq!(c.tr_cache.unwrap().base, 0xfedcba98); // cache is not A20-masked
        assert_eq!(c.tr_cache.unwrap().access, if a20 { 0x83 } else { 0x8b });
        assert_eq!(b.mem[0x5025], if a20 { 0x89 } else { 0x8b });
        assert_eq!(b.mem[0x105025], if a20 { 0x83 } else { 0x81 });
        c.step(&mut b).unwrap();
        assert_eq!(c.r[AX], c.tr);
    }
}

#[test]
fn str_before_initialization_and_after_gdt_edits_never_reloads_descriptor() {
    let (mut c, mut b) = protected(&[0x0f, 0x00, 0xc8], true, true);
    c.gdtr_base = 0xa8000; // querying reset TR must not read EGC or even the GDT
    c.gdtr_limit = 0;
    c.set_reg32(AX, 0xbeefffff);
    seed_stack_operand_egc(&mut b);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0xbeef0000);
    assert_eq!(c.tr_cache, None);
    assert_stack_operand_egc_latches_unchanged(&mut b);
}

#[test]
fn ltr_accepts_readonly_rom_source_and_preflights_truncated_decoding() {
    let (mut c, mut b) = protected(&[0x0f, 0x00, 0x1d, 0, 1, 0, 0], true, true);
    c.segment_caches[DS].as_mut().unwrap().base = 0xf0000;
    descriptor(&mut b, 32, 0x31000, 103, 0x89, 0);
    b.mem[0xf0100..0xf0102].copy_from_slice(&0x20u16.to_le_bytes());
    c.step(&mut b).unwrap();
    assert_eq!(c.tr, 0x20);
    for last_byte in [0x1001, 0x1004] {
        let (mut c, mut b) = protected(&[0x0f, 0x00, 0x1d, 0, 0, 0, 0], true, true);
        c.segment_caches[DS].as_mut().unwrap().base = 0xa8000;
        c.segment_caches[CS].as_mut().unwrap().limit = last_byte;
        seed_stack_operand_egc(&mut b);
        c.step(&mut b).unwrap();
        fault_frame(&c, &b, 13, 0);
        assert_eq!((c.tr, c.tr_cache), (0, None));
        assert_stack_operand_egc_latches_unchanged(&mut b);
    }
}

#[test]
fn tss_stack_switch_interrupt_boundary_precedes_memory() {
    let (mut c, mut b) = protected(&[0x90], true, true);
    descriptor(&mut b, 32, 0x10000, 0x3ffff, 0x9b, 0x40);
    // A pending hardware event can be delivered before the general CPL0 guard.
    // The external-interrupt API checks that same protected gate routine.
    gate(&mut b, 0x30, 32, 0x2000, 0xee);
    c.cpl = 3;
    c.tr = 0x28;
    let saved = format!("{c:?}");
    let memory = b.mem.clone();
    let error = c.interrupt(&mut b, 0x30).unwrap_err();
    assert!(error.contains("privilege/TSS stack switch"), "{error}");
    assert_eq!(format!("{c:?}"), saved);
    assert_eq!(b.mem, memory);
}

// Original i486 VM86 instruction/descriptor probes. SPDX-License-Identifier: MIT
use pc98_rust::{bus::Bus, cpu::*, Machine, Profile};
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
fn frame(c: &Cpu, b: &Bus, n: u8, error: Option<u32>, ip: u32, flags: u32) {
    assert!(!c.vm86());
    assert_eq!(c.cpl, 0);
    assert_eq!((c.s[CS], c.ip), (8, 0x11000));
    assert_eq!(c.s, [0, 8, 16, 0]);
    assert_eq!(c.extra_segments, [0, 0]);
    let start = 0x18000 - 36 - if error.is_some() { 4 } else { 0 };
    assert_eq!(c.reg32(SP), start as u32);
    let mut expected = vec![
        ip, 0x2000, flags, 0xabcd7000, 0x3000, 0x4000, 0x5000, 0x6000, 0x7000,
    ];
    if let Some(e) = error {
        assert_eq!(c.last_fault, Some((n, e)));
        expected.insert(0, e);
    }
    for (i, v) in expected.iter().enumerate() {
        assert_eq!(get(b, start + 4 * i), *v, "frame slot {i}");
    }
}
#[test]
fn entry_restores_every_segment_and_executes_overrides_with_and_without_nonidentity_paging() {
    for pg in [false, true] {
        // MOV EAX,12345678; a32 MOV [DS:00000010],EAX; MOV FS,AX; far JMP.
        let code = [
            0x66, 0xb8, 0x78, 0x56, 0x34, 0x12, 0x67, 0x66, 0xa3, 0x10, 0, 0, 0, 0xb8, 0x11, 0x11,
            0x8e, 0xe0, 0xea, 0x20, 0, 0x34, 0x12,
        ];
        let (mut c, mut b) = setup(&code, pg, 0x23002);
        enter(&mut c, &mut b);
        assert_eq!(c.s, [0x4000, 0x2000, 0x3000, 0x5000]);
        assert_eq!(c.extra_segments, [0x6000, 0x7000]);
        assert_eq!(c.reg32(SP), 0xabcd7000);
        assert_eq!(c.eflags(), 0x23002);
        for s in 0..6 {
            assert_eq!(c.segment_cache(s).limit, 0xffff);
            assert!(!c.segment_cache(s).big());
        }
        c.step(&mut b).unwrap();
        c.step(&mut b).unwrap();
        assert_eq!(get(&b, if pg { 0x310010 } else { 0x50010 }), 0x12345678);
        for _ in 0..3 {
            c.step(&mut b).unwrap();
        }
        assert_eq!((c.s[CS], c.ip), (0x1234, 0x20));
        assert_eq!(c.cpl, 3);
        assert_eq!(c.extra_segments[0], 0x1111);
        assert_eq!(c.segment_cache(4).base, 0x11110);
        if pg {
            assert_eq!(get(&b, 0x2140) & 0x60, 0x60);
            assert_eq!(get(&b, 0x2080) & 0x60, 0x20);
        }
    }
}
#[test]
fn cs_override_store_and_rmw_work_with_and_without_nonidentity_paging() {
    for pg in [false, true] {
        // MOV byte [CS:0100h],90h; INC byte [CS:0100h].
        let code = [0x2e, 0xc6, 0x06, 0, 1, 0x90, 0x2e, 0xfe, 0x06, 0, 1];
        let (mut c, mut b) = setup(&code, pg, 0x23002);
        let target = if pg { 0x300100 } else { 0x20100 };
        b.mem[target - 1..target + 2].copy_from_slice(&[0x5a, 0x12, 0xa5]);
        enter(&mut c, &mut b);
        for (ip, value) in [(6, 0x90), (11, 0x91)] {
            c.step(&mut b).unwrap();
            assert!(c.vm86());
            assert_eq!(c.cpl, 3);
            assert_eq!(c.ip, ip);
            assert_eq!(c.last_fault, None);
            assert_eq!(&b.mem[target - 1..target + 2], &[0x5a, value, 0xa5]);
        }
        if pg {
            assert_eq!(get(&b, 0x2080) & 0x60, 0x60); // CS page accessed/dirty
            assert_eq!(b.mem[0x20100], 0); // no write to the identity address
        }
    }
}

#[test]
fn cs_override_writes_retain_user_paging_permissions_and_fault_atomically() {
    for code in [
        vec![0x2e, 0xc6, 0x06, 0, 0x11, 0x90], // MOV byte [CS:1100h],90h
        vec![0x2e, 0xfe, 0x06, 0, 0x11],       // INC byte [CS:1100h]
    ] {
        for (permissions, error) in [(5, 7), (3, 7)] {
            let (mut c, mut b) = setup(&code, true, 0x23002);
            // A separate target page keeps instruction fetch user-accessible.
            put(&mut b, 0x2084, 0x330000 | permissions);
            b.mem[0x3300ff..0x330102].copy_from_slice(&[0x5a, 0x12, 0xa5]);
            enter(&mut c, &mut b);
            c.step(&mut b).unwrap();
            frame(&c, &b, 14, Some(error), 0, 0x33002);
            assert_eq!(c.cr2, 0x21100);
            assert_eq!(&b.mem[0x3300ff..0x330102], &[0x5a, 0x12, 0xa5]);
            assert_eq!(get(&b, 0x2084) & 0x40, 0); // denied write never sets D
        }
    }
}

#[test]
fn cs_override_writes_retain_vm86_segment_bounds() {
    for pg in [false, true] {
        for code in [
            // MOV dword [CS:fffeh],12345678h spans the 64 KiB limit.
            vec![0x2e, 0x66, 0xc7, 0x06, 0xfe, 0xff, 0x78, 0x56, 0x34, 0x12],
            // a32 INC byte [CS:00010000h] starts beyond the limit.
            vec![0x2e, 0x67, 0xfe, 0x05, 0, 0, 1, 0],
        ] {
            let (mut c, mut b) = setup(&code, pg, 0x23002);
            b.mem[0x2fffc..0x30004].fill(0xa5);
            enter(&mut c, &mut b);
            c.step(&mut b).unwrap();
            frame(&c, &b, 13, Some(0), 0, 0x33002);
            assert_eq!(&b.mem[0x2fffc..0x30004], &[0xa5; 8]);
        }
    }
}

#[test]
fn system_store_limit_faults_use_gp_in_vm86_and_ss_in_protected_mode() {
    // Intel486 PRM instruction exception tables: these stores use #GP for
    // VM86 bounds violations even through SS; protected mode retains #SS.
    for (opcode, extension, width) in [
        (&[0x0f, 0x01][..], 0, 6), // SGDT
        (&[0x0f, 0x01][..], 1, 6), // SIDT
        (&[0x0f, 0x01][..], 4, 2), // SMSW
        (&[0xd9][..], 7, 2),       // FNSTCW
        (&[0xdd][..], 7, 2),       // FNSTSW
    ] {
        for pg in [false, true] {
            for virtual_mode in [false, true] {
                for operand32 in [false, true] {
                    for address32 in [false, true] {
                        let mut code = vec![0x36]; // explicit SS override
                        if operand32 {
                            code.push(0x66);
                        }
                        if address32 {
                            code.push(0x67);
                        }
                        code.extend_from_slice(opcode);
                        // [BP+0] / [EBP+0], so the monitor can repair and retry.
                        code.extend_from_slice(&[
                            (if address32 { 0x45 } else { 0x46 }) | extension << 3,
                            0,
                        ]);
                        let (mut c, mut b) = setup(&code, pg, 0x23002);
                        if virtual_mode {
                            enter(&mut c, &mut b);
                        } else {
                            c.ip = 0x12000;
                            b.mem[0x12000..0x12000 + code.len()].copy_from_slice(&code);
                            c.segment_caches[CS].as_mut().unwrap().attributes = 0;
                            c.segment_caches[SS].as_mut().unwrap().limit = 0xffff;
                            c.set_reg32(SP, 0xf000);
                        }
                        c.set_reg32(BP, 0x10001 - width); // FFFBh / FFFFh
                        let target = if virtual_mode { 0x3fff0 } else { 0xfff0 };
                        b.mem[target..target + 32].fill(0xa5);
                        b.egc.enabled = true;
                        b.grcg_mode = 0x80;
                        let saved = c.clone();
                        let egc = b.egc.clone();
                        let graphics = b.graphics.clone();
                        c.step(&mut b).unwrap();
                        let vector = if virtual_mode { 13 } else { 12 };
                        assert_eq!(c.last_fault, Some((vector, 0)), "{code:02x?} PG={pg}");
                        if virtual_mode {
                            frame(&c, &b, 13, Some(0), 0, 0x33002);
                        } else {
                            assert_eq!(c.reg32(SP), 0xeff0);
                            for (i, value) in [0, 0x12000, 8, 0x10002].into_iter().enumerate() {
                                assert_eq!(get(&b, 0xeff0 + 4 * i), value);
                            }
                        }
                        assert_eq!(&b.mem[target..target + 32], &[0xa5; 32]);
                        assert_eq!(b.egc, egc);
                        assert_eq!(b.graphics, graphics);
                        assert_eq!(b.grcg_mode, 0x80);
                        assert_eq!(c.fpu, saved.fpu);
                        assert_eq!(c.cr2, saved.cr2);
                        for r in 0..8 {
                            if r != SP {
                                assert_eq!(c.reg32(r), saved.reg32(r));
                            }
                        }
                        if virtual_mode {
                            // Guest monitor drops the error and IRETD restores
                            // the faulting EIP; a repaired operand then succeeds.
                            b.mem[0x11000..0x11004].copy_from_slice(&[0x83, 0xc4, 4, 0xcf]);
                            c.set_reg32(BP, 0x4000);
                            c.step(&mut b).unwrap();
                            c.step(&mut b).unwrap();
                            assert!(c.vm86());
                            assert_eq!(c.ip, 0);
                            c.step(&mut b).unwrap();
                            assert!(c.vm86());
                            assert_eq!(c.ip, code.len() as u32);
                            let expected = match (opcode, extension) {
                                ([0x0f, 0x01], 0) => vec![31, 0, 0, 0x50, 0, 0],
                                ([0x0f, 0x01], 1) => vec![0xff, 7, 0, 0x60, 0, 0],
                                ([0x0f, 0x01], 4) => (saved.cr0 as u16).to_le_bytes().to_vec(),
                                ([0xd9], _) => saved.fpu.control.to_le_bytes().to_vec(),
                                _ => saved.fpu.status.to_le_bytes().to_vec(),
                            };
                            assert_eq!(&b.mem[0x34000..0x34000 + width as usize], expected);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn interrupt_trap_and_error_frames_run_guest_monitor_then_iretd_back() {
    for (code, source, access, err, ip) in [
        (vec![0xcd, 0x30], false, 0xef, None, 2),
        (vec![0x90], true, 0x8e, None, 0),
        (vec![0xfa], false, 0x8e, Some(0), 0),
    ] {
        let flags = if err.is_some() { 0x20302 } else { 0x23302 }; // TF deliberately suppressed on transfer
        let (mut c, mut b) = setup(&code, false, flags);
        gate(&mut b, if err.is_some() { 13 } else { 0x30 }, access);
        enter(&mut c, &mut b);
        if source {
            c.interrupt(&mut b, 0x30).unwrap();
        } else {
            c.step(&mut b).unwrap();
        }
        frame(
            &c,
            &b,
            13,
            err,
            ip,
            flags | if err.is_some() { 0x10000 } else { 0 },
        );
        assert_eq!(c.flags & IF, if access & 1 != 0 { IF } else { 0 });
        assert_eq!(c.flags & TF, 0);
        // Actual handler writes a result through SS, drops an error if present,
        // clears stacked TF, and returns through the shared interpreter.
        let handler = if err.is_some() {
            vec![
                0x83, 0xc4, 4, 0x36, 0x81, 0x64, 0x24, 8, 0xff, 0xfe, 0xff, 0xff, 0xcf,
            ]
        } else {
            vec![0x36, 0x81, 0x64, 0x24, 8, 0xff, 0xfe, 0xff, 0xff, 0xcf]
        };
        b.mem[0x11000..0x11000 + handler.len()].copy_from_slice(&handler);
        for _ in 0..if err.is_some() { 3 } else { 2 } {
            c.step(&mut b).unwrap();
        }
        assert!(c.vm86());
        assert_eq!(c.cpl, 3);
        assert_eq!(c.ip, ip);
        assert_eq!(c.reg32(SP), 0xabcd7000);
    }
}
#[test]
fn invalid_gate_system_targets_deliver_gp_and_run_guest_handler() {
    for pg in [false, true] {
        for external in [false, true] {
            for gate_access in [0xee, 0xef] {
                // Available/busy 16/32-bit TSS, call gates and task gates.
                for kind in [9, 1, 3, 4, 5, 11, 12] {
                    for present in [0, 0x80] {
                        let (mut c, mut b) = setup(&[0xcd, 0x30, 0x90], pg, 0x23202);
                        c.gdtr_limit = 39;
                        desc(&mut b, 32, present | kind);
                        gate(&mut b, 0x30, gate_access);
                        // RPL must be removed from the selector error code.
                        b.mem[0x6182] = 35;
                        enter(&mut c, &mut b);
                        let target = b.mem[0x5020..0x5028].to_vec();
                        let egc = b.egc.clone();
                        let graphics = b.graphics.clone();
                        let a20 = b.a20;
                        let result = if external {
                            c.interrupt(&mut b, 0x30)
                        } else {
                            c.step(&mut b)
                        };
                        assert!(
                            result.is_ok(),
                            "pg={pg} external={external} gate={gate_access:02x} target={:02x}: {result:?}",
                            present | kind
                        );
                        frame(&c, &b, 13, Some(32 | u32::from(external)), 0, 0x33202);
                        assert!(!c.shutdown);
                        assert_eq!(&b.mem[0x5020..0x5028], target);
                        // Only the #GP frame may occupy the monitor stack.
                        assert!(b.mem[0x17fd0..0x17fd8].iter().all(|&v| v == 0));
                        assert_eq!(b.egc, egc);
                        assert_eq!(b.graphics, graphics);
                        assert_eq!(b.a20, a20);
                        // Original guest handler writes a guard, advances the
                        // faulting IP past INT, drops the error, then IRETD.
                        let handler = [
                            0x36, 0xc7, 0x05, 0, 0x90, 1, 0, 0x78, 0x56, 0x34, 0x12, 0x36, 0x83,
                            0x44, 0x24, 4, 2, 0x83, 0xc4, 4, 0xcf,
                        ];
                        b.mem[0x11000..0x11000 + handler.len()].copy_from_slice(&handler);
                        for _ in 0..4 {
                            c.step(&mut b).unwrap();
                        }
                        assert_eq!(get(&b, 0x19000), 0x12345678);
                        assert!(c.vm86());
                        assert_eq!(c.cpl, 3);
                        assert_eq!((c.s[CS], c.ip), (0x2000, 2));
                        assert_eq!(c.reg32(SP), 0xabcd7000);
                        c.step(&mut b).unwrap();
                        assert_eq!(c.ip, 3);
                    }
                }
            }
        }
    }
}
#[test]
fn vm86_privileged_instructions_fault_before_operands_or_devices() {
    for code in [
        vec![0xfa],
        vec![0xfb],
        vec![0x9c],
        vec![0x9d],
        vec![0xcf],
        vec![0xcd, 0x30],
        vec![0xf4],
        vec![0x0f, 0x20, 0xc0],
        vec![0x0f, 0x22, 0xc0],
        vec![0x0f, 0x06],
        vec![0x0f, 0x21, 0xc0],
        vec![0x0f, 0x23, 0xc0],
        vec![0x0f, 0x24, 0xd8],
        vec![0x0f, 0x26, 0xd8],
        vec![0x0f, 0x08],
        vec![0x0f, 0x09],
        vec![0x0f, 0x01, 0x16, 0, 0],
        vec![0x0f, 0x01, 0x3e, 0, 0],
    ] {
        let (mut c, mut b) = setup(&code, false, 0x20002);
        enter(&mut c, &mut b);
        let cr0 = c.cr0;
        let fpu = c.fpu;
        let a20 = b.a20;
        c.step(&mut b).unwrap();
        frame(&c, &b, 13, Some(0), 0, 0x30002);
        assert_eq!(c.cr0, cr0);
        assert_eq!(c.fpu, fpu);
        assert_eq!(b.a20, a20);
    }
}
#[test]
fn vm86_flags_iopl3_and_local_iret_cannot_clear_vm_or_change_iopl() {
    for pg in [false, true] {
        let (mut c, mut b) = setup(
            &[
                0x66, 0x9c, 0x66, 0x58, 0x66, 0x68, 2, 0, 4, 0, 0x66, 0x9d, 0xfa, 0xfb, 0x90,
            ],
            pg,
            0x63002,
        );
        enter(&mut c, &mut b);
        for _ in 0..4 {
            c.step(&mut b).unwrap();
        }
        assert_eq!(c.reg32(AX), 0x43002);
        assert_eq!(c.eflags(), 0x63002);
        for _ in 0..3 {
            c.step(&mut b).unwrap();
        }
        assert_ne!(c.flags & IF, 0);
        // local IRETD restores an 8086-style CS and ignores popped VM/IOPL.
        let a = if pg { 0x300000 } else { 0x20000 };
        b.mem[a + 15..a + 17].copy_from_slice(&[0x66, 0xcf]);
        c.ip = 15;
        c.r[SP] = 0x7100;
        let a = if pg { 0x320100 } else { 0x37100 };
        for (i, v) in [0x20, 0x2222, 0x40202].into_iter().enumerate() {
            put(&mut b, a + 4 * i, v);
        }
        c.step(&mut b).unwrap();
        assert!(c.vm86());
        assert_eq!(c.cpl, 3);
        assert_eq!(c.s[CS], 0x2222);
        assert_eq!(c.ip, 0x20);
        assert_eq!(c.eflags(), 0x63202);
    }
}
#[test]
fn vm86_bounds_and_user_page_fault_error_codes_and_restart() {
    for (code, pg, page, error, cr2) in [
        (vec![0x67, 0xa1, 0xff, 0xff, 0, 0], false, 0, 0, 0),
        (vec![0x67, 0xa1, 0, 0, 1, 0], false, 0, 0, 0),
        (vec![0x66, 0xa1, 0x10, 0], true, 0x310003, 5, 0x50010),
        (vec![0x66, 0xa3, 0x10, 0], true, 0x310005, 7, 0x50010),
        (vec![0x66, 0xa1, 0x10, 0], true, 0, 4, 0x50010),
    ] {
        let (mut c, mut b) = setup(&code, pg, 0x23002);
        if pg {
            put(&mut b, 0x2140, page);
        }
        enter(&mut c, &mut b);
        c.step(&mut b).unwrap();
        frame(&c, &b, if pg { 14 } else { 13 }, Some(error), 0, 0x33002);
        assert_eq!(c.cr2, cr2);
        if pg {
            // Guest CPL0 handler repairs PTE, invalidates, drops error, IRETD.
            let handler = [
                0x36, 0xc7, 0x05, 0x40, 0x21, 0, 0, 7, 0, 0x31, 0, 0x0f, 0x01, 0x3d, 0x10, 0, 5, 0,
                0x83, 0xc4, 4, 0xcf,
            ];
            b.mem[0x11000..0x11000 + handler.len()].copy_from_slice(&handler);
            for _ in 0..5 {
                c.step(&mut b).unwrap();
            }
            assert!(c.vm86());
            assert_eq!(c.ip, 4);
            assert_eq!(get(&b, 0x2140) & 0x20, 0x20);
        }
    }
}
#[test]
fn io_bitmap_always_applies_at_iopl3_widths_boundaries_and_no_denied_side_effects() {
    for width in [1, 2, 4] {
        for port in [0xf2u16, 7, 0xfffd, 0xfffe, 0xffff] {
            for denied in [false, true] {
                let mut code = if width == 4 { vec![0x66] } else { vec![] };
                code.push(if width == 1 { 0xee } else { 0xef });
                let (mut c, mut b) = setup(&code, false, 0x23002);
                enter(&mut c, &mut b);
                c.r[DX] = port;
                c.set_reg32(AX, 0x01010101);
                // Clear sentinel only to explicitly test wrapped bus cycles;
                // production software is required to retain an FF sentinel.
                b.mem[0xa080] = if denied { 255 } else { 0 };
                if denied {
                    let bit = port as usize + width as usize - 1;
                    b.mem[0x8080 + bit / 8] |= 1 << (bit & 7);
                }
                let before = b.unknown_ports.clone();
                let a20 = b.a20;
                c.step(&mut b).unwrap();
                if denied {
                    frame(&c, &b, 13, Some(0), 0, 0x33002);
                    assert_eq!(b.unknown_ports, before);
                    assert_eq!(b.a20, a20);
                } else {
                    assert!(c.vm86());
                    assert_eq!(c.ip, if width == 4 { 2 } else { 1 });
                }
            }
        }
    }
}
#[test]
fn io_denial_precedes_string_memory_and_input_read_latches() {
    for code in [
        vec![0xec],
        vec![0xed],
        vec![0x66, 0xed],
        vec![0x6c],
        vec![0x6d],
        vec![0x66, 0x6d],
        vec![0x6e],
        vec![0x6f],
        vec![0x66, 0x6f],
        vec![0xf3, 0x6c],
        vec![0xf3, 0x6e],
    ] {
        let (mut c, mut b) = setup(&code, true, 0x20002);
        enter(&mut c, &mut b);
        c.r[DX] = 0x11;
        c.r[CX] = 2;
        c.r[SI] = 0;
        c.r[DI] = 0;
        b.mem[0x8082] |= 2; // DMA status, read-to-clear
        put(&mut b, 0x2100, 0);
        put(&mut b, 0x2140, 0); // string memory absent
        b.dma.status = 4;
        let dma = b.dma.status;
        let mem = b.mem[0x40000..0x40010].to_vec();
        c.step(&mut b).unwrap();
        frame(&c, &b, 13, Some(0), 0, 0x30002);
        assert_eq!(b.dma.status, dma);
        assert_eq!(b.mem[0x40000..0x40010], mem);
    }
}
#[test]
fn tss_bitmap_missing_short_16bit_and_sentinel_faults() {
    for case in 0..5 {
        let (mut c, mut b) = setup(&[0xec], false, 0x23002);
        enter(&mut c, &mut b);
        c.r[DX] = 0xffff;
        // Keep valid SS0 for exception delivery; the map itself is unavailable.
        match case {
            0 => c.tr_cache.as_mut().unwrap().limit = 102,
            1 => b.mem[0x8066..0x8068].copy_from_slice(&0xffffu16.to_le_bytes()),
            2 => c.tr_cache.as_mut().unwrap().limit = 0x207f,
            3 => b.mem[0xa07f] = 128,
            _ => b.mem[0x8066..0x8068].copy_from_slice(&0x2080u16.to_le_bytes()),
        }
        c.step(&mut b).unwrap();
        frame(&c, &b, 13, Some(0), 0, 0x33002);
    }
}
#[test]
fn invalid_tss_stack_gate_shutdown_is_bounded_and_frame_memory_is_atomic() {
    for case in 0..10 {
        let (mut c, mut b) = setup(&[0xcd, 0x30], false, 0x23002);
        enter(&mut c, &mut b);
        match case {
            0 => c.tr_cache = None,
            1 => c.tr_cache.as_mut().unwrap().access = 0x83,
            2 => c.tr_cache.as_mut().unwrap().limit = 8,
            3 => put(&mut b, 0x8008, 0),
            4 => put(&mut b, 0x8008, 19),
            5 => desc(&mut b, 16, 0x91),
            6 => desc(&mut b, 16, 0x73),
            7 => put(&mut b, 0x8004, 2),
            8 => {
                for n in [0x30, 13, 8] {
                    gate(&mut b, n, 0x86)
                }
            }
            _ => desc(&mut b, 8, 0x9f),
        }
        let memory = b.mem.clone();
        let egc = b.egc.clone();
        let graphics = b.graphics.clone();
        let old = c.clone();
        let e = c.step(&mut b).unwrap_err();
        assert!(e.contains("shutdown"), "{case}: {e}");
        assert!(c.shutdown);
        assert_eq!(b.mem, memory, "case {case}");
        assert_eq!(b.egc, egc);
        assert_eq!(b.graphics, graphics);
        assert_eq!(c.s, old.s);
        assert_eq!(c.ip, old.ip);
        assert_eq!(c.reg32(SP), old.reg32(SP));
        assert!(c.vm86());
    }
}
#[test]
fn original_boot_guest_cold_reset() {
    // Filled by the pinned original guest; never a private DOS/game image.
    let image = std::fs::read("fixtures/vm86/vm86.hdm").expect("run tools/prepare_vm86_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "vm86.hdm").unwrap();
    for run in 0..2 {
        if run != 0 {
            m.reset();
        }
        let mut seen = [false; 2];
        for _ in 0..200 {
            m.step_with_observer(10000, |c, _| {
                if c.vm86() {
                    seen[usize::from(c.paging_enabled())] = true;
                    assert_eq!(c.cpl, 3);
                }
            });
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.cpu.vm86() {
                seen[usize::from(m.cpu.paging_enabled())] = true;
                assert_eq!(m.cpu.cpl, 3);
            }
            if m.bus.text().contains("VM86 PASS") || m.bus.text().contains("VM86 FAIL") {
                break;
            }
        }
        assert!(
            m.bus.text().contains("VM86 PASS"),
            "{} {:?}",
            m.bus.text(),
            m.cpu
        );
        assert_eq!(seen, [true, true]);
        let mut expected = [0xa5; 64];
        expected[..6].copy_from_slice(&[2, 4, 3, 2, 2, 1]);
        for (offset, value) in [
            (8, 0xe0010011u32),
            (12, 0x55000),
            (16, 0x12345678),
            (20, 0x23046),
            (24, 0xabcd7000),
            (28, 0x4000),
            (32, 0x6000),
            (36, 0xa000),
            (40, 0x5000),
            (44, 0x0800),
            (48, 0x9000),
            (52, 2),
            (56, 0),
        ] {
            expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(&m.bus.mem[0x8000..0x8040], expected);
        for a in [0x17f80, 0x18000, 0x66ff0, 0x67000, 0x116ff0, 0x117000] {
            assert_eq!(get(&m.bus, a), 0xa5a5a5a5, "guard {a:x}");
        }
        assert_eq!(m.cpu.cr0, 0xe0010011);
        assert_eq!(m.cpu.cpl, 0);
        assert_eq!(m.cpu.cr2, 0x55000);
        assert_eq!(m.cpu.s, [16, 8, 16, 16]);
        assert!(m.cpu.halted);
        assert!(!m.cpu.shutdown);
        assert_eq!(m.bus.fdc.writes, 0);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
    }
}

#[test]
fn cross_page_instruction_faults_report_user_fetch_and_original_prefix_ip() {
    for prefix_bytes in 1..=3 {
        let (mut c, mut b) = setup(&[], true, 0x23002);
        let start = 0x1000 - prefix_bytes;
        put(&mut b, 0x18000, start);
        put(&mut b, 0x2084, 0); // next VM86 code page absent
        let code = [0x66, 0xb8, 0x78, 0x56, 0x34, 0x12];
        let a = 0x300000 + start as usize;
        b.mem[a..a + code.len()].copy_from_slice(&code);
        enter(&mut c, &mut b);
        c.step(&mut b).unwrap();
        frame(&c, &b, 14, Some(4), start, 0x33002);
        assert_eq!(c.cr2, 0x21000);
    }
}

#[test]
fn permitted_input_and_string_io_use_supervisor_nonidentity_tss_bitmap() {
    for width in [1, 2, 4] {
        for string in [false, true] {
            let mut code = vec![];
            if width == 4 {
                code.push(0x66);
            }
            code.push(if string {
                if width == 1 {
                    0x6c
                } else {
                    0x6d
                }
            } else if width == 1 {
                0xec
            } else {
                0xed
            });
            let (mut c, mut b) = setup(&code, true, 0x20002);
            // TSS and bitmap supervisor pages physically live elsewhere. CPU
            // instruction/operand accesses still use user permissions.
            for i in 0..3 {
                b.mem
                    .copy_within(0x8000 + i * 4096..0x9000 + i * 4096, 0x120000 + i * 4096);
                put(&mut b, 0x2020 + i * 4, (0x120000 + i as u32 * 4096) | 3);
            }
            enter(&mut c, &mut b);
            c.r[DX] = 0x200;
            c.r[DI] = 0x10;
            c.set_reg32(AX, 0x12345678);
            c.step(&mut b).unwrap();
            assert!(c.vm86());
            let mask = if width == 4 {
                u32::MAX
            } else {
                (1u32 << (width * 8)) - 1
            };
            if string {
                assert_eq!(c.r[DI], 0x10 + width as u16);
                assert_eq!(get(&b, 0x40010) & mask, mask);
            } else {
                assert_eq!(c.reg32(AX) & mask, mask);
            }
            for p in 0x200..0x200 + width as u16 {
                assert!(b.unknown_ports.contains(&p));
            }
        }
    }
}

#[test]
fn denied_user_graphics_rmw_and_ins_destination_fault_preserve_device_state() {
    for code in [vec![0x66, 0x83, 0x06, 0, 0, 1], vec![0x6c]] {
        let (mut c, mut b) = setup(&code, true, 0x23002);
        put(&mut b, 0x2140, 0xa8005); // read-only user graphics
        put(&mut b, 0x2100, 0xa8005); // INS destination read-only
        b.egc.enabled = true;
        b.grcg_mode = 0x80;
        enter(&mut c, &mut b);
        c.r[DX] = 0x11;
        c.r[SI] = 0;
        c.r[DI] = 0;
        b.dma.status = 4;
        let egc = b.egc.clone();
        let graphics = b.graphics.clone();
        c.step(&mut b).unwrap();
        frame(&c, &b, 14, Some(7), 0, 0x33002);
        assert_eq!(b.egc, egc);
        assert_eq!(b.graphics, graphics);
        assert_eq!(b.dma.status, 4);
    }
}

#[test]
fn iret_entry_missing_last_frame_page_fault_is_ring0_restart_not_partial_vm86() {
    let (mut c, mut b) = setup(&[], true, 0x23002);
    // Valid header and all but the final selector; only that last dword is
    // unmapped. A separate exception stack area remains mapped below it.
    let values = [
        0u32, 0x2000, 0x23002, 0xabcd7000, 0x3000, 0x4000, 0x5000, 0x6000,
    ];
    for (i, v) in values.into_iter().enumerate() {
        put(&mut b, 0x18fe0 + i * 4, v);
    }
    c.set_reg32(SP, 0x18fe0);
    put(&mut b, 0x2064, 0);
    let caches = c.segment_caches;
    let memory = b.mem[0x18fe0..0x19000].to_vec();
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((14, 0)));
    assert_eq!(c.cr2, 0x19000);
    assert!(!c.vm86());
    assert_eq!(c.cpl, 0);
    assert_eq!(c.segment_caches, caches);
    assert_eq!(c.s, [16, 8, 16, 16]);
    assert_eq!(get(&b, 0x18fd4), 0x10000); // saved restart EIP
    assert_eq!(&b.mem[0x18fe0..0x19000], memory);
}
#[test]
fn permitted_repeated_outs_and_denied_next_iteration_are_restartable() {
    let (mut c, mut b) = setup(&[0xf3, 0x66, 0x6f], true, 0x23002);
    enter(&mut c, &mut b);
    c.r[DX] = 0x200;
    c.r[SI] = 0xffc;
    c.r[CX] = 2;
    put(&mut b, 0x310ffc, 0x12345678);
    put(&mut b, 0x2144, 0); // next source page absent
    c.step(&mut b).unwrap();
    assert!(c.vm86());
    assert_eq!(c.ip, 0);
    assert_eq!(c.r[SI], 0x1000);
    assert_eq!(c.r[CX], 1);
    let ports = b.unknown_ports.clone();
    c.step(&mut b).unwrap();
    frame(&c, &b, 14, Some(4), 0, 0x33002);
    assert_eq!(c.cr2, 0x51000);
    assert_eq!(b.unknown_ports, ports);
    assert_eq!(c.r[SI], 0x1000);
    assert_eq!(c.r[CX], 1);
}

#[test]
fn vm86_gate_and_return_support_16bit_monitor_stack_with_full_esp0() {
    let (mut c, mut b) = setup(&[0xcd, 0x30], false, 0x23002);
    // SS0 has a 16-bit stack pointer, independently of the 32-bit IDT gate.
    b.mem[0x5016] = 0; // descriptor G/D/B clear, limit FFFFh
    put(&mut b, 0x8004, 0xabcd_f000);
    enter(&mut c, &mut b);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(SP), 0xabcd_efdc);
    assert!(!c.segment_cache(SS).big());
    for (i, v) in [
        2, 0x2000, 0x23002, 0xabcd7000, 0x3000, 0x4000, 0x5000, 0x6000, 0x7000,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(get(&b, 0xefdc + 4 * i), v);
    }
    b.mem[0x11000] = 0xcf;
    c.step(&mut b).unwrap();
    assert!(c.vm86());
    assert_eq!(c.ip, 2);
    assert_eq!(c.reg32(SP), 0xabcd7000);
}
#[test]
fn vm86_stack_boundaries_fault_ss_and_do_not_wrap_a_dword_operand() {
    let (mut c, mut b) = setup(&[0x66, 0x50], false, 0x23002);
    put(&mut b, 0x1800c, 0xabcd0001);
    enter(&mut c, &mut b);
    let guard = b.mem[0x3fff0..0x40000].to_vec();
    c.step(&mut b).unwrap();
    assert_eq!(c.last_fault, Some((12, 0)));
    assert_eq!(get(&b, 0x17fe8), 0xabcd0001);
    assert_eq!(&b.mem[0x3fff0..0x40000], guard);
    assert_eq!(c.cpl, 0);
}

#[test]
fn protected_only_and_invalid_system_encodings_in_vm86_raise_ud() {
    for code in [
        vec![0x0f, 0x00, 0xc8],
        vec![0x0f, 0x02, 0xc0],
        vec![0x0f, 0x03, 0xc0],
        vec![0x63, 0xc0],
        vec![0x0f, 0x21, 0],
        vec![0x0f, 0x24, 0xc0],
        vec![0xf0, 0x0f, 0x06],
    ] {
        let (mut c, mut b) = setup(&code, false, 0x23002);
        enter(&mut c, &mut b);
        c.step(&mut b).unwrap();
        assert_eq!(c.last_fault, Some((6, 0)));
        assert_eq!(get(&b, 0x17fdc), 0);
        assert_eq!(get(&b, 0x17fe4), 0x33002);
        assert_eq!(c.cpl, 0);
        assert!(!c.vm86());
    }
}

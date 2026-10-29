// Original byte-level and boot probes. SPDX-License-Identifier: MIT
use pc98_rust::{bus::Bus, cpu::*, Machine, Profile};

fn cpu(code: &[u8], protected: bool, big: bool) -> (Cpu, Bus) {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    let mut c = Cpu {
        s: [0, 0x100, 0, 0],
        ip: 0,
        flags: 2 | CF | PF | AF | ZF | SF | DF | OF,
        flags_upper: 4,
        cr0: 0x60000010 | u32::from(protected),
        gdtr_base: 0x5000,
        gdtr_limit: 0x17,
        idtr_base: if protected { 0x6000 } else { 0 },
        idtr_limit: if protected { 0x7ff } else { 0x3ff },
        tr: 0x28,
        ..Cpu::default()
    };
    c.r = [0x1234; 8];
    c.upper = [0xabcd; 8];
    c.set_reg32(SP, 0x9000);
    c.cr2 = 0x12345678;
    c.cr3 = 0x23456018;
    c.fpu.status = 0x1234;
    c.fpu.tag = 0x5678;
    c.tr_cache = Some(SegmentCache {
        base: 0x7000,
        limit: 103,
        access: 0x8b,
        attributes: 0,
    });
    if protected {
        c.s = [16, 8, 16, 16];
        for s in 0..6 {
            let d = SegmentCache {
                base: if s == CS { 0x1000 } else { 0 },
                limit: 0xffff,
                access: if s == CS { 0x9b } else { 0x93 },
                attributes: if big { 0x40 } else { 0 },
            };
            c.segment_caches[s] = Some(d);
        }
        // Fault delivery reloads CS from this GDT descriptor, base 1000h.
        b.mem[0x5008..0x5010].copy_from_slice(&[
            0xff,
            0xff,
            0,
            0x10,
            0,
            0x9b,
            if big { 0x40 } else { 0 },
            0,
        ]);
    }
    for n in [6, 7, 13] {
        if protected {
            b.mem[0x6000 + n * 8..0x6008 + n * 8].copy_from_slice(&[0, 0x10, 8, 0, 0, 0x8e, 0, 0]);
            // Gate is always 32-bit, independent of interrupted CS.D.
            b.mem[0x2000..0x2002].copy_from_slice(if big { &[0xcf, 0x90] } else { &[0x66, 0xcf] });
        } else {
            b.mem[n * 4..n * 4 + 4].copy_from_slice(&[0, 0x20, 0, 0]);
            b.mem[0x2000] = 0xcf;
        }
    }
    b.mem[0x1000..0x1000 + code.len()].copy_from_slice(code);
    (c, b)
}

#[test]
fn clears_only_ts_full_state_prefixes_sizes_modes_and_boundary() {
    for (pm, big) in [(false, false), (true, false), (true, true)] {
        for bits in [0, 2, 4, 8, 12, 0x5002f] {
            for prefixes in [
                vec![],
                vec![0x66],
                vec![0x67],
                vec![0xf2], // Reserved non-string REP: existing ignore policy.
                vec![0xf3],
                vec![0x26, 0x2e, 0x36, 0x3e, 0x64, 0x65],
                vec![0x66, 0x66, 0x67, 0x67],
                vec![0x66; 13],
            ] {
                let mut code = prefixes.clone();
                code.extend([0x0f, 0x06, 0xb0, 0x5a]); // following MOV must not be consumed
                let (mut c, mut b) = cpu(&code, pm, big);
                c.cr0 |= bits & !1;
                let mut expected = c.clone();
                expected.cr0 &= !8;
                expected.ip = code.len() as u32 - 2;
                expected.retired += 1;
                expected.operand32 = big ^ prefixes.contains(&0x66);
                expected.address32 = big ^ prefixes.contains(&0x67);
                expected.instruction_bytes = expected.ip as u8;
                expected.extended_retired += u64::from(expected.operand32 || expected.address32);
                let memory = b.mem.clone();
                let graphics = b.graphics.clone();
                let registers = b.egc.registers;
                c.step(&mut b).unwrap();
                assert_eq!(format!("{c:?}"), format!("{expected:?}"));
                assert_eq!(b.mem, memory);
                assert_eq!(b.graphics, graphics);
                assert_eq!(b.egc.registers, registers);
                assert_eq!((b.cycles, b.fdc.reads, b.fdc.writes), (0, 0, 0));
                assert!(b.unknown_ports.is_empty());
                c.step(&mut b).unwrap();
                assert_eq!(c.byte(0), 0x5a);
            }
        }
    }
}

#[test]
fn mov_lmsw_smsw_readback_and_pe_stays_set() {
    for pm in [false, true] {
        let (mut c, mut b) = cpu(
            &[
                0x0f, 0x22, 0xc0, // MOV CR0,EAX
                0x0f, 0x06, // CLTS
                0x0f, 0x20, 0xc3, // MOV EBX,CR0
                0x0f, 0x01, 0xe1, // SMSW CX
                0x0f, 0x01, 0xf2, // LMSW DX, set TS/EM, cannot clear PE
                0x0f, 0x06,
            ],
            pm,
            false,
        );
        let value = 0x6005003e | u32::from(pm);
        c.set_reg32(AX, value);
        c.r[DX] = 12;
        for _ in 0..4 {
            c.step(&mut b).unwrap();
        }
        assert_eq!(c.reg32(BX), value & !8);
        assert_eq!(c.reg32(CX), 0xabcd0000 | (value & !8) & 0xffff);
        c.step(&mut b).unwrap();
        assert_eq!(c.cr0 & 15, 12 | u32::from(pm));
        c.step(&mut b).unwrap();
        assert_eq!(c.cr0, (value & !15) | 4 | u32::from(pm));
    }
}

#[test]
fn lock_and_overlength_fault_restart_without_ts_or_operand_mutation() {
    for (pm, big) in [(false, false), (true, false), (true, true)] {
        for (mut code, vector) in [(vec![0x64, 0xf0, 0x0f, 0x06], 6), (vec![0x66; 14], 13)] {
            if vector == 13 {
                code.extend([0x0f, 0x06]);
            }
            let (mut c, mut b) = cpu(&code, pm, big);
            c.cr0 |= 12;
            let saved = c.clone();
            let mut memory = b.mem.clone();
            c.step(&mut b).unwrap();
            let sp = c.reg32(SP) as usize;
            // The only RAM changes are the architectural exception frame.
            memory[sp..0x9000].copy_from_slice(&b.mem[sp..0x9000]);
            assert_eq!(b.mem, memory);
            assert_eq!(c.cr0, saved.cr0);
            assert_eq!(c.fpu, saved.fpu);
            assert_eq!(c.tr_cache, saved.tr_cache);
            assert_eq!(c.tr, saved.tr);
            assert_eq!(c.upper, saved.upper);
            for r in [AX, CX, DX, BX, BP, SI, DI] {
                assert_eq!(c.r[r], saved.r[r]);
            }
            if pm {
                let error_bytes = if vector == 13 { 4 } else { 0 };
                assert_eq!(sp, 0x9000 - 12 - error_bytes);
                assert_eq!(c.last_fault, Some((vector, 0)));
                assert_eq!(
                    &b.mem[sp + error_bytes..sp + error_bytes + 4],
                    &0u32.to_le_bytes()
                );
                assert_eq!(
                    &b.mem[sp + error_bytes + 4..sp + error_bytes + 8],
                    &8u32.to_le_bytes()
                );
                assert_eq!(b.mem[sp + error_bytes + 10] & 1, 1); // RF
                if error_bytes != 0 {
                    c.set_reg32(SP, c.reg32(SP) + 4);
                }
            } else {
                assert_eq!(sp, 0x8ffa);
                assert_eq!(&b.mem[sp..sp + 4], &[0, 0, 0, 1]);
            }
            c.step(&mut b).unwrap(); // IRET(D) to the first prefix
            assert_eq!((c.s[CS], c.ip, c.reg32(SP)), (saved.s[CS], 0, 0x9000));
            assert_eq!(c.flags, saved.flags);
            assert_eq!(c.cr0, saved.cr0);
        }
    }
}

#[test]
fn no_modrm_fetch_at_cs_limit_but_truncated_opcode_faults() {
    for limit in [0, 1] {
        let (mut c, mut b) = cpu(&[0x0f, 0x06], true, false);
        c.segment_caches[CS].as_mut().unwrap().limit = limit;
        c.cr0 |= 8;
        c.step(&mut b).unwrap();
        if limit == 1 {
            assert_eq!((c.ip, c.cr0 & 8), (2, 0));
        } else {
            assert_eq!(c.last_fault, Some((13, 0)));
            assert_eq!(c.cr0 & 8, 8);
        }
    }
}

#[test]
fn wait_mp_ts_matrix_and_pending_exception_boundary() {
    for pm in [false, true] {
        for bits in (0..16).rev() {
            let (mut c, mut b) = cpu(&[0x9b, 0x0f, 0x06, 0x9b], pm, false);
            c.cr0 = 0x10 | (bits & !1) | u32::from(pm);
            c.step(&mut b).unwrap();
            if bits & 10 == 10 {
                assert_eq!(c.ip, if pm { 0x1000 } else { 0x2000 });
                c.step(&mut b).unwrap(); // IRET
                c.ip = 1; // execute CLTS, then WAIT with TS=0
            } else {
                assert_eq!(c.ip, 1);
            }
            c.step(&mut b).unwrap();
            c.step(&mut b).unwrap();
            assert_eq!(c.ip, 4);
        }
        let (mut c, mut b) = cpu(&[0x9b], pm, false);
        c.fpu.status |= 0x80;
        assert!(c
            .step(&mut b)
            .unwrap_err()
            .contains("WAIT pending numeric exception"));
    }
}

#[test]
fn esc_nm_precedes_unsupported_numerics_without_fabricating_results() {
    for pm in [false, true] {
        for op in 0xd8..=0xdf {
            for bits in [4, 8, 12] {
                let (mut c, mut b) = cpu(&[op, 0xc0], pm, false);
                c.cr0 |= bits & !1;
                let fpu = c.fpu;
                c.step(&mut b).unwrap();
                assert_eq!(c.ip, if pm { 0x1000 } else { 0x2000 });
                assert_eq!(c.fpu, fpu);
                c.step(&mut b).unwrap(); // IRET to the same instruction
                c.cr0 &= !12;
                let error = c.step(&mut b).unwrap_err();
                assert!(error.starts_with("unimplemented x87 opcode"), "{error}");
                assert_eq!(c.fpu, fpu);
            }
        }
    }
}

#[test]
fn reset_and_v30_decoder_remain_independent() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu.cr0 |= 12;
    m.reset();
    assert_eq!(m.cpu.cr0, 0x60000010);
    let (mut c, mut b) = cpu(&[0x0f, 0x06], false, false);
    b.profile = Profile::Pc9801Vm;
    c.cr0 |= 8;
    assert!(c.step(&mut b).unwrap_err().contains("V30"));
    assert_eq!(c.cr0 & 8, 8);
}

#[test]
fn original_clts_boot_guest_cold_reset_results_guards_and_media() {
    let image = std::fs::read("fixtures/clts/clts.hdm").expect("run tools/prepare_clts_fixture.py");
    let mut expected = [0xa5; 64];
    for offset in [0, 16] {
        expected[offset + 1..offset + 8].copy_from_slice(&[6, 1, 0xa5, 0x7f, 3, 0, 0]);
    }
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "clts.hdm").unwrap();
    for run in 0..2 {
        if run != 0 {
            m.reset();
        }
        assert_eq!(m.cpu.cr0, 0x60000010);
        m.step(100_000);
        assert!(m.error.is_none(), "{:?}", m.error);
        assert!(m.bus.text().contains("CLTS PASS"), "{}", m.bus.text());
        assert!(!m.bus.text().contains("FAIL"));
        assert_eq!(&m.bus.mem[0x8000..0x8040], &expected);
        for address in [0x7b00, 0x7c00] {
            assert_eq!(&m.bus.mem[address..address + 2], &[0xa5; 2]);
        }
        assert_eq!(m.cpu.cr0, 0x60000011);
        assert_eq!(m.cpu.fpu, Fpu::default());
        assert_eq!(m.cpu.reg32(SP), 0x7c00);
        assert_eq!(m.cpu.last_fault, Some((7, 0)));
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
    }
}

#[test]
fn nonzero_cpl_clts_delivers_guest_gp_or_lock_ud_without_enabling_user_execution() {
    for cpl in 1..=3 {
        for big in [false, true] {
            for locked in [false, true] {
                let code: &[u8] = if locked {
                    &[0x64, 0xf0, 0x0f, 0x06]
                } else {
                    &[0x64, 0x0f, 0x06]
                };
                let (mut c, mut b) = cpu(code, true, big);
                c.cpl = cpl;
                c.cr0 |= 12;
                c.s[CS] |= cpl as u16;
                for d in c.segment_caches.iter_mut().flatten() {
                    d.access |= cpl << 5;
                }
                b.mem[0x500d] |= cpl << 5; // same-ring fault target, no TSS stack switch
                b.mem[0x2000] = 0x90; // would succeed if general user execution were enabled
                let saved = c.clone();
                let mut memory = b.mem.clone();
                c.step(&mut b).unwrap();
                let vector = if locked { 6 } else { 13 };
                assert_eq!(c.last_fault, Some((vector, 0)));
                assert_eq!((c.s[CS], c.ip), (8 | cpl as u16, 0x1000));
                let sp = c.reg32(SP) as usize;
                let error_bytes = if locked { 0 } else { 4 };
                assert_eq!(sp, 0x9000 - 12 - error_bytes);
                assert_eq!(&b.mem[sp..sp + 4], &0u32.to_le_bytes());
                assert_eq!(
                    &b.mem[sp + error_bytes..sp + error_bytes + 4],
                    &0u32.to_le_bytes()
                );
                assert_eq!(
                    &b.mem[sp + error_bytes + 4..sp + error_bytes + 8],
                    &(8u32 | cpl as u32).to_le_bytes()
                );
                memory[sp..0x9000].copy_from_slice(&b.mem[sp..0x9000]);
                assert_eq!(b.mem, memory);
                assert_eq!(c.cr0, saved.cr0);
                assert_eq!(c.fpu, saved.fpu);
                assert_eq!(c.tr_cache, saved.tr_cache);
                assert_eq!(c.tr, saved.tr);
                assert_eq!(c.retired, saved.retired);
                let snapshot = format!("{c:?}");
                assert!(c.step(&mut b).unwrap_err().contains("only ring 0"));
                assert_eq!(format!("{c:?}"), snapshot);
                assert_eq!(b.mem, memory);
            }
        }
    }
}

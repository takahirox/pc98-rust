// Original instruction probes for Issue #99. SPDX-License-Identifier: MIT
use pc98_rust::{bus::Bus, cpu::*, Machine, Profile};

fn cpu(code: &[u8]) -> (Cpu, Bus) {
    let c = Cpu {
        s: [0; 4],
        ip: 0x1000,
        flags: 2,
        ..Cpu::default()
    };
    let mut b = Bus::new(Profile::Pc9821ApDos);
    b.mem[0x1000..0x1000 + code.len()].copy_from_slice(code);
    (c, b)
}

// Compare every CPU field, allowing only instruction decoding/accounting changes.
fn preserved(c: &Cpu, mut expected: Cpu) {
    expected.operand32 = c.operand32;
    expected.address32 = c.address32;
    expected.lock_prefix = c.lock_prefix;
    expected.instruction_bytes = c.instruction_bytes;
    expected.trap_suppressed = c.trap_suppressed;
    expected.retired = c.retired;
    expected.extended_retired = c.extended_retired;
    assert_eq!(format!("{c:?}"), format!("{expected:?}"));
}

#[test]
fn all_cr_selectors_sources_prefixes_roundtrip_and_preservation() {
    let prefixes: &[&[u8]] = &[
        &[],
        &[0x66],
        &[0x67],
        &[0x26],
        &[0x2e],
        &[0x36],
        &[0x3e],
        &[0x64],
        &[0x65],
        &[0xf2],
        &[0xf3],
        &[0x66, 0x66, 0x67, 0x67, 0x26, 0x65, 0xf2, 0xf3],
    ];
    for prefix in prefixes {
        for (selector, value) in [(0, 0x60050032), (2, 0xdeadbeef), (3, 0xabcde018)] {
            for source in 0..8 {
                let mut code = prefix.to_vec();
                code.extend_from_slice(&[
                    0x0f,
                    0x22,
                    0xc0 | selector << 3 | source,
                    0x0f,
                    0x20,
                    0xc0 | selector << 3 | source,
                ]);
                let (mut c, mut b) = cpu(&code);
                for r in 0..8 {
                    c.set_reg32(r, 0xabcd1234 + r as u32 * 0x10001);
                }
                c.set_reg32(source as usize, value);
                c.s[DS] = 0x3000;
                c.s[SS] = 0x4000;
                c.extra_segments = [0x5000, 0x6000];
                c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
                c.flags_upper = 4;
                c.gdtr_base = 0xabcdef01;
                c.idtr_base = 0x5000;
                let mut expected = c.clone();
                let memory = b.mem.clone();
                c.step(&mut b).unwrap();
                match selector {
                    0 => expected.cr0 = value,
                    2 => expected.cr2 = value,
                    3 => expected.cr3 = value,
                    _ => unreachable!(),
                }
                expected.ip += prefix.len() as u32 + 3;
                preserved(&c, expected.clone());
                assert_eq!(c.retired, 1);
                c.step(&mut b).unwrap();
                expected.ip += 3;
                preserved(&c, expected);
                assert_eq!(b.mem, memory);
                assert!(b.unknown_ports.is_empty());
                assert_eq!(
                    (
                        b.cycles,
                        b.fdc.reads,
                        b.fdc.writes,
                        b.ide.reads,
                        b.ide.writes
                    ),
                    (0, 0, 0, 0, 0)
                );
                assert!(b.audio.is_empty());
            }
        }
    }
}

fn fault_setup(code: &[u8], vector: usize) -> (Cpu, Bus) {
    let (mut c, mut b) = cpu(code);
    c.s[CS] = 0x100;
    c.ip = 0;
    c.set_reg32(SP, 0xabcd9000);
    c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
    c.idtr_base = 0x6000;
    let entry = 0x6000 + vector * 4;
    b.mem[entry..entry + 4].copy_from_slice(&[0, 1, 0, 2]);
    b.mem[0x2100] = 0xcf; // actual guest IRET
    (c, b)
}

#[test]
fn all_invalid_modrm_and_lock_ud_with_atomic_restart_and_retry() {
    for m in 0..=255u8 {
        for locked in [false, true] {
            if !locked && m >= 0xc0 && matches!((m >> 3) & 7, 0 | 2 | 3) {
                continue;
            }
            let code = [0x66, 0x67, if locked { 0xf0 } else { 0x65 }, 0x0f, 0x22, m];
            let (mut c, mut b) = fault_setup(&code, 6);
            c.set_reg32(AX, 0x60000010);
            let saved = c.clone();
            let mut memory = b.mem.clone();
            c.step(&mut b).unwrap();
            memory[0x8ffa..0x9000].copy_from_slice(&[0, 0, 0, 1, 0xd7, 0x0e]);
            assert_eq!(b.mem, memory);
            let mut expected = saved.clone();
            expected.s[CS] = 0x200;
            expected.ip = 0x100;
            expected.r[SP] -= 6;
            expected.flags &= !(IF | TF);
            expected.last_fault = Some((6, 0));
            preserved(&c, expected);
            assert_eq!((c.retired, c.extended_retired), (0, 0));
            b.mem[0x1002] = 0x65;
            b.mem[0x1005] = 0xc0;
            c.step(&mut b).unwrap();
            c.step(&mut b).unwrap();
            let mut expected = saved;
            expected.ip = 6;
            expected.last_fault = Some((6, 0));
            preserved(&c, expected);
        }
    }
}

#[test]
fn invalid_cr0_combinations_gp_before_mutation_and_iret_retry() {
    for value in [0x80000010, 0xe0000010, 0x20000010, 0x2005003f] {
        let (mut c, mut b) = fault_setup(&[0x66, 0x0f, 0x22, 0xc0], 13);
        c.set_reg32(AX, value);
        let saved = c.clone();
        let mut memory = b.mem.clone();
        c.step(&mut b).unwrap();
        memory[0x8ffa..0x9000].copy_from_slice(&[0, 0, 0, 1, 0xd7, 0x0e]);
        assert_eq!(b.mem, memory);
        let mut expected = saved.clone();
        expected.s[CS] = 0x200;
        expected.ip = 0x100;
        expected.r[SP] -= 6;
        expected.flags &= !(IF | TF);
        expected.last_fault = Some((13, 0));
        preserved(&c, expected);
        assert_eq!((c.retired, c.extended_retired), (0, 0));
        c.step(&mut b).unwrap();
        c.set_reg32(AX, 0x60000010);
        c.step(&mut b).unwrap();
        assert_eq!(c.cr0, 0x60000010);
        assert_eq!((c.s[CS], c.ip, c.r[SP]), (0x100, 4, 0x9000));
    }
}

#[test]
fn reserved_bits_and_modes_are_precise_atomic_diagnostics() {
    let mut cases = Vec::new();
    for bit in 0..32 {
        if (1u32 << bit) & 0xe005003f == 0 {
            cases.push((0, 0x60000010 | 1 << bit, "undefined reserved-bit"));
        }
        if (1u32 << bit) & 0xfffff018 == 0 {
            cases.push((3, 1 << bit, "undefined reserved-bit"));
        }
    }
    for (selector, value, reason) in cases {
        let (mut c, mut b) = cpu(&[0x66, 0x67, 0x65, 0x0f, 0x22, 0xc3 | selector << 3]);
        c.set_reg32(BX, value);
        c.flags = 2 | CF | TF | IF | DF | OF;
        let saved = format!("{c:?}");
        let memory = b.mem.clone();
        for _ in 0..2 {
            // repeating step cannot partially advance or retire
            let error = c.step(&mut b).unwrap_err();
            assert!(error.contains(reason), "{error}");
            assert!(
                error.contains("CS:IP=0000:00001000 opcode=0f 22"),
                "{error}"
            );
            assert!(error.contains(&format!("CR{selector} GPR=ebx")), "{error}");
            assert!(error.contains(&format!("written={value:08x}")), "{error}");
            assert!(
                error.contains("next-ip=00001006; write not committed"),
                "{error}"
            );
            assert_eq!(format!("{c:?}"), saved);
            assert_eq!(b.mem, memory);
        }
        assert_eq!(b.cycles, 0);
    }
}

#[test]
fn legal_cr0_bits_fixed_et_smsw_fpu_and_reset() {
    for value in [
        0, 0x10, 0x12, 0x14, 0x18, 0x30, 0x10010, 0x40010, 0x40000010, 0x6005003e,
    ] {
        let (mut c, mut b) = cpu(&[
            0x0f, 0x22, 0xc0, 0x0f, 0x20, 0xc3, 0x0f, 0x01, 0xe1, 0xdb, 0xe3,
        ]);
        c.set_reg32(AX, value);
        c.r[SP] = 0x9000;
        b.mem[7 * 4..7 * 4 + 4].copy_from_slice(&[0, 0x20, 0, 0]);
        c.fpu.control = 0x1234;
        c.step(&mut b).unwrap();
        c.step(&mut b).unwrap();
        c.step(&mut b).unwrap();
        assert_eq!(c.cr0, value | 0x10); // ET cannot be cleared
        assert_eq!(c.reg32(BX), value | 0x10);
        assert_eq!(c.r[CX], (value | 0x10) as u16);
        c.step(&mut b).unwrap();
        if value & 0xc != 0 {
            assert_eq!(c.ip, 0x2000);
            assert_eq!(b.read16(0x8ffa), 0x1009);
            assert_eq!(c.fpu.control, 0x1234);
        } else {
            assert_eq!(c.ip, 0x100b);
            assert_eq!(c.fpu.control, 0x37f);
        }
    }
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x1000;
    m.bus.mem[0x1000..0x1009]
        .copy_from_slice(&[0x0f, 0x22, 0xc0, 0x0f, 0x22, 0xd0, 0x0f, 0x22, 0xd8]);
    m.cpu.set_reg32(AX, 0x10);
    for _ in 0..3 {
        m.cpu.step(&mut m.bus).unwrap();
    }
    assert_eq!((m.cpu.cr0, m.cpu.cr2, m.cpu.cr3), (0x10, 0x10, 0x10));
    m.reset();
    assert_eq!((m.cpu.cr0, m.cpu.cr2, m.cpu.cr3), (0x60000010, 0, 0));
    // VM retains its existing unsupported V30 0F 22 diagnostic; no ModR/M fetch.
    let (mut c, _) = cpu(&[]);
    let mut b = Bus::new(Profile::Pc9801Vm);
    b.mem[0x1000..0x1003].copy_from_slice(&[0x0f, 0x22, 0xc0]);
    c.set_reg32(AX, 0xdeadbeef);
    assert_eq!(c.step(&mut b).unwrap_err(), "unsupported V30 opcode 0F 22");
    assert_eq!((c.ip, c.reg32(AX), c.cr0), (0x1002, 0xdeadbeef, 0x60000010));
}

#[test]
fn instruction_fetch_limits_and_tf_trap_after_success() {
    for prefix_count in [12, 13] {
        let mut code = vec![0x66; prefix_count];
        code.extend_from_slice(&[0x0f, 0x22, 0xd0]);
        let (mut c, mut b) = fault_setup(&code, 13);
        c.set_reg32(AX, 0xdeadbeef);
        c.step(&mut b).unwrap();
        if prefix_count == 12 {
            assert_eq!((c.ip, c.cr2), (15, 0xdeadbeef));
        } else {
            assert_eq!((c.s[CS], c.ip, c.cr2), (0x200, 0x100, 0));
            assert_eq!(b.read16(0x8ffa), 0);
        }
    }
    for start in [0xfffd, 0xfffe, 0xffff] {
        let (mut c, mut b) = fault_setup(&[], 13);
        c.s[CS] = 0;
        c.ip = start;
        c.set_reg32(AX, 0xdeadbeef);
        b.mem[start as usize..start as usize + 3].copy_from_slice(&[0x0f, 0x22, 0xd0]);
        c.step(&mut b).unwrap();
        if start == 0xfffd {
            assert_eq!((c.ip, c.cr2), (0x10000, 0xdeadbeef));
        } else {
            assert_eq!((c.s[CS], c.ip, c.cr2), (0x200, 0x100, 0));
            assert_eq!(b.read16(0x8ffa), start as u16);
        }
    }
    let (mut c, mut b) = fault_setup(&[0x66, 0x0f, 0x22, 0xd0], 1);
    c.set_reg32(AX, 0xdeadbeef);
    c.flags |= TF;
    c.step(&mut b).unwrap();
    assert_eq!(c.cr2, 0xdeadbeef);
    assert_eq!((c.s[CS], c.ip), (0x200, 0x100));
    assert_eq!(b.read16(0x8ffa), 4);
    c.step(&mut b).unwrap();
    assert_eq!((c.s[CS], c.ip, c.r[SP]), (0x100, 4, 0x9000));
}

#[test]
fn original_mit_boot_probe_cold_and_reset() {
    let image = std::fs::read("fixtures/mov_cr_write/mov_cr_write.hdm")
        .expect("run python3 tools/prepare_mov_cr_write_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "mov_cr_write.hdm").unwrap();
    for run in 0..2 {
        if run == 1 {
            m.reset();
        }
        for _ in 0..20 {
            m.step(100_000);
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("MOV CR WRITE") {
                break;
            }
        }
        assert!(
            m.bus.text().contains("MOV CR WRITE PASS"),
            "{}",
            m.bus.text()
        );
        let mut expected = [0xa5; 128];
        for i in 0..8 {
            let value: u32 = if i == 4 {
                0x12345018
            } else {
                0x12340000 + i as u32
            };
            expected[1 + 8 * i..5 + 8 * i].copy_from_slice(&value.to_le_bytes());
        }
        expected[65..69].copy_from_slice(&0x60050032u32.to_le_bytes());
        expected[73..77].copy_from_slice(&0x12345018u32.to_le_bytes());
        expected[81..83].copy_from_slice(&0x32u16.to_le_bytes());
        expected[89..91].copy_from_slice(&0x8d7u16.to_le_bytes());
        expected[97] = 4;
        assert_eq!(&m.bus.mem[0x8000..0x8080], &expected);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
    }
}

#[test]
fn original_transition_without_page_tables_commits_then_shuts_down() {
    for (name, value) in [("pe_pg", 0xe0000011)] {
        let image = std::fs::read(format!("fixtures/mov_cr_write/{name}.hdm")).unwrap();
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.insert(0, image, "transition.hdm").unwrap();
        for run in 0..2 {
            if run == 1 {
                m.reset();
            }
            m.step(10000);
            let error = m.error.as_ref().unwrap();
            assert!(
                error.contains("protected page fault delivery failed"),
                "{error}"
            );
            assert!(error.contains("triple fault/shutdown"), "{error}");
            assert_eq!((m.cpu.cr0, m.cpu.cr3), (value, 0));
            assert_eq!(
                (m.cpu.s[CS], m.cpu.ip, m.cpu.reg32(AX)),
                (0x1fc0, 13, value)
            );
            assert!(m.cpu.shutdown);
            let saved = format!("{:?}", m.cpu);
            let memory = m.bus.mem.clone();
            assert_eq!(m.step(100), 0);
            assert_eq!(format!("{:?}", m.cpu), saved);
            assert_eq!(m.bus.mem, memory);
            assert!(!m.bus.text().contains("PASS"));
        }
    }
}

#[test]
fn original_pe_only_transition_commits_and_halts_in_cached_segment_window() {
    let image = std::fs::read("fixtures/mov_cr_write/pe.hdm").unwrap();
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image, "pe.hdm").unwrap();
    for run in 0..2 {
        if run == 1 {
            m.reset();
        }
        m.step(10000);
        assert!(m.error.is_none(), "{:?}", m.error);
        assert!(m.cpu.halted);
        assert_eq!(m.cpu.cr0, 0x60000011);
        assert_eq!(
            (m.cpu.s[CS], m.cpu.ip, m.cpu.segment_cache(CS).base),
            (0x1fc0, 14, 0x1fc00)
        );
        assert!(!m.bus.text().contains("PASS"));
    }
}

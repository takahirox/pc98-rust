// Original instruction probes for Issue #97. SPDX-License-Identifier: MIT
use pc98_rust::{bus::Bus, cpu::*, Profile};

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

#[test]
fn mov_cr_minimal_register_reproduction() {
    // Exact 0F 20 C0 bytes are independently authored, not extracted from media.
    let (mut c, mut b) = cpu(&[0x0f, 0x20, 0xc0, 0x90]);
    c.set_reg32(AX, 0xabcdef01);
    c.flags = 2 | CF | PF | AF | ZF | SF | DF | OF;
    let flags = c.flags;
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x60000010);
    assert_eq!(c.flags, flags);
    assert_eq!(c.ip, 0x1003);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x1004);
}

#[test]
fn all_selectors_destinations_prefixes_and_unrelated_state() {
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
        for (selector, value) in [(0, 0x60050032), (2, 0x1234abcd), (3, 0x76543018)] {
            for destination in 0..8 {
                let mut code = prefix.to_vec();
                code.extend_from_slice(&[0x0f, 0x20, 0xc0 | selector << 3 | destination]);
                let (mut c, mut b) = cpu(&code);
                c.cr0 = 0x60050032; // Full high bits, PE/PG clear; host-only variation.
                c.cr2 = 0x1234abcd;
                c.cr3 = 0x76543018;
                for r in 0..8 {
                    c.set_reg32(r, 0xabcd1234 + r as u32 * 0x10001);
                }
                c.s[DS] = 0x3000;
                c.s[SS] = 0x4000;
                c.extra_segments = [0x5000, 0x6000];
                c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
                c.flags_upper = 4;
                c.gdtr_base = 0xabcdef01;
                c.idtr_base = 0x5000;
                let saved = c.clone();
                let memory = b.mem.clone();
                c.step(&mut b).unwrap();
                for r in 0..8 {
                    assert_eq!(
                        c.reg32(r),
                        if r == destination as usize {
                            value
                        } else {
                            saved.reg32(r)
                        }
                    );
                }
                assert_eq!(c.s, saved.s);
                assert_eq!(c.extra_segments, saved.extra_segments);
                assert_eq!((c.flags, c.flags_upper), (saved.flags, saved.flags_upper));
                assert_eq!((c.cr0, c.cr2, c.cr3), (saved.cr0, saved.cr2, saved.cr3));
                assert_eq!(
                    (c.gdtr_base, c.gdtr_limit, c.idtr_base, c.idtr_limit),
                    (
                        saved.gdtr_base,
                        saved.gdtr_limit,
                        saved.idtr_base,
                        saved.idtr_limit
                    )
                );
                assert_eq!(c.fpu, saved.fpu);
                assert_eq!((c.shutdown, c.halted, c.inhibit), (false, false, 0));
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
                assert_eq!(c.ip, 0x1000 + code.len() as u32);
                assert_eq!(c.retired, 1);
            }
        }
    }
}

fn fault_setup(code: &[u8], vector: usize) -> (Cpu, Bus) {
    let (mut c, mut b) = cpu(code);
    c.s[CS] = 0x100;
    c.ip = 0; // Nonzero CS; first prefix is still physical 1000h.
    c.r[SP] = 0x9000;
    c.upper[SP] = 0xabcd;
    c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
    c.idtr_base = 0x6000;
    let entry = 0x6000 + vector * 4;
    b.mem[entry..entry + 4].copy_from_slice(&[0, 1, 0, 2]);
    b.mem[0x2100] = 0xcf; // Guest IRET.
    (c, b)
}

#[test]
fn invalid_modrm_reserved_selectors_and_lock_ud_restart_and_retry() {
    // Exhaust all invalid ModR/M bytes; memory forms must not consume a SIB,
    // displacement, or touch the apparent operand. All reserved CRn are #UD.
    for m in 0..=255u8 {
        for locked in [false, true] {
            if !locked && m >= 0xc0 && matches!((m >> 3) & 7, 0 | 2 | 3) {
                continue;
            }
            let code = [0x66, 0x67, if locked { 0xf0 } else { 0x65 }, 0x0f, 0x20, m];
            let (mut c, mut b) = fault_setup(&code, 6);
            c.set_reg32(AX, 0xdeadbeef);
            let saved = c.clone();
            let mut expected = b.mem.clone();
            c.step(&mut b).unwrap();
            assert_eq!((c.s[CS], c.ip, c.r[SP]), (0x200, 0x100, 0x8ffa));
            expected[0x8ffa..0x9000].copy_from_slice(&[0, 0, 0, 1, 0xd7, 0x0e]);
            assert_eq!(b.mem, expected);
            assert_eq!(c.flags, saved.flags & !(IF | TF));
            assert_eq!(c.upper, saved.upper);
            let mut regs = saved.r;
            regs[SP] -= 6;
            assert_eq!(c.r, regs);
            assert_eq!((c.cr0, c.cr2, c.cr3), (saved.cr0, saved.cr2, saved.cr3));
            assert_eq!(c.retired, 0);
            // Repair the fault, execute real IRET and retry the same prefixes.
            b.mem[0x1002] = 0x65;
            b.mem[0x1005] = 0xc0;
            c.step(&mut b).unwrap();
            assert_eq!(
                (c.s[CS], c.ip, c.r[SP], c.flags),
                (0x100, 0, 0x9000, saved.flags)
            );
            c.step(&mut b).unwrap();
            assert_eq!(c.reg32(AX), 0x60000010);
            assert_eq!(c.ip, 6);
        }
    }
}

#[test]
fn instruction_length_segment_limits_and_single_step() {
    for prefix_count in [12, 13] {
        let mut code = vec![0x66; prefix_count];
        code.extend_from_slice(&[0x0f, 0x20, 0xc0]);
        let (mut c, mut b) = fault_setup(&code, 13);
        c.step(&mut b).unwrap();
        if prefix_count == 12 {
            assert_eq!(c.ip, 15);
            assert_eq!(c.reg32(AX), 0x60000010);
        } else {
            assert_eq!((c.s[CS], c.ip, c.r[SP]), (0x200, 0x100, 0x8ffa));
            assert_eq!(b.read16(0x8ffa), 0);
        }
    }
    for start in [0xfffd, 0xfffe, 0xffff] {
        let (mut c, mut b) = fault_setup(&[], 13);
        c.s[CS] = 0;
        c.ip = start;
        c.set_reg32(AX, 0xabcdef01);
        b.mem[start as usize..start as usize + 3].copy_from_slice(&[0x0f, 0x20, 0xc0]);
        c.step(&mut b).unwrap();
        if start == 0xfffd {
            assert_eq!(c.ip, 0x10000);
            assert_eq!(c.reg32(AX), 0x60000010);
        } else {
            assert_eq!((c.s[CS], c.ip), (0x200, 0x100));
            assert_eq!(b.read16(0x8ffa), start as u16);
            assert_eq!(c.reg32(AX), 0xabcdef01);
        }
    }
    let (mut c, mut b) = fault_setup(&[0x66, 0x0f, 0x20, 0xc0], 1);
    c.flags |= TF;
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x60000010);
    assert_eq!((c.s[CS], c.ip), (0x200, 0x100));
    assert_eq!(&b.mem[0x8ffa..0x9000], &[4, 0, 0, 1, 0xd7, 0x0f]);
    c.step(&mut b).unwrap(); // IRET suppresses its own single-step trap.
    assert_eq!((c.s[CS], c.ip, c.r[SP]), (0x100, 4, 0x9000));
}

#[test]
fn cr0_smsw_x87_reset_isolation_and_v30_decoder() {
    use pc98_rust::Machine;
    let (mut c, mut b) = cpu(&[0x0f, 0x20, 0xc0, 0x0f, 0x01, 0xe3, 0xdb, 0xe3]);
    c.cr0 = 0x6000001c; // Host-only EM/TS variation; no guest writer.
    c.r[SP] = 0x9000;
    b.mem[7 * 4..7 * 4 + 4].copy_from_slice(&[0, 0x20, 0, 0]);
    c.step(&mut b).unwrap();
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x6000001c);
    assert_eq!(c.r[BX], 0x1c);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x2000); // Same CR0 gives x87 #NM with first-byte restart.
    assert_eq!(b.read16(0x8ffa), 0x1006);
    let mut m = Machine::new(Profile::Pc9821ApDos);
    assert_eq!((m.cpu.cr0, m.cpu.cr2, m.cpu.cr3), (0x60000010, 0, 0));
    m.cpu.cr0 = 0x6000001c;
    m.cpu.cr2 = 0x1234;
    m.cpu.cr3 = 0x5000;
    m.reset();
    assert_eq!((m.cpu.cr0, m.cpu.cr2, m.cpu.cr3), (0x60000010, 0, 0));
    assert_eq!(c.cr0, 0x6000001c);
    let (mut v, _) = cpu(&[]);
    let mut vm = Bus::new(Profile::Pc9801Vm);
    vm.mem[0x1000..0x1003].copy_from_slice(&[0x0f, 0x20, 0xc0]);
    v.set_reg32(AX, 0xabcdef01);
    v.step(&mut vm).unwrap(); // V30 ADD4S with CL=0, not MOV CR0.
    assert_eq!(v.ip, 0x1002); // C0 is the next opcode, not a ModR/M byte.
    assert_eq!(v.reg32(AX), 0xabcdef01);
}

#[test]
fn original_mit_boot_probe_cold_and_after_reset() {
    use pc98_rust::Machine;
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/mov_cr/mov_cr.hdm"
    ))
    .expect("run python3 tools/prepare_mov_cr_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "mov_cr.hdm").unwrap();
    for run in 0..2 {
        if run == 1 {
            m.reset();
        }
        let mut slots = 0;
        for _ in 0..20 {
            slots += m.step(100_000);
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("MOV CR PASS") || m.bus.text().contains("MOV CR FAIL") {
                break;
            }
        }
        assert!(m.bus.text().contains("MOV CR PASS"), "{}", m.bus.text());
        assert!(!m.bus.text().contains("FAIL"));
        assert!(slots <= 2_000_000);
        let mut expected = [0xa5; 128];
        for offset in (1..64).step_by(8) {
            expected[offset..offset + 4].copy_from_slice(&0x60000010u32.to_le_bytes());
        }
        for offset in [65, 73] {
            expected[offset..offset + 4].fill(0);
        }
        expected[81..83].copy_from_slice(&[0x10, 0]);
        expected[89..91].copy_from_slice(&[0xd7, 0x08]);
        expected[97] = 3;
        assert_eq!(&m.bus.mem[0x8000..0x8080], &expected);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
    }
}

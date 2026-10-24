use pc98_rust::{bus::Bus, cpu::*, Machine, Profile};

fn cpu(code: &[u8]) -> (Cpu, Bus) {
    let mut c = Cpu {
        s: [0; 4],
        ip: 0x1000,
        flags: 2,
        ..Cpu::default()
    };
    c.r[SP] = 0x9000;
    let mut b = Bus::new(Profile::Pc9821ApDos);
    b.mem[0x1000..0x1000 + code.len()].copy_from_slice(code);
    for n in [6, 7, 13] {
        b.mem[n * 4..n * 4 + 2].copy_from_slice(&(0x2000u16 + n as u16 * 16).to_le_bytes());
        b.mem[0x2000 + n * 16] = 0xcf; // IRET
    }
    (c, b)
}

#[test]
fn minimal_reproduction() {
    let (mut c, mut b) = cpu(&[
        0xdb, 0xe3, 0xd9, 0x3e, 0, 0x80, 0xdd, 0x3e, 2, 0x80, 0xdf, 0xe0,
    ]);
    for _ in 0..4 {
        c.step(&mut b).unwrap();
    }
    assert_eq!(&b.mem[0x8000..0x8004], &[0x7f, 3, 0, 0]);
    assert_eq!(c.r[AX], 0);
    assert_eq!(c.ip, 0x100c);
}

fn seeded() -> Fpu {
    Fpu {
        control: 0x0840,
        status: 0xffbf,
        tag: 0x1357,
        instruction_pointer: 0x12345678,
        instruction_selector: 0xabcd,
        data_pointer: 0x87654321,
        data_selector: 0xdcba,
        last_opcode: 0x753,
    }
}

fn initialized(f: &Fpu) {
    assert_eq!(f.control, 0x037f);
    assert_eq!(f.status, 0);
    assert_eq!(f.tag, 0xffff);
    assert_eq!(f.instruction_pointer, 0);
    assert_eq!(f.instruction_selector, 0);
    assert_eq!(f.data_pointer, 0);
    assert_eq!(f.data_selector, 0);
    assert_eq!(f.last_opcode, 0);
}

fn preserved(c: &Cpu, saved: &Cpu) {
    assert_eq!(c.s, saved.s);
    assert_eq!(c.extra_segments, saved.extra_segments);
    assert_eq!(c.flags, saved.flags);
    assert_eq!(c.flags_upper, saved.flags_upper);
    assert_eq!(c.machine_status, saved.machine_status);
    assert_eq!(
        (c.gdtr_limit, c.gdtr_base),
        (saved.gdtr_limit, saved.gdtr_base)
    );
    assert_eq!(
        (c.idtr_limit, c.idtr_base),
        (saved.idtr_limit, saved.idtr_base)
    );
    assert_eq!(c.halted, saved.halted);
}

#[test]
fn init_all_metadata_repeatable_pending_status_and_register_readback() {
    for prefixes in [
        &[][..],
        &[0x66][..],
        &[0x66, 0x66, 0x67, 0x67, 0x64, 0x65][..],
    ] {
        let mut code = prefixes.to_vec();
        code.extend_from_slice(&[0xdb, 0xe3]);
        let (mut c, mut b) = cpu(&code);
        c.fpu = seeded();
        c.r = [0x1234; 8];
        c.upper = [0xabcd; 8];
        c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
        c.flags_upper = 4;
        let saved = c.clone();
        let memory = b.mem.clone();
        for _ in 0..2 {
            c.ip = 0x1000;
            c.step(&mut b).unwrap();
            initialized(&c.fpu);
            preserved(&c, &saved);
            assert_eq!(c.r, saved.r);
            assert_eq!(c.upper, saved.upper);
            assert_eq!(c.ip, 0x1000 + code.len() as u32);
            assert_eq!(b.mem, memory); // No numerical exception handler or writes.
        }
        code.truncate(prefixes.len());
        code.extend_from_slice(&[0xdf, 0xe0]);
        b.mem[0x1000..0x1000 + code.len()].copy_from_slice(&code);
        c.fpu = seeded(); // Pending unmasked exceptions must not block FNSTSW.
        c.ip = 0x1000;
        c.step(&mut b).unwrap();
        assert_eq!(c.reg32(AX), 0xabcdffbf);
        assert_eq!(&c.r[1..], &saved.r[1..]);
        assert_eq!(c.upper, saved.upper);
        assert_eq!(c.fpu, seeded());
        preserved(&c, &saved);
    }
}

#[test]
fn stores_memory_sizes_sib_segments_unaligned_and_exact_width() {
    // 16-bit [bx+si], [bp+si], disp16 and all segment overrides;
    // 32-bit scaled SIB, EBP/ESP defaults, index-only EBP (DS), disp32.
    type MemoryCase = (bool, &'static [u8], &'static [u8], usize, usize);
    let cases: &[MemoryCase] = &[
        (false, &[], &[0x38], DS, 0x4021),
        (false, &[], &[0x3a], SS, 0x4121),
        (false, &[], &[0x3e, 1, 0x40], DS, 0x4001),
        (false, &[0x26], &[0x3f], ES, 0x4001),
        (false, &[0x2e], &[0x3f], CS, 0x4001),
        (false, &[0x36], &[0x3f], SS, 0x4001),
        (false, &[0x3e], &[0x7a, 0], DS, 0x4121),
        (false, &[0x64], &[0x3f], 4, 0x4001),
        (false, &[0x26, 0x64, 0x65], &[0x3f], 5, 0x4001),
        (true, &[], &[0x7c, 0x8b, 1], DS, 0x4022),
        (true, &[], &[0xbc, 0x8b, 0xf0, 0xff, 0xff, 0xff], DS, 0x4011),
        (true, &[], &[0x7c, 0x25, 0], SS, 0x4101),
        (true, &[], &[0x3c, 0x24], SS, 0x9000),
        (true, &[], &[0x3c, 0x2d, 0, 0, 0, 0], DS, 0x4101),
        (true, &[], &[0x3d, 1, 0x40, 0, 0], DS, 0x4001),
        (true, &[0x26], &[0x3b], ES, 0x4001),
        (true, &[0x2e], &[0x3b], CS, 0x4001),
        (true, &[0x36], &[0x3b], SS, 0x4001),
        (true, &[0x3e], &[0x7d, 0], DS, 0x4101),
        (true, &[0x64], &[0x3b], 4, 0x4001),
        (true, &[0x26, 0x64, 0x65], &[0x3b], 5, 0x4001),
    ];
    for &(address32, prefixes, encoding, segment, offset) in cases {
        for operand32 in [false, true] {
            for (opcode, word) in [(0xd9, 0x0840u16), (0xdd, 0xffbf)] {
                let mut code = prefixes.to_vec();
                if operand32 {
                    code.extend_from_slice(&[0x66, 0x66]);
                }
                if address32 {
                    code.extend_from_slice(&[0x67, 0x67]);
                }
                code.push(opcode);
                code.extend_from_slice(encoding);
                let (mut c, mut b) = cpu(&code);
                c.fpu = seeded(); // Host-only variation proves current CPU-local readback.
                c.s[DS] = 0x3000;
                c.s[ES] = 0x4000;
                c.s[SS] = 0x5000;
                c.extra_segments = [0x6000, 0x7000];
                c.set_reg32(BX, 0x4001);
                c.set_reg32(BP, 0x4101);
                c.set_reg32(CX, 8);
                c.set_reg32(SI, 0x20);
                c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
                c.flags_upper = 4;
                let seg = if segment < 4 {
                    c.s[segment]
                } else {
                    c.extra_segments[segment - 4]
                };
                let address = seg as usize * 16 + offset;
                b.mem[address - 1..address + 5].fill(0xa5);
                let saved = c.clone();
                let mut expected = b.mem.clone();
                expected[address..address + 2].copy_from_slice(&word.to_le_bytes());
                c.step(&mut b).unwrap();
                assert_eq!(b.mem, expected, "{code:02x?}");
                assert_eq!(c.r, saved.r);
                assert_eq!(c.upper, saved.upper);
                preserved(&c, &saved);
                assert_eq!(c.fpu, saved.fpu);
                assert_eq!(c.ip, 0x1000 + code.len() as u32);
            }
        }
    }
}

#[test]
fn store_limits_gp_even_ss_atomic_frame_and_retry() {
    for opcode in [0xd9, 0xdd] {
        for wide in [false, true] {
            for address32 in [false, true] {
                for stack in [false, true] {
                    for offset in [0xfffe, 0xffff, 0x10000, u32::MAX] {
                        if !address32 && offset > 0xffff {
                            continue;
                        }
                        let mut code = vec![if stack { 0x36 } else { 0x3e }];
                        if wide {
                            code.push(0x66);
                        }
                        if address32 {
                            code.push(0x67);
                        }
                        code.extend_from_slice(&[
                            opcode,
                            if stack {
                                if address32 {
                                    0x7d
                                } else {
                                    0x7e
                                }
                            } else if address32 {
                                0x7b
                            } else {
                                0x7f
                            },
                            0,
                        ]);
                        let (mut c, mut b) = cpu(&code);
                        c.fpu = seeded();
                        let reg = if stack { BP } else { BX };
                        c.set_reg32(reg, offset);
                        c.flags = 2 | CF | DF | OF;
                        b.mem[0xfffd..0x10004].fill(0xa5);
                        let saved = c.clone();
                        let mut expected = b.mem.clone();
                        c.step(&mut b).unwrap();
                        let word = if opcode == 0xd9 {
                            [0x40, 8]
                        } else {
                            [0xbf, 0xff]
                        };
                        if offset == 0xfffe {
                            expected[0xfffe..0x10000].copy_from_slice(&word);
                            assert_eq!(b.mem, expected);
                            assert_eq!(c.r, saved.r);
                            assert_eq!(c.upper, saved.upper);
                            preserved(&c, &saved);
                            assert_eq!(c.fpu, saved.fpu);
                            continue;
                        }
                        assert_eq!(c.ip, 0x20d0);
                        assert_eq!(c.r[SP], 0x8ffa);
                        expected[0x8ffa..0x9000].copy_from_slice(&[0, 0x10, 0, 0, 3, 0x0c]);
                        assert_eq!(b.mem, expected); // Only six-byte frame changed.
                        assert_eq!(c.fpu, saved.fpu);
                        assert_eq!(c.reg32(reg), offset);
                        c.set_reg32(reg, 0x4001); // Repair, IRET, retry.
                        c.step(&mut b).unwrap();
                        assert_eq!((c.ip, c.r[SP], c.flags), (0x1000, 0x9000, saved.flags));
                        b.mem[0x4000..0x4005].fill(0xa5);
                        c.step(&mut b).unwrap();
                        assert_eq!(
                            &b.mem[0x4000..0x4005],
                            &[0xa5, word[0], word[1], 0xa5, 0xa5]
                        );
                        assert_eq!(c.fpu, saved.fpu);
                        preserved(&c, &saved);
                    }
                }
            }
        }
    }
}

#[test]
fn em_ts_and_lock_all_forms_default_and_relocated_vectors_retry() {
    for form in [
        &[0xdb, 0xe3][..],
        &[0xd9, 0x3f],
        &[0xdd, 0x3f],
        &[0xdf, 0xe0],
    ] {
        for status in [0x14, 0x18, 0x1c, 0x10] {
            for relocated in [false, true] {
                let locked = status == 0x10;
                let vector = if locked { 6 } else { 7 };
                let mut code = vec![0x26, 0x66, 0x67, 0x67];
                if locked {
                    code.push(0xf0);
                }
                code.extend_from_slice(form);
                if matches!(form[0], 0xd9 | 0xdd) {
                    *code.last_mut().unwrap() = 0x3b;
                }
                let (mut c, mut b) = cpu(&code);
                c.s[CS] = 0x100;
                c.ip = 0; // Physical first prefix remains at 1000h.
                c.idtr_base = if relocated { 0x6000 } else { 0 };
                let entry = c.idtr_base as usize + vector * 4;
                b.mem[entry..entry + 4].copy_from_slice(&[0, 1, 0, 2]); // 0200:0100
                b.mem[0x2100] = 0xcf;
                c.fpu = seeded();
                c.machine_status = status;
                c.flags = 2 | CF | PF | AF | ZF | SF | DF | OF;
                c.set_reg32(AX, 0xdead1234);
                c.r[BX] = 0x4001;
                b.mem[0x4000..0x4005].fill(0xa5);
                let saved = c.clone();
                let mut expected = b.mem.clone();
                c.step(&mut b).unwrap();
                assert_eq!((c.s[CS], c.ip, c.r[SP]), (0x200, 0x100, 0x8ffa));
                expected[0x8ffa..0x9000].copy_from_slice(&[0, 0, 0, 1, 0xd7, 0x0c]);
                assert_eq!(b.mem, expected);
                assert_eq!(c.fpu, seeded());
                assert_eq!(c.reg32(AX), 0xdead1234);
                assert_eq!(c.upper, saved.upper);
                if locked {
                    b.mem[0x1004] = 0x3e;
                } else {
                    c.machine_status = 0x10;
                }
                c.step(&mut b).unwrap(); // IRET to first prefix.
                assert_eq!(
                    (c.s[CS], c.ip, c.r[SP], c.flags),
                    (0x100, 0, 0x9000, saved.flags)
                );
                c.step(&mut b).unwrap();
                assert_eq!(c.ip, code.len() as u32);
                if form[0] == 0xdb {
                    initialized(&c.fpu);
                } else {
                    assert_eq!(c.fpu, seeded());
                }
                assert_eq!(
                    c.reg32(AX),
                    if form[0] == 0xdf {
                        0xdeadffbf
                    } else {
                        0xdead1234
                    }
                );
                let result = match form[0] {
                    0xd9 => [0xa5, 0x40, 8, 0xa5, 0xa5],
                    0xdd => [0xa5, 0xbf, 0xff, 0xa5, 0xa5],
                    _ => [0xa5; 5],
                };
                assert_eq!(&b.mem[0x4000..0x4005], &result);
                assert_eq!(c.flags, saved.flags);
            }
        }
    }
}

#[test]
fn cold_policy_isolation_reset_retention_and_explicit_init() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    initialized(&m.cpu.fpu);
    let other = Machine::new(Profile::Pc9821ApDos);
    m.cpu.fpu = seeded();
    initialized(&other.cpu.fpu);
    m.reset();
    assert_eq!(m.cpu.fpu, seeded());
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x1000;
    m.bus.mem[0x1000..0x1004].copy_from_slice(&[0xdf, 0xe0, 0xdb, 0xe3]);
    m.cpu.step(&mut m.bus).unwrap();
    assert_eq!(m.cpu.r[AX], 0xffbf);
    assert_eq!(m.cpu.fpu, seeded());
    m.cpu.step(&mut m.bus).unwrap();
    initialized(&m.cpu.fpu);
}

#[test]
fn unsupported_neighbors_vm_and_cpuid() {
    let (mut c, mut b) = cpu(&[0x0f, 0xa2]);
    c.step(&mut b).unwrap();
    assert_eq!((c.ip, c.r[SP]), (0x2060, 0x8ffa));
    assert_eq!(&b.mem[0x8ffa..0x9000], &[0, 0x10, 0, 0, 2, 0]);
    for code in [
        &[0xd9, 0xe8][..],
        &[0xdb, 0xe2],
        &[0xdd, 0xe0],
        &[0xdf, 0xe1],
        &[0xd9, 0x2f],
    ] {
        let (mut c, mut b) = cpu(code);
        c.fpu = seeded();
        assert!(
            c.step(&mut b)
                .unwrap_err()
                .starts_with("unimplemented x87 opcode"),
            "{code:02x?}"
        );
        assert_eq!(c.fpu, seeded());
    }
    for code in [
        &[0xdb, 0xe3][..],
        &[0xd9, 0x3f],
        &[0xdd, 0x3f],
        &[0xdf, 0xe0],
    ] {
        let (mut c, mut b) = cpu(code);
        b.profile = Profile::Pc9801Vm;
        assert!(c.step(&mut b).unwrap_err().contains("unsupported opcode"));
    }
}

#[test]
fn stores_keep_bus_a20_and_rom_protection() {
    for (opcode, word) in [(0xd9, [0x40, 8]), (0xdd, [0xbf, 0xff])] {
        let (mut c, mut b) = cpu(&[0x64, 0x66, opcode, 0x3e, 0x20, 0]);
        c.fpu = seeded();
        c.extra_segments[0] = 0xffff;
        for a20 in [false, true] {
            if a20 {
                b.output(0xf2, 0);
            } else {
                b.output(0xf6, 3);
            }
            assert_eq!(b.a20, a20);
            b.mem[0xf..0x15].fill(0xa5);
            b.mem[0x10000f..0x100015].fill(0xa5);
            c.ip = 0x1000;
            c.step(&mut b).unwrap();
            let address = if a20 { 0x10000f } else { 0xf };
            let untouched = if a20 { 0xf } else { 0x10000f };
            assert_eq!(
                &b.mem[address..address + 6],
                &[0xa5, word[0], word[1], 0xa5, 0xa5, 0xa5]
            );
            assert_eq!(&b.mem[untouched..untouched + 6], &[0xa5; 6]);
        }
        c.extra_segments[0] = 0xf000;
        b.mem[0xf001f..0xf0025].fill(0xa5);
        c.ip = 0x1000;
        c.step(&mut b).unwrap();
        assert_eq!(&b.mem[0xf001f..0xf0025], &[0xa5; 6]);
        assert_eq!(c.fpu, seeded());
    }
}

#[test]
fn original_boot_probe_cold_and_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/fpu_init/fpu_init.hdm"
    ))
    .expect("run python3 tools/prepare_fpu_init_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "fpu_init.hdm").unwrap();
    // Literal specification-derived bytes; no CPU or manifest helpers.
    let expected: [u8; 64] = [
        0xa5, 0, 0, 0xcd, 0xab, 0xa5, 0xa5, 0xa5, 0xa5, 0x7f, 3, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
        0xa5, 0, 0, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0x7f, 3, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
        0xa5, 0, 0, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0x7f, 3, 0, 0, 0xa5, 0xa5, 0xa5, 0xa5, 0,
        0, 0xef, 0xcd, 0xa5, 0xa5, 0xa5, 0xa5, 2, 4, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    ];
    for run in 0..2 {
        if run == 1 {
            m.reset();
        }
        let mut slots = 0;
        for _ in 0..20 {
            slots += m.step(100_000);
            assert!(m.error.is_none(), "{:?}", m.error);
            let text = m.bus.text();
            if text.contains("FPU PASS") || text.contains("FPU FAIL") {
                break;
            }
        }
        assert!(m.bus.text().contains("FPU PASS"), "{}", m.bus.text());
        assert!(!m.bus.text().contains("FAIL"));
        assert!(slots <= 2_000_000);
        assert_eq!(&m.bus.mem[0x8000..0x8040], &expected);
        assert_eq!(&m.bus.mem[0xfffd..0x10000], &[0xa5, 0x7f, 3]);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        initialized(&m.cpu.fpu);
        eprintln!("FPU boot run {run}: {slots} slots");
    }
}

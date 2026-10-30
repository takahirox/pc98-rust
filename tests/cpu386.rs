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
    for n in [0, 1, 5, 6, 12, 13] {
        let address = 0x2000 + n * 16;
        b.mem[n * 4..n * 4 + 2].copy_from_slice(&(address as u16).to_le_bytes());
        b.mem[address] = 0xf4;
    }
    (c, b)
}
fn steps(c: &mut Cpu, b: &mut Bus, n: usize) {
    for _ in 0..n {
        c.step(b).unwrap();
    }
}
fn fault(c: &Cpu, b: &Bus, vector: u32, restart: u16) {
    assert_eq!(c.ip, 0x2000 + vector * 16);
    assert_eq!(c.r[SP], 0x8ffa);
    assert_eq!(b.read16(0x8ffa), restart);
    assert_eq!(b.read16(0x8ffc), 0);
}

#[test]
fn smsw_minimal_register_reproduction() {
    let (mut c, mut b) = cpu(&[0x0f, 0x01, 0xe0, 0x90]);
    c.set_reg32(AX, 0xabcdffff);
    c.flags = 2 | CF | PF | AF | ZF | SF | DF | OF;
    let flags = c.flags;
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0xabcd0010);
    assert_eq!(c.flags, flags);
    assert_eq!(c.ip, 0x1003);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x1004);
}

#[test]
fn operands_preserve_upper_words_and_repeated_overrides_do_not_toggle() {
    let (mut c, mut b) = cpu(&[
        0x66, 0xb8, 0x78, 0x56, 0x34, 0x12, // mov eax,12345678
        0xb8, 0xcd, 0xab, // mov ax,abcd
        0xb4, 0xef, // mov ah,ef
        0x66, 0x66, 0x40, // inc eax (one override, even when repeated)
        0x66, 0x50, 0x66, 0x5b, // push eax / pop ebx
        0x66, 0x93, // xchg eax,ebx
    ]);
    steps(&mut c, &mut b, 7);
    assert_eq!(c.reg32(AX), 0x1234efce);
    assert_eq!(c.reg32(BX), c.reg32(AX));
    assert_eq!(c.r[SP], 0x9000);
    assert_eq!(c.extended_retired, 5);
}

#[test]
fn dword_adc_sbb_and_logic_flags_match_wide_integer_arithmetic() {
    let values = [0, 1, 15, 0x7fffffff, 0x80000000, 0xfffffff0, u32::MAX];
    for a in values {
        for rhs in values {
            for carry in [0u16, 1] {
                for sub in [false, true] {
                    let (mut c, mut b) = cpu(&[0x66, if sub { 0x19 } else { 0x11 }, 0xd8]); // sbb/adc eax,ebx
                    c.set_reg32(AX, a);
                    c.set_reg32(BX, rhs);
                    c.flags = 2 | carry;
                    c.step(&mut b).unwrap();
                    let exact = if sub {
                        a as i64 - rhs as i64 - carry as i64
                    } else {
                        a as i64 + rhs as i64 + carry as i64
                    };
                    let signed = if sub {
                        a as i32 as i64 - rhs as i32 as i64 - carry as i64
                    } else {
                        a as i32 as i64 + rhs as i32 as i64 + carry as i64
                    };
                    let result = exact as u32;
                    assert_eq!(c.reg32(AX), result);
                    assert_eq!(c.flags & CF != 0, !(0..=u32::MAX as i64).contains(&exact));
                    assert_eq!(c.flags & OF != 0, signed != signed as i32 as i64);
                    assert_eq!(c.flags & AF != 0, (a ^ rhs ^ result) & 16 != 0);
                    assert_eq!(c.flags & ZF != 0, result == 0);
                    assert_eq!(c.flags & SF != 0, result >> 31 != 0);
                    assert_eq!(
                        c.flags & PF != 0,
                        (result as u8).count_ones().is_multiple_of(2)
                    );
                }
            }
        }
    }
}

#[test]
fn sib_scale_displacement_and_default_segments() {
    // Address expressions intentionally share DS/SS offsets to distinguish which
    // segment is selected. An EBP index does not make DS into SS.
    let cases: &[(&[u8], u32, usize)] = &[
        (&[0x67, 0x8b, 0x04, 0x8b], 0x1020, DS),       // [ebx+ecx*4]
        (&[0x67, 0x8b, 0x44, 0x8b, 0xf0], 0x1010, DS), // disp8 -16
        (
            &[0x67, 0x8b, 0x84, 0x8b, 0xe0, 0xff, 0xff, 0xff],
            0x1000,
            DS,
        ), // disp32 -32
        (&[0x67, 0x8b, 0x44, 0x25, 0], 0x1100, SS),    // [ebp] with no index
        (&[0x67, 0x8b, 0x04, 0x24], 0x9000, SS),       // [esp]
        (&[0x67, 0x8b, 0x04, 0x6b], 0x3200, DS),       // [ebx+ebp*2]
        (&[0x67, 0x8b, 0x04, 0x8d, 0x00, 0x10, 0, 0], 0x1020, DS), // index, no base
        (&[0x67, 0x8b, 0x05, 0x80, 0x10, 0, 0], 0x1080, DS), // disp32 only
        (&[0x3e, 0x67, 0x8b, 0x04, 0x24], 0x9000, DS), // DS overrides ESP
    ];
    for &(code, offset, segment) in cases {
        let (mut c, mut b) = cpu(code);
        c.s[DS] = 0x3000;
        c.s[SS] = 0x4000;
        c.set_reg32(BX, 0x1000);
        c.set_reg32(CX, 8);
        c.set_reg32(BP, 0x1100);
        b.write_seg(c.s[segment], offset as u16, 0xabcd, true);
        c.step(&mut b).unwrap();
        assert_eq!(c.r[AX], 0xabcd, "{code:02x?}");
    }
    let (mut c, mut b) = cpu(&[0x66, 0x67, 0x8d, 0x44, 0x8b, 0x40]); // lea eax,[ebx+ecx*4+64]
    c.set_reg32(BX, 0xfffffff0);
    c.set_reg32(CX, 4);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0x40); // Addition wraps at 32 bits before any segment check.
}

#[test]
fn address32_does_not_bypass_real_mode_segment_limits_or_write_partial_data() {
    for (code, vector) in [
        (&[0x66, 0x67, 0x89, 0x03][..], 13), // mov [ebx],eax, DS crossing
        (&[0x66, 0x67, 0x89, 0x45, 0][..], 12), // mov [ebp],eax, SS crossing
    ] {
        let (mut c, mut b) = cpu(code);
        c.set_reg32(AX, 0x12345678);
        c.set_reg32(BX, 0xfffe);
        c.set_reg32(BP, 0xfffe);
        b.mem[0xfffe..0x10002].fill(0xaa);
        c.step(&mut b).unwrap();
        fault(&c, &b, vector, 0x1000);
        assert_eq!(&b.mem[0xfffe..0x10002], &[0xaa; 4]);
        assert_eq!(c.reg32(AX), 0x12345678);
    }
    let (mut c, mut b) = cpu(&[0x67, 0x8a, 0x03]);
    c.set_reg32(BX, 0x10000); // Not an extended-memory shortcut: #GP even for a byte.
    c.step(&mut b).unwrap();
    fault(&c, &b, 13, 0x1000);
    let (mut c, mut b) = cpu(&[0x8b, 0x07]); // 16-bit word crossing is also #GP on Ap.
    c.r[BX] = 0xffff;
    c.step(&mut b).unwrap();
    fault(&c, &b, 13, 0x1000);
}

#[test]
fn fs_gs_moffs_and_hma_respect_a20_and_rom() {
    let (mut c, mut b) = cpu(&[
        0xb8, 0xff, 0xff, 0x8e, 0xe0, // mov ax,ffff / mov fs,ax
        0x64, 0x66, 0x67, 0xa1, 0x10, 0, 0, 0, // mov eax,fs:[dword 10h]
        0x64, 0x66, 0x67, 0xa3, 0x14, 0, 0, 0, // mov fs:[dword 14h],eax
    ]);
    b.mem[0..4].copy_from_slice(&0x11223344u32.to_le_bytes());
    b.mem[0x100000..0x100004].copy_from_slice(&0xaabbccddu32.to_le_bytes());
    steps(&mut c, &mut b, 3);
    assert_eq!(c.reg32(AX), 0x11223344);
    c.ip = 0x1005;
    b.output(0xf2, 0);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX), 0xaabbccdd);
    c.step(&mut b).unwrap();
    assert_eq!(b.read16(0x100004), 0xccdd);
    assert_eq!(b.read16(4), 0x2010);
    // A legal segmented write to firmware never changes ROM.
    c.extra_segments[0] = 0xf000;
    c.ip = 0x100d;
    b.mem[0xf0014..0xf0018].fill(0x55);
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0xf0014..0xf0018], &[0x55; 4]);
}

#[test]
fn rep_dword_strings_have_independent_operand_and_address_sizes_and_are_budgeted() {
    let (mut c, mut b) = cpu(&[0x66, 0x67, 0xf3, 0xa5]); // rep movsd using ECX,ESI,EDI
    c.set_reg32(SI, 0x4000);
    c.set_reg32(DI, 0x5000);
    c.set_reg32(CX, 0x10002);
    b.mem[0x4000..0x4008].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(CX), 0x10001);
    assert_eq!(c.reg32(SI), 0x4004);
    assert_eq!(c.ip, 0x1000);
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0x5000..0x5008], &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(c.reg32(CX), 0x10000); // Cannot mistakenly terminate on CX == 0.
    c.set_reg32(CX, 1);
    c.flags |= DF;
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x1004);
    assert_eq!(c.reg32(SI), 0x4004);
    assert_eq!(c.reg32(DI), 0x5004);
    // A zero full ECX suppresses all memory accesses, even invalid indices.
    c.ip = 0x1000;
    c.set_reg32(SI, u32::MAX);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x1004);
}

#[test]
fn signed_dword_multiply_divide_and_overflow_faults() {
    let (mut c, mut b) = cpu(&[0x66, 0xf7, 0xeb, 0x66, 0xf7, 0xfb]); // imul ebx / idiv ebx
    c.set_reg32(AX, (-12345i32) as u32);
    c.set_reg32(BX, 200_000);
    c.step(&mut b).unwrap();
    assert_eq!(
        ((c.reg32(DX) as u64) << 32 | c.reg32(AX) as u64) as i64,
        -2_469_000_000
    );
    assert_ne!(c.flags & OF, 0);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(AX) as i32, -12345);
    assert_eq!(c.reg32(DX), 0);
    for divisor in [0, u32::MAX] {
        let (mut c, mut b) = cpu(&[0x66, 0xf7, 0xfb]);
        c.set_reg32(DX, u32::MAX);
        c.set_reg32(AX, 0x80000000); // INT32_MIN / -1 exceeds signed 32 bits.
        c.set_reg32(BX, divisor);
        c.step(&mut b).unwrap();
        fault(&c, &b, 0, 0x1000);
        assert_eq!(c.reg32(AX), 0x80000000);
        assert_eq!(c.reg32(DX), u32::MAX);
    }
    // INT64_MIN/-1 must fault without overflowing the Rust host division.
    let (mut c, mut b) = cpu(&[0x66, 0xf7, 0xfb]);
    c.set_reg32(DX, 0x80000000);
    c.set_reg32(BX, u32::MAX);
    c.step(&mut b).unwrap();
    fault(&c, &b, 0, 0x1000);
}

#[test]
fn extension_bit_scan_shift_and_486_atomic_operations() {
    let (mut c, mut b) = cpu(&[
        0x66, 0x0f, 0xbe, 0xc3, // movsx eax,bl
        0x66, 0x0f, 0xb7, 0xcb, // movzx ecx,bx
        0x66, 0x0f, 0xc8, // bswap eax
        0x66, 0x0f, 0xbc, 0xd0, // bsf edx,eax
        0x66, 0x0f, 0xbd, 0xf0, // bsr esi,eax
        0x66, 0x0f, 0xa4, 0xc8, 4, // shld eax,ecx,4
        0x66, 0x0f, 0xac, 0xc8, 4, // shrd eax,ecx,4
    ]);
    c.r[BX] = 0x8080;
    steps(&mut c, &mut b, 2);
    assert_eq!(c.reg32(AX), 0xffffff80);
    assert_eq!(c.reg32(CX), 0x8080);
    steps(&mut c, &mut b, 3);
    assert_eq!(c.reg32(AX), 0x80ffffff);
    assert_eq!(c.reg32(DX), 0);
    assert_eq!(c.reg32(SI), 31);
    steps(&mut c, &mut b, 2);
    assert_eq!(c.reg32(AX), 0x00ffffff);
    let (mut c, mut b) = cpu(&[0x66, 0x0f, 0xc1, 0xd8, 0x66, 0x0f, 0xb1, 0xca]); // xadd eax,ebx / cmpxchg edx,ecx
    c.set_reg32(AX, 10);
    c.set_reg32(BX, 20);
    c.set_reg32(DX, 30);
    c.set_reg32(CX, 0x12345678);
    steps(&mut c, &mut b, 2);
    assert_eq!(c.reg32(AX), 30);
    assert_eq!(c.reg32(BX), 10);
    assert_eq!(c.reg32(DX), 0x12345678);
    assert_ne!(c.flags & ZF, 0);
}

#[test]
fn bit_string_signed_memory_index_and_register_mask() {
    let (mut c, mut b) = cpu(&[0x66, 0x0f, 0xab, 0x0f, 0x66, 0x0f, 0xa3, 0xc8]); // bts [bx],ecx / bt eax,ecx
    c.r[BX] = 0x4004;
    c.set_reg32(CX, u32::MAX); // bit -1 => preceding dword, bit 31.
    c.set_reg32(AX, 0x80000000);
    c.step(&mut b).unwrap();
    assert_eq!(b.read16(0x4002), 0x8000);
    assert_eq!(c.flags & CF, 0);
    c.step(&mut b).unwrap();
    assert_ne!(c.flags & CF, 0);
}

#[test]
fn flags_push_pop_and_near_call_use_dword_stack_entries() {
    let (mut c, mut b) = cpu(&[0x66, 0x9d, 0x66, 0x9c]);
    b.mem[0x9000..0x9004].copy_from_slice(&0xffffffffu32.to_le_bytes());
    c.step(&mut b).unwrap();
    assert_eq!(c.flags, 0x7fd7);
    assert_eq!(c.flags_upper, 4); // AC supported, ID/VM/RF/reserved bits cannot be set.
    c.flags &= !TF;
    c.step(&mut b).unwrap();
    assert_eq!(b.read16(0x9002), 4);
    let (mut c, mut b) = cpu(&[0x66, 0xe8, 0xfa, 0x1f, 0, 0]); // call dword 3000h
    b.mem[0x3000..0x3002].copy_from_slice(&[0x66, 0xc3]);
    steps(&mut c, &mut b, 2);
    assert_eq!(c.ip, 0x1006);
    assert_eq!(c.r[SP], 0x9000);
}

#[test]
fn segment_pushes_preserve_upper_stack_bytes_and_check_only_the_selector() {
    for (code, value) in [
        (&[0x66, 0x06][..], 0x1234u16),    // ES
        (&[0x66, 0x0e][..], 0),            // CS
        (&[0x66, 0x16][..], 0x2000),       // SS
        (&[0x66, 0x1e][..], 0x3456),       // DS
        (&[0x66, 0x0f, 0xa0][..], 0x5678), // FS
        (&[0x66, 0x0f, 0xa8][..], 0x6789), // GS
    ] {
        for sp in [0x9000u16, 2, 3] {
            let (mut c, mut b) = cpu(code);
            c.s = [0x1234, 0, 0x2000, 0x3456];
            c.extra_segments = [0x5678, 0x6789];
            c.set_reg32(SP, 0xabcd0000 | sp as u32);
            let off = sp.wrapping_sub(4);
            let address = Cpu::phys(c.s[SS], off);
            b.mem[address..address + 4].fill(0xaa);
            if sp == 3 {
                // The selector crosses SS:FFFF. Even the #SS word frame cannot
                // fit this odd SP; neither operation may leave partial writes.
                assert_eq!(
                    c.step(&mut b).unwrap_err(),
                    "fault while delivering real-mode exception"
                );
                assert_eq!(c.ip, 0x1000);
                assert_eq!(c.reg32(SP), 0xabcd0003);
                assert_eq!(&b.mem[address..address + 4], &[0xaa; 4]);
            } else {
                // SP=2 wraps to FFFE: the selector fits, though a dword would not.
                c.step(&mut b).unwrap();
                assert_eq!(c.ip, 0x1000 + code.len() as u32);
                assert_eq!(c.reg32(SP), 0xabcd0000 | off as u32);
                assert_eq!(b.read16(address), value, "{code:02x?}");
                assert_eq!(&b.mem[address + 2..address + 4], &[0xaa; 2]);
            }
        }
    }
}

#[test]
fn nested_dword_enter_traverses_ebp_on_a_word_stack() {
    // Both operand and address overrides leave the real-mode stack size at 16.
    for code in [&[0x66, 0xc8, 0, 0, 3][..], &[0x67, 0x66, 0xc8, 0, 0, 3][..]] {
        for bp in [0x7000u32, 0x10000] {
            let (mut c, mut b) = cpu(code);
            c.s[SS] = 0x2000;
            c.set_reg32(BP, bp);
            let first = Cpu::phys(c.s[SS], 0) + (bp - 4) as usize;
            let second = Cpu::phys(c.s[SS], 0) + (bp - 8) as usize;
            b.mem[first..first + 4].copy_from_slice(&0x12345678u32.to_le_bytes());
            b.mem[second..second + 4].copy_from_slice(&0x9abcdef0u32.to_le_bytes());
            c.step(&mut b).unwrap();
            assert_eq!(c.ip, 0x1000 + code.len() as u32);
            assert_eq!(c.reg32(BP), 0x8ffc);
            assert_eq!(c.r[SP], 0x8ff0);
            let frame = Cpu::phys(c.s[SS], c.r[SP]);
            let expected: Vec<u8> = [0x8ffcu32, 0x9abcdef0, 0x12345678, bp]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect();
            assert_eq!(&b.mem[frame..frame + 16], expected.as_slice());
        }
        // A 32-bit chain above the real-mode SS limit must fault, not read
        // the low 16-bit alias. Only the real-mode #SS frame may be written.
        let (mut c, mut b) = cpu(code);
        c.s[SS] = 0x2000;
        c.set_reg32(BP, 0x17000);
        let mut expected = b.mem.clone();
        let frame = Cpu::phys(c.s[SS], 0x8ffa);
        expected[frame..frame + 6].copy_from_slice(&[0, 0x10, 0, 0, 2, 0]);
        c.step(&mut b).unwrap();
        assert_eq!((c.ip, c.r[SP], c.reg32(BP)), (0x20c0, 0x8ffa, 0x17000));
        assert!(
            b.mem == expected,
            "rejected ENTER changed memory outside #SS frame"
        );
    }
}

#[test]
fn software_interrupts_ignore_operand_size_and_clear_ac() {
    for (instruction, vector) in [
        (&[0xcd, 0x30][..], 0x30),
        (&[0xcc][..], 3),
        (&[0xce][..], 4),
    ] {
        for prefixed in [true, false] {
            let mut code = Vec::new();
            if prefixed {
                code.push(0x66);
            }
            code.extend_from_slice(instruction);
            let (mut c, mut b) = cpu(&[]);
            c.s[CS] = 0x100;
            c.s[SS] = 0x2000;
            b.mem[0x2000..0x2000 + code.len()].copy_from_slice(&code);
            b.mem[vector * 4..vector * 4 + 4].copy_from_slice(&[0, 4, 0, 3]);
            b.mem[0x3400] = 0xcf; // Ordinary IRET, even for a prefixed INT.
            let flags = 2 | CF | IF | TF | OF;
            c.flags = flags;
            c.flags_upper = 4;
            c.step(&mut b).unwrap();
            assert_eq!((c.s[CS], c.ip), (0x300, 0x400));
            assert_eq!(c.r[SP], 0x8ffa);
            assert_eq!(b.read16(0x28ffa), 0x1000 + code.len() as u16);
            assert_eq!(b.read16(0x28ffc), 0x100);
            assert_eq!(b.read16(0x28ffe), flags);
            assert_eq!(c.flags, flags & !(IF | TF));
            assert_eq!(c.flags_upper, 0);
            c.step(&mut b).unwrap();
            assert_eq!((c.s[CS], c.ip), (0x100, 0x1000 + code.len() as u32));
            assert_eq!(c.r[SP], 0x9000);
            assert_eq!(c.flags, flags);
            assert_eq!(c.flags_upper, 0); // A word IRET cannot restore AC.
        }
    }
    let (mut c, mut b) = cpu(&[0x66, 0xce]); // INTO without overflow never enters.
    c.flags_upper = 4;
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x1002);
    assert_eq!(c.r[SP], 0x9000);
    assert_eq!(c.flags_upper, 4);
}

#[test]
fn iretd_restores_an_explicit_dword_frame_including_ac() {
    let (mut c, mut b) = cpu(&[0x66, 0xcf]);
    // IRETD consumes three dwords supplied by software, unlike an INT frame.
    for (i, value) in [0x4567u32, 0x1234, 0x40000 | 2 | CF as u32 | IF as u32]
        .into_iter()
        .enumerate()
    {
        b.mem[0x9000 + i * 4..0x9004 + i * 4].copy_from_slice(&value.to_le_bytes());
    }
    c.step(&mut b).unwrap();
    assert_eq!((c.s[CS], c.ip), (0x1234, 0x4567));
    assert_eq!(c.r[SP], 0x900c);
    assert_eq!(c.flags, 2 | CF | IF);
    assert_eq!(c.flags_upper, 4);
}

#[test]
fn real_iret_wraps_between_stack_operands_but_not_inside_them() {
    for wide in [false, true] {
        let bytes = if wide { 4u16 } else { 2 };
        for sp in [0xfff0u16, 0xfff8, 0xfffc, 0xfffe, 0xffff] {
            let (mut c, mut b) = cpu(if wide { &[0x66, 0xcf] } else { &[0xcf] });
            c.s[SS] = 0x3000;
            c.r[SP] = sp;
            c.flags = 2;
            let fits = (0..3).all(|i| sp.wrapping_add(i * bytes) as u32 + bytes as u32 <= 0x10000);
            for (i, value) in [0x1234u32, 0x200, 0x40203].into_iter().enumerate() {
                let off = sp.wrapping_add(i as u16 * bytes) as usize;
                let raw = value.to_le_bytes();
                // A rejected dword's bytes beyond the limit are guards, not
                // wrapped pieces of a legal operand.
                b.mem[0x30000 + off..0x30000 + off + bytes as usize]
                    .copy_from_slice(&raw[..bytes as usize]);
            }
            let memory = b.mem[0x30000..0x40004].to_vec();
            assert_eq!(c.iret_frame_metadata(&b, wide).3, fits);
            c.step(&mut b).unwrap();
            if fits {
                assert_eq!(c.fault_count, 0, "wide={wide} sp={sp:04x}");
                assert_eq!(
                    (c.s[CS], c.ip, c.r[SP]),
                    (0x200, 0x1234, sp.wrapping_add(3 * bytes))
                );
                assert_eq!(c.flags & (CF | IF), CF | IF);
                assert_eq!(c.flags_upper & 4, if wide { 4 } else { 0 });
                assert_eq!(&b.mem[0x30000..0x40004], memory);
            } else {
                assert_eq!(c.last_fault, Some((12, 0)));
                assert_eq!(c.ip, 0x20c0);
                assert_eq!(c.r[SP], sp.wrapping_sub(6));
                assert_eq!(b.read16(0x30000 + c.r[SP] as usize), 0x1000);
                assert_eq!(&b.mem[0x40000..0x40004], &memory[0x10000..]);
            }
        }
    }
}

#[test]
fn hardware_irq_and_fault_entry_clear_ac() {
    let (mut c, mut b) = cpu(&[0x90]);
    b.mem[9 * 4..9 * 4 + 4].copy_from_slice(&[0, 0x30, 0, 0]);
    b.mem[0x3000] = 0xf4;
    c.flags = 2 | IF | TF;
    c.flags_upper = 4;
    c.halted = true;
    b.pics[0].mask = 0xfd;
    b.pics[0].request(1);
    c.step(&mut b).unwrap();
    assert_eq!(b.pics[0].isr, 2);
    assert_eq!(c.ip, 0x3001);
    assert_eq!(c.r[SP], 0x8ffa);
    assert_eq!(b.read16(0x8ffa), 0x1000);
    assert_eq!(b.read16(0x8ffe), 2 | IF | TF);
    assert_eq!(c.flags, 2);
    assert_eq!(c.flags_upper, 0);

    for (code, vector) in [
        (&[0x66, 0xf7, 0xf3][..], 0), // DIV EBX with a zero divisor: #DE.
        (&[0x8e, 0xc8][..], 6),       // MOV CS,AX: #UD.
        (&[0x8b, 0x07][..], 13),      // MOV AX,[BX] across DS limit: #GP.
        (&[0x8b, 0x46, 0][..], 12),   // MOV AX,[BP] across SS limit: #SS.
    ] {
        let (mut c, mut b) = cpu(code);
        if vector >= 12 {
            c.r[BX] = 0xffff;
            c.r[BP] = 0xffff;
        }
        c.flags = 2 | IF | TF;
        c.flags_upper = 4;
        c.step(&mut b).unwrap();
        fault(&c, &b, vector, 0x1000);
        assert_eq!(b.read16(0x8ffe), 2 | IF | TF);
        assert_eq!(c.flags, 2);
        assert_eq!(c.flags_upper, 0);
    }
}

#[test]
fn prefix_length_fetch_boundary_and_invalid_encodings_fault_at_start() {
    let mut too_long = vec![0x66; 15];
    too_long.push(0x90);
    let (mut c, mut b) = cpu(&too_long);
    c.step(&mut b).unwrap();
    fault(&c, &b, 13, 0x1000);
    let mut exact = vec![0x66; 14];
    exact.push(0x90);
    let (mut c, mut b) = cpu(&exact);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x100f);
    let (mut c, mut b) = cpu(&[]);
    c.ip = 0xffff;
    b.mem[0xffff] = 0xb8; // Immediate fetch exceeds the 64 KiB CS limit.
    c.step(&mut b).unwrap();
    fault(&c, &b, 13, 0xffff);
    for code in [
        &[0x0f, 0x10, 0xc0][..], // V30-only / SSE (not i486)
        &[0x8d, 0xc0][..],       // LEA requires memory
        &[0xff, 0xd8][..],       // far call requires memory
        &[0xff, 0xf8][..],       // invalid FF group 7
        &[0x63, 0xc0][..],       // ARPL is invalid in real mode
        &[0x8e, 0xc8][..],       // mov cs,ax
        &[0x8f, 0xc8][..],       // invalid pop group
        &[0xc7, 0xc8, 0, 0][..], // invalid mov immediate group
        &[0xfe, 0xd0][..],       // byte indirect call is invalid
    ] {
        let (mut c, mut b) = cpu(code);
        c.step(&mut b).unwrap();
        fault(&c, &b, 6, 0x1000);
    }
}

#[test]
fn independent_cpulevel_runs_386_code_and_detects_486_flags_through_guest_dos() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/dos-apps/cpu386.hdm"
    ))
    .expect("run python3 tools/prepare_cpu_fixture.py first");
    m.insert(0, image, "cpu386.hdm").unwrap();
    m.reset();
    for _ in 0..100 {
        m.step(100_000);
        assert!(m.error.is_none(), "{:?}", m.error);
        if m.bus.text().contains("CPULEVEL 486 VERIFIED") {
            break;
        }
    }
    let text = m.bus.text();
    assert!(text.contains("i486 or compatible"), "{text}");
    assert!(text.contains("CPUID support check negative"), "{text}");
    assert!(text.contains("CPULEVEL 486 VERIFIED"), "{text}");
    assert!(
        m.cpu.extended_retired >= 26,
        "independent guest must execute operand overrides: {}",
        m.cpu.extended_retired
    );
    assert!(m.bus.fdc.reads > 10);
    m.reset();
    assert_eq!(m.cpu.reg32(AX), 0);
    assert_eq!(m.cpu.flags_upper, 0);
    assert_eq!(m.cpu.extra_segments, [0; 2]);
    assert_eq!(m.cpu.extended_retired, 0);
}

#[test]
fn lock_only_accepts_memory_read_modify_write_operations() {
    for code in [
        &[0xf0, 0x66, 0x01, 0xd8][..],          // LOCK add eax,ebx (register)
        &[0xf0, 0x66, 0x89, 0x07][..],          // LOCK mov [bx],eax
        &[0xf0, 0x66, 0x83, 0x3f, 0][..],       // LOCK cmp [bx],0
        &[0xf0, 0x66, 0x0f, 0xba, 0x27, 0][..], // LOCK bt [bx],0
        &[0xf0, 0x66, 0xf7, 0x27][..],          // LOCK mul [bx]
    ] {
        let (mut c, mut b) = cpu(code);
        c.r[BX] = 0x4000;
        b.mem[0x4000..0x4004].fill(0xaa);
        c.step(&mut b).unwrap();
        fault(&c, &b, 6, 0x1000);
        assert_eq!(&b.mem[0x4000..0x4004], &[0xaa; 4]);
    }
    let (mut c, mut b) = cpu(&[0xf0, 0x66, 0x01, 0x07]); // LOCK add [bx],eax
    c.r[BX] = 0x4000;
    c.set_reg32(AX, 0x12345678);
    c.step(&mut b).unwrap();
    assert_eq!(b.read16(0x4000), 0x5678);
    assert_eq!(b.read16(0x4002), 0x1234);
}

#[test]
fn stack_fault_restores_sp_and_does_not_write_a_partial_dword() {
    let (mut c, mut b) = cpu(&[0x66, 0x8f, 0x07]); // pop dword [bx], bad destination
    c.r[BX] = 0xfffe;
    b.mem[0xfffe..0x10002].fill(0xaa);
    c.step(&mut b).unwrap();
    fault(&c, &b, 13, 0x1000); // SP points to original stack minus the exception frame.
    assert_eq!(&b.mem[0xfffe..0x10002], &[0xaa; 4]);
    // The push itself crosses the SS limit; delivery of #SS still fits a word frame.
    let (mut c, mut b) = cpu(&[0x66, 0x50]);
    c.r[SP] = 2;
    c.set_reg32(AX, 0x12345678);
    b.mem[0xfffe..0x10002].fill(0xaa);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x20c0);
    assert_eq!(c.r[SP], 0xfffc);
    assert_eq!(b.read16(0xfffc), 0x1000);
    assert_eq!(&b.mem[0x10000..0x10002], &[0xaa; 2]);
}

#[test]
fn single_step_is_a_trap_and_ss_load_shadow_differs_from_sti() {
    let (mut c, mut b) = cpu(&[0x66, 0x40]); // inc eax
    c.flags |= TF;
    c.step(&mut b).unwrap();
    fault(&c, &b, 1, 0x1002); // Trap resumes after the instruction.
    assert_eq!(c.reg32(AX), 1);
    let (mut c, mut b) = cpu(&[0x8e, 0xd0, 0x90]); // mov ss,ax / nop
    c.flags |= TF;
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x1002);
    c.step(&mut b).unwrap();
    fault(&c, &b, 1, 0x1003);
    let (mut c, mut b) = cpu(&[0xfb]); // STI shadows maskable IRQ, not #DB.
    c.flags |= TF;
    c.step(&mut b).unwrap();
    fault(&c, &b, 1, 0x1001);
}

#[test]
fn short_branches_use_operand_size_and_loop_uses_address_size_for_count() {
    // Short displacements still obey the operand-size attribute on 386/486.
    // A 16-bit target wraps, a 32-bit target beyond the real-mode CS limit faults.
    for opcode in [0xeb, 0x74, 0xe2, 0xe3] {
        for wide in [false, true] {
            let (mut c, mut b) = cpu(&[]);
            c.flags |= ZF;
            c.r[CX] = if opcode == 0xe3 { 0 } else { 2 };
            c.ip = if wide { 0xfffc } else { 0xfffd };
            let start = c.ip as u16;
            let code: &[u8] = if wide {
                &[0x66, opcode, 0x7f]
            } else {
                &[opcode, 0x7f]
            };
            b.mem[c.ip as usize..0xffff].copy_from_slice(code);
            c.step(&mut b).unwrap();
            if wide {
                fault(&c, &b, 13, start);
            } else {
                assert_eq!(c.ip, 0x7e);
            }
        }
    }
    let (mut c, mut b) = cpu(&[0x67, 0xe2, 0xfd]); // loop to itself using ECX
    c.set_reg32(CX, 0x10001);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(CX), 0x10000);
    assert_eq!(c.ip, 0x1000);
    let (mut c, mut b) = cpu(&[0xe2, 0xfe]); // same CX low word but default address size
    c.set_reg32(CX, 0x10001);
    c.step(&mut b).unwrap();
    assert_eq!(c.reg32(CX), 0x10000);
    assert_eq!(c.ip, 0x1002);
}

// Original Issue #35 GDTR acceptance. No descriptor table is dereferenced.
fn gdtr_bytes(limit: u16, base: u32) -> [u8; 6] {
    let mut bytes = [0; 6];
    bytes[..2].copy_from_slice(&limit.to_le_bytes());
    bytes[2..].copy_from_slice(&base.to_le_bytes());
    bytes
}

#[test]
fn gdtr_operand_and_address_sizes_preserve_state_and_exact_store_width() {
    for operand32 in [false, true] {
        for address32 in [false, true] {
            for (limit, base) in [
                (0, 0),
                (0xffff, u32::MAX),
                (0xffff, 0xfedcba98),
                (0x1234, 0x12345678),
            ] {
                let mut code = vec![];
                if operand32 {
                    code.push(0x66);
                }
                if address32 {
                    code.push(0x67);
                }
                code.extend_from_slice(&[
                    0x0f,
                    0x01,
                    if address32 { 0x15 } else { 0x16 },
                    0x01,
                    0x40,
                ]);
                if address32 {
                    code.extend_from_slice(&[0, 0]);
                }
                let store_start = code.len();
                if operand32 {
                    code.push(0x66);
                }
                if address32 {
                    code.push(0x67);
                }
                code.extend_from_slice(&[
                    0x0f,
                    0x01,
                    if address32 { 0x05 } else { 0x06 },
                    0x11,
                    0x40,
                ]);
                if address32 {
                    code.extend_from_slice(&[0, 0]);
                }
                let (mut c, mut b) = cpu(&code);
                for r in 0..8 {
                    c.set_reg32(r, 0xabc00000 + r as u32);
                }
                c.r[SP] = 0x9000;
                c.s[DS] = 0x3000;
                c.s[ES] = 0x4000;
                c.extra_segments = [0x5000, 0x6000];
                c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
                c.flags_upper = 4;
                let saved = c.clone();
                b.mem[0x34001..0x34007].copy_from_slice(&gdtr_bytes(limit, base));
                b.mem[0x34010..0x34018].fill(0xa5);
                c.step(&mut b).unwrap();
                assert_eq!(c.r, saved.r);
                assert_eq!(c.upper, saved.upper);
                assert_eq!(c.s, saved.s);
                assert_eq!(c.extra_segments, saved.extra_segments);
                assert_eq!((c.flags, c.flags_upper), (saved.flags, saved.flags_upper));
                assert_eq!(c.ip, 0x1000 + store_start as u32);
                c.step(&mut b).unwrap();
                assert_eq!(c.ip, 0x1000 + code.len() as u32);
                let expected_base = if operand32 { base } else { base & 0xffffff };
                assert_eq!(&b.mem[0x34011..0x34017], &gdtr_bytes(limit, expected_base));
                assert_eq!((b.mem[0x34010], b.mem[0x34017]), (0xa5, 0xa5));
                assert_eq!(c.r, saved.r);
                assert_eq!(c.upper, saved.upper);
                assert_eq!(c.s, saved.s);
                assert_eq!(c.extra_segments, saved.extra_segments);
                assert_eq!((c.flags, c.flags_upper), (saved.flags, saved.flags_upper));
            }
        }
    }
}

#[test]
fn gdtr_full_base_survives_both_stores_then_word_load_clears_high_byte() {
    let (mut c, mut b) = cpu(&[
        0x66, 0x0f, 0x01, 0x16, 0, 0x40, // lgdt dword [4000]
        0x0f, 0x01, 0x06, 0x10, 0x40, // sgdt word [4010]
        0x66, 0x0f, 0x01, 0x06, 0x20, 0x40, // sgdt dword [4020]
        0x0f, 0x01, 0x16, 0, 0x40, // lgdt word [4000]
        0x0f, 0x01, 0x06, 0x30, 0x40,
    ]);
    let source = [0x34, 0x12, 0x78, 0x56, 0x34, 0x12];
    b.mem[0x4000..0x4006].copy_from_slice(&source);
    steps(&mut c, &mut b, 5);
    assert_eq!(&b.mem[0x4010..0x4016], &source);
    assert_eq!(&b.mem[0x4020..0x4026], &source);
    assert_eq!(&b.mem[0x4030..0x4036], &[0x34, 0x12, 0x78, 0x56, 0x34, 0]);
}

#[test]
fn gdtr_sib_default_ss_and_all_segment_overrides_with_repeated_prefixes() {
    // Prefixes plus ModR/M+SIB/displacement; last segment override wins.
    let cases: &[(&[u8], &[u8], usize, usize)] = &[
        (&[], &[0x54, 0x8b, 1], DS, 0x4022), // [ebx+ecx*4+1]
        (&[], &[0x54, 0x25, 0], SS, 0x4101), // [ebp]
        (&[], &[0x14, 0x24], SS, 0x9000),    // [esp]
        (&[0x26], &[0x13], ES, 0x4001),
        (&[0x2e], &[0x13], CS, 0x4001),
        (&[0x36], &[0x13], SS, 0x4001),
        (&[0x3e], &[0x54, 0x25, 0], DS, 0x4101),
        (&[0x64], &[0x13], 4, 0x4001),
        (&[0x65], &[0x13], 5, 0x4001),
        (&[0x26, 0x64, 0x65], &[0x13], 5, 0x4001),
    ];
    for &(prefixes, encoding, segment, offset) in cases {
        for address32 in [false, true] {
            // SIB encodings apply to address32; word case [bx] covers the overrides.
            let mut code = prefixes.to_vec();
            code.extend_from_slice(&[0x66, 0x66]);
            if address32 {
                code.extend_from_slice(&[0x67, 0x67]);
            }
            code.extend_from_slice(&[0x0f, 0x01]);
            code.extend_from_slice(if address32 { encoding } else { &[0x17] });
            let (mut c, mut b) = cpu(&code);
            c.s[DS] = 0x3000;
            c.s[ES] = 0x4000;
            c.s[SS] = 0x5000;
            c.extra_segments = [0x6000, 0x7000];
            c.set_reg32(BX, 0x4001);
            c.set_reg32(BP, 0x4101);
            c.set_reg32(CX, 8);
            let s = if !address32 && prefixes.is_empty() {
                DS
            } else {
                segment
            };
            let off = if address32 { offset } else { 0x4001 };
            let seg = if s < 4 {
                c.s[s]
            } else {
                c.extra_segments[s - 4]
            };
            let a = seg as usize * 16 + off;
            b.mem[a..a + 6].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
            c.step(&mut b).unwrap();
            assert_eq!(
                (c.gdtr_limit, c.gdtr_base),
                (0x201, 0x6050403),
                "{code:02x?}"
            );
            // Exercise the identical addressing expression for SGDT.
            c.ip = 0x1000;
            let modrm = 0x1000 + prefixes.len() + 2 + if address32 { 2 } else { 0 } + 2;
            b.mem[modrm] &= !0x38;
            b.mem[a - 1..a + 7].fill(0xa5);
            c.step(&mut b).unwrap();
            assert_eq!(&b.mem[a..a + 6], &[1, 2, 3, 4, 5, 6]);
            assert_eq!((b.mem[a - 1], b.mem[a + 6]), (0xa5, 0xa5));
        }
    }
}

#[test]
fn gdtr_six_byte_limit_faults_are_atomic_gp_even_for_ss_and_restart() {
    for operand32 in [false, true] {
        for load in [false, true] {
            for stack in [false, true] {
                for address32 in [false, true] {
                    for offset in [0xfffa, 0xfffb, 0x10000, u32::MAX] {
                        if !address32 && offset > 0xffff {
                            continue;
                        }
                        let mut code = vec![];
                        if operand32 {
                            code.push(0x66);
                        }
                        if address32 {
                            code.push(0x67);
                        }
                        code.extend_from_slice(&[
                            0x0f,
                            0x01,
                            (if stack { 0x46 } else { 0x47 }) | if load { 0x10 } else { 0 },
                            0,
                        ]);
                        // 32-bit rm=5 is EBP; 16-bit rm=6 is BP.
                        if address32 {
                            let m = code.len() - 2;
                            code[m] = (code[m] & !7) | if stack { 5 } else { 3 };
                        }
                        let (mut c, mut b) = cpu(&code);
                        c.set_reg32(if stack { BP } else { BX }, offset);
                        c.gdtr_limit = 0x1234;
                        c.gdtr_base = 0x98765432;
                        b.mem[0xfff9..0x10008].fill(0xa5);
                        if offset == 0xfffa {
                            c.step(&mut b).unwrap();
                            if load {
                                assert_eq!(
                                    (c.gdtr_limit, c.gdtr_base),
                                    (0xa5a5, if operand32 { 0xa5a5a5a5 } else { 0x00a5a5a5 })
                                );
                            } else {
                                assert_eq!(
                                    &b.mem[0xfffa..0x10000],
                                    &gdtr_bytes(0x1234, 0x98765432)
                                );
                            }
                            assert_eq!((b.mem[0xfff9], b.mem[0x10000]), (0xa5, 0xa5));
                            continue;
                        }
                        let before = b.mem[0xfff9..0x10008].to_vec();
                        c.flags = 2 | CF | DF | OF;
                        c.step(&mut b).unwrap();
                        fault(&c, &b, 13, 0x1000);
                        assert_eq!(b.read16(0x8ffe), 2 | CF | DF | OF);
                        assert_eq!((c.gdtr_limit, c.gdtr_base), (0x1234, 0x98765432));
                        assert_eq!(&b.mem[0xfff9..0x10008], &before);
                        // Handler returns to the full prefixed instruction. Repair EA,
                        // IRET, and retry without changing the saved instruction IP.
                        b.mem[c.ip as usize] = 0xcf;
                        c.set_reg32(if stack { BP } else { BX }, 0x4000);
                        b.mem[0x3fff..0x4007].fill(0xa5);
                        c.step(&mut b).unwrap();
                        assert_eq!(c.ip, 0x1000);
                        assert_eq!(c.r[SP], 0x9000);
                        c.step(&mut b).unwrap();
                        assert_eq!(c.ip, 0x1000 + code.len() as u32);
                        if load {
                            assert_eq!(
                                (c.gdtr_limit, c.gdtr_base),
                                (0xa5a5, if operand32 { 0xa5a5a5a5 } else { 0x00a5a5a5 })
                            );
                        } else {
                            assert_eq!(&b.mem[0x4000..0x4006], &gdtr_bytes(0x1234, 0x98765432));
                        }
                        assert_eq!((b.mem[0x3fff], b.mem[0x4006]), (0xa5, 0xa5));
                    }
                }
            }
        }
    }
}

#[test]
fn gdtr_invalid_register_and_lock_forms_fault_at_first_prefix() {
    for code in [
        vec![0x66, 0x67, 0x0f, 0x01, 0xc0],
        vec![0x65, 0x66, 0x0f, 0x01, 0xd7],
        vec![0x64, 0xf0, 0x0f, 0x01, 0x16, 0, 0x40],
        vec![0x66, 0xf0, 0x0f, 0x01, 0x06, 0, 0x40],
    ] {
        let (mut c, mut b) = cpu(&code);
        c.gdtr_limit = 0x1234;
        c.gdtr_base = 0x87654321;
        b.mem[0x3fff..0x4007].fill(0xa5);
        c.step(&mut b).unwrap();
        fault(&c, &b, 6, 0x1000);
        assert_eq!((c.gdtr_limit, c.gdtr_base), (0x1234, 0x87654321));
        assert_eq!(&b.mem[0x3fff..0x4007], &[0xa5; 8]);
        // A plain IRET retries and faults again, rather than executing past it.
        b.mem[c.ip as usize] = 0xcf;
        c.step(&mut b).unwrap();
        c.step(&mut b).unwrap();
        fault(&c, &b, 6, 0x1000);
    }
}

#[test]
fn gdtr_bus_mapping_a20_rom_and_independent_reset_state() {
    let (mut c, mut b) = cpu(&[
        0x64, 0x66, 0x0f, 0x01, 0x16, 0x10, 0, 0x65, 0x0f, 0x01, 0x06, 0x10, 0, 0x64, 0x0f, 0x01,
        0x06, 0x20, 0,
    ]);
    c.extra_segments = [0xffff, 0xf000];
    b.mem[..6].copy_from_slice(&gdtr_bytes(0, 0xfedcba98));
    b.mem[0x100000..0x100006].copy_from_slice(&gdtr_bytes(0xffff, 0x87654321));
    c.step(&mut b).unwrap();
    assert_eq!((c.gdtr_limit, c.gdtr_base), (0, 0xfedcba98));
    b.output(0xf2, 0);
    c.ip = 0x1000;
    c.step(&mut b).unwrap();
    assert_eq!((c.gdtr_limit, c.gdtr_base), (0xffff, 0x87654321));
    b.output(0xf6, 3); // A20 disabling cannot mask stored linear metadata.
    b.mem[0xf000f..0xf0017].fill(0xa5);
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0xf000f..0xf0017], &[0xa5; 8]);
    assert_eq!(c.gdtr_base, 0x87654321);
    assert!(!b.a20);
    b.mem[0x0f..0x17].fill(0xa5);
    b.mem[0x10000f..0x100017].fill(0xa5);
    let store_ip = c.ip;
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0x10..0x16], &gdtr_bytes(0xffff, 0x87654321));
    assert_eq!((b.mem[0x0f], b.mem[0x16]), (0xa5, 0xa5));
    assert_eq!(&b.mem[0x10000f..0x100017], &[0xa5; 8]);
    b.output(0xf2, 0);
    c.ip = store_ip;
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0x100010..0x100016], &gdtr_bytes(0xffff, 0x87654321));
    assert_eq!((b.mem[0x10000f], b.mem[0x100016]), (0xa5, 0xa5));
    let (mut other, mut other_bus) = cpu(&[0x0f, 0x01, 0x16, 0, 0x40]);
    assert_eq!((other.gdtr_limit, other.gdtr_base), (0, 0));
    other_bus.mem[0x4000..0x4006].copy_from_slice(&gdtr_bytes(0x1111, 0x22222222));
    other.step(&mut other_bus).unwrap();
    assert_eq!((other.gdtr_limit, other.gdtr_base), (0x1111, 0x00222222));
    assert_eq!((c.gdtr_limit, c.gdtr_base), (0xffff, 0x87654321));
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu = c;
    m.reset();
    assert_eq!((m.cpu.gdtr_limit, m.cpu.gdtr_base), (0, 0));
    // Reset is observable via the guest instruction, not just public fields.
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x1000;
    m.bus.mem[0x1000..0x1005].copy_from_slice(&[0x0f, 0x01, 0x06, 0, 0x40]);
    m.cpu.step(&mut m.bus).unwrap();
    assert_eq!(&m.bus.mem[0x4000..0x4006], &[0; 6]);
}

#[test]
fn gdtr_other_group_operations_and_vm_remain_explicitly_unsupported() {
    let (mut c, mut b) = cpu(&[0x0f, 0x01, (5 << 3) | 6, 0, 0x40]);
    assert!(c
        .step(&mut b)
        .unwrap_err()
        .contains("unimplemented i486 opcode 0F 01"));
    // Ap INVLPG in real mode has no operand access or observable paging state.
    let (mut c, mut b) = cpu(&[0x0f, 0x01, 0x3e, 0, 0x40]);
    let memory = b.mem.clone();
    let flags = c.flags;
    let ip = c.ip;
    c.step(&mut b).unwrap();
    assert_eq!((c.ip, c.flags), (ip + 5, flags));
    assert_eq!(b.mem, memory);
    for modrm in [0x16, 0x26, 0xe0, 0x3e] {
        // LGDT, both SMSW destination forms and INVLPG remain unavailable on V30.
        let (mut c, mut b) = cpu(&[0x0f, 0x01, modrm, 0, 0x40]);
        b.profile = Profile::Pc9801Vm;
        assert!(c
            .step(&mut b)
            .unwrap_err()
            .contains("unsupported V30 opcode 0F 01"));
    }
}

#[test]
fn gdtr_original_boot_probe_before_and_after_machine_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/gdtr/gdtr.hdm"
    ))
    .expect("run python3 tools/prepare_gdtr_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "gdtr.hdm").unwrap();
    for run in 0..2 {
        if run == 1 {
            m.reset();
        }
        let mut slots = 0;
        for _ in 0..20 {
            slots += m.step(100_000);
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("GDTR PASS") || m.bus.text().contains("GDTR FAIL") {
                break;
            }
        }
        assert!(m.bus.text().contains("GDTR PASS"), "{}", m.bus.text());
        assert!(!m.bus.text().contains("FAIL"));
        assert!(slots <= 2_000_000);
        let full = [0x34, 0x12, 0x78, 0x56, 0x34, 0x12];
        let mut expected = [0xa5; 80];
        for offset in [1, 9, 25, 33, 41, 57, 65] {
            expected[offset..offset + 6].copy_from_slice(&full);
        }
        expected[17..23].copy_from_slice(&[0x34, 0x12, 0x78, 0x56, 0x34, 0]);
        expected[49..55].fill(0);
        expected[73] = 4;
        assert_eq!(&m.bus.mem[0x8000..0x8050], &expected);
        assert_eq!(m.bus.mem[0xfff9], 0xa5);
        assert_eq!(&m.bus.mem[0xfffa..0x10000], &full);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
    }
}

#[test]
fn idtr_minimal_lidt_sidt_reproduction() {
    let (mut c, mut b) = cpu(&[0x0f, 0x01, 0x1e, 0, 0x40, 0x0f, 0x01, 0x0e, 0x10, 0x40]);
    b.mem[0x4000..0x4006].copy_from_slice(&[0xff, 3, 0, 0x50, 0, 0xab]);
    steps(&mut c, &mut b, 2);
    assert_eq!(&b.mem[0x4010..0x4016], &[0xff, 3, 0, 0x50, 0, 0]);
}

// Original Issue #37 instruction acceptance, using six-byte encoding helper above.
#[test]
fn idtr_operand_and_address_sizes_preserve_state_and_exact_store_width() {
    for operand32 in [false, true] {
        for address32 in [false, true] {
            for (limit, base) in [
                (0, 0),
                (0xffff, u32::MAX),
                (0xffff, 0xfedcba98),
                (0x1234, 0x12345678),
            ] {
                let mut code = vec![];
                if operand32 {
                    code.push(0x66);
                }
                if address32 {
                    code.push(0x67);
                }
                code.extend_from_slice(&[
                    0x0f,
                    0x01,
                    if address32 { 0x1d } else { 0x1e },
                    0x01,
                    0x40,
                ]);
                if address32 {
                    code.extend_from_slice(&[0, 0]);
                }
                let store_start = code.len();
                if operand32 {
                    code.push(0x66);
                }
                if address32 {
                    code.push(0x67);
                }
                code.extend_from_slice(&[
                    0x0f,
                    0x01,
                    if address32 { 0x0d } else { 0x0e },
                    0x11,
                    0x40,
                ]);
                if address32 {
                    code.extend_from_slice(&[0, 0]);
                }
                let (mut c, mut b) = cpu(&code);
                for r in 0..8 {
                    c.set_reg32(r, 0xabc00000 + r as u32);
                }
                c.r[SP] = 0x9000;
                c.s[DS] = 0x3000;
                c.s[ES] = 0x4000;
                c.extra_segments = [0x5000, 0x6000];
                c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
                c.flags_upper = 4;
                let saved = c.clone();
                b.mem[0x34001..0x34007].copy_from_slice(&gdtr_bytes(limit, base));
                b.mem[0x34010..0x34018].fill(0xa5);
                c.step(&mut b).unwrap();
                assert_eq!(c.r, saved.r);
                assert_eq!(c.upper, saved.upper);
                assert_eq!(c.s, saved.s);
                assert_eq!(c.extra_segments, saved.extra_segments);
                assert_eq!((c.flags, c.flags_upper), (saved.flags, saved.flags_upper));
                assert_eq!(c.ip, 0x1000 + store_start as u32);
                c.step(&mut b).unwrap();
                assert_eq!(c.ip, 0x1000 + code.len() as u32);
                let expected_base = if operand32 { base } else { base & 0xffffff };
                assert_eq!(&b.mem[0x34011..0x34017], &gdtr_bytes(limit, expected_base));
                assert_eq!((b.mem[0x34010], b.mem[0x34017]), (0xa5, 0xa5));
                assert_eq!(c.r, saved.r);
                assert_eq!(c.upper, saved.upper);
                assert_eq!(c.s, saved.s);
                assert_eq!(c.extra_segments, saved.extra_segments);
                assert_eq!((c.flags, c.flags_upper), (saved.flags, saved.flags_upper));
            }
        }
    }
}

#[test]
fn idtr_full_base_survives_both_stores_then_word_load_clears_high_byte() {
    let (mut c, mut b) = cpu(&[
        0x66, 0x0f, 0x01, 0x1e, 0, 0x40, // lidt dword [4000]
        0x0f, 0x01, 0x0e, 0x10, 0x40, // sidt word [4010]
        0x66, 0x0f, 0x01, 0x0e, 0x20, 0x40, // sidt dword [4020]
        0x0f, 0x01, 0x1e, 0, 0x40, // lidt word [4000]
        0x0f, 0x01, 0x0e, 0x30, 0x40,
    ]);
    let source = [0x34, 0x12, 0x78, 0x56, 0x34, 0x12];
    b.mem[0x4000..0x4006].copy_from_slice(&source);
    steps(&mut c, &mut b, 5);
    assert_eq!(&b.mem[0x4010..0x4016], &source);
    assert_eq!(&b.mem[0x4020..0x4026], &source);
    assert_eq!(&b.mem[0x4030..0x4036], &[0x34, 0x12, 0x78, 0x56, 0x34, 0]);
}

#[test]
fn idtr_sib_default_ss_and_all_segment_overrides_with_repeated_prefixes() {
    // Prefixes plus ModR/M+SIB/displacement; last segment override wins.
    let cases: &[(&[u8], &[u8], usize, usize)] = &[
        (&[], &[0x5c, 0x8b, 1], DS, 0x4022), // [ebx+ecx*4+1]
        (&[], &[0x5c, 0x25, 0], SS, 0x4101), // [ebp]
        (&[], &[0x1c, 0x24], SS, 0x9000),    // [esp]
        (&[0x26], &[0x1b], ES, 0x4001),
        (&[0x2e], &[0x1b], CS, 0x4001),
        (&[0x36], &[0x1b], SS, 0x4001),
        (&[0x3e], &[0x5c, 0x25, 0], DS, 0x4101),
        (&[0x64], &[0x1b], 4, 0x4001),
        (&[0x65], &[0x1b], 5, 0x4001),
        (&[0x26, 0x64, 0x65], &[0x1b], 5, 0x4001),
    ];
    for &(prefixes, encoding, segment, offset) in cases {
        for address32 in [false, true] {
            // SIB encodings apply to address32; word case [bx] covers the overrides.
            let mut code = prefixes.to_vec();
            code.extend_from_slice(&[0x66, 0x66]);
            if address32 {
                code.extend_from_slice(&[0x67, 0x67]);
            }
            code.extend_from_slice(&[0x0f, 0x01]);
            code.extend_from_slice(if address32 { encoding } else { &[0x1f] });
            let (mut c, mut b) = cpu(&code);
            c.s[DS] = 0x3000;
            c.s[ES] = 0x4000;
            c.s[SS] = 0x5000;
            c.extra_segments = [0x6000, 0x7000];
            c.set_reg32(BX, 0x4001);
            c.set_reg32(BP, 0x4101);
            c.set_reg32(CX, 8);
            let s = if !address32 && prefixes.is_empty() {
                DS
            } else {
                segment
            };
            let off = if address32 { offset } else { 0x4001 };
            let seg = if s < 4 {
                c.s[s]
            } else {
                c.extra_segments[s - 4]
            };
            let a = seg as usize * 16 + off;
            b.mem[a..a + 6].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
            c.step(&mut b).unwrap();
            assert_eq!(
                (c.idtr_limit, c.idtr_base),
                (0x201, 0x6050403),
                "{code:02x?}"
            );
            // Exercise the identical addressing expression for SIDT.
            c.ip = 0x1000;
            let modrm = 0x1000 + prefixes.len() + 2 + if address32 { 2 } else { 0 } + 2;
            b.mem[modrm] = (b.mem[modrm] & !0x38) | 8;
            b.mem[a - 1..a + 7].fill(0xa5);
            c.step(&mut b).unwrap();
            assert_eq!(&b.mem[a..a + 6], &[1, 2, 3, 4, 5, 6]);
            assert_eq!((b.mem[a - 1], b.mem[a + 6]), (0xa5, 0xa5));
        }
    }
}

#[test]
fn idtr_six_byte_limit_faults_are_atomic_gp_even_for_ss_and_restart() {
    for operand32 in [false, true] {
        for load in [false, true] {
            for stack in [false, true] {
                for address32 in [false, true] {
                    for offset in [0xfffa, 0xfffb, 0x10000, u32::MAX] {
                        if !address32 && offset > 0xffff {
                            continue;
                        }
                        let mut code = vec![];
                        if operand32 {
                            code.push(0x66);
                        }
                        if address32 {
                            code.push(0x67);
                        }
                        code.extend_from_slice(&[
                            0x0f,
                            0x01,
                            (if stack { 0x46 } else { 0x47 }) | if load { 0x18 } else { 8 },
                            0,
                        ]);
                        // 32-bit rm=5 is EBP; 16-bit rm=6 is BP.
                        if address32 {
                            let m = code.len() - 2;
                            code[m] = (code[m] & !7) | if stack { 5 } else { 3 };
                        }
                        let (mut c, mut b) = cpu(&code);
                        c.set_reg32(if stack { BP } else { BX }, offset);
                        c.idtr_limit = 0x1234;
                        c.idtr_base = 0x6000;
                        b.mem.copy_within(0..0x400, 0x6000);
                        b.mem[0xfff9..0x10008].fill(0xa5);
                        if offset == 0xfffa {
                            c.step(&mut b).unwrap();
                            if load {
                                assert_eq!(
                                    (c.idtr_limit, c.idtr_base),
                                    (0xa5a5, if operand32 { 0xa5a5a5a5 } else { 0x00a5a5a5 })
                                );
                            } else {
                                assert_eq!(&b.mem[0xfffa..0x10000], &gdtr_bytes(0x1234, 0x6000));
                            }
                            assert_eq!((b.mem[0xfff9], b.mem[0x10000]), (0xa5, 0xa5));
                            continue;
                        }
                        let before = b.mem[0xfff9..0x10008].to_vec();
                        c.flags = 2 | CF | DF | OF;
                        c.step(&mut b).unwrap();
                        fault(&c, &b, 13, 0x1000);
                        assert_eq!(b.read16(0x8ffe), 2 | CF | DF | OF);
                        assert_eq!((c.idtr_limit, c.idtr_base), (0x1234, 0x6000));
                        assert_eq!(&b.mem[0xfff9..0x10008], &before);
                        // Handler returns to the full prefixed instruction. Repair EA,
                        // IRET, and retry without changing the saved instruction IP.
                        b.mem[c.ip as usize] = 0xcf;
                        c.set_reg32(if stack { BP } else { BX }, 0x4000);
                        b.mem[0x3fff..0x4007].fill(0xa5);
                        c.step(&mut b).unwrap();
                        assert_eq!(c.ip, 0x1000);
                        assert_eq!(c.r[SP], 0x9000);
                        c.step(&mut b).unwrap();
                        assert_eq!(c.ip, 0x1000 + code.len() as u32);
                        if load {
                            assert_eq!(
                                (c.idtr_limit, c.idtr_base),
                                (0xa5a5, if operand32 { 0xa5a5a5a5 } else { 0x00a5a5a5 })
                            );
                        } else {
                            assert_eq!(&b.mem[0x4000..0x4006], &gdtr_bytes(0x1234, 0x6000));
                        }
                        assert_eq!((b.mem[0x3fff], b.mem[0x4006]), (0xa5, 0xa5));
                    }
                }
            }
        }
    }
}

#[test]
fn idtr_invalid_register_and_lock_forms_fault_at_first_prefix() {
    for code in [
        vec![0x66, 0x67, 0x0f, 0x01, 0xc8],
        vec![0x65, 0x66, 0x0f, 0x01, 0xdf],
        vec![0x64, 0xf0, 0x0f, 0x01, 0x1e, 0, 0x40],
        vec![0x66, 0xf0, 0x0f, 0x01, 0x0e, 0, 0x40],
    ] {
        let (mut c, mut b) = cpu(&code);
        c.idtr_limit = 0x1234;
        c.idtr_base = 0x6000;
        b.mem.copy_within(0..0x400, 0x6000);
        b.mem[0x3fff..0x4007].fill(0xa5);
        c.step(&mut b).unwrap();
        fault(&c, &b, 6, 0x1000);
        assert_eq!((c.idtr_limit, c.idtr_base), (0x1234, 0x6000));
        assert_eq!(&b.mem[0x3fff..0x4007], &[0xa5; 8]);
        // A plain IRET retries and faults again, rather than executing past it.
        b.mem[c.ip as usize] = 0xcf;
        c.step(&mut b).unwrap();
        c.step(&mut b).unwrap();
        fault(&c, &b, 6, 0x1000);
    }
}

#[test]
fn idtr_bus_mapping_a20_rom_and_independent_reset_state() {
    let (mut c, mut b) = cpu(&[
        0x64, 0x66, 0x0f, 0x01, 0x1e, 0x10, 0, 0x65, 0x0f, 0x01, 0x0e, 0x10, 0, 0x64, 0x0f, 0x01,
        0x0e, 0x20, 0,
    ]);
    c.extra_segments = [0xffff, 0xf000];
    b.mem[..6].copy_from_slice(&gdtr_bytes(0, 0xfedcba98));
    b.mem[0x100000..0x100006].copy_from_slice(&gdtr_bytes(0xffff, 0x87654321));
    c.step(&mut b).unwrap();
    assert_eq!((c.idtr_limit, c.idtr_base), (0, 0xfedcba98));
    b.output(0xf2, 0);
    c.ip = 0x1000;
    c.step(&mut b).unwrap();
    assert_eq!((c.idtr_limit, c.idtr_base), (0xffff, 0x87654321));
    b.output(0xf6, 3); // A20 disabling cannot mask stored linear metadata.
    b.mem[0xf000f..0xf0017].fill(0xa5);
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0xf000f..0xf0017], &[0xa5; 8]);
    assert_eq!(c.idtr_base, 0x87654321);
    assert!(!b.a20);
    b.mem[0x0f..0x17].fill(0xa5);
    b.mem[0x10000f..0x100017].fill(0xa5);
    let store_ip = c.ip;
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0x10..0x16], &gdtr_bytes(0xffff, 0x87654321));
    assert_eq!((b.mem[0x0f], b.mem[0x16]), (0xa5, 0xa5));
    assert_eq!(&b.mem[0x10000f..0x100017], &[0xa5; 8]);
    b.output(0xf2, 0);
    c.ip = store_ip;
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0x100010..0x100016], &gdtr_bytes(0xffff, 0x87654321));
    assert_eq!((b.mem[0x10000f], b.mem[0x100016]), (0xa5, 0xa5));
    let (mut other, mut other_bus) = cpu(&[0x0f, 0x01, 0x1e, 0, 0x40]);
    assert_eq!((other.idtr_limit, other.idtr_base), (0x3ff, 0));
    other_bus.mem[0x4000..0x4006].copy_from_slice(&gdtr_bytes(0x1111, 0x22222222));
    other.step(&mut other_bus).unwrap();
    assert_eq!((other.idtr_limit, other.idtr_base), (0x1111, 0x00222222));
    assert_eq!((c.idtr_limit, c.idtr_base), (0xffff, 0x87654321));
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu = c;
    m.reset();
    assert_eq!((m.cpu.idtr_limit, m.cpu.idtr_base), (0x3ff, 0));
    // Reset is observable via the guest instruction, not just public fields.
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x1000;
    m.bus.mem[0x1000..0x1005].copy_from_slice(&[0x0f, 0x01, 0x0e, 0, 0x40]);
    m.cpu.step(&mut m.bus).unwrap();
    assert_eq!(&m.bus.mem[0x4000..0x4006], &[0xff, 3, 0, 0, 0, 0]);
}

fn relocated(c: &mut Cpu, b: &mut Bus, vectors: &[u8]) {
    c.idtr_base = 0x6001; // deliberately unaligned table
    for &n in vectors {
        let at = 0x6001 + n as usize * 4;
        b.mem[at..at + 4].copy_from_slice(&[0, 4, 0, 3]);
        // Conflicting fixed IVT entry must never be followed by Ap.
        b.mem[n as usize * 4..n as usize * 4 + 4].copy_from_slice(&[0, 5, 0, 4]);
    }
    b.mem[0x3400] = 0xcf;
    b.mem[0x4500] = 0xf4;
}

#[test]
fn idtr_relocated_software_entries_frames_and_iret() {
    for (instruction, vector) in [
        (&[0xcd, 0x30][..], 0x30),
        (&[0xcc][..], 3),
        (&[0xce][..], 4),
    ] {
        for prefix in [false, true] {
            let mut code = if prefix { vec![0x66, 0x2e] } else { vec![] };
            code.extend_from_slice(instruction);
            let (mut c, mut b) = cpu(&[]);
            c.s[CS] = 0x100;
            c.s[SS] = 0x2000;
            b.mem[0x2000..0x2000 + code.len()].copy_from_slice(&code);
            relocated(&mut c, &mut b, &[vector]);
            let flags = 2 | CF | IF | TF | OF;
            c.flags = flags;
            c.flags_upper = 4;
            c.step(&mut b).unwrap();
            assert_eq!((c.s[CS], c.ip), (0x300, 0x400));
            assert_eq!(c.r[SP], 0x8ffa);
            assert_eq!(
                &b.mem[0x28ffa..0x29000],
                &[
                    ((0x1000 + code.len()) & 255) as u8,
                    0x10,
                    0,
                    1,
                    flags as u8,
                    (flags >> 8) as u8
                ]
            );
            assert_eq!((c.flags, c.flags_upper), (flags & !(IF | TF), 0));
            c.step(&mut b).unwrap();
            assert_eq!(
                (c.s[CS], c.ip, c.r[SP], c.flags),
                (0x100, 0x1000 + code.len() as u32, 0x9000, flags)
            );
        }
    }
    let (mut c, mut b) = cpu(&[0x66, 0xce]);
    relocated(&mut c, &mut b, &[4]);
    c.flags_upper = 4;
    c.step(&mut b).unwrap();
    assert_eq!((c.ip, c.r[SP], c.flags_upper), (0x1002, 0x9000, 4));
}

#[test]
fn idtr_relocated_fault_trap_native_injection_and_vm_fixed_ivt() {
    for (code, vector, saved_ip) in [
        (&[0x66, 0xf7, 0xf3][..], 0, 0x1000), // #DE restart
        (&[0x66, 0x40][..], 1, 0x1002),       // TF trap resume
        (&[0x66, 0x8e, 0xc8][..], 6, 0x1000), // #UD restart
    ] {
        let (mut c, mut b) = cpu(code);
        relocated(&mut c, &mut b, &[vector]);
        c.flags = 2 | IF | TF;
        c.flags_upper = 4;
        c.step(&mut b).unwrap();
        assert_eq!((c.s[CS], c.ip, c.r[SP]), (0x300, 0x400, 0x8ffa));
        assert_eq!(
            (b.read16(0x8ffa), b.read16(0x8ffc), b.read16(0x8ffe)),
            (saved_ip, 0, 2 | IF | TF)
        );
        assert_eq!((c.flags, c.flags_upper), (2, 0));
        c.step(&mut b).unwrap();
        assert_eq!((c.s[CS], c.ip, c.r[SP]), (0, saved_ip as u32, 0x9000));
    }
    for vm in [false, true] {
        let (mut c, mut b) = cpu(&[]);
        relocated(&mut c, &mut b, &[0x30]);
        if vm {
            b.profile = Profile::Pc9801Vm;
            c.idtr_limit = 0;
        }
        c.flags = 2 | IF | TF;
        c.flags_upper = 4;
        c.interrupt(&mut b, 0x30).unwrap();
        assert_eq!(
            (c.s[CS], c.ip),
            if vm { (0x400, 0x500) } else { (0x300, 0x400) }
        );
        assert_eq!(c.r[SP], 0x8ffa);
        assert_eq!((b.read16(0x8ffa), b.read16(0x8ffe)), (0x1000, 2 | IF | TF));
        assert_eq!((c.flags, c.flags_upper), (2, 0));
    }
    for kind in [1, 3] {
        let (mut c, mut b) = cpu(&[0x0f, 0x01, (kind << 3) | 6, 0, 0x40]);
        b.profile = Profile::Pc9801Vm;
        assert!(c
            .step(&mut b)
            .unwrap_err()
            .contains("unsupported V30 opcode 0F 01"));
    }
}

#[test]
fn idtr_relocated_pic_irq_hlt_eoi_and_interrupt_shadows() {
    for code in [
        &[0xfb, 0x90][..],
        &[0x8e, 0xd0, 0x90][..],
        &[0x17, 0x90][..],
    ] {
        let (mut c, mut b) = cpu(code);
        if code[0] == 0x17 {
            b.mem[0x9000..0x9002].fill(0);
        }
        relocated(&mut c, &mut b, &[9]);
        b.mem[0x3400..0x3405].copy_from_slice(&[0xb0, 0x20, 0xe6, 0, 0xcf]); // EOI / IRET
        c.flags = 2; // establish the shadow before requesting IRQ
        c.step(&mut b).unwrap();
        c.flags |= IF;
        c.flags_upper = 4;
        let resume = c.ip + 1;
        let sp = c.r[SP];
        b.pics[0].mask = 0xfd;
        b.pics[0].request(1);
        c.step(&mut b).unwrap(); // shadowed NOP
        assert_eq!(c.ip, resume);
        assert_eq!(b.pics[0].isr, 0);
        c.halted = true;
        c.step(&mut b).unwrap(); // IRQ wakes HLT; executes MOV AL
        assert!(!c.halted);
        assert_eq!((c.s[CS], c.ip), (0x300, 0x402));
        assert_eq!(b.pics[0].isr, 2);
        assert_eq!(c.r[SP], sp - 6);
        assert_eq!(
            (
                b.read16((sp - 6) as usize),
                b.read16((sp - 4) as usize),
                b.read16((sp - 2) as usize)
            ),
            (resume as u16, 0, 2 | IF)
        );
        assert_eq!((c.flags, c.flags_upper), (2, 0));
        steps(&mut c, &mut b, 2); // EOI / IRET
        assert_eq!(b.pics[0].isr, 0);
        assert_eq!((c.s[CS], c.ip, c.r[SP], c.flags), (0, resume, sp, 2 | IF));
    }
    // MOV SS suppresses its own TF trap; STI does not.
    for (code, first_ip) in [(&[0x8e, 0xd0, 0x90][..], 0x1002), (&[0xfb][..], 0x400)] {
        let (mut c, mut b) = cpu(code);
        relocated(&mut c, &mut b, &[1]);
        c.flags |= TF;
        c.step(&mut b).unwrap();
        assert_eq!(c.ip, first_ip);
        if code[0] == 0x8e {
            c.step(&mut b).unwrap();
            assert_eq!((c.s[CS], c.ip, b.read16(0x8ffa)), (0x300, 0x400, 0x1003));
        }
    }
}

#[test]
fn idtr_inclusive_limit_double_fault_and_terminal_shutdown() {
    for (limit, vector, saved_ip) in [(0xc3, 0x30, 0x1004), (0xc2, 8, 0x1000), (0x23, 8, 0x1000)] {
        let (mut c, mut b) = cpu(&[0x66, 0x2e, 0xcd, 0x30]);
        relocated(&mut c, &mut b, &[0x30]);
        let a = c.idtr_base as usize + 8 * 4;
        b.mem[a..a + 4].copy_from_slice(&[0, 5, 0, 3]);
        b.mem[0x3500] = 0xcf;
        c.idtr_limit = limit;
        c.flags = 2 | CF | IF | TF;
        c.flags_upper = 4;
        b.mem[0x8ff8..0x9002].fill(0xa5);
        c.step(&mut b).unwrap();
        assert_eq!(
            (c.s[CS], c.ip),
            (0x300, if vector == 8 { 0x500 } else { 0x400 })
        );
        assert!(!c.shutdown);
        assert_eq!(c.r[SP], 0x8ffa); // no partial original frame or error-code word
        assert_eq!(
            (b.read16(0x8ffa), b.read16(0x8ffc), b.read16(0x8ffe)),
            (saved_ip, 0, 2 | CF | IF | TF)
        );
        assert_eq!((b.read16(0x8ff8), b.read16(0x9000)), (0xa5a5, 0xa5a5));
        assert_eq!((c.flags, c.flags_upper), (2 | CF, 0));
        c.idtr_limit = 0x3ff; // handler repair makes a retry possible
        c.step(&mut b).unwrap();
        assert_eq!((c.s[CS], c.ip, c.r[SP]), (0, saved_ip as u32, 0x9000));
        if vector == 8 {
            c.step(&mut b).unwrap();
            assert_eq!((c.s[CS], c.ip, b.read16(0x8ffa)), (0x300, 0x400, 0x1004));
        }
    }
    let (mut c, mut b) = cpu(&[0x66, 0x2e, 0xcd, 0x30]);
    c.idtr_limit = 0x22;
    c.flags |= IF;
    b.mem[0x8ff0..0x9000].fill(0xa5);
    c.step(&mut b).unwrap();
    assert!(c.shutdown);
    assert!(!c.halted); // distinct from HLT
    assert_eq!((c.ip, c.r[SP]), (0x1000, 0x9000));
    assert_eq!(&b.mem[0x8ff0..0x9000], &[0xa5; 16]);
    let retired = c.retired;
    b.pics[0].mask = 0xfd;
    b.pics[0].request(1);
    steps(&mut c, &mut b, 100);
    c.interrupt(&mut b, 0).unwrap();
    assert_eq!(
        (c.ip, c.r[SP], c.retired, b.pics[0].isr),
        (0x1000, 0x9000, retired, 0)
    );
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu = c;
    m.reset();
    assert!(!m.cpu.shutdown);
    assert!(!m.cpu.halted);
    assert_eq!((m.cpu.idtr_base, m.cpu.idtr_limit), (0, 0x3ff));
    assert_eq!(m.step(10), 10);
    assert!(m.error.is_none());
}

#[test]
fn idtr_vector_bus_a20_and_wrapping_linear_addresses() {
    for a20 in [false, true] {
        let (mut c, mut b) = cpu(&[0xcc]);
        c.idtr_base = 0x100000;
        b.a20 = a20;
        b.mem[12..16].copy_from_slice(&[0, 4, 0, 3]);
        b.mem[0x10000c..0x100010].copy_from_slice(&[0, 5, 0, 4]);
        c.step(&mut b).unwrap();
        assert_eq!(
            (c.s[CS], c.ip),
            if a20 { (0x400, 0x500) } else { (0x300, 0x400) }
        );
        assert_eq!(c.idtr_base, 0x100000);
    }
    for (base, vector, expected) in [
        (0xffff_fffc, 1, (0x300, 0x400)),
        (0xffff_fffe, 0, (0x400, 0xffff)),
    ] {
        let (mut c, mut b) = cpu(&[]);
        b.a20 = true;
        c.idtr_base = base;
        b.mem[..4].copy_from_slice(&[0, 4, 0, 3]);
        c.interrupt(&mut b, vector).unwrap();
        assert_eq!((c.s[CS], c.ip), expected);
    }
}

#[test]
fn idtr_external_delivery_limit_failures_stay_inside_guest() {
    for event in [0, 1, 2] {
        let (mut c, mut b) = cpu(&[0x8b, 0x07]); // GP for invalid DS word
        relocated(&mut c, &mut b, &[8]);
        c.idtr_limit = 0x23;
        c.flags = 2 | IF;
        c.r[BX] = 0xffff;
        if event == 0 {
            c.interrupt(&mut b, 0x30).unwrap();
        } else if event == 1 {
            b.mem[0x3400] = 0x90; // first instruction at #DF executes on IRQ step
            b.pics[0].mask = 0xfd;
            b.pics[0].request(1);
            c.step(&mut b).unwrap();
            assert_eq!(b.pics[0].isr, 2);
        } else {
            c.step(&mut b).unwrap();
        }
        assert_eq!(
            (c.s[CS], c.ip),
            (0x300, if event == 1 { 0x401 } else { 0x400 })
        );
        assert_eq!(
            (
                c.r[SP],
                b.read16(0x8ffa),
                b.read16(0x8ffc),
                b.read16(0x8ffe)
            ),
            (0x8ffa, 0x1000, 0, 2 | IF)
        );
    }
    for event in [0, 1, 2, 3] {
        let (mut c, mut b) = cpu(&[0x90]);
        c.idtr_limit = 2; // neither requested vector nor #DF is available
        if event == 0 {
            c.interrupt(&mut b, 3).unwrap();
        } else {
            if event == 1 {
                c.flags |= TF;
            } else if event == 2 {
                c.flags |= IF;
                b.pics[0].mask = 0xfd;
                b.pics[0].request(1);
            } else {
                b.mem[0x1000] = 0xcc;
            }
            c.step(&mut b).unwrap();
        }
        assert!(c.shutdown);
        assert_eq!(c.r[SP], 0x9000);
        assert_eq!(c.ip, if event == 1 { 0x1001 } else { 0x1000 });
        let before = c.retired;
        steps(&mut c, &mut b, 100);
        assert_eq!(c.retired, before);
    }
}

#[test]
fn idtr_original_boot_probe_before_and_after_machine_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/idtr/idtr.hdm"
    ))
    .expect("run python3 tools/prepare_idtr_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "idtr.hdm").unwrap();
    for run in 0..2 {
        if run == 1 {
            m.reset();
        }
        assert!(!m.cpu.shutdown);
        let mut slots = 0;
        for _ in 0..20 {
            slots += m.step(100_000);
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.cpu.shutdown || m.bus.text().contains("IDTR FAIL") {
                break;
            }
        }
        assert!(
            m.bus.text().contains("IDTR PASS"),
            "{}; CS:IP={:04x}:{:04x}",
            m.bus.text(),
            m.cpu.s[CS],
            m.cpu.ip
        );
        assert!(!m.bus.text().contains("FAIL"));
        assert!(slots <= 2_000_000);
        let mut expected = [0xa5; 80];
        expected[1..7].copy_from_slice(&gdtr_bytes(0x3ff, 0));
        for offset in [9, 17, 49] {
            expected[offset..offset + 6].copy_from_slice(&gdtr_bytes(0x3ff, 0x12345678));
        }
        expected[25..31].copy_from_slice(&gdtr_bytes(0x3ff, 0x345678));
        for offset in [33, 73] {
            expected[offset..offset + 6].copy_from_slice(&gdtr_bytes(0xc3, 0x6001));
        }
        expected[41..44].copy_from_slice(&[3, 2, 2]);
        expected[57] = 0x88;
        expected[65] = 0x77;
        assert_eq!(&m.bus.mem[0x8000..0x8050], &expected);
        assert_eq!(&m.bus.mem[0xfff9..0x10000], &[0xa5, 0xc3, 0, 1, 0x60, 0, 0]);
        assert!(m.cpu.shutdown);
        assert!(!m.cpu.halted);
        let before = (m.cpu.ip, m.cpu.r[SP], m.cpu.retired);
        m.cpu.flags |= IF;
        m.bus.pics[0].mask = 0xfd;
        m.bus.pics[0].request(1);
        assert_eq!(m.step(100_000), 100_000);
        assert!(m.error.is_none());
        assert_eq!((m.cpu.ip, m.cpu.r[SP], m.cpu.retired), before);
        assert_eq!(m.bus.pics[0].isr, 0);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
    }
}

// Original Issue #39 acceptance. No status-writing guest instructions are added.
#[test]
fn smsw_all_registers_sizes_repeated_overrides_and_unrelated_state() {
    for operand32 in [false, true] {
        for address32 in [false, true] {
            for destination in 0..8 {
                let mut code = vec![0x26, 0x65];
                if operand32 {
                    code.extend_from_slice(&[0x66, 0x66]);
                }
                if address32 {
                    code.extend_from_slice(&[0x67, 0x67]);
                }
                code.extend_from_slice(&[0x0f, 0x01, 0xe0 | destination as u8]);
                let (mut c, mut b) = cpu(&code);
                for r in 0..8 {
                    c.set_reg32(r, 0xabcd1234 + r as u32 * 0x10001);
                }
                c.s[DS] = 0x3000;
                c.extra_segments = [0x5000, 0x6000];
                c.flags = 2 | CF | PF | AF | ZF | SF | IF | DF | OF;
                c.flags_upper = 4;
                c.gdtr_base = 0xabcdef01;
                let saved = c.clone();
                let memory = b.mem.clone();
                c.step(&mut b).unwrap();
                let mut expected = saved.r;
                expected[destination] = 0x10;
                assert_eq!(c.r, expected);
                assert_eq!(c.upper, saved.upper);
                smsw_preserved(&c, &saved);
                assert_eq!(b.mem, memory);
                assert_eq!(c.ip, 0x1000 + code.len() as u32);
                assert_eq!(c.retired, 1);
            }
        }
    }
}

fn smsw_preserved(c: &Cpu, saved: &Cpu) {
    assert_eq!(c.s, saved.s);
    assert_eq!(c.extra_segments, saved.extra_segments);
    assert_eq!((c.flags, c.flags_upper), (saved.flags, saved.flags_upper));
    assert_eq!(
        (c.gdtr_limit, c.gdtr_base),
        (saved.gdtr_limit, saved.gdtr_base)
    );
    assert_eq!(
        (c.idtr_limit, c.idtr_base),
        (saved.idtr_limit, saved.idtr_base)
    );
    assert_eq!(c.cr0, saved.cr0);
    assert_eq!(c.shutdown, saved.shutdown);
    assert_eq!(c.halted, saved.halted);
}

#[test]
fn smsw_memory_sizes_sib_segments_unaligned_and_exact_width() {
    // 16-bit [bx+si], [bp+si], disp16 and all segment overrides;
    // 32-bit scaled SIB, EBP/ESP defaults, index-only EBP (DS), disp32.
    type MemoryCase = (bool, &'static [u8], &'static [u8], usize, usize);
    let cases: &[MemoryCase] = &[
        (false, &[], &[0x20], DS, 0x4021),
        (false, &[], &[0x22], SS, 0x4121),
        (false, &[], &[0x26, 1, 0x40], DS, 0x4001),
        (false, &[0x26], &[0x27], ES, 0x4001),
        (false, &[0x2e], &[0x27], CS, 0x4001),
        (false, &[0x36], &[0x27], SS, 0x4001),
        (false, &[0x3e], &[0x62, 0], DS, 0x4121),
        (false, &[0x64], &[0x27], 4, 0x4001),
        (false, &[0x26, 0x64, 0x65], &[0x27], 5, 0x4001),
        (true, &[], &[0x64, 0x8b, 1], DS, 0x4022),
        (true, &[], &[0xa4, 0x8b, 0xf0, 0xff, 0xff, 0xff], DS, 0x4011),
        (true, &[], &[0x64, 0x25, 0], SS, 0x4101),
        (true, &[], &[0x24, 0x24], SS, 0x9000),
        (true, &[], &[0x24, 0x2d, 0, 0, 0, 0], DS, 0x4101),
        (true, &[], &[0x25, 1, 0x40, 0, 0], DS, 0x4001),
        (true, &[0x26], &[0x23], ES, 0x4001),
        (true, &[0x2e], &[0x23], CS, 0x4001),
        (true, &[0x36], &[0x23], SS, 0x4001),
        (true, &[0x3e], &[0x65, 0], DS, 0x4101),
        (true, &[0x64], &[0x23], 4, 0x4001),
        (true, &[0x26, 0x64, 0x65], &[0x23], 5, 0x4001),
    ];
    for &(address32, prefixes, encoding, segment, offset) in cases {
        for operand32 in [false, true] {
            for status in [0x10, 0xa53e] {
                let mut code = prefixes.to_vec();
                if operand32 {
                    code.extend_from_slice(&[0x66, 0x66]);
                }
                if address32 {
                    code.extend_from_slice(&[0x67, 0x67]);
                }
                code.extend_from_slice(&[0x0f, 0x01]);
                code.extend_from_slice(encoding);
                let (mut c, mut b) = cpu(&code);
                c.cr0 = status; // Host-only PE-clear variation; protected SMSW is tested separately.
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
                expected[address..address + 2].copy_from_slice(&(status as u16).to_le_bytes());
                c.step(&mut b).unwrap();
                assert_eq!(b.mem, expected, "{code:02x?}");
                assert_eq!(c.r, saved.r);
                assert_eq!(c.upper, saved.upper);
                smsw_preserved(&c, &saved);
                assert_eq!(c.ip, 0x1000 + code.len() as u32);
            }
        }
    }
}

#[test]
fn smsw_limits_gp_even_for_ss_atomic_frame_and_iret_retry() {
    for operand32 in [false, true] {
        for address32 in [false, true] {
            for stack in [false, true] {
                for offset in [0xfffe, 0xffff, 0x10000, u32::MAX] {
                    if !address32 && offset > 0xffff {
                        continue;
                    }
                    let mut code = vec![if stack { 0x36 } else { 0x3e }];
                    if operand32 {
                        code.push(0x66);
                    }
                    if address32 {
                        code.push(0x67);
                    }
                    code.extend_from_slice(&[
                        0x0f,
                        0x01,
                        if stack {
                            if address32 {
                                0x65
                            } else {
                                0x66
                            }
                        } else if address32 {
                            0x63
                        } else {
                            0x67
                        },
                        0,
                    ]);
                    let (mut c, mut b) = cpu(&code);
                    let reg = if stack { BP } else { BX };
                    c.set_reg32(reg, offset);
                    c.flags = 2 | CF | DF | OF;
                    b.mem[0xfffd..0x10004].fill(0xa5);
                    let saved = c.clone();
                    let mut expected = b.mem.clone();
                    c.step(&mut b).unwrap();
                    if offset == 0xfffe {
                        expected[0xfffe..0x10000].copy_from_slice(&[0x10, 0]);
                        assert_eq!(b.mem, expected);
                        assert_eq!(c.r, saved.r);
                        assert_eq!(c.upper, saved.upper);
                        smsw_preserved(&c, &saved);
                        continue;
                    }
                    fault(&c, &b, 13, 0x1000);
                    expected[0x8ffa..0x9000].copy_from_slice(&[
                        0,
                        0x10,
                        0,
                        0,
                        saved.flags as u8,
                        (saved.flags >> 8) as u8,
                    ]);
                    assert_eq!(b.mem, expected); // Only the six-byte exception frame changed.
                    assert_eq!(c.reg32(reg), offset);
                    assert_eq!(c.cr0, saved.cr0);
                    assert_eq!(c.flags_upper, saved.flags_upper);
                    b.mem[c.ip as usize] = 0xcf;
                    c.set_reg32(reg, 0x4001);
                    b.mem[0x4000..0x4005].fill(0xa5);
                    c.step(&mut b).unwrap();
                    assert_eq!((c.ip, c.r[SP], c.flags), (0x1000, 0x9000, saved.flags));
                    c.step(&mut b).unwrap();
                    assert_eq!(c.ip, 0x1000 + code.len() as u32);
                    assert_eq!(&b.mem[0x4000..0x4005], &[0xa5, 0x10, 0, 0xa5, 0xa5]);
                    smsw_preserved(&c, &saved);
                }
            }
        }
    }
}

#[test]
fn smsw_lock_ud_register_memory_first_prefix_repair_and_retry() {
    for operand32 in [false, true] {
        for memory in [false, true] {
            let mut code = vec![0x26, 0xf0];
            if operand32 {
                code.push(0x66);
            }
            code.extend_from_slice(&[0x0f, 0x01, if memory { 0x27 } else { 0xe0 }]);
            let (mut c, mut b) = cpu(&code);
            c.set_reg32(AX, 0xabcdffff);
            c.r[BX] = 0x4001;
            c.flags = 2 | CF | DF | OF;
            let saved = c.clone();
            b.mem[0x4000..0x4005].fill(0xa5);
            c.step(&mut b).unwrap();
            fault(&c, &b, 6, 0x1000);
            assert_eq!(b.read16(0x8ffe), saved.flags);
            assert_eq!(c.reg32(AX), saved.reg32(AX));
            assert_eq!(&b.mem[0x4000..0x4005], &[0xa5; 5]);
            b.mem[c.ip as usize] = 0xcf;
            c.step(&mut b).unwrap();
            c.step(&mut b).unwrap(); // Unrepaired LOCK faults again.
            fault(&c, &b, 6, 0x1000);
            b.mem[0x1001] = 0x3e; // Repair LOCK in place, retain prefix length.
            c.step(&mut b).unwrap();
            assert_eq!(c.ip, 0x1000);
            c.step(&mut b).unwrap();
            assert_eq!(c.ip, 0x1000 + code.len() as u32);
            assert_eq!(c.r[SP], 0x9000);
            assert_eq!(c.reg32(AX), if memory { 0xabcdffff } else { 0xabcd0010 });
            assert_eq!(
                &b.mem[0x4000..0x4005],
                if memory {
                    &[0xa5, 0x10, 0, 0xa5, 0xa5]
                } else {
                    &[0xa5; 5]
                }
            );
            smsw_preserved(&c, &saved);
        }
    }
}

#[test]
fn smsw_bus_a20_rom_cpu_isolation_and_machine_reset_readback() {
    let (mut c, mut b) = cpu(&[0x64, 0x66, 0x0f, 0x01, 0x26, 0x20, 0]);
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
            &[0xa5, 0x10, 0, 0xa5, 0xa5, 0xa5]
        );
        assert_eq!(&b.mem[untouched..untouched + 6], &[0xa5; 6]);
    }
    c.extra_segments[0] = 0xf000;
    b.mem[0xf001f..0xf0025].fill(0xa5);
    c.ip = 0x1000;
    c.step(&mut b).unwrap();
    assert_eq!(&b.mem[0xf001f..0xf0025], &[0xa5; 6]);
    let (mut other, mut other_bus) = cpu(&[0x0f, 0x01, 0xe0]);
    c.cr0 = 0xa53e;
    other.step(&mut other_bus).unwrap();
    assert_eq!(other.r[AX], 0x10);
    c.ip = 0x1000;
    b.mem[0x1000..0x1003].copy_from_slice(&[0x0f, 0x01, 0xe0]);
    c.step(&mut b).unwrap();
    assert_eq!(c.r[AX], 0xa53e);
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu = c;
    m.reset();
    assert_eq!(m.cpu.cr0, 0x60000010);
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x1000;
    m.cpu.set_reg32(AX, 0xdeadffff);
    m.bus.mem[0x1000..0x1004].copy_from_slice(&[0x66, 0x0f, 0x01, 0xe0]);
    m.cpu.step(&mut m.bus).unwrap();
    assert_eq!(m.cpu.reg32(AX), 0xdead0010);
    assert_eq!(other.cr0, 0x60000010);
}

#[test]
fn smsw_original_boot_probe_before_and_after_machine_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/smsw/smsw.hdm"
    ))
    .expect("run python3 tools/prepare_smsw_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "smsw.hdm").unwrap();
    for run in 0..2 {
        if run == 1 {
            m.reset();
        }
        let mut slots = 0;
        for _ in 0..20 {
            slots += m.step(100_000);
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("SMSW PASS") || m.bus.text().contains("SMSW FAIL") {
                break;
            }
        }
        assert!(m.bus.text().contains("SMSW PASS"), "{}", m.bus.text());
        assert!(!m.bus.text().contains("FAIL"));
        assert!(slots <= 2_000_000);
        let mut expected = [0xa5; 80];
        for (offset, value) in [(1, 0xabcd0010u32), (9, 0xbcde0010), (65, 0xcdef0010)] {
            expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for offset in [17, 25, 33, 41, 49, 57] {
            expected[offset..offset + 2].copy_from_slice(&[0x10, 0]);
        }
        expected[73..75].copy_from_slice(&[2, 2]);
        assert_eq!(&m.bus.mem[0x8000..0x8050], &expected);
        assert_eq!(&m.bus.mem[0xfffd..0x10000], &[0xa5, 0x10, 0]);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
    }
}

#[test]
fn bound_signed_word_dword_size_and_address_overrides() {
    for wide in [false, true] {
        for address32 in [false, true] {
            let extremes = if wide {
                (i32::MIN as i64, i32::MAX as i64)
            } else {
                (i16::MIN as i64, i16::MAX as i64)
            };
            for (low, high) in [(0, 10), (-10, -2), (-2, 2), extremes, (5, 5), (10, -10)] {
                for value in [low - 1, low, low + (high - low) / 2, high, high + 1] {
                    if value < extremes.0 || value > extremes.1 {
                        continue;
                    }
                    let mut code = vec![0x26];
                    if wide {
                        code.push(0x66);
                    }
                    if address32 {
                        code.push(0x67);
                    }
                    code.extend(if address32 {
                        vec![0x62, 0x04, 0x8b]
                    } else {
                        vec![0x62, 0x06, 1, 0x30]
                    });
                    for reg in 0..8 {
                        let mut code = code.clone();
                        let modrm = code.iter().position(|byte| *byte == 0x62).unwrap() + 1;
                        code[modrm] |= (reg as u8) << 3;
                        let (mut c, mut b) = cpu(&code);
                        c.s[ES] = 0x4000;
                        c.set_reg32(BX, 0x3001);
                        c.set_reg32(CX, 4);
                        c.set_reg32(reg, value as u32);
                        // Avoid an unrelated #SS from an odd stack wrapping inside a word.
                        if reg == SP && [1, 3, 5].contains(&c.r[SP]) {
                            continue;
                        }
                        // SIB source registers can themselves be the index operand.
                        let off = if address32 {
                            c.reg32(BX).wrapping_add(c.reg32(CX).wrapping_mul(4))
                        } else {
                            0x3001
                        };
                        if off > 0xfff8 {
                            continue;
                        }
                        let width = if wide { 4 } else { 2 };
                        let start = 0x40000 + off as usize;
                        b.mem[start..start + width]
                            .copy_from_slice(&(low as u32).to_le_bytes()[..width]);
                        b.mem[start + width..start + 2 * width]
                            .copy_from_slice(&(high as u32).to_le_bytes()[..width]);
                        c.flags = 2 | CF | PF | AF | ZF | SF | DF | OF;
                        let saved = c.clone();
                        let memory = b.mem.clone();
                        c.step(&mut b).unwrap();
                        if value < low || value > high {
                            assert_eq!((c.s[CS], c.ip), (0, 0x2050));
                            let mut expected = memory;
                            for (i, word) in [0x1000, 0, saved.flags].iter().enumerate() {
                                let off = saved.r[SP].wrapping_sub(6).wrapping_add((i * 2) as u16)
                                    as usize;
                                assert_eq!(b.read16(off), *word);
                                expected[off..off + 2].copy_from_slice(&word.to_le_bytes());
                            }
                            assert_eq!(b.mem, expected);
                        } else {
                            assert_eq!(c.ip, 0x1000 + code.len() as u32);
                            for r in 0..8 {
                                assert_eq!(c.reg32(r), saved.reg32(r));
                            }
                            assert_eq!(c.flags, saved.flags);
                            assert_eq!(b.mem, memory);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn bound_first_prefix_fault_guest_repair_iret_retry() {
    for wide in [false, true] {
        // Nonzero CS and three prefixes prove restoration to the first prefix.
        let code = if wide {
            vec![0x26, 0x67, 0x66, 0x62, 0x05, 1, 0x30, 0, 0, 0x90]
        } else {
            vec![0x26, 0x67, 0x62, 0x05, 1, 0x30, 0, 0, 0x90]
        };
        let (mut c, mut b) = cpu(&[]);
        c.s[CS] = 0x1000;
        c.s[ES] = 0x4000;
        c.set_reg32(AX, if wide { 0x80000000 } else { 0x8000 });
        c.flags = 2 | CF | IF;
        b.mem[0x11000..0x11000 + code.len()].copy_from_slice(&code);
        let width = if wide { 4 } else { 2 };
        b.mem[0x43001..0x43001 + width].copy_from_slice(&0u32.to_le_bytes()[..width]);
        b.mem[0x43001 + width..0x43001 + 2 * width].copy_from_slice(&10u32.to_le_bytes()[..width]);
        // Guest repairs the register, records invocation, IRET retries BOUND.
        let handler = if wide {
            vec![0x66, 0xb8, 5, 0, 0, 0, 0xfe, 0x06, 0, 0x60, 0xcf]
        } else {
            vec![0xb8, 5, 0, 0xfe, 0x06, 0, 0x60, 0xcf]
        };
        b.mem[0x2050..0x2050 + handler.len()].copy_from_slice(&handler);
        c.step(&mut b).unwrap();
        assert_eq!((c.s[CS], c.ip), (0, 0x2050));
        assert_eq!(&b.mem[0x8ffa..0x9000], &[0, 0x10, 0, 0x10, 3, 2]);
        steps(&mut c, &mut b, 3);
        assert_eq!((c.s[CS], c.ip), (0x1000, 0x1000));
        c.step(&mut b).unwrap();
        assert_eq!(c.ip, 0x1000 + code.len() as u32 - 1);
        c.step(&mut b).unwrap();
        assert_eq!(c.ip, 0x1000 + code.len() as u32);
        assert_eq!(b.mem[0x6000], 1);
        assert_eq!(c.r[SP], 0x9000);
        assert_eq!(c.flags, 2 | CF | IF);
    }
}

#[test]
fn bound_register_and_lock_rejection_restart_before_first_prefix() {
    for code in [
        vec![0x26, 0x62, 0xc0],
        vec![0x66, 0x62, 0xff],
        vec![0x26, 0xf0, 0x62, 0x06, 0, 0x30],
        vec![0x67, 0x66, 0xf0, 0x62, 0x05, 0, 0x30, 0, 0],
    ] {
        let (mut c, mut b) = cpu(&code);
        let memory = b.mem.clone();
        c.step(&mut b).unwrap();
        fault(&c, &b, 6, 0x1000);
        assert_eq!(&b.mem[..0x8ffa], &memory[..0x8ffa]);
        assert_eq!(&b.mem[0x9000..], &memory[0x9000..]);
    }
}

#[test]
fn bound_segment_limits_preflight_both_bounds_before_comparison() {
    for wide in [false, true] {
        for stack in [false, true] {
            let width = if wide { 4 } else { 2 };
            for off in [0x10000 - width * 2, 0x10001 - width * 2, 0xffff, 0x10000] {
                let mut code = vec![if stack { 0x36 } else { 0x3e }, 0x67];
                if wide {
                    code.push(0x66);
                }
                code.extend([0x62, 0x05]);
                code.extend((off as u32).to_le_bytes());
                let (mut c, mut b) = cpu(&code);
                c.s[SS] = 0x2000;
                let base = if stack { 0x20000 } else { 0 };
                b.mem[base + off..base + off + width * 2].fill(0);
                c.r[AX] = 1; // outside, but segment fault must take precedence
                c.step(&mut b).unwrap();
                assert_eq!(
                    c.ip,
                    if off == 0x10000 - width * 2 {
                        0x2050
                    } else if stack {
                        0x20c0
                    } else {
                        0x20d0
                    }
                );
                assert_eq!(b.read16(0x28ffa), 0x1000);
                assert_eq!(c.r[AX], 1);
            }
        }
    }
}

#[test]
fn fs_gs_string_overrides_preserve_repeat_operand_and_address_sizes() {
    for segment in [0x64, 0x65] {
        for width in [1u32, 2, 4] {
            for address32 in [false, true] {
                for repeat in [0, 0xf2, 0xf3] {
                    let mut code = vec![segment];
                    if width == 4 {
                        code.push(0x66);
                    }
                    if address32 {
                        code.push(0x67);
                    }
                    if repeat != 0 {
                        code.push(repeat);
                    }
                    // MOVS tests copy/segment selection; CMPS distinguishes F2/F3.
                    code.push(if repeat == 0 {
                        if width == 1 {
                            0xa4
                        } else {
                            0xa5
                        }
                    } else if width == 1 {
                        0xa6
                    } else {
                        0xa7
                    });
                    let (mut c, mut b) = cpu(&code);
                    c.s[DS] = 0x1000;
                    c.s[ES] = 0x3000;
                    c.extra_segments = [0x4000, 0x5000];
                    let source = if segment == 0x64 { 0x44001 } else { 0x54001 };
                    c.set_reg32(SI, if address32 { 0x4001 } else { 0xabcd4001 });
                    c.set_reg32(DI, if address32 { 0x5001 } else { 0xbcde5001 });
                    c.set_reg32(CX, 0x10002);
                    c.flags = 2 | CF;
                    b.mem[source..source + 8].fill(5);
                    b.mem[0x35001..0x35009].fill(if repeat == 0xf2 { 6 } else { 5 });
                    let initial = b.mem.clone();
                    c.step(&mut b).unwrap();
                    assert_eq!(
                        c.reg32(SI),
                        if address32 { 0x4001 } else { 0xabcd4001 } + width
                    );
                    assert_eq!(
                        c.reg32(DI),
                        if address32 { 0x5001 } else { 0xbcde5001 } + width
                    );
                    assert_eq!(c.reg32(CX), if repeat == 0 { 0x10002 } else { 0x10001 });
                    assert_eq!(
                        c.ip,
                        if repeat == 0 {
                            0x1000 + code.len() as u32
                        } else {
                            0x1000
                        }
                    );
                    if repeat == 0 {
                        assert_eq!(
                            &b.mem[0x35001..0x35001 + width as usize],
                            &vec![5; width as usize]
                        );
                        assert_eq!(c.flags, 2 | CF);
                    } else {
                        assert_eq!(c.flags & (CF | ZF), if repeat == 0xf2 { CF } else { ZF });
                        // Opposite ZF stops either REP form with residual full ECX.
                        b.mem[0x35001 + width as usize..0x35001 + 2 * width as usize]
                            .fill(if repeat == 0xf2 { 5 } else { 6 });
                        let operands = b.mem.clone();
                        c.step(&mut b).unwrap();
                        assert_eq!(c.reg32(CX), 0x10000);
                        assert_eq!(c.ip, 0x1000 + code.len() as u32);
                        assert_eq!(b.mem, operands);
                    }
                    assert_eq!(&b.mem[source..source + 8], &initial[source..source + 8]);
                    assert_eq!(&b.mem[0x14001..0x14009], &initial[0x14001..0x14009]);
                    assert_eq!(c.extra_segments, [0x4000, 0x5000]);
                }
            }
        }
    }
}

#[test]
fn ap_v30_nibble_mem_retains_unimplemented_diagnostic_without_writes() {
    for ext in [0x28, 0x2a] {
        for modrm in [6, 0x0e, 0xc0, 0xc3] {
            let (mut c, mut b) = cpu(&[0x0f, ext, modrm, 0, 0x30]);
            c.r[AX] = 0x5a83;
            c.flags = 2 | CF | PF | AF | ZF | SF | DF | OF;
            b.mem[0x3000] = 0x12;
            let before = c.clone();
            let ram = b.mem.clone();
            // This is an emulator diagnostic, not an i486 exception claim.
            assert_eq!(
                c.step(&mut b).unwrap_err(),
                format!("unimplemented i486 opcode 0F {ext:02x}")
            );
            assert_eq!(c.r, before.r);
            assert_eq!(c.upper, before.upper);
            assert_eq!(c.s, before.s);
            assert_eq!(c.flags, before.flags);
            assert_eq!(b.mem, ram);
        }
    }
}

#[test]
fn ap_v30_segment_irq_non_ss_forms_remain_immediately_interruptible() {
    let mut producers = vec![vec![0x07], vec![0x1f]];
    for op in [0x8c, 0x8e] {
        for seg in [ES, CS, SS, DS] {
            if op == 0x8e && (seg == CS || seg == SS) {
                continue;
            }
            for memory in [false, true] {
                let mut code = vec![op, (seg as u8) << 3 | if memory { 6 } else { 0xc0 }];
                if memory {
                    code.extend_from_slice(&[1, 0x50]);
                }
                producers.push(code);
            }
        }
    }
    for producer in producers {
        let code = [producer.as_slice(), &[0x43, 0x90]].concat();
        let (mut c, mut b) = cpu(&code);
        c.flags = 2 | IF | CF | DF | OF;
        c.r[AX] = 0x1234;
        b.write_seg(0, 0x5001, 0x1234, true);
        b.write_seg(0, 0x9000, 0x1234, true);
        relocated(&mut c, &mut b, &[9]);
        b.mem[0x3400..0x3406].copy_from_slice(&[0x90, 0xb0, 0x20, 0xe6, 0, 0xcf]);
        let flags = c.flags;
        c.step(&mut b).unwrap();
        assert_eq!(c.flags, flags);
        let (resume, sp) = (c.ip, c.r[SP]);
        b.pics[0].mask = 0xfd;
        b.pics[0].request(1);
        c.step(&mut b).unwrap();
        assert_eq!(
            (c.s[CS], c.ip, c.r[BX]),
            (0x300, 0x401, 0),
            "{producer:02x?}"
        );
        assert_eq!((b.pics[0].irr, b.pics[0].isr), (0, 2));
        assert_eq!(
            (
                b.read16((sp - 6) as usize),
                b.read16((sp - 4) as usize),
                b.read16((sp - 2) as usize)
            ),
            (resume as u16, 0, flags)
        );
        steps(&mut c, &mut b, 3);
        assert_eq!(
            (c.ip, c.r[SP], c.flags, b.pics[0].isr),
            (resume, sp, flags, 0)
        );
        c.step(&mut b).unwrap();
        assert_eq!(c.r[BX], 1);
        // Non-SS forms must also retain their own single-step trap.
        let (mut c, mut b) = cpu(&code);
        c.flags |= TF;
        relocated(&mut c, &mut b, &[1]);
        c.step(&mut b).unwrap();
        assert_eq!((c.s[CS], c.ip), (0x300, 0x400));
        assert_eq!(b.read16((c.r[SP]) as usize), 0x1000 + producer.len() as u16);
    }
}

#[test]
fn ap_v30_add4s_bytes_are_invalid_i486_memory_control_read() {
    for prefix in [None, Some(0x26), Some(0x2e), Some(0x36), Some(0x3e)] {
        let mut code: Vec<u8> = prefix.into_iter().collect();
        code.extend([0x0f, 0x20, 0x90]);
        let (mut c, mut b) = cpu(&code);
        c.r[SI] = 0x3000;
        c.r[DI] = 0x4000;
        c.r[CX] = 0xab02;
        c.flags = 2 | CF | ZF | AF | OF | SF | PF | DF;
        b.mem[0x3000] = 0x09;
        b.mem[0x4000] = 1;
        let mut before = c.clone();
        let mut ram = b.mem.clone();
        let graphics = b.graphics.clone();
        // Ap decodes 90h as an invalid memory ModR/M for MOV r32,CR2.
        // VM's ADD4S remains a different, two-byte instruction.
        c.step(&mut b).unwrap();
        fault(&c, &b, 6, 0x1000);
        before.r[SP] -= 6;
        ram[0x8ffa..0x9000].copy_from_slice(&[0, 0x10, 0, 0, 0xd7, 0x0c]);
        assert_eq!(c.r, before.r);
        assert_eq!(c.upper, before.upper);
        assert_eq!(c.s, before.s);
        assert_eq!(c.flags, before.flags);
        assert_eq!(b.mem, ram);
        assert_eq!(b.graphics, graphics);
    }
}

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
fn flags_push_pop_interrupt_iretd_and_near_call_use_dword_stack_entries() {
    let (mut c, mut b) = cpu(&[0x66, 0x9d, 0x66, 0x9c]);
    b.mem[0x9000..0x9004].copy_from_slice(&0xffffffffu32.to_le_bytes());
    c.step(&mut b).unwrap();
    assert_eq!(c.flags, 0x7fd7);
    assert_eq!(c.flags_upper, 4); // AC supported, ID/VM/RF/reserved bits cannot be set.
    c.flags &= !TF;
    c.step(&mut b).unwrap();
    assert_eq!(b.read16(0x9002), 4);
    let (mut c, mut b) = cpu(&[0x66, 0xcd, 0x30]);
    b.mem[0x30 * 4..0x30 * 4 + 4].copy_from_slice(&[0, 0x30, 0, 0]);
    b.mem[0x3000..0x3002].copy_from_slice(&[0x66, 0xcf]);
    c.flags_upper = 4;
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x3000);
    assert_eq!(c.r[SP], 0x8ff4);
    assert_eq!(b.read16(0x8ff4), 0x1003);
    assert_eq!(b.read16(0x8ffe), 4);
    c.step(&mut b).unwrap();
    assert_eq!(c.ip, 0x1003);
    assert_eq!(c.r[SP], 0x9000);
    assert_eq!(c.flags_upper, 4);
    let (mut c, mut b) = cpu(&[0x66, 0xe8, 0xfa, 0x1f, 0, 0]); // call dword 3000h
    b.mem[0x3000..0x3002].copy_from_slice(&[0x66, 0xc3]);
    steps(&mut c, &mut b, 2);
    assert_eq!(c.ip, 0x1006);
    assert_eq!(c.r[SP], 0x9000);
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

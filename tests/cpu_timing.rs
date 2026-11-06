//! Literal timing expectations from NEC V20/V30 (Oct 1986), section 12,
//! and Intel486 DX2 data book (July 1992), tables 10.1/10.2.
use pc98_rust::{cpu::*, timing::ClockConverter, Machine, Profile};

fn machine(profile: Profile, code: &[u8]) -> Machine {
    let mut m = Machine::new(profile);
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x100;
    m.cpu.r[SP] = 0x8000;
    m.cpu.r[BX] = 0x2002;
    m.bus.mem[0x100..0x100 + code.len()].copy_from_slice(code);
    m.bus.mem[0x2000] = 2;
    m.bus.mem[0x2002] = 2;
    m
}
fn clocks(profile: Profile, code: &[u8]) -> u32 {
    let mut m = machine(profile, code);
    let (time, result) = m.cpu.step_timed(&mut m.bus);
    result.unwrap();
    assert_eq!(
        m.bus.cycles, 0,
        "CPU API leaves device conversion to Machine"
    );
    time.execution
}
#[test]
fn nec_documented_register_memory_immediate_stack_and_control_forms() {
    // NEC pp.12-4..12-20,12-30..12-83,12-142..12-176. Memory
    // operands here are aligned RAM, and the stack is even.
    let cases: &[(&[u8], u32)] = &[
        (&[0x89, 0xc1], 2),
        (&[0x89, 0x07], 9),
        (&[0x8b, 0x07], 11),
        (&[0xb8, 1, 0], 4),
        (&[0xc7, 0x07, 1, 0], 11),
        (&[0xa1, 0, 0x20], 10),
        (&[0xa3, 0, 0x20], 9),
        (&[0x01, 0xc1], 2),
        (&[0x01, 0x07], 16),
        (&[0x03, 0x07], 11),
        (&[0x83, 0xc0, 1], 4),
        (&[0x83, 0x07, 1], 18),
        (&[0x39, 0x07], 11),
        (&[0x83, 0x3f, 1], 13),
        (&[0x85, 0x07], 10),
        (&[0xf7, 0x07, 1, 0], 11),
        (&[0x87, 0xc1], 3),
        (&[0x87, 0x07], 16),
        (&[0x90], 3),
        (&[0x50], 8),
        (&[0x58], 8),
        (&[0x60], 35),
        (&[0x61], 43),
        (&[0x68, 0, 0], 8),
        (&[0x6a, 0], 7),
        (&[0xe8, 0, 0], 16),
        (&[0xff, 0xd3], 14),
        (&[0xff, 0x17], 23),
        (&[0xc3], 15),
        (&[0xc2, 2, 0], 20),
        (&[0xcb], 21),
        (&[0xcf], 27),
        (&[0xd0, 0xe0], 2),
        (&[0xd0, 0x27], 16),
        (&[0xc0, 0xe0, 5], 12),
        (&[0xc0, 0x27, 5], 24),
        (&[0xf6, 0xe3], 22),
        (&[0xf7, 0xe3], 30),
        (&[0xf6, 0xf3], 19),
        (&[0xf7, 0xf3], 25),
        (&[0xf7, 0x37], 31),
        (&[0xf7, 0xfb], 43),
        (&[0xe4, 0x40], 8),
        (&[0xec], 8),
        (&[0xee], 8),
        (&[0xf4], 2),
        (&[0xfa], 2),
        (&[0xfb], 2),
        (&[0x26, 0x8b, 0x07], 13),
        (&[0xf3, 0x90], 5),
        (&[0x8d, 0x40, 2], 4),
        (&[0x0f, 0x18, 0xc0, 0], 4),
        (&[0x0f, 0x12, 0x07], 14),
        (&[0x0f, 0x1e, 0x07, 1], 19),
        (&[0x0f, 0x2a, 0x07], 33),
    ];
    for &(code, expected) in cases {
        assert_eq!(clocks(Profile::Pc9801Vm, code), expected, "{code:02x?}");
    }
}
#[test]
fn dx2_documented_core_clocks_are_not_386_or_early_486_tables() {
    // DX2 pp.10-2..10-14. Bus penalties are asserted separately below.
    let cases: &[(&[u8], u32)] = &[
        (&[0x89, 0xc1], 1),
        (&[0x89, 0x07], 1),
        (&[0x8b, 0x07], 1),
        (&[0xb8, 1, 0], 1),
        (&[0x01, 0xc1], 1),
        (&[0x01, 0x07], 3),
        (&[0x03, 0x07], 2),
        (&[0x39, 0x07], 2),
        (&[0x83, 0xc0, 1], 1),
        (&[0x83, 0x07, 1], 3),
        (&[0x50], 1),
        (&[0x58], 1),
        (&[0xff, 0xf3], 4),
        (&[0x8f, 0xc0], 4),
        (&[0x8f, 0x07], 5),
        (&[0x60], 11),
        (&[0x61], 9),
        (&[0xe8, 0, 0], 3),
        (&[0xc3], 5),
        (&[0xcb], 13),
        (&[0xcf], 15),
        (&[0xc0, 0xe0, 5], 2),
        (&[0xd0, 0xe0], 3),
        (&[0xd0, 0x27], 4),
        (&[0xf7, 0xe3], 26),
        (&[0xf7, 0xf3], 24),
        (&[0xf7, 0xfb], 27),
        (&[0xf7, 0x3f], 28),
        (&[0xe4, 0x40], 17),
        (&[0xec], 17),
        (&[0xee], 19),
        (&[0xf4], 4),
        (&[0xfa], 5),
        (&[0xfb], 5),
        (&[0xcd, 0x20], 30),
        (&[0xcc], 26),
        (&[0xce], 3),
        (&[0x0f, 0x06], 7),
        (&[0x0f, 0x20, 0xc0], 4),
        (&[0x0f, 0x22, 0xc0], 17),
        (&[0x0f, 0x01, 0x17], 12),
        (&[0x0f, 0x01, 0x27], 3),
        (&[0x0f, 0xb6, 0xc0], 3),
        (&[0x66, 0x90], 2),
        (&[0xf3, 0x90], 2),
        (&[0x8d, 0x00], 2),
    ];
    for &(code, expected) in cases {
        assert_eq!(clocks(Profile::Pc9821ApDos, code), expected, "{code:02x?}");
    }
}
#[test]
fn branch_paths_and_shift_counts_use_execution_conditions() {
    for (profile, taken, untaken) in [(Profile::Pc9801Vm, 14, 4), (Profile::Pc9821ApDos, 3, 1)] {
        for flag in [0, ZF] {
            let mut m = machine(profile, &[0x74, 0]); // Taken branch can have displacement zero.
            m.cpu.flags = flag | 2;
            let (t, r) = m.cpu.step_timed(&mut m.bus);
            r.unwrap();
            assert_eq!(t.execution, if flag == 0 { untaken } else { taken });
        }
    }
    for count in [0, 1, 8, 31, 255] {
        let mut m = machine(Profile::Pc9801Vm, &[0xd2, 0xe0]);
        m.cpu.r[CX] = count;
        assert_eq!(
            m.cpu.step_timed(&mut m.bus).0.execution,
            7 + u32::from(count)
        );
        let mut m = machine(Profile::Pc9821ApDos, &[0xd2, 0xe0]);
        m.cpu.r[CX] = count;
        assert_eq!(m.cpu.step_timed(&mut m.bus).0.execution, 3);
    }
}
#[test]
fn rep_setup_is_once_per_instruction_and_elements_remain_bounded() {
    for (profile, setup, iteration) in [(Profile::Pc9801Vm, 7, 4), (Profile::Pc9821ApDos, 7, 4)] {
        for count in [0, 1, 2, 17] {
            let mut m = machine(profile, &[0xf3, 0xaa]);
            m.cpu.r[CX] = count;
            m.cpu.r[DI] = 0x3000;
            let mut total = 0;
            for remaining in (0..count.max(1)).rev() {
                let (t, r) = m.cpu.step_timed(&mut m.bus);
                r.unwrap();
                total += t.execution;
                assert_eq!(m.cpu.r[CX], remaining);
                assert_eq!(t.rep_elements, u32::from(count != 0));
            }
            assert_eq!(
                total,
                if count == 0 {
                    if profile == Profile::Pc9801Vm {
                        7
                    } else {
                        5
                    }
                } else {
                    setup + iteration * u32::from(count)
                }
            );
            assert_eq!(m.cpu.completed_instructions, 1);
            assert_eq!(m.cpu.rep_elements, u64::from(count));
        }
    }
    for (count, expected) in [(0, 5), (1, 13), (2, 18), (5, 27)] {
        let mut m = machine(Profile::Pc9821ApDos, &[0xf3, 0xa4]);
        m.cpu.r[CX] = count;
        m.cpu.r[SI] = 0x2000;
        m.cpu.r[DI] = 0x3000;
        let total: u32 = (0..count.max(1))
            .map(|_| m.cpu.step_timed(&mut m.bus).0.execution)
            .sum();
        assert_eq!(total, expected);
    }
}
#[test]
fn rep_early_exit_and_interrupt_resume_charge_setup_without_losing_shadow() {
    let mut m = machine(Profile::Pc9801Vm, &[0xf3, 0xa6]);
    m.cpu.r[CX] = 5;
    m.cpu.r[SI] = 0x2000;
    m.cpu.r[DI] = 0x3000;
    let (t, r) = m.cpu.step_timed(&mut m.bus);
    r.unwrap();
    assert_eq!((t.execution, t.instructions, t.rep_elements), (21, 1, 1));
    assert_eq!(m.cpu.r[CX], 4); // unequal first bytes end REPE.

    let mut m = machine(Profile::Pc9801Vm, &[0xfb, 0xf3, 0xaa]);
    m.cpu.r[CX] = 3;
    m.cpu.r[DI] = 0x3000;
    m.bus.mem[9 * 4..9 * 4 + 4].copy_from_slice(&[0, 4, 0, 0]);
    m.bus.mem[0x400] = 0xcf; // IRQ returns to REP prefix.
    m.bus.pics[0].mask = 0xfd;
    m.step(1);
    m.bus.pics[0].request(1);
    m.step(1);
    assert_eq!(m.cpu.irq_acks, 0);
    assert_eq!(m.cpu.last_timing.execution, 11);
    m.step(1);
    assert_eq!(m.cpu.irq_acks, 1);
    assert_eq!(m.cpu.ip, 0x101);
    m.step(1);
    assert_eq!(m.cpu.last_timing.execution, 11); // restart setup after IRET
    m.step(1);
    assert_eq!(m.cpu.last_timing.execution, 4);
    assert_eq!(m.cpu.rep_elements, 3);
}
#[test]
fn bus_policy_distinguishes_alignment_vram_io_and_cache_misses() {
    for (address, expected) in [(0x2000, 0), (0x2001, 4), (0xa8000, 2), (0xa8001, 8)] {
        let mut m = machine(Profile::Pc9801Vm, &[0x8b, 0x07]);
        m.cpu.r[BX] = address as u16;
        m.cpu.s[DS] = (address >> 16) as u16 * 0x1000;
        let (t, r) = m.cpu.step_timed(&mut m.bus);
        r.unwrap();
        assert_eq!(t.bus, expected);
    }
    let mut m = machine(Profile::Pc9801Vm, &[0xed]);
    m.cpu.r[DX] = 0x41;
    assert_eq!(m.cpu.step_timed(&mut m.bus).0.bus, 4);
    let mut m = machine(Profile::Pc9821ApDos, &[0x8b, 0x07]);
    m.cpu.cr0 &= !0x40000000; // Enable the timing cache.
    let cold = m.cpu.step_timed(&mut m.bus).0;
    m.cpu.ip = 0x100;
    let warm = m.cpu.step_timed(&mut m.bus).0;
    assert_eq!((cold.bus, warm.bus), (20, 0)); // Separate code/data line fills.
    m.bus.write(0x2000, 3); // External/DMA write invalidates timing residency.
    m.cpu.ip = 0x100;
    assert_eq!(m.cpu.step_timed(&mut m.bus).0.bus, 10);
    m.cpu.cr0 |= 0x40000000;
    m.cpu.ip = 0x100;
    assert_eq!(m.cpu.step_timed(&mut m.bus).0.bus, 8);
}
#[test]
fn clock_domains_fractional_batches_halt_and_device_time() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut small = ClockConverter::default();
        let mut batched = ClockConverter::default();
        let cycles = profile.cpu_hz() as u32;
        let one_by_one: u32 = (0..cycles / 1000)
            .map(|_| small.advance(profile, 1000))
            .sum();
        assert_eq!(one_by_one, 8_000_000);
        assert_eq!(batched.advance(profile, cycles), one_by_one);
        let mut m = machine(profile, &[0xf4]);
        m.step(1);
        let retired = m.cpu.retired;
        let before = m.bus.cycles;
        for _ in 0..10 {
            m.step_scheduler(80_000, 100_000);
            m.bus.audio.clear();
        }
        assert_eq!(m.cpu.retired, retired);
        assert_eq!(m.bus.cycles - before, 800_000);
        assert_eq!(m.cpu.last_timing.instructions, 0);
        assert_ne!(m.cpu.last_timing.idle, 0);
    }
    let mut m = machine(Profile::Pc9821ApDos, &[0x90; 16]);
    m.cpu.cr0 &= !0x40000000;
    for _ in 0..10 {
        m.step(1);
    }
    assert_eq!(m.cpu.cycles, 20); // ten 1-clock NOPs plus one cold line fill
    assert_eq!(m.bus.cycles, 2); // 20*8/66, fractional remainder retained
}
#[test]
fn faults_keep_time_but_do_not_retire_and_firmware_waits_are_not_ticked_twice() {
    let mut m = machine(Profile::Pc9821ApDos, &[0xf7, 0xf1]); // DIV CX=0
    m.bus.mem[..4].copy_from_slice(&[0, 4, 0, 0]);
    m.bus.mem[0x400] = 0xf4;
    let before = m.cpu.cycles;
    m.step(1);
    assert!(m.error.is_none(), "{:?}", m.error);
    assert_eq!(m.cpu.fault_count, 1);
    assert_eq!(m.cpu.completed_instructions, 0);
    assert!(m.cpu.cycles > before);
    assert!(m.cpu.last_timing.approximated);
    assert_eq!(m.bus.cycles, m.cpu.cycles * 8 / 66);
    let mut m = Machine::default();
    m.insert(
        0,
        std::fs::read("fixtures/fdc_bios/fdc_bios.hdm").unwrap(),
        "probe.hdm",
    )
    .unwrap();
    m.reset();
    let mut firmware_clocks = 0;
    for _ in 0..10000 {
        m.step(1);
        firmware_clocks += m.cpu.last_timing.firmware_scheduler;
        assert!(m.error.is_none(), "{:?}", m.error);
        assert_eq!(m.bus.cycles, m.cpu.cycles * 8 / 10 + firmware_clocks);
        if firmware_clocks != 0 {
            break;
        }
    }
    assert!(firmware_clocks > 0);
}

#[test]
fn pit_and_pcm_cadence_follow_guest_time_across_cpu_throughputs() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for code in [&[0x90, 0xeb, 0xfd][..], &[0xf7, 0xf3, 0xeb, 0xfc][..]] {
            let mut m = machine(profile, code);
            m.cpu.r[BX] = 2;
            m.bus.pit.write(3, 0x34); // mode 2, 2000 input clocks
            m.bus.pit.write(0, 0xd0);
            m.bus.pit.write(0, 7);
            let mut samples = 0;
            while m.bus.cycles < 8_000_000 {
                m.step_scheduler((8_000_000 - m.bus.cycles).min(80_000) as u32, 100_000);
                assert!(m.error.is_none(), "{:?}", m.error);
                samples += m.bus.audio.len();
                m.bus.audio.clear();
            }
            assert!((8_000_000..8_000_020).contains(&m.bus.cycles));
            assert_eq!(samples, 48_000);
            assert_ne!(m.bus.pics[0].irr & 1, 0);
            // 1,996,800 input clocks = 998 complete 2000-clock periods,
            // plus 800 clocks; the first clock loads without decrementing.
            assert!((1196..=1201).contains(&m.bus.pit.c[0].remaining));
        }
    }
}

#[test]
fn planar_transfer_workload_matches_table_plus_explicit_video_wait_policy() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = machine(profile, &[0xf3, 0xab]);
        m.cpu.s[ES] = 0xa800;
        m.cpu.r[CX] = 16384; // one complete 32 KiB plane
        m.cpu.r[AX] = 0x5aa5;
        m.step(16384);
        assert!(m.error.is_none(), "{:?}", m.error);
        assert_eq!(m.cpu.completed_instructions, 1);
        assert_eq!(m.cpu.rep_elements, 16384);
        let expected = if profile == Profile::Pc9801Vm {
            7 + 16384 * (4 + 2) // NEC STMW iterations + two VRAM waits
        } else {
            7 + 16384 * (4 + 8) + 4 // DX2 iterations + 16-bit video beats + code read
        };
        assert_eq!(m.cpu.cycles, expected);
        assert_eq!(m.bus.cycles, expected * 8_000_000 / profile.cpu_hz());
        assert!(m.bus.graphics[..32768]
            .chunks_exact(2)
            .all(|w| w == [0xa5, 0x5a]));
        println!(
            "{profile:?} 32 KiB STOSW: {expected} core clocks, {:.4} ms guest",
            m.bus.cycles as f64 / 8000.0
        );
    }
}

#[test]
fn external_entry_and_address_dependencies_are_separate_from_handler_retirement() {
    for (profile, entry) in [(Profile::Pc9801Vm, 50), (Profile::Pc9821ApDos, 37)] {
        let mut m = machine(profile, &[0x90]);
        m.bus.mem[9 * 4..9 * 4 + 4].copy_from_slice(&[0, 4, 0, 0]);
        m.bus.mem[0x400] = 0x90;
        let (t, r) = m.cpu.interrupt_timed(&mut m.bus, 9);
        r.unwrap();
        assert_eq!(t.execution, entry);
        assert_eq!(t.instructions, 0);
        assert_eq!(m.cpu.completed_instructions, 0);
        assert_eq!(m.bus.cycles, 0);
        m.step(1);
        assert_eq!(m.cpu.completed_instructions, 1);
    }
    let mut m = machine(Profile::Pc9821ApDos, &[0x43, 0x8b, 0x07]);
    m.step(1);
    m.step(1);
    assert_eq!(m.cpu.last_timing.execution, 2); // base written immediately before MOV
    for (code, expected) in [(&[0x0f, 0xbc, 0xc0][..], 6), (&[0x0f, 0xbd, 0xc0][..], 6)] {
        assert_eq!(clocks(Profile::Pc9821ApDos, code), expected);
    }
}

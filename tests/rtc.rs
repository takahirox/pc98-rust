//! Original RTC read-path regressions (MIT); expectations from NEC IC-1755.
use pc98_rust::{bus::Bus, Machine, Profile};

fn command(b: &mut Bus, command: u8) {
    b.output(0x20, command);
    b.output(0x20, command | 8);
    b.output(0x20, command);
}

fn read_bits(b: &mut Bus, control: u8, count: usize) -> u64 {
    let mut value = 0;
    for bit in 0..count {
        value |= u64::from(b.input(0x33) & 1) << bit;
        b.output(0x20, control | 16);
        b.output(0x20, control);
    }
    value
}

#[test]
fn original_parallel_port_reproduction() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.bus.tick(8_000_000);
    command(&mut m.bus, 3);
    command(&mut m.bus, 1);
    // LSB-first seconds, minutes, hours, day, month/weekday (Thursday=4).
    assert_eq!(read_bits(&mut m.bus, 1, 40), 0x14_01_00_00_01);
}

fn serial_command(b: &mut Bus, value: u8) {
    for i in 0..4 {
        let control = 7 | ((value >> i & 1) << 5);
        b.output(0x20, control);
        b.output(0x20, control | 16);
        b.output(0x20, control);
    }
    command(b, 7);
}
fn seed(y: u8, mo: u8, d: u8, w: u8, h: u8, mi: u8, s: u8) -> Machine {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.bus.rtc =
        pc98_rust::rtc::Rtc::new(pc98_rust::rtc::Calendar::new(y, mo, d, w, h, mi, s).unwrap());
    m
}
fn capture(b: &mut Bus, extended: bool) -> u64 {
    if extended {
        serial_command(b, 3);
        serial_command(b, 1);
    } else {
        command(b, 3);
        command(b, 1);
    }
    read_bits(
        b,
        if extended { 7 } else { 1 },
        if extended { 52 } else { 40 },
    )
}

#[test]
fn exact_parallel_and_extended_streams_and_command_tail() {
    let mut m = seed(86, 11, 25, 6, 2, 23, 49);
    // Literal BCD bytes, hexadecimal November and weekday=6; IC-1755 Fig. 4.
    assert_eq!(capture(&mut m.bus, false), 0xb6_25_02_23_49);
    // Compatibility mode feeds DATA IN directly into the 40-bit register.
    read_bits(&mut m.bus, 0x21, 40);
    assert_eq!(read_bits(&mut m.bus, 1, 40), (1 << 40) - 1);
    assert_eq!(capture(&mut m.bus, true), 0x1_86_b6_25_02_23_49);
    // A complete 52-bit zero-input transfer drains both shift registers.
    assert_eq!(read_bits(&mut m.bus, 7, 52), 0);
}

#[test]
fn rising_edges_initial_lsb_trailing_sampling_and_unchanged_writes() {
    let mut m = seed(26, 10, 31, 6, 23, 59, 25);
    command(&mut m.bus, 3);
    command(&mut m.bus, 1);
    assert_eq!(m.bus.input(0x33), 0x39); // initial seconds LSB
    for _ in 0..5 {
        m.bus.output(0x20, 1);
    }
    assert_eq!(m.bus.input(0x33), 0x39);
    m.bus.output(0x20, 17); // rising shifts: next bit=0
    for _ in 0..5 {
        m.bus.output(0x20, 17);
    }
    assert_eq!(m.bus.input(0x33), 0x38);
    m.bus.output(0x20, 1); // trailing edge leaves bit stable for sampling
    assert_eq!(m.bus.input(0x33), 0x38);
    m.bus.output(0x20, 17);
    m.bus.output(0x20, 1);
    assert_eq!(m.bus.input(0x33), 0x39); // third bit of 25h
    assert_eq!(read_bits(&mut m.bus, 1, 38), 0xa6_31_23_59_25 >> 2);
}

#[test]
fn snapshot_hold_partial_reads_rollover_and_recapture() {
    let mut m = seed(99, 12, 31, 5, 23, 59, 59);
    serial_command(&mut m.bus, 3);
    // Clocks in Time Read must not shift calendar data, but frame Shift.
    serial_command(&mut m.bus, 1);
    let first = read_bits(&mut m.bus, 7, 13);
    m.bus.tick(8_000_000);
    // No extra clocks when STB or CLK remains unchanged.
    assert_eq!(m.bus.rtc.snapshot(), [0, 0x16, 1, 0, 0, 0]);
    let rest = read_bits(&mut m.bus, 7, 39);
    assert_eq!(first | rest << 13, 0x1_99_c5_31_23_59_59);
    assert_eq!(capture(&mut m.bus, true), 0x1_00_16_01_00_00_00);
    // Parallel hold freezes a partial transfer and does not reload it.
    command(&mut m.bus, 3);
    command(&mut m.bus, 1);
    let first = read_bits(&mut m.bus, 1, 9);
    command(&mut m.bus, 0);
    read_bits(&mut m.bus, 0, 50);
    m.bus.tick(8_000_000);
    command(&mut m.bus, 1);
    assert_eq!(first | read_bits(&mut m.bus, 1, 31) << 9, 0x16_01_00_00_00);
    // Abort a partial transaction with a fresh capture of the running counter.
    command(&mut m.bus, 3);
    command(&mut m.bus, 1);
    read_bits(&mut m.bus, 1, 7);
    m.bus.tick(8_000_000);
    command(&mut m.bus, 3);
    command(&mut m.bus, 1);
    assert_eq!(read_bits(&mut m.bus, 1, 40), 0x16_01_00_00_02);
}

#[test]
fn strobe_is_an_edge_and_command_register_is_independent() {
    let mut m = seed(26, 1, 1, 4, 0, 0, 59);
    m.bus.output(0x20, 3);
    m.bus.output(0x20, 11); // capture 59
    m.bus.tick(8_000_000);
    m.bus.output(0x20, 11); // unchanged high cannot capture 00
    m.bus.output(0x20, 3);
    command(&mut m.bus, 1);
    assert_eq!(read_bits(&mut m.bus, 1, 40), 0x14_01_00_00_59);
    serial_command(&mut m.bus, 0);
    // The most recent four bits form the command, irrespective of framing.
    for bit in [1, 0, 1, 1, 0, 0] {
        m.bus.output(0x20, 7 | (bit << 5));
        m.bus.output(0x20, 23 | (bit << 5));
        m.bus.output(0x20, 7 | (bit << 5));
    }
    // Trailing 1,1,0,0 forms Time Read.
    command(&mut m.bus, 7);
    // Hold still permits loading the next command, but does not shift data.
    serial_command(&mut m.bus, 0);
    read_bits(&mut m.bus, 7, 15); // zeroes through the independent command reg
    serial_command(&mut m.bus, 1);
    assert_eq!(read_bits(&mut m.bus, 7, 52), 0x1_26_14_01_00_01_00);
}

#[test]
fn one_hz_output_preserves_system_port_inputs_and_capture() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    for mode in [0, 3] {
        command(&mut m.bus, mode);
        assert_eq!(m.bus.input(0x33), 0x38);
        m.bus.tick(3_999_999);
        assert_eq!(m.bus.input(0x33), 0x38);
        m.bus.tick(1);
        assert_eq!(m.bus.input(0x33), 0x39);
        m.bus.tick(4_000_000);
        assert_eq!(m.bus.input(0x33), 0x38);
    }
    assert!(!m.bus.unknown_ports.contains(&0x20));
    let mut vm = Machine::new(Profile::Pc9801Vm);
    vm.bus.output(0x20, 11);
    vm.bus.tick(4_000_000);
    assert_eq!(vm.bus.input(0x33), 0x38);
    assert_eq!(vm.bus.rtc.snapshot(), [0x26, 0x14, 1, 0, 0, 0]);
}

#[test]
fn calendar_boundaries_and_mode_specific_year_and_leap_days() {
    // Independently listed seeds and expected BIOS bytes after one second.
    let cases = [
        ((26, 1, 1, 4, 0, 0, 59), [0x26, 0x14, 1, 0, 1, 0]),
        ((26, 1, 1, 4, 0, 59, 59), [0x26, 0x14, 1, 1, 0, 0]),
        ((26, 1, 1, 4, 23, 59, 59), [0x26, 0x15, 2, 0, 0, 0]),
        ((26, 4, 30, 4, 23, 59, 59), [0x26, 0x55, 1, 0, 0, 0]),
        ((26, 1, 31, 6, 23, 59, 59), [0x26, 0x20, 1, 0, 0, 0]),
        ((26, 2, 28, 6, 23, 59, 59), [0x26, 0x30, 1, 0, 0, 0]),
        ((28, 2, 28, 1, 23, 59, 59), [0x28, 0x22, 0x29, 0, 0, 0]),
        ((28, 2, 29, 2, 23, 59, 59), [0x28, 0x33, 1, 0, 0, 0]),
        ((99, 12, 31, 5, 23, 59, 59), [0, 0x16, 1, 0, 0, 0]),
        ((0, 2, 28, 1, 23, 59, 59), [0, 0x22, 0x29, 0, 0, 0]),
    ];
    for ((y, mo, d, w, h, mi, s), expected) in cases {
        let mut m = seed(y, mo, d, w, h, mi, s);
        serial_command(&mut m.bus, 0);
        m.bus.tick(8_000_000);
        assert_eq!(m.bus.rtc.snapshot(), expected);
    }
    for (y, mo, d, w, expected) in [
        (28, 2, 28, 1, [0x28, 0x32, 1, 0, 0, 0]), // no automatic Feb 29
        (28, 2, 29, 2, [0x28, 0x33, 1, 0, 0, 0]), // seeded Feb 29 rolls normally
        (99, 12, 31, 5, [0x99, 0x16, 1, 0, 0, 0]), // year counter disabled
    ] {
        let mut m = seed(y, mo, d, w, 23, 59, 59);
        command(&mut m.bus, 0);
        m.bus.tick(8_000_000);
        assert_eq!(m.bus.rtc.snapshot(), expected);
    }
}

#[test]
fn small_irregular_and_max_ticks_keep_fraction_and_instances_independent() {
    // Individual 8-clock scheduler slots versus batches straddling seconds.
    let mut slots = pc98_rust::rtc::Rtc::default();
    let mut irregular = pc98_rust::rtc::Rtc::default();
    for _ in 0..2_000_000 {
        slots.tick(8);
    }
    slots.tick(1);
    for n in [0, 1, 7_999_998, 2, 8_000_000] {
        irregular.tick(n);
    }
    assert_eq!(slots.snapshot(), [0x26, 0x14, 1, 0, 0, 2]);
    assert_eq!(slots.snapshot(), irregular.snapshot());
    for rtc in [&mut slots, &mut irregular] {
        rtc.tick(3_999_998);
        assert_eq!(rtc.data_out(), 0);
        rtc.tick(1);
        assert_eq!(rtc.data_out(), 1);
    }
    let mut small = seed(28, 2, 28, 1, 23, 50, 0);
    let mut batches = seed(28, 2, 28, 1, 23, 50, 0);
    serial_command(&mut small.bus, 0);
    serial_command(&mut batches.bus, 0);
    let clocks = u64::from(u32::MAX) * 2 + 1_234_567;
    let mut remaining = clocks;
    while remaining != 0 {
        let n = remaining.min(1_000_003) as u32;
        small.bus.tick(n);
        remaining -= u64::from(n);
    }
    for n in [u32::MAX, 17, u32::MAX - 17, 1_234_567] {
        batches.bus.tick(n);
    }
    // 1073 seconds = 17m53s, remainder 7,169,157 clocks.
    assert_eq!(small.bus.rtc.snapshot(), [0x28, 0x22, 0x29, 0, 7, 0x53]);
    assert_eq!(small.bus.rtc.snapshot(), batches.bus.rtc.snapshot());
    for m in [&mut small, &mut batches] {
        m.bus.tick(8_000_000 - (clocks % 8_000_000) as u32 - 1);
        assert_eq!(m.bus.rtc.snapshot()[5], 0x53);
        m.bus.tick(1);
        assert_eq!(m.bus.rtc.snapshot()[5], 0x54);
    }
    let independent = Machine::new(Profile::Pc9821ApDos);
    assert_eq!(independent.bus.rtc.snapshot(), [0x26, 0x14, 1, 0, 0, 0]);
}

#[test]
fn soft_reset_retains_calendar_fraction_and_inflight_serial_interface() {
    let mut m = seed(99, 12, 31, 5, 23, 59, 59);
    serial_command(&mut m.bus, 3);
    serial_command(&mut m.bus, 1);
    let low = read_bits(&mut m.bus, 7, 17);
    m.bus.tick(7_999_999);
    m.reset();
    assert_eq!(
        low | read_bits(&mut m.bus, 7, 35) << 17,
        0x1_99_c5_31_23_59_59
    );
    m.bus.tick(1);
    assert_eq!(m.bus.rtc.snapshot(), [0, 0x16, 1, 0, 0, 0]);
}

#[test]
fn bios_guest_int_iret_preserves_registers_flags_and_segment_offset_wrapping() {
    use pc98_rust::cpu::*;
    for offset in [0x200u16, 0xfffd] {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        load_calendar(&mut m.bus, true, &[0x49, 0x23, 0x02, 0x25, 0xb6, 0x86]);
        let es = 0x1000;
        for i in 0..8 {
            m.bus
                .write_seg(es, offset.wrapping_sub(1).wrapping_add(i), 0xa5, false);
        }
        m.cpu.s = [es, 0, 0x3000, 0x4567];
        m.cpu.r = [
            0x00ab, 0x5678, 0x9876, offset, 0x8000, 0x4567, 0x6789, 0x789a,
        ];
        m.cpu.flags = IF | DF | CF | OF | 2;
        m.cpu.ip = 0x100;
        m.bus.mem[0x100..0x103].copy_from_slice(&[0xcd, 0x1c, 0xf4]);
        let regs = m.cpu.r;
        let segments = m.cpu.s;
        let flags = m.cpu.flags;
        assert_eq!(m.step(3), 3); // INT, ROM OUT, IRET
        assert!(m.error.is_none(), "{:?}", m.error);
        assert_eq!(m.cpu.r, regs);
        assert_eq!(m.cpu.s, segments);
        assert_eq!(m.cpu.flags, flags);
        assert_eq!(m.cpu.ip, 0x102);
        let actual: Vec<u8> = (0..6)
            .map(|i| m.bus.read_seg(es, offset.wrapping_add(i), false) as u8)
            .collect();
        assert_eq!(actual, [0x86, 0xb6, 0x25, 2, 0x23, 0x49]);
        assert_eq!(m.bus.read_seg(es, offset.wrapping_sub(1), false), 0xa5);
        assert_eq!(m.bus.read_seg(es, offset.wrapping_add(6), false), 0xa5);
        assert_eq!(
            capture(&mut m.bus, true) & ((1 << 48) - 1),
            0x86_b6_25_02_23_49
        );
    }
}

#[test]
fn guest_scheduler_advances_with_if_clear_and_halted() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.cpu.flags = 2;
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x100;
    m.bus.mem[0x100..0x102].copy_from_slice(&[0xeb, 0xfe]); // JMP, IF clear
    for _ in 0..5 {
        assert_eq!(m.step(100_000), 100_000);
    }
    assert_eq!(m.bus.rtc.snapshot()[5], 0);
    m.cpu.halted = true;
    for _ in 0..5 {
        assert_eq!(m.step(100_000), 100_000);
    }
    assert!(m.error.is_none());
    assert_eq!(m.bus.rtc.snapshot()[5], 1);
    assert!(m.cpu.halted);
}

#[test]
fn original_rtc_boot_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/rtc/rtc.hdm"))
        .expect("run python3 tools/prepare_rtc_fixture.py");
    let manifest = include_str!("../fixtures/rtc-manifest.json");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "rtc.hdm").unwrap();
    for run in 0..2 {
        if run != 0 {
            m.reset();
        }
        let mut slots = 0;
        while slots < 2_000_000 {
            slots += m.step(1000);
            assert!(m.error.is_none(), "{:?}", m.error);
            let text = m.bus.text();
            assert!(
                !text.contains("FAIL"),
                "boot {run}, {slots} slots: {text}; {:02x?}",
                &m.bus.mem[0x8000..0x8040]
            );
            if text.contains("RTC CALENDAR PASS") {
                break;
            }
        }
        assert!(slots <= 2_000_000);
        assert!(
            m.bus.text().contains("RTC CALENDAR PASS"),
            "{}",
            m.bus.text()
        );
        let marker = if run == 0 {
            "\"result_hex\": \""
        } else {
            "\"reset_result_hex\": \""
        };
        let hex = manifest
            .split_once(marker)
            .unwrap()
            .1
            .split('"')
            .next()
            .unwrap();
        let expected: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(&m.bus.mem[0x8000..0x8040], expected.as_slice());
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        println!("Ap RTC boot {run}: {slots} slots");
    }
}

fn load_calendar(b: &mut Bus, extended: bool, bytes: &[u8]) {
    if extended {
        serial_command(b, 1);
    } else {
        command(b, 1);
    }
    // Serial Shift is a 52-bit chain: calendar first, then the final command.
    // Six calendar bytes alone leave four bits in the command register;
    // the final command clocks complete the chain before STB.
    for byte in bytes {
        for bit in 0..8 {
            let control = if extended { 7 } else { 1 } | ((byte >> bit & 1) << 5);
            b.output(0x20, control);
            b.output(0x20, control | 16);
            b.output(0x20, control);
        }
    }
    if extended {
        serial_command(b, 2);
    } else {
        command(b, 2);
    }
}

#[test]
fn original_time_set_port_reproduction() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    load_calendar(&mut m.bus, false, &[0x59, 0x58, 0x23, 0x30, 0x46]);
    assert_eq!(m.bus.rtc.snapshot(), [0x26, 0x46, 0x30, 0x23, 0x58, 0x59]);
    m.bus.tick(24_000_000);
    assert_eq!(m.bus.rtc.snapshot(), [0x26, 0x46, 0x30, 0x23, 0x58, 0x59]);
    assert_eq!(m.bus.input(0x33), 0x39);
    command(&mut m.bus, 0);
    m.bus.tick(8_000_000);
    assert_eq!(m.bus.rtc.snapshot(), [0x26, 0x46, 0x30, 0x23, 0x59, 0]);
}

#[test]
fn time_set_forms_hold_data_and_independent_command_shifting() {
    for extended in [false, true] {
        for second in [0x48, 0x49] {
            let mut m = Machine::new(Profile::Pc9821ApDos);
            let bytes = [second, 0x23, 0x02, 0x25, 0xb6, 0x86];
            load_calendar(&mut m.bus, extended, &bytes[..if extended { 6 } else { 5 }]);
            let expected = [
                if extended { 0x86 } else { 0x26 },
                0xb6,
                0x25,
                2,
                0x23,
                second,
            ];
            assert_eq!(m.bus.rtc.snapshot(), expected);
            let port = 0x38 | (second & 1);
            assert_eq!(m.bus.input(0x33), port);
            // Neither CLK edges nor changing the serial command register may
            // displace held calendar data. Repeated high STB is not an edge.
            m.bus.output(0x20, if extended { 15 } else { 10 });
            for _ in 0..7 {
                m.bus.output(0x20, if extended { 15 } else { 10 });
                m.bus.tick(1_234_567);
                assert_eq!(m.bus.input(0x33), port);
            }
            for input in [0, 32, 32, 0, 32, 0, 0, 32] {
                m.bus.output(0x20, 7 | input);
                m.bus.output(0x20, 23 | input);
                m.bus.output(0x20, 23 | input);
                m.bus.output(0x20, 7 | input);
                assert_eq!(m.bus.input(0x33), port);
            }
            assert_eq!(m.bus.rtc.snapshot(), expected);
            // Serial 0000 is framed independently while the data register holds.
            // After release into Shift, the untouched stream must be exact.
            if extended {
                serial_command(&mut m.bus, 0);
                serial_command(&mut m.bus, 1);
                assert_eq!(
                    read_bits(&mut m.bus, 7, 52),
                    0x1_86_b6_25_02_23_00 | u64::from(second)
                );
            } else {
                command(&mut m.bus, 1);
                assert_eq!(
                    read_bits(&mut m.bus, 1, 40),
                    0xb6_25_02_23_00 | u64::from(second)
                );
            }
        }
    }
}

#[test]
fn guest_written_calendar_transitions_have_literal_serial_and_bios_oracles() {
    for (extended, payload, before, after, stream) in [
        (
            false,
            [0x59, 0x59, 0x23, 0x30, 0x46, 0],
            [0x26, 0x46, 0x30, 0x23, 0x59, 0x59],
            [0x26, 0x50, 1, 0, 0, 0],
            0x50_01_00_00_00,
        ),
        (
            true,
            [0x59, 0x59, 0x23, 0x28, 0x21, 0x28],
            [0x28, 0x21, 0x28, 0x23, 0x59, 0x59],
            [0x28, 0x22, 0x29, 0, 0, 0],
            0x1_28_22_29_00_00_00,
        ),
        (
            true,
            [0x59, 0x59, 0x23, 0x29, 0x22, 0x28],
            [0x28, 0x22, 0x29, 0x23, 0x59, 0x59],
            [0x28, 0x33, 1, 0, 0, 0],
            0x1_28_33_01_00_00_00,
        ),
        (
            true,
            [0x59, 0x59, 0x23, 0x31, 0xc5, 0x99],
            [0x99, 0xc5, 0x31, 0x23, 0x59, 0x59],
            [0, 0x16, 1, 0, 0, 0],
            0x1_00_16_01_00_00_00,
        ),
    ] {
        for release in [0, 1, 3] {
            let mut m = Machine::new(Profile::Pc9821ApDos);
            load_calendar(
                &mut m.bus,
                extended,
                &payload[..if extended { 6 } else { 5 }],
            );
            assert_eq!(m.bus.rtc.snapshot(), before);
            m.bus.rtc.tick(u32::MAX);
            assert_eq!(m.bus.rtc.snapshot(), before);
            if extended {
                serial_command(&mut m.bus, release);
            } else {
                command(&mut m.bus, release);
            }
            m.bus.tick(8_000_000);
            assert_eq!(m.bus.rtc.snapshot(), after);
            assert_eq!(capture(&mut m.bus, extended), stream);
        }
    }
}

// Independent divider oracle: free-running 64 Hz edge numbers continue through
// hold/reset; a release starts counting 64 subsequent edges toward one second.
// No production fraction/decoder or calendar arithmetic is used by this model.
#[test]
fn time_set_divider_keeps_lower_stages_across_entry_hold_release_and_reset() {
    for phase in [0, 1, 124_999, 125_000, 3_999_999, 7_999_999] {
        for duration in [0, 1, 124_999, 125_001, 8_000_001, u32::MAX] {
            for extended in [false, true] {
                for release in [0, 1, 3] {
                    let mut m = Machine::new(Profile::Pc9821ApDos);
                    m.bus.rtc.tick(phase);
                    load_calendar(
                        &mut m.bus,
                        extended,
                        &[0x21, 0x34, 0x12, 0x25, 0xb6, 0x86][..if extended { 6 } else { 5 }],
                    );
                    m.bus.rtc.tick(duration);
                    m.reset(); // Retain hold, calendar, lower divider, data and pins.
                    m.bus.rtc.tick(8_000_000);
                    assert_eq!(m.bus.rtc.snapshot()[5], 0x21);
                    assert_eq!(m.bus.input(0x33), 0x39);
                    if extended {
                        serial_command(&mut m.bus, 2);
                    } else {
                        command(&mut m.bus, 2);
                    }
                    assert_eq!(m.bus.rtc.snapshot()[5], 0x21);
                    assert_eq!(m.bus.input(0x33), 0x39);
                    if extended {
                        serial_command(&mut m.bus, release);
                    } else {
                        command(&mut m.bus, release);
                    }
                    let total = u64::from(phase) + u64::from(duration) + 8_000_000;
                    let release_edge = total / 125_000;
                    let next_second_clock = (release_edge + 64) * 125_000;
                    let remaining = (next_second_clock - total) as u32;
                    // Check the half-second DATA OUT edge as well as carry.
                    m.bus.rtc.tick(remaining - 4_000_001);
                    if release != 1 {
                        assert_eq!(m.bus.input(0x33), 0x38);
                    }
                    m.bus.rtc.tick(1);
                    if release != 1 {
                        assert_eq!(m.bus.input(0x33), 0x39);
                    }
                    m.bus.rtc.tick(3_999_999);
                    assert_eq!(
                        m.bus.rtc.snapshot()[5],
                        0x21,
                        "{phase} {duration} {extended} {release}"
                    );
                    m.bus.rtc.tick(1);
                    assert_eq!(
                        m.bus.rtc.snapshot()[5],
                        0x22,
                        "{phase} {duration} {extended} {release}"
                    );
                    if release != 1 {
                        assert_eq!(m.bus.input(0x33), 0x38);
                    }
                }
            }
        }
    }
}

fn tick_partition(b: &mut Bus, mut total: u64, chunks: &[u32]) {
    let mut index = 0;
    while total != 0 {
        let n = total.min(u64::from(chunks[index % chunks.len()])) as u32;
        b.rtc.tick(n);
        total -= u64::from(n);
        index += 1;
    }
}

#[test]
fn time_set_batch_sizes_and_unchanged_strobe_preserve_release_phase() {
    for (held, partitions) in [
        (
            8_123_457,
            vec![vec![8], vec![1, 123_457, 999_983], vec![u32::MAX]],
        ),
        (
            u64::from(u32::MAX) * 2 + 124_999,
            vec![vec![100_003], vec![17, 1_234_567, 333_333], vec![u32::MAX]],
        ),
    ] {
        for chunks in partitions {
            let mut m = Machine::new(Profile::Pc9821ApDos);
            tick_partition(&mut m.bus, 7_999_999, &chunks);
            load_calendar(&mut m.bus, true, &[0x20, 0, 0, 1, 0x14, 0x26]);
            tick_partition(&mut m.bus, held, &chunks);
            m.reset();
            // Release into Register Hold and leave STB high. An unchanged high
            // must not retrigger the command or alter the running phase.
            serial_command(&mut m.bus, 0);
            m.bus.output(0x20, 15);
            let total = 7_999_999 + held;
            let remaining = ((total / 125_000 + 64) * 125_000 - total) as u32;
            tick_partition(&mut m.bus, u64::from(remaining - 1), &chunks);
            m.bus.output(0x20, 15);
            assert_eq!(m.bus.rtc.snapshot(), [0x26, 0x14, 1, 0, 0, 0x20]);
            m.bus.rtc.tick(1);
            assert_eq!(m.bus.rtc.snapshot(), [0x26, 0x14, 1, 0, 0, 0x21]);
        }
    }
}

#[test]
fn malformed_time_set_keeps_calendar_and_still_holds_safely() {
    let valid = [0x59, 0x58, 0x23, 0x25, 0xb6, 0x86];
    for extended in [false, true] {
        for (field, invalid) in [
            (0, 0x6a),
            (0, 0x60),
            (1, 0x60),
            (2, 0x24),
            (3, 0),
            (3, 0x31),
            (4, 0),
            (4, 0xd0),
            (4, 0xb7),
            (5, 0xfa),
        ] {
            if !extended && field == 5 {
                continue;
            }
            let mut payload = valid;
            payload[field] = invalid;
            // November 31 is invalid even though its BCD representation is valid.
            let mut m = Machine::new(Profile::Pc9821ApDos);
            load_calendar(
                &mut m.bus,
                extended,
                &payload[..if extended { 6 } else { 5 }],
            );
            m.bus.rtc.tick(u32::MAX);
            assert_eq!(m.bus.rtc.snapshot(), [0x26, 0x14, 1, 0, 0, 0]);
            if extended {
                serial_command(&mut m.bus, 0);
            } else {
                command(&mut m.bus, 0);
            }
            m.bus.rtc.tick(8_000_000);
            assert_eq!(m.bus.rtc.snapshot(), [0x26, 0x14, 1, 0, 0, 1]);
        }
    }
    // Explicit February 29 is valid in parallel mode, even in stored year 26.
    let mut m = Machine::new(Profile::Pc9821ApDos);
    load_calendar(&mut m.bus, false, &[0x59, 0x59, 0x23, 0x29, 0x22]);
    assert_eq!(m.bus.rtc.snapshot(), [0x26, 0x22, 0x29, 0x23, 0x59, 0x59]);
}

#[test]
fn original_rtc_set_boot_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/rtc_set/rtc_set.hdm"
    ))
    .expect("run python3 tools/prepare_rtc_set_fixture.py");
    let manifest = include_str!("../fixtures/rtc-set-manifest.json");
    let hex = manifest
        .split_once("\"result_hex\": \"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap();
    let expected: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "rtc_set.hdm").unwrap();
    for run in 0..2 {
        if run != 0 {
            m.reset();
        }
        let mut slots = 0;
        while slots < 8_000_000 {
            slots += m.step(1000);
            assert!(m.error.is_none(), "{:?}", m.error);
            let text = m.bus.text();
            assert!(
                !text.contains("FAIL"),
                "boot {run}, {slots} slots: {text}; {:02x?}",
                &m.bus.mem[0x8000..0x8080]
            );
            if text.contains("RTC SET PASS") {
                break;
            }
        }
        assert!(slots <= 8_000_000);
        assert!(
            m.bus.text().contains("RTC SET PASS"),
            "{}; {:02x?}",
            m.bus.text(),
            &m.bus.mem[0x8000..0x8080]
        );
        assert_eq!(&m.bus.mem[0x8000..0x8080], expected.as_slice());
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        println!("Ap RTC Time Set boot {run}: {slots} slots");
    }
}

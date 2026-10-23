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
        let mut m = seed(86, 11, 25, 6, 2, 23, 49);
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

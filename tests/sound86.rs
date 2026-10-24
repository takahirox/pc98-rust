use pc98_rust::{bus::Bus, pcm::Pcm86, sound::Opn, Machine, Profile};

fn opn_register(opn: &mut Opn, address: u8, value: u8) {
    opn.address = address;
    opn.write(value);
}

#[test]
fn opna_irq_disabled_timer_regression() {
    let mut opn = Opn::default();
    for (address, value) in [(0x29, 0x80), (0x24, 0xff), (0x25, 3), (0x27, 5)] {
        opn_register(&mut opn, address, value);
    }
    opn.tick(145);
    assert_eq!(opn.status() & 3, 1);
    assert!(
        !opn.irq(),
        "register 29h must suppress the pending Timer A IRQ"
    );
}

fn register(b: &mut Bus, bank: bool, a: u8, value: u8) {
    b.output(if bank { 0x18c } else { 0x188 }, a);
    b.output(if bank { 0x18e } else { 0x18a }, value);
}

#[test]
fn opna_extended_bank_mask_sixth_channel_and_timer_irq() {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    assert_eq!(b.input(0xa460), 0x40);
    register(&mut b, true, 0x30, 3);
    assert_eq!(b.opn.registers[0x130], 0);
    b.output(0xa460, 1);
    assert_eq!(b.input(0xa460), 0x41);
    register(&mut b, false, 0x29, 0x83);
    for slot in [0, 4, 8, 12] {
        for (reg, value) in [(0x32, 1), (0x42, 24), (0x52, 31), (0x82, 15)] {
            register(&mut b, true, reg + slot, value);
        }
    }
    for (reg, value) in [(0xb2, 7), (0xa6, 0x22), (0xa2, 0x69)] {
        register(&mut b, true, reg, value);
    }
    assert_eq!(b.opn.registers[0xa2], 0);
    register(&mut b, false, 0x28, 0xf6); // FM6, bank 1 channel 2
    b.tick(100_000);
    assert!(b.audio.iter().any(|v| v.abs() > 0.01));
    assert_eq!(b.opn.key_edges, 4);
    b.output(0xa460, 3);
    let writes = b.opn.writes;
    register(&mut b, false, 0x28, 0);
    assert_eq!(writes, b.opn.writes);
    b.output(0xa460, 1);
    for (reg, value) in [(0x24, 255), (0x25, 3), (0x27, 5)] {
        register(&mut b, false, reg, value);
    }
    b.tick(145);
    assert_ne!(b.pics[1].irr & 16, 0);
}

#[test]
fn pcm_fifo_signed_samples_endianness_stereo_irq_reset_and_bounds() {
    let mut p = Pcm86::default();
    assert_eq!(p.input(0xa466) & 0xc0, 0x40);
    p.output(0xa46a, 0xd2); // 8-bit right channel
    p.output(0xa468, 0x28);
    p.output(0xa468, 0x20);
    p.output(0xa46a, 1); // interrupt at 128 bytes
    for _ in 0..256 {
        p.output(0xa46c, 0x40);
    }
    p.output(0xa468, 0xa0);
    let mut peak: f32 = 0.0;
    for _ in 0..150 {
        peak = peak.max(p.sample());
    }
    assert_eq!(peak, 0.15);
    assert!(p.irq);
    assert_ne!(p.input(0xa468) & 16, 0);
    p.output(0xa468, 0x80);
    assert!(!p.irq);
    p.output(0xa468, 8);
    assert!(p.fifo.is_empty());
    assert_eq!(p.sample(), 0.0);
    p.output(0xa468, 0);
    p.output(0xa46a, 0xb2); // signed 16-bit stereo, high byte first
    for v in [0x40, 0x00, 0xc0, 0x00] {
        p.output(0xa46c, v);
    }
    p.output(0xa468, 0x80);
    for _ in 0..2 {
        assert_eq!(p.sample(), 0.0);
    }
    p.output(0xa468, 8);
    p.output(0xa468, 0);
    p.output(0xa46a, 0x92);
    for v in [0x80, 0x00] {
        p.output(0xa46c, v);
    }
    p.output(0xa468, 0x80);
    p.sample();
    assert_eq!(p.sample(), -0.3);
    p.output(0xa468, 8);
    p.output(0xa468, 0);
    for _ in 0..40000 {
        p.output(0xa46c, 0);
    }
    assert_eq!(p.fifo.len(), 32768);
    assert_ne!(p.input(0xa466) & 128, 0);
}

// Generate real overflows; no host writes to status, timer counters or PIC state.
fn program_timers(opn: &mut Opn, flags: u8) {
    for (a, v) in [
        (0x24, 0xff),
        (0x25, 3),
        (0x26, 0xff),
        (0x27, flags | (flags << 2)),
    ] {
        opn_register(opn, a, v);
    }
}

#[test]
fn opna_irq_flags_by_enables_matrix_and_sch_independence() {
    for flags in 0..4 {
        let mut opn = Opn::default();
        assert_eq!(opn.registers[0x29], 0x1f);
        program_timers(&mut opn, flags);
        opn.tick(2308);
        opn_register(&mut opn, 0x27, 0); // stop, retain flags
        for sch in [0, 0x80] {
            for enables in 0..4 {
                // D4:D2 and reserved bits must never create additional flags/IRQs.
                opn_register(&mut opn, 0x29, sch | 0x7c | enables);
                for _ in 0..3 {
                    assert_eq!(opn.status() & 0x1f, flags);
                    assert_eq!(
                        opn.irq(),
                        flags & enables != 0,
                        "flags={flags} enables={enables} SCH={sch}"
                    );
                }
            }
        }
    }
    let mut opn = Opn::default();
    opn_register(&mut opn, 0x28, 0xf6);
    assert_eq!(opn.key_edges, 0); // SCH reset blocks FM6
    opn_register(&mut opn, 0x29, 0x80); // FM6 enabled, timers suppressed
    opn_register(&mut opn, 0x28, 0xf6);
    assert_eq!(opn.key_edges, 4);
    program_timers(&mut opn, 3);
    opn.tick(2308);
    assert!(!opn.irq());
    opn_register(&mut opn, 0x29, 3); // timer enables independent of SCH
    assert!(opn.irq());
    assert_eq!(opn.key_edges, 4);
}

#[test]
fn opna_irq_pending_transitions_independent_clearing_and_nondestructive_reads() {
    let mut opn = Opn::default();
    opn_register(&mut opn, 0x29, 0);
    program_timers(&mut opn, 3);
    opn.tick(2308);
    opn_register(&mut opn, 0x27, 0);
    for enable in [1, 0, 2, 0, 3, 0, 3] {
        opn_register(&mut opn, 0x29, enable);
        assert_eq!(opn.irq(), enable != 0); // no tick needed to expose pending flags
        assert_eq!(opn.status() & 3, 3);
    }
    for (clear, remaining) in [(0x10, 2), (0x20, 0)] {
        opn_register(&mut opn, 0x27, clear);
        for _ in 0..4 {
            assert_eq!(opn.status() & 3, remaining);
            assert_eq!(opn.irq(), remaining != 0);
        }
    }
    // Also clear B first and show that 29h cannot override 27h flag generation.
    program_timers(&mut opn, 3);
    opn.tick(2308);
    opn_register(&mut opn, 0x27, 0x20);
    assert_eq!(opn.status() & 3, 1);
    assert!(opn.irq());
    opn_register(&mut opn, 0x27, 0x33); // load both, clear both, flag generation off
    opn.tick(4616);
    assert_eq!(opn.status() & 3, 0);
    assert!(!opn.irq());
}

#[test]
fn opna_irq_mask_changes_preserve_accumulated_clocks_and_fraction() {
    for (flag, period) in [(1, 145), (2, 2308)] {
        let mut opn = Opn::default();
        program_timers(&mut opn, flag);
        opn.tick(period / 2);
        opn_register(&mut opn, 0x29, 0x80);
        opn.tick(period - period / 2 - 1);
        assert_eq!(opn.status() & 3, 0);
        opn_register(&mut opn, 0x29, 0);
        opn.tick(1);
        assert_eq!(opn.status() & 3, flag);
        assert!(!opn.irq());
        opn_register(&mut opn, 0x29, flag);
        assert!(opn.irq());
        opn_register(&mut opn, 0x27, flag << 4); // stop and clear
        opn.tick(period * 2);
        assert_eq!(opn.status() & 3, 0);
        program_timers(&mut opn, flag); // load edge restarts the counter
        opn.tick(period / 2);
        assert_eq!(opn.status() & 3, 0);
        opn.tick(period);
        assert_eq!(opn.status() & 3, flag);
    }
}

// Program slave IR4 at vector 1Ch explicitly, with only the cascade unmasked.
fn sound_pic(b: &mut Bus) {
    for (command, data, base, cascade, mask) in [(0, 2, 8, 0x80, 0x7f), (8, 10, 0x18, 7, 0xef)] {
        for (port, value) in [
            (command, 0x11),
            (data, base),
            (data, cascade),
            (data, 1),
            (data, mask),
        ] {
            b.output(port, value);
        }
    }
}

fn sound_eoi(b: &mut Bus) {
    b.output(8, 0x20);
    b.output(0, 0x20);
}

#[test]
fn opna_irq_ports_pic_retention_acknowledgement_eoi_and_recovery() {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    sound_pic(&mut b);
    for (a, v) in [
        (0x29, 0x80),
        (0x24, 0xff),
        (0x25, 3),
        (0x26, 0xff),
        (0x27, 15),
    ] {
        register(&mut b, false, a, v);
    }
    b.tick(2308);
    register(&mut b, false, 0x27, 0);
    for _ in 0..3 {
        assert_eq!(b.input(0x188) & 3, 3);
        assert_eq!(b.pics[1].irr & 16, 0);
        assert_eq!(b.irq(), None);
    }
    // Each timer independently routes by the next ordinary scheduler tick.
    for enable in [1, 2] {
        register(&mut b, false, 0x29, enable);
        b.tick(8);
        assert_eq!(b.pics[1].irr & 16, 16);
        register(&mut b, false, 0x29, 0);
        b.tick(8);
        assert!(!b.opn.irq());
        assert_eq!(b.pics[1].irr & 16, 16); // suppression retains latched request
        assert_eq!(b.irq(), Some(0x1c));
        assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 16));
        register(&mut b, false, 0x29, 0x80);
        b.tick(8);
        assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 16));
        assert_eq!(b.irq(), None);
        sound_eoi(&mut b);
        b.tick(8);
        assert_eq!(b.irq(), None);
        assert_eq!((b.pics[0].isr, b.pics[1].isr), (0, 0));
        assert_eq!(b.input(0x188) & 3, 3);
    }
    // Suppress an enabled source after CPU acknowledgement, while in service.
    register(&mut b, false, 0x29, 1);
    b.tick(8);
    assert_eq!(b.irq(), Some(0x1c));
    assert!(b.opn.irq());
    register(&mut b, false, 0x29, 0);
    b.tick(8);
    assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 16));
    assert_eq!(b.irq(), None);
    sound_eoi(&mut b);
    b.tick(8);
    assert_eq!(b.irq(), None);
    register(&mut b, false, 0x27, 0x30);
    register(&mut b, false, 0x29, 3);
    b.tick(8);
    assert_eq!(b.input(0x188) & 3, 0);
    assert_eq!(b.irq(), None);
}

#[test]
fn opna_irq_board_gating_upper_bank_and_pcm_independence() {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    sound_pic(&mut b);
    b.output(0xa460, 1);
    register(&mut b, false, 0x29, 0);
    for (a, v) in [(0x29, 0xff), (0x24, 0xff), (0x25, 3), (0x27, 15)] {
        register(&mut b, true, a, v);
    }
    b.tick(2308);
    assert_eq!(b.opn.registers[0x29], 0);
    assert_eq!(b.input(0x188) & 3, 0);
    assert_eq!(b.irq(), None);
    for (a, v) in [(0x24, 0xff), (0x25, 3), (0x27, 5)] {
        register(&mut b, false, a, v);
    }
    b.tick(145);
    assert_eq!(b.input(0x18c) & 3, 1);
    assert_eq!(b.irq(), None);
    b.output(0xa460, 3); // board I/O mask blocks both banks, including enables/clears
    let writes = b.opn.writes;
    register(&mut b, false, 0x29, 3);
    register(&mut b, false, 0x27, 0x30);
    register(&mut b, true, 0x29, 0);
    assert_eq!(b.opn.writes, writes);
    assert_eq!(b.input(0x188), 0xff);
    b.output(0xa460, 0);
    assert_eq!(b.input(0x188) & 3, 1);
    assert_eq!(b.opn.registers[0x129], 0xff);
    register(&mut b, true, 0x29, 0); // upper bank hidden with mask=0
    assert_eq!(b.opn.registers[0x129], 0xff);
    // Real PCM FIFO threshold crossing while the OPNA flag is suppressed.
    b.output(0xa46a, 0xd2);
    b.output(0xa468, 0x20);
    b.output(0xa46a, 1);
    for _ in 0..256 {
        b.output(0xa46c, 0x40);
    }
    b.output(0xa468, 0xa0);
    b.tick(100_000);
    // The threshold crossing must be routed in the same tick as audio sampling.
    assert!(b.pcm.irq);
    assert_eq!(b.irq(), Some(0x1c));
    b.output(0xa468, 0x80); // clear only PCM
    sound_eoi(&mut b);
    b.tick(8);
    assert_eq!(b.irq(), None);
    assert_eq!(b.input(0x188) & 3, 1);
    register(&mut b, false, 0x29, 1);
    b.tick(8);
    assert_eq!(b.irq(), Some(0x1c)); // OPNA recovers independently

    // VM still has no exposed sound board.
    let mut vm = Bus::new(Profile::Pc9801Vm);
    register(&mut vm, false, 0x29, 0x83);
    assert_eq!(vm.opn.writes, 0);
    assert_eq!(vm.input(0x188), 0xff);
}

#[test]
fn opna_irq_instances_and_machine_reset() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    let other = Machine::new(Profile::Pc9821ApDos);
    assert_eq!(m.bus.opn.registers[0x29], 0x1f);
    program_timers(&mut m.bus.opn, 3);
    m.bus.tick(2308);
    assert!(m.bus.opn.irq());
    register(&mut m.bus, false, 0x29, 0xe0);
    assert_eq!(m.bus.opn.status() & 3, 3);
    assert!(!m.bus.opn.irq());
    assert_eq!(other.bus.opn.registers[0x29], 0x1f);
    assert_eq!(other.bus.opn.status() & 3, 0);
    assert!(!other.bus.opn.irq());
    m.reset();
    m.bus.output(0x188, 0x29);
    assert_eq!(m.bus.input(0x18a), 0x1f);
    assert_eq!(m.bus.input(0x188), 0);
    assert!(!m.bus.opn.irq());
    assert_eq!((m.bus.pics[1].irr, m.bus.pics[1].isr), (0, 0));
    // Default enables actually deliver a new overflow after reset.
    program_timers(&mut m.bus.opn, 1);
    m.bus.tick(145);
    assert!(m.bus.opn.irq());
    assert_eq!(m.bus.pics[1].irr & 16, 16);
}

#[test]
fn opna_irq_original_guest_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/opna_irq/opna_irq.hdm"
    ))
    .expect("run python3 tools/prepare_opna_irq_fixture.py");
    let mut expected = [0xa5; 128];
    expected[1] = 4;
    for (timer, flag) in [1, 2].into_iter().enumerate() {
        expected[16 + timer * 16..32 + timer * 16].copy_from_slice(&[
            flag,
            flag,
            0,
            16,
            16,
            flag,
            16,
            128,
            2 * timer as u8 + 1,
            0,
            0,
            flag,
            16,
            128,
            2 * timer as u8 + 2,
            0,
        ]);
    }
    expected[64..72].copy_from_slice(&[0; 8]);
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "opna_irq.hdm").unwrap();
    for run in 0..2 {
        if run != 0 {
            m.reset();
        }
        let mut slots = 0;
        while slots < 500_000 {
            slots += m.step(1000);
            m.bus.audio.clear();
            assert!(m.error.is_none(), "{:?}", m.error);
            let text = m.bus.text();
            assert!(
                !text.contains("FAIL"),
                "boot {run}: {slots} slots; {text}; results={:02x?}; CS:IP={:x}:{:x}",
                &m.bus.mem[0x8000..0x8080],
                m.cpu.s[1],
                m.cpu.ip
            );
            if text.contains("OPNA IRQ PASS") {
                break;
            }
        }
        assert!(slots <= 500_000);
        assert!(
            m.bus.text().contains("OPNA IRQ PASS"),
            "{}; results={:02x?}",
            m.bus.text(),
            &m.bus.mem[0x8000..0x8080]
        );
        assert_eq!(&m.bus.mem[0x8000..0x8080], &expected);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
        assert!(!m.bus.opn.irq());
        assert_eq!(m.bus.opn.status() & 3, 0);
        assert_eq!((m.bus.pics[0].isr, m.bus.pics[1].isr), (0, 0));
        println!("Ap OPNA IRQ boot {run}: {slots} slots, 4 handler entries");
    }
}

#[derive(Clone, Copy, Debug)]
enum SoundInput {
    TimerA,
    TimerB,
    Pcm,
}
impl SoundInput {
    fn period(self) -> u32 {
        match self {
            Self::TimerA => 145,
            Self::TimerB => 2308,
            // Two 48 kHz samples: the first accumulates phase, the second
            // consumes byte 129 and crosses the real 128-byte FIFO threshold.
            Self::Pcm => 334,
        }
    }
    fn program(self, b: &mut Bus) {
        match self {
            Self::TimerA | Self::TimerB => {
                let flag = if matches!(self, Self::TimerA) { 1 } else { 2 };
                for (a, v) in [
                    (0x29, flag),
                    (0x24, 0xff),
                    (0x25, 3),
                    (0x26, 0xff),
                    (0x27, flag | (flag << 2)),
                ] {
                    register(b, false, a, v);
                }
            }
            Self::Pcm => {
                b.output(0xa468, 8); // reset FIFO and resampling phase
                b.output(0xa468, 0);
                b.output(0xa46a, 0xd0); // signed 8-bit mono, divisor 12
                b.output(0xa468, 0x20);
                b.output(0xa46a, 1);
                for _ in 0..129 {
                    b.output(0xa46c, 0x40);
                }
                b.output(0xa468, 0xa0);
            }
        }
    }
    fn hold(self, b: &mut Bus) {
        match self {
            Self::Pcm => b.output(0xa468, 0x30), // stop playback, retain enabled IRQ
            _ => register(b, false, 0x27, 0),    // stop timer, retain enabled flag
        }
    }
    fn raise(self, b: &mut Bus) {
        self.program(b);
        b.tick(self.period());
        self.hold(b);
        assert!(match self {
            Self::Pcm => b.pcm.irq,
            _ => b.opn.irq(),
        });
    }
    fn clear(self, b: &mut Bus) {
        match self {
            Self::Pcm => b.output(0xa468, 0),
            _ => register(b, false, 0x27, 0x30),
        }
    }
}

fn sound_poll(b: &mut Bus, port: u16) -> u8 {
    b.output(port, 0x0c);
    b.tick(8); // an asserted input must not retrigger the frozen poll
    b.input(port)
}
fn sound_empty(b: &mut Bus) {
    assert_eq!(b.pics[1].irr & 16, 0);
    assert_eq!(sound_poll(b, 0), 0);
    assert_eq!(sound_poll(b, 8), 0);
    assert_eq!(b.irq(), None);
}

#[test]
fn shared_sound_held_input_regression() {
    for source in [SoundInput::TimerA, SoundInput::TimerB, SoundInput::Pcm] {
        for poll in [false, true] {
            let mut b = Bus::new(Profile::Pc9821ApDos);
            sound_pic(&mut b);
            source.raise(&mut b);
            assert_eq!(b.pics[1].irr & 16, 16, "{source:?}");
            if poll {
                assert_eq!(sound_poll(&mut b, 0), 0x87);
                assert_eq!(sound_poll(&mut b, 8), 0x84);
            } else {
                assert_eq!(b.irq(), Some(0x1c));
            }
            assert_eq!((b.pics[0].isr, b.pics[1].isr), (128, 16));
            b.tick(10_000);
            sound_empty(&mut b);
            sound_eoi(&mut b);
            b.tick(10_000);
            sound_empty(&mut b);
            source.clear(&mut b);
            source.raise(&mut b);
            assert_eq!(b.irq(), Some(0x1c), "fresh edge: {source:?}");
        }
    }
}

#[test]
fn shared_sound_masked_capture_suppression_and_edge_during_service() {
    for source in [SoundInput::TimerA, SoundInput::TimerB, SoundInput::Pcm] {
        for slave_first in [false, true] {
            let mut b = Bus::new(Profile::Pc9821ApDos);
            sound_pic(&mut b);
            b.output(10, 0xff);
            source.raise(&mut b);
            assert_eq!(b.pics[1].irr & 16, 16);
            assert_eq!(b.irq(), None);
            source.clear(&mut b);
            assert_eq!((b.pics[1].irr & 16, b.pics[1].isr), (16, 0));
            b.output(10, 0xef);
            assert_eq!(b.irq(), Some(0x1c));
            source.raise(&mut b); // genuine edge while IR4 is in service
            source.clear(&mut b); // suppression cannot erase either register
            assert_eq!((b.pics[1].irr & 16, b.pics[1].isr), (16, 16));
            assert_eq!(b.pics[0].isr, 128);
            assert_eq!(b.irq(), None);
            b.output(if slave_first { 8 } else { 0 }, 0x20);
            assert_eq!(b.irq(), None, "both cascade EOIs are necessary");
            b.output(if slave_first { 0 } else { 8 }, 0x20);
            assert_eq!(b.irq(), Some(0x1c));
            assert_eq!((b.pics[1].irr & 16, b.pics[1].isr), (0, 16));
            sound_eoi(&mut b);
            sound_empty(&mut b);
        }
    }
}

#[test]
fn shared_sound_consecutive_pending_flag_and_pcm_control_writes() {
    for source in [SoundInput::TimerA, SoundInput::TimerB] {
        let mut b = Bus::new(Profile::Pc9821ApDos);
        sound_pic(&mut b);
        source.raise(&mut b);
        assert_eq!(b.irq(), Some(0x1c));
        let flag = b.input(0x188) & 3;
        register(&mut b, false, 0x29, 0);
        assert_eq!((b.pics[1].irr & 16, b.pics[1].isr), (0, 16));
        register(&mut b, false, 0x29, flag); // no tick between disable/enable
        assert_eq!(b.input(0x188) & 3, flag);
        assert_eq!((b.pics[1].irr & 16, b.pics[1].isr), (16, 16));
        register(&mut b, false, 0x29, 0);
        assert_eq!((b.pics[1].irr & 16, b.pics[1].isr), (16, 16));
        sound_eoi(&mut b);
        assert_eq!(b.irq(), Some(0x1c));
    }
    let mut b = Bus::new(Profile::Pc9821ApDos);
    sound_pic(&mut b);
    SoundInput::Pcm.raise(&mut b);
    assert_eq!(b.irq(), Some(0x1c));
    b.output(0xa468, 0x10); // disable output but retain pending PCM IRQ
    b.output(0xa468, 0x30); // enable without an intervening tick
    assert!(b.pcm.irq);
    assert_eq!((b.pics[1].irr & 16, b.pics[1].isr), (16, 16));
    sound_eoi(&mut b);
    assert_eq!(b.irq(), Some(0x1c));
    b.output(0xa468, 0x20); // acknowledge, preserving FIFO and IRQ enable
    assert!(!b.pcm.irq);
    assert_eq!(b.pcm.fifo.len(), 128);
    b.output(0xa46c, 0x40); // refill above threshold
    b.output(0xa468, 0xa0);
    b.tick(334);
    assert!(b.pcm.irq);
    assert_eq!((b.pics[1].irr & 16, b.pics[1].isr), (16, 16));
    sound_eoi(&mut b);
    assert_eq!(b.irq(), Some(0x1c));
}

#[test]
fn shared_sound_overlapping_contributors_do_not_create_edges() {
    for timer in [SoundInput::TimerA, SoundInput::TimerB] {
        for pcm_first in [false, true] {
            let (first, second) = if pcm_first {
                (SoundInput::Pcm, timer)
            } else {
                (timer, SoundInput::Pcm)
            };
            let mut b = Bus::new(Profile::Pc9821ApDos);
            sound_pic(&mut b);
            first.raise(&mut b);
            assert_eq!(b.irq(), Some(0x1c));
            second.raise(&mut b);
            // Neither contributor's consecutive output-enable writes create
            // an edge while the other holds the combined input asserted.
            let flag = b.input(0x188) & 3;
            register(&mut b, false, 0x29, 0);
            register(&mut b, false, 0x29, flag);
            b.output(0xa468, 0x10);
            b.output(0xa468, 0x30);
            b.output(0xa460, 3); // board I/O gating does not gate the IRQ wire
            b.output(0xa460, 0);
            first.clear(&mut b);
            b.tick(10_000);
            sound_empty(&mut b);
            sound_eoi(&mut b);
            sound_empty(&mut b); // second contributor still holds the shared wire
            second.clear(&mut b);
            first.raise(&mut b);
            assert_eq!(b.irq(), Some(0x1c));
        }
    }
}

#[test]
fn shared_sound_clock_partitioning_observes_audio_threshold_same_tick() {
    for source in [SoundInput::TimerA, SoundInput::TimerB, SoundInput::Pcm] {
        let period = source.period();
        for partition in [vec![period], vec![period - 1, 1], vec![1; period as usize]] {
            let mut b = Bus::new(Profile::Pc9821ApDos);
            sound_pic(&mut b);
            source.program(&mut b);
            let mut elapsed = 0;
            for clocks in partition {
                b.tick(clocks);
                elapsed += clocks;
                assert_eq!(b.pics[1].irr & 16, if elapsed == period { 16 } else { 0 });
            }
            if matches!(source, SoundInput::Pcm) {
                assert_eq!(b.pcm.fifo.len(), 128);
                assert_eq!(b.pcm.samples, 1);
            }
            source.hold(&mut b);
            assert_eq!(b.irq(), Some(0x1c));
            b.tick(period * 2);
            sound_eoi(&mut b);
            sound_empty(&mut b);
        }
    }
}

#[test]
fn shared_sound_pic_reinitialization_instances_and_machine_reset() {
    for source in [SoundInput::TimerA, SoundInput::TimerB, SoundInput::Pcm] {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        let mut other = Machine::new(Profile::Pc9821ApDos);
        sound_pic(&mut m.bus);
        sound_pic(&mut other.bus);
        source.raise(&mut m.bus);
        sound_empty(&mut other.bus);
        sound_pic(&mut m.bus); // clears IRR; a held input must not re-latch
        m.bus.tick(10_000);
        sound_empty(&mut m.bus);
        source.clear(&mut m.bus);
        source.raise(&mut m.bus);
        assert_eq!(m.bus.irq(), Some(0x1c));
        sound_pic(&mut m.bus); // also clear ISR without fabricating an edge
        m.bus.tick(10_000);
        sound_empty(&mut m.bus);
        m.reset();
        sound_pic(&mut m.bus);
        sound_empty(&mut m.bus);
        source.raise(&mut m.bus);
        assert_eq!(m.bus.irq(), Some(0x1c));
        source.raise(&mut other.bus); // independent detector, including PCM
        assert_eq!(other.bus.irq(), Some(0x1c));
    }
}

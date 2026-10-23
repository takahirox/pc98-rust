use pc98_rust::{bus::Bus, cpu::IF, devices::Pit, Machine, Profile};

const PROFILES: [Profile; 2] = [Profile::Pc9801Vm, Profile::Pc9821ApDos];

// Exact scheduler conversion, independent of the counter implementation:
// 1,996,800 / 8,000,000 = 156 / 625. Stop at each requested PIT clock.
#[derive(Default)]
struct Clock {
    pit: Pit,
    clocks: u64,
    cycles: u64,
}
impl Clock {
    fn advance(&mut self, clocks: u32) -> bool {
        self.clocks += clocks as u64;
        let cycles = (self.clocks * 625).div_ceil(156);
        let edge = self.pit.tick((cycles - self.cycles) as u32);
        self.cycles = cycles;
        edge
    }
}
fn program(p: &mut Pit, channel: usize, access: u8, mode: u8, value: u16) {
    p.write(3, ((channel as u8) << 6) | (access << 4) | (mode << 1));
    write_count(p, channel, access, value);
}
fn write_count(p: &mut Pit, channel: usize, access: u8, value: u16) {
    if access != 2 {
        p.write(channel, value as u8);
    }
    if access != 1 {
        p.write(channel, (value >> 8) as u8);
    }
}
fn state(c: &Clock, count: u32, output: bool) {
    assert_eq!((c.pit.c[0].remaining, c.pit.c[0].output), (count, output));
}

#[test]
fn completed_rewrite_preserves_active_count_reproduction() {
    let mut b = Bus::default();
    b.output(0x77, 0x14);
    b.output(0x71, 100);
    b.tick(40);
    let active = b.pit.c[0].remaining;
    b.output(0x71, 200);
    assert_eq!(b.pit.c[0].remaining, active);
}

#[test]
fn specification_small_divisor_clock_count_out_traces_and_alias() {
    // Each tuple is (count, OUT, rising edge), starting with the load clock.
    // Intel mode 2: N-1 high clocks, one low clock, then reload/rise.
    type Sample = (u32, bool, bool);
    let traces: &[(u16, &[Sample])] = &[
        (
            2,
            &[
                (2, true, false),
                (1, false, false),
                (2, true, true),
                (1, false, false),
                (2, true, true),
            ],
        ),
        (
            3,
            &[
                (3, true, false),
                (2, true, false),
                (1, false, false),
                (3, true, true),
                (2, true, false),
                (1, false, false),
                (3, true, true),
            ],
        ),
        (
            4,
            &[
                (4, true, false),
                (3, true, false),
                (2, true, false),
                (1, false, false),
                (4, true, true),
                (3, true, false),
                (2, true, false),
                (1, false, false),
                (4, true, true),
            ],
        ),
    ];
    for mode in [2, 6] {
        for &(divisor, trace) in traces {
            let mut c = Clock::default();
            program(&mut c.pit, 0, 3, mode, divisor);
            assert!(c.pit.c[0].output);
            let before = c.pit.clone();
            assert!(!c.advance(0));
            assert_eq!(c.pit, before);
            for &(count, high, edge) in trace {
                assert_eq!(c.advance(1), edge, "mode {mode}, divisor {divisor}");
                state(&c, count, high);
            }
        }
    }
}

#[test]
fn large_divisors_and_all_byte_loading_forms() {
    for mode in [2, 6] {
        // MSB-only 0200h, LSB-only 04h, pair FFFFh and binary zero in all forms.
        for (access, encoded, n) in [
            (1, 4, 4),
            (2, 512, 512),
            (3, 65535, 65535),
            (1, 0, 65536),
            (2, 0, 65536),
            (3, 0, 65536),
        ] {
            let mut c = Clock::default();
            program(&mut c.pit, 0, access, mode, encoded);
            assert!(!c.advance(1));
            state(&c, n, true);
            assert!(!c.advance(n - 2));
            state(&c, 2, true);
            assert!(!c.advance(1));
            state(&c, 1, false);
            assert!(c.advance(1));
            state(&c, n, true);
            assert!(c.advance(n));
            state(&c, n, true);
        }
    }
}

#[test]
fn partial_initial_write_waits_for_complete_count_and_load_clock() {
    let mut c = Clock::default();
    c.pit.write(3, 0x34);
    c.pit.write(0, 4);
    let before = c.pit.c[0];
    assert!(!c.advance(100));
    assert_eq!(c.pit.c[0], before);
    c.pit.write(0, 0);
    assert!(!c.advance(0));
    assert!(!c.advance(1));
    state(&c, 4, true);
    assert!(!c.advance(1));
    state(&c, 3, true);
}

#[test]
fn rewrites_at_every_period_boundary_latest_complete_value_wins() {
    for elapsed in 0..=4 {
        let mut c = Clock::default();
        program(&mut c.pit, 0, 3, 2, 4);
        c.advance(1); // load
        c.advance(elapsed);
        let old = c.pit.c[0].remaining;
        let output = c.pit.c[0].output;
        write_count(&mut c.pit, 0, 3, 2);
        write_count(&mut c.pit, 0, 3, 3);
        state(&c, old, output);
        assert_eq!(c.pit.c[0].reload, 3);
        assert!(!c.advance(old - 1));
        state(&c, 1, false);
        assert!(c.advance(1));
        state(&c, 3, true);
        assert!(!c.advance(1));
        state(&c, 2, true);
        assert!(!c.advance(1));
        state(&c, 1, false);
        assert!(c.advance(1));
        state(&c, 3, true);
    }
}

#[test]
fn rewrites_with_each_access_form_and_before_initial_load() {
    for (access, old, new) in [(1, 4, 3), (2, 512, 768), (3, 4, 3)] {
        let mut c = Clock::default();
        program(&mut c.pit, 0, access, 2, old);
        write_count(&mut c.pit, 0, access, new);
        assert!(!c.advance(1));
        state(&c, new as u32, true); // latest count on the initial load clock
        c.advance(1);
        write_count(&mut c.pit, 0, access, old);
        state(&c, new as u32 - 1, true);
        assert!(!c.advance(new as u32 - 2));
        state(&c, 1, false);
        assert!(c.advance(1));
        state(&c, old as u32, true);
    }
    let mut c = Clock::default();
    program(&mut c.pit, 0, 3, 2, 4);
    c.advance(1);
    c.pit.write(0, 3); // unfinished rewrite leaves both active count and reload alone
    c.advance(4);
    state(&c, 4, true);
    assert_eq!(c.pit.c[0].reload, 4);
    c.pit.write(0, 0); // complete it at a reload boundary
    c.advance(4);
    state(&c, 3, true);
}

#[test]
fn ordinary_latched_read_is_frozen_while_counting_continues() {
    let mut c = Clock::default();
    program(&mut c.pit, 0, 3, 2, 0x102);
    c.advance(2); // count 0101h
    c.pit.write(3, 0); // ordinary channel-0 count latch
    assert_eq!(c.pit.read(0), 1);
    assert!(!c.advance(3));
    state(&c, 254, true);
    assert_eq!(c.pit.read(0), 1); // high byte from 0101h, not 00FEh
    assert_eq!(c.pit.read(0), 254);
    assert_eq!(c.pit.read(0), 0);
    assert!(!c.advance(253));
    state(&c, 1, false);
    assert!(c.advance(1));
    state(&c, 258, true);
}

#[test]
fn single_clocks_irregular_batches_and_many_periods_have_identical_state_and_edges() {
    for encoded in [2, 3, 4, 65535, 0] {
        let mut single = Clock::default();
        program(&mut single.pit, 0, 3, 2, encoded);
        let mut batch = Clock {
            pit: single.pit.clone(),
            ..Clock::default()
        };
        for n in [0, 1, 3, 17, 0, 65536, 7, 134437] {
            let mut single_edges = false;
            for _ in 0..n {
                single_edges |= single.advance(1);
            }
            assert_eq!(single_edges, batch.advance(n));
            assert_eq!(single.pit, batch.pit);
        }
        // Deferred value must also take effect in a batch crossing the old boundary.
        write_count(&mut single.pit, 0, 3, 3);
        write_count(&mut batch.pit, 0, 3, 3);
        let mut a = false;
        for _ in 0..100_000 {
            a |= single.advance(1);
        }
        let b = batch.advance(100_000);
        assert_eq!(single.pit, batch.pit);
        assert_eq!(a, b);
        // u32::MAX scheduler cycles: bounded arithmetic and constant-time counter skip.
        let mut split = batch.pit.clone();
        let edge = batch.pit.tick(u32::MAX);
        let split_edge = split.tick(2_000_000_000) | split.tick(2_294_967_295);
        assert_eq!(batch.pit, split);
        assert_eq!(edge, split_edge);
    }
    let mut c = Clock::default();
    program(&mut c.pit, 0, 1, 2, 4);
    c.advance(1);
    assert!(c.advance(4)); // includes the entire low pulse; both endpoints high
    state(&c, 4, true);
}

#[test]
fn guest_cycle_fraction_is_preserved_across_zero_ticks_and_single_cycle_batches() {
    let mut a = Pit::default();
    program(&mut a, 0, 3, 2, 4);
    let mut b = a.clone();
    assert!(!a.tick(4)); // 0.9984 PIT clocks, no load yet
    let saved = a.clone();
    assert!(!a.tick(0));
    assert_eq!(a, saved);
    assert!(!a.tick(1));
    assert!(!b.tick(5));
    assert_eq!(a, b);
    assert_eq!(a.c[0].remaining, 4);
    let edge_a = (0..101).fold(false, |edge, _| a.tick(1) | edge);
    assert_eq!(edge_a, b.tick(101));
    assert_eq!(a, b);
}

#[test]
fn both_profiles_port_banks_irq_edges_masks_isr_eoi_and_channel_isolation() {
    for profile in PROFILES {
        for base in [0x71, 0x3fd9] {
            let mut b = Bus::new(profile);
            b.output(2, 0xff);
            // Channel 1 and 2 programming must not request IRQ0.
            for channel in [1, 2] {
                b.output(base + 6, (channel << 6) | 0x14);
                b.output(base + 2 * channel as u16, 3);
            }
            assert_eq!(b.pics[0].irr, 0);
            let other = [b.pit.c[1], b.pit.c[2]];
            b.output(base + 6, 0x14);
            assert_eq!([b.pit.c[1], b.pit.c[2]], other);
            assert_eq!(b.pics[0].irr & 1, 1); // programming low -> high
            b.output(base + 6, 0x14); // held high
            b.output(base, 4);
            assert_eq!(b.irq(), None); // masked setup edge retained
            b.output(2, 0xfe);
            assert_eq!(b.irq(), Some(8));
            b.output(0, 0x20); // acknowledge setup separately
            assert_eq!(b.irq(), None);
            b.tick(5); // initial load only
            assert_eq!((b.pit.c[0].remaining, b.pit.c[0].output), (4, true));
            assert_eq!(b.pics[0].irr & 1, 0);
            b.tick(12); // 3 more clocks: low, no IRQ
            assert_eq!((b.pit.c[0].remaining, b.pit.c[0].output), (1, false));
            assert_eq!(b.pics[0].irr & 1, 0);
            // Programming during low rises immediately, then holds pending load.
            b.output(base + 6, 0x1c); // mode-6 alias
            assert_eq!(b.pics[0].irr & 1, 1);
            assert_eq!(b.irq(), Some(8));
            b.output(0, 0x20);
            b.tick(100);
            assert_eq!(b.pics[0].irr & 1, 0); // no count loaded, held high
            b.output(base, 4);
            b.tick(5);
            b.tick(12);
            assert!(!b.pit.c[0].output);
            assert_eq!(b.pics[0].irr & 1, 0);
            b.tick(4);
            assert!(b.pit.c[0].output);
            b.output(0, 0x0a);
            assert_eq!(b.input(0) & 1, 1);
            assert_eq!(b.irq(), Some(8));
            b.output(0, 0x0b);
            assert_eq!(b.input(0) & 1, 1);
            b.tick(4_000_000); // many periods while ISR0 is active
            assert_eq!(b.irq(), None);
            b.output(2, 0xff);
            b.output(0, 0x20);
            assert_eq!(b.irq(), None);
            b.output(0, 0x0a);
            assert_eq!(b.input(0) & 1, 1);
            b.output(2, 0xfe);
            assert_eq!(b.irq(), Some(8)); // one latched request, no backlog
            b.output(0, 0x20);
            assert_eq!(b.irq(), None);
            b.tick(0);
            assert_eq!(b.irq(), None);
        }
    }
}

#[test]
fn periodic_irq_waits_for_if_and_wakes_hlt_then_iret_and_reset_clear_pending_state() {
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.cpu.s = [0; 4];
        m.cpu.ip = 0x100;
        m.cpu.r[4] = 0x7c00;
        m.cpu.flags = 2;
        m.cpu.halted = true;
        // HLT at resume address. Hardware handler increments guest memory and EOIs.
        m.bus.mem[0x100] = 0xf4;
        m.bus.mem[0x200..0x20b].copy_from_slice(&[
            0x50, 0xfe, 0x06, 0x00, 0x81, 0xb0, 0x20, 0xe6, 0, 0x58, 0xcf,
        ]);
        m.bus.write_seg(0, 8 * 4, 0x200, true);
        m.bus.write_seg(0, 8 * 4 + 2, 0, true);
        m.bus.output(2, 0xfe);
        m.bus.output(0x77, 0x34);
        assert_eq!(m.bus.irq(), Some(8)); // consume setup before periodic test
        m.bus.output(0, 0x20);
        m.bus.output(0x71, 0xe8);
        m.bus.output(0x71, 3); // divisor 1000
        m.step(600); // timer expires with IF clear and HLT set
        assert!(m.cpu.halted);
        assert_eq!(m.bus.read(0x8100), 0);
        assert_eq!(m.bus.pics[0].irr & 1, 1);
        m.cpu.flags |= IF;
        m.step(20);
        assert!(m.cpu.halted); // resumed, serviced, IRET, HLT again
        assert_eq!(m.bus.read(0x8100), 1);
        assert_eq!(m.bus.pics[0].isr & 1, 0);
        assert!(m.error.is_none());
        // Active deferred reload, incomplete write, latch, fractional clock and IRR.
        m.bus.output(0x71, 2);
        m.bus.output(0x71, 0);
        m.bus.output(0x71, 7);
        m.bus.output(0x77, 0);
        m.bus.tick(4001);
        assert_eq!(m.bus.pics[0].irr & 1, 1);
        m.reset();
        assert_eq!(m.bus.pit, Pit::default());
        assert_eq!(m.bus.pics[0].irr & 1, 0);
        assert_eq!(m.bus.pics[0].isr & 1, 0);
    }
}

#[test]
fn original_mode2_boot_probe_before_and_after_reset_in_both_profiles() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/pit_mode2/pit_mode2.hdm"
    ))
    .expect("run python3 tools/prepare_pit_mode2_fixture.py");
    let mut expected = [0xa5; 32];
    expected[..8].copy_from_slice(&[0xa5, 1, 2, 1, 1, 0, 0, 1]);
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "pit_mode2.hdm").unwrap();
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
                    "{text}; {:02x?}",
                    &m.bus.mem[0x8000..0x8020]
                );
                if text.contains("PIT MODE2 PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(m.bus.text().contains("PIT MODE2 PASS"), "{}", m.bus.text());
            assert_eq!(&m.bus.mem[0x8000..0x8020], &expected);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

#[test]
fn issue47_initial_load_reproduction() {
    let mut pit = Pit::default();
    pit.write(3, 0x36);
    pit.write(0, 4);
    pit.write(0, 0);
    pit.tick(5);
    assert_eq!((pit.c[0].remaining, pit.c[0].output), (4, true));
}

#[test]
fn mode3_specification_count_out_edge_traces_all_channels_and_encodings() {
    // Intel 8253 Mode 3, starting on the load clock. These explicit traces
    // distinguish subtract-1/subtract-3 odd halves from a generic countdown.
    type Sample = (u32, bool, bool);
    let traces: &[(u16, &[Sample])] = &[
        (
            2,
            &[
                (2, true, false),
                (2, false, false),
                (2, true, true),
                (2, false, false),
                (2, true, true),
            ],
        ),
        (
            3,
            &[
                (3, true, false),
                (2, true, false),
                (3, false, false),
                (3, true, true),
                (2, true, false),
                (3, false, false),
                (3, true, true),
            ],
        ),
        (
            4,
            &[
                (4, true, false),
                (2, true, false),
                (4, false, false),
                (2, false, false),
                (4, true, true),
            ],
        ),
        (
            5,
            &[
                (5, true, false),
                (4, true, false),
                (2, true, false),
                (5, false, false),
                (2, false, false),
                (5, true, true),
            ],
        ),
    ];
    for mode in [3, 7] {
        for channel in 0..3 {
            for &(divisor, trace) in traces {
                for access in [1, 3] {
                    let mut c = Clock::default();
                    program(&mut c.pit, channel, access, mode, divisor);
                    assert!(c.pit.c[channel].output);
                    let saved = c.pit.clone();
                    assert!(!c.advance(0));
                    assert_eq!(c.pit, saved);
                    for &(count, high, edge) in trace {
                        assert_eq!(c.advance(1), channel == 0 && edge);
                        assert_eq!(
                            (c.pit.c[channel].remaining, c.pit.c[channel].output),
                            (count, high),
                            "N={divisor}, channel={channel}, mode={mode}"
                        );
                        // Ordinary reads expose the running sequence.
                        assert_eq!(c.pit.read(channel), count as u8);
                        if access == 3 {
                            assert_eq!(c.pit.read(channel), (count >> 8) as u8);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mode3_boundaries_access_forms_and_initial_load() {
    for mode in [3, 7] {
        for channel in 0..3 {
            for (access, encoded, n) in [
                (1, 4, 4u32),
                (2, 512, 512),
                (3, 65535, 65535),
                (1, 0, 65536),
                (2, 0, 65536),
                (3, 0, 65536),
            ] {
                let mut c = Clock::default();
                c.pit
                    .write(3, ((channel as u8) << 6) | (access << 4) | (mode << 1));
                assert!(!c.pit.c[channel].enabled);
                assert!(!c.advance(19));
                if access == 3 {
                    c.pit.write(channel, encoded as u8);
                    assert!(!c.advance(19));
                    assert!(!c.pit.c[channel].enabled);
                    c.pit.write(channel, (encoded >> 8) as u8);
                } else {
                    write_count(&mut c.pit, channel, access, encoded);
                }
                assert!(!c.advance(0));
                assert!(!c.advance(1));
                assert_eq!(
                    (c.pit.c[channel].remaining, c.pit.c[channel].output),
                    (n, true)
                );
                assert!(!c.advance(n.div_ceil(2) - 1));
                assert_eq!(
                    (c.pit.c[channel].remaining, c.pit.c[channel].output),
                    (2, true)
                );
                assert!(!c.advance(1));
                assert_eq!(
                    (c.pit.c[channel].remaining, c.pit.c[channel].output),
                    (n, false)
                );
                assert!(!c.advance(n / 2 - 1));
                assert_eq!(
                    (c.pit.c[channel].remaining, c.pit.c[channel].output),
                    (2, false)
                );
                assert_eq!(c.advance(1), channel == 0);
                assert_eq!(
                    (c.pit.c[channel].remaining, c.pit.c[channel].output),
                    (n, true)
                );
                c.pit.write(3, (channel as u8) << 6);
                let byte = c.pit.read(channel);
                assert_eq!(byte, if access == 2 { (n >> 8) as u8 } else { n as u8 });
            }
        }
    }
}

#[test]
fn mode3_latched_reads_continue_counting_through_both_halves() {
    for channel in 0..3 {
        for n in [0x102, 0x103] {
            let mut c = Clock::default();
            program(&mut c.pit, channel, 3, 3, n);
            c.advance(2); // 0100h even, 0102h odd
            let expected = if n == 0x102 { 0x100 } else { 0x102 };
            c.pit.write(3, (channel as u8) << 6);
            assert_eq!(c.pit.read(channel), expected as u8);
            c.advance(133); // low half, high byte has changed
            assert!(!c.pit.c[channel].output);
            assert!(c.pit.c[channel].remaining < 256);
            assert_eq!(c.pit.read(channel), 1); // latched high byte
            assert_eq!(c.pit.read(channel), c.pit.c[channel].remaining as u8);
            assert_eq!(c.pit.read(channel), 0);
        }
    }
}

#[test]
fn mode3_single_clocks_batches_hidden_edges_and_maximum_scheduler_cycles() {
    for mode in [3, 7] {
        for n in [2, 3, 4, 5, 65535, 0] {
            let mut a = Clock::default();
            for channel in 0..3 {
                program(&mut a.pit, channel, 3, mode, n);
            }
            let mut b = Clock {
                pit: a.pit.clone(),
                ..Clock::default()
            };
            for clocks in [0, 1, 1, 3, 17, 0, 65536, 7, 134437] {
                let mut edges = false;
                for _ in 0..clocks {
                    edges |= a.advance(1);
                }
                assert_eq!(edges, b.advance(clocks));
                assert_eq!(a.pit, b.pit);
            }
            let mut split = b.pit.clone();
            let edge = b.pit.tick(u32::MAX);
            let split_edge = split.tick(2_000_000_000) | split.tick(2_294_967_295);
            assert_eq!(edge, split_edge);
            assert_eq!(b.pit, split);
        }
    }
    let mut c = Clock::default();
    program(&mut c.pit, 0, 1, 3, 5);
    c.advance(1);
    assert!(c.advance(5)); // both endpoints high, intervening rise retained
    state(&c, 5, true);
    c.advance(3);
    state(&c, 5, false);
    assert!(c.advance(5)); // both endpoints low, intervening rise retained
    state(&c, 5, false);
    let mut a = c.pit.clone();
    let mut b = a.clone();
    let edge = (0..101).fold(false, |edge, _| a.tick(1) | edge);
    assert_eq!(edge, b.tick(101));
    assert_eq!(a, b);
}

#[test]
fn mode3_both_profiles_banks_programming_rises_pic_latching_and_mode_transitions() {
    for profile in PROFILES {
        for base in [0x71, 0x3fd9] {
            let mut b = Bus::new(profile);
            let (mut clocks, mut cycles) = (0u64, 0u64);
            let mut advance = |b: &mut Bus, n: u32| {
                clocks += n as u64;
                let next_cycles = (clocks * 625).div_ceil(156);
                b.tick((next_cycles - cycles) as u32);
                cycles = next_cycles;
            };
            b.output(2, 0xff);
            for channel in [1, 2] {
                b.output(base + 6, (channel << 6) | 0x16);
                b.output(base + 2 * channel as u16, 4);
            }
            assert_eq!(b.pics[0].irr, 0);
            let others = [b.pit.c[1], b.pit.c[2]];
            b.output(base + 6, 0x16);
            assert_eq!([b.pit.c[1], b.pit.c[2]], others);
            assert_eq!(b.pics[0].irr & 1, 1);
            assert_eq!(b.irq(), None);
            b.output(2, 0xfe);
            assert_eq!(b.irq(), Some(8));
            b.output(0, 0x20);
            b.output(base + 6, 0x16); // held high
            advance(&mut b, 100);
            assert_eq!(b.pics[0].irr & 1, 0);
            b.output(base, 4);
            advance(&mut b, 1); // load
            for counter in &b.pit.c {
                assert_eq!((counter.remaining, counter.output), (4, true));
            }
            advance(&mut b, 2); // falling half-period boundary
            for channel in 0..3 {
                assert_eq!(
                    (b.pit.c[channel].remaining, b.pit.c[channel].output),
                    (4, false)
                );
                b.output(base + 6, (channel as u8) << 6);
                assert_eq!(b.input(base + 2 * channel as u16), 4);
            }
            assert_eq!(b.pics[0].irr & 1, 0);
            b.output(base + 6, 0x1e); // alias 7, low-to-high reprogramming
            assert_eq!(b.irq(), Some(8));
            b.output(0, 0x20);
            b.output(base, 4);
            advance(&mut b, 1);
            advance(&mut b, 2);
            assert!(!b.pit.c[0].output);
            assert_eq!(b.pics[0].irr & 1, 0);
            advance(&mut b, 2);
            assert!(b.pit.c[0].output);
            b.output(0, 0x0a);
            assert_eq!(b.input(0) & 1, 1);
            assert_eq!(b.irq(), Some(8));
            b.output(0, 0x0b);
            assert_eq!(b.input(0) & 1, 1);
            advance(&mut b, 1_000_000);
            assert_eq!(b.irq(), None);
            b.output(2, 0xff);
            b.output(0, 0x20);
            assert_eq!(b.irq(), None);
            b.output(2, 0xfe);
            assert_eq!(b.irq(), Some(8));
            b.output(0, 0x20);
            assert_eq!(b.irq(), None);
            advance(&mut b, 0);
            assert_eq!(b.irq(), None);
            // Each new mode word clears partial writes, pending loads and latches.
            for (mode, low_clocks) in [(2, 3), (3, 2), (2, 3), (3, 2)] {
                if let Some(vector) = b.irq() {
                    assert_eq!(vector, 8);
                    b.output(0, 0x20);
                }
                b.output(base + 6, 0x30 | (mode << 1));
                b.output(base, 0xff); // incomplete initial pair
                advance(&mut b, 100);
                assert!(!b.pit.c[0].enabled);
                b.output(base + 6, 0); // stale latch must be cleared by next mode word
                b.output(base + 6, 0x10 | (mode << 1));
                b.output(base, 4);
                assert_eq!(b.pics[0].irr & 1, 0);
                advance(&mut b, 1);
                assert_eq!((b.pit.c[0].remaining, b.pit.c[0].output), (4, true));
                advance(&mut b, low_clocks);
                assert!(!b.pit.c[0].output);
                assert_eq!(b.pics[0].irr & 1, 0);
                advance(&mut b, 4 - low_clocks);
                assert_eq!(b.pics[0].irr & 1, 1);
            }
        }
    }
}

#[test]
fn mode3_beeper_uses_out_including_reload_at_start_of_low_half() {
    for profile in PROFILES {
        for n in [1000u16, 1001] {
            let mut b = Bus::new(profile);
            b.output(0x77, 0x76);
            b.output(0x73, n as u8);
            b.output(0x73, (n >> 8) as u8);
            b.output(0x37, 6);
            b.tick(((1 + (n as u32).div_ceil(2)) * 625).div_ceil(156));
            assert_eq!(b.pit.c[1].remaining, n as u32);
            assert!(!b.pit.c[1].output);
            assert_eq!(b.audio.last(), Some(&-0.15));
            b.output(0x37, 7);
            b.tick(167);
            assert_eq!(b.audio.last(), Some(&0.0));
        }
    }
}

#[test]
fn mode3_periodic_irq_waits_for_if_and_wakes_hlt_then_iret_and_reset_clear_pending_state() {
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.cpu.s = [0; 4];
        m.cpu.ip = 0x100;
        m.cpu.r[4] = 0x7c00;
        m.cpu.flags = 2;
        m.cpu.halted = true;
        // HLT at resume address. Hardware handler increments guest memory and EOIs.
        m.bus.mem[0x100] = 0xf4;
        m.bus.mem[0x200..0x20b].copy_from_slice(&[
            0x50, 0xfe, 0x06, 0x00, 0x81, 0xb0, 0x20, 0xe6, 0, 0x58, 0xcf,
        ]);
        m.bus.write_seg(0, 8 * 4, 0x200, true);
        m.bus.write_seg(0, 8 * 4 + 2, 0, true);
        m.bus.output(2, 0xfe);
        m.bus.output(0x77, 0x36);
        assert_eq!(m.bus.irq(), Some(8)); // consume setup before periodic test
        m.bus.output(0, 0x20);
        m.bus.output(0x71, 0xe8);
        m.bus.output(0x71, 3); // divisor 1000
        m.step(600); // timer expires with IF clear and HLT set
        assert!(m.cpu.halted);
        assert_eq!(m.bus.read(0x8100), 0);
        assert_eq!(m.bus.pics[0].irr & 1, 1);
        m.cpu.flags |= IF;
        m.step(20);
        assert!(m.cpu.halted); // resumed, serviced, IRET, HLT again
        assert_eq!(m.bus.read(0x8100), 1);
        assert_eq!(m.bus.pics[0].isr & 1, 0);
        assert!(m.error.is_none());
        // New mode word, incomplete count, latch, fractional clock and pending IRQ.
        m.bus.output(0x77, 0x36);
        m.bus.output(0x71, 2);
        m.bus.output(0x71, 0);
        m.bus.output(0x71, 7);
        m.bus.output(0x77, 0);
        m.bus.tick(4001);
        assert_eq!(m.bus.pics[0].irr & 1, 1);
        m.reset();
        assert_eq!(m.bus.pit, Pit::default());
        assert_eq!(m.bus.pics[0].irr & 1, 0);
        assert_eq!(m.bus.pics[0].isr & 1, 0);
    }
}

#[test]
fn original_mode3_boot_probe_before_and_after_reset_in_both_profiles() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/pit_mode3/pit_mode3.hdm"
    ))
    .expect("run python3 tools/prepare_pit_mode3_fixture.py");
    let mut expected = [0xa5; 32];
    expected[..8].copy_from_slice(&[0xa5, 1, 2, 1, 1, 0, 0, 1]);
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "pit_mode3.hdm").unwrap();
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
                    "{text}; {:02x?}",
                    &m.bus.mem[0x8000..0x8020]
                );
                if text.contains("PIT MODE3 PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(m.bus.text().contains("PIT MODE3 PASS"), "{}", m.bus.text());
            assert_eq!(&m.bus.mem[0x8000..0x8020], &expected);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

#[test]
fn mode3_reset_during_initial_partial_and_active_programming() {
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        for active in [false, true] {
            for channel in 0..3 {
                m.bus.output(0x77, (channel << 6) | 0x36);
                m.bus.output(0x71 + 2 * channel as u16, 5);
                if active {
                    m.bus.output(0x71 + 2 * channel as u16, 0);
                }
            }
            m.bus.tick(17); // partial programming holds high; active odd count is low
            for channel in 0..3 {
                assert_eq!(m.bus.pit.c[channel].enabled, active);
                assert_eq!(m.bus.pit.c[channel].output, !active);
                m.bus.output(0x77, (channel as u8) << 6);
                m.bus.input(0x71 + 2 * channel as u16); // partial latched read
            }
            m.reset();
            assert_eq!(m.bus.pit, Pit::default());
            assert_eq!(m.bus.pics[0].irr, 0);
            assert_eq!(m.bus.pics[0].isr, 0);
        }
    }
}

#[test]
fn issue49_mode0_initial_load_reproduction() {
    let mut pit = Pit::default();
    pit.write(3, 0x36);
    pit.write(3, 0x30);
    pit.write(0, 4);
    pit.write(0, 0);
    assert!(!pit.tick(5));
    assert_eq!((pit.c[0].remaining, pit.c[0].output), (4, false));
}

#[test]
fn mode0_specification_traces_all_channels_and_access_forms() {
    // Intel 8253 Mode 0: load, decrement to terminal rise, then keep counting.
    type Sample = (u32, bool, bool);
    let traces: &[(u16, &[Sample])] = &[
        (
            1,
            &[
                (1, false, false),
                (0, true, true),
                (65535, true, false),
                (65534, true, false),
            ],
        ),
        (
            2,
            &[
                (2, false, false),
                (1, false, false),
                (0, true, true),
                (65535, true, false),
                (65534, true, false),
            ],
        ),
        (
            4,
            &[
                (4, false, false),
                (3, false, false),
                (2, false, false),
                (1, false, false),
                (0, true, true),
                (65535, true, false),
                (65534, true, false),
            ],
        ),
    ];
    for channel in 0..3 {
        for access in [1, 3] {
            for &(n, trace) in traces {
                let mut c = Clock::default();
                program(&mut c.pit, channel, access, 0, n);
                for &(count, high, edge) in trace {
                    assert_eq!(c.advance(1), channel == 0 && edge);
                    assert_eq!(
                        (c.pit.c[channel].remaining, c.pit.c[channel].output),
                        (count, high)
                    );
                    assert_eq!(c.pit.read(channel), count as u8);
                    if access == 3 {
                        assert_eq!(c.pit.read(channel), (count >> 8) as u8);
                    }
                }
            }
        }
        for (access, encoded, n) in [
            (1, 4, 4),
            (2, 512, 512),
            (3, 65535, 65535),
            (1, 0, 65536),
            (2, 0, 65536),
            (3, 0, 65536),
        ] {
            let mut c = Clock::default();
            c.pit.write(3, ((channel as u8) << 6) | (access << 4));
            assert!(!c.pit.c[channel].enabled);
            assert!(!c.pit.c[channel].output);
            assert!(!c.advance(100)); // no assertion about undefined pre-load count
            if access == 3 {
                c.pit.write(channel, encoded as u8);
                assert!(!c.advance(100));
                assert!(!c.pit.c[channel].enabled);
                assert!(!c.pit.c[channel].output);
                c.pit.write(channel, (encoded >> 8) as u8);
            } else {
                write_count(&mut c.pit, channel, access, encoded);
            }
            let saved = c.pit.clone();
            assert!(!c.advance(0));
            assert_eq!(c.pit, saved);
            assert!(!c.advance(1)); // separate load clock
            assert_eq!(
                (c.pit.c[channel].remaining, c.pit.c[channel].output),
                (n, false)
            );
            assert!(!c.advance(n - 1));
            assert_eq!(
                (c.pit.c[channel].remaining, c.pit.c[channel].output),
                (1, false)
            );
            assert_eq!(c.advance(1), channel == 0);
            assert_eq!(
                (c.pit.c[channel].remaining, c.pit.c[channel].output),
                (0, true)
            );
            // Multiple wraps must neither stop counting nor create another edge.
            for count in [65535u32, 0, 65535, 0, 65535] {
                assert!(!c.advance(if count == 0 { 65535 } else { 1 }));
                assert_eq!(
                    (c.pit.c[channel].remaining, c.pit.c[channel].output),
                    (count, true)
                );
                c.pit.write(3, (channel as u8) << 6);
                assert_eq!(
                    c.pit.read(channel),
                    if access == 2 {
                        (count >> 8) as u8
                    } else {
                        count as u8
                    }
                );
                if access == 3 {
                    assert_eq!(c.pit.read(channel), (count >> 8) as u8);
                }
            }
        }
    }
}

#[test]
fn mode0_latch_snapshot_across_terminal_and_wrap_and_fresh_modes() {
    for channel in 0..3 {
        let mut c = Clock::default();
        program(&mut c.pit, channel, 3, 0, 0x102);
        c.advance(2); // 0101h
        c.pit.write(3, (channel as u8) << 6);
        assert_eq!(c.pit.read(channel), 1);
        assert_eq!(c.advance(258), channel == 0); // terminal then FFFFh
        assert_eq!(c.pit.c[channel].remaining, 65535);
        assert_eq!(c.pit.read(channel), 1); // old high byte survives terminal
        assert_eq!(c.pit.read(channel), 255);
        assert_eq!(c.pit.read(channel), 255);
        c.pit.write(3, (channel as u8) << 6);
        assert_eq!(c.pit.read(channel), 255);
        assert!(!c.advance(65536));
        assert_eq!(c.pit.read(channel), 255);
        for mode in [2, 0, 3, 0, 0] {
            // Leave a partial latched read behind, then interrupt an active
            // lifecycle with a new mode word and an incomplete initial pair.
            c.pit.write(3, (channel as u8) << 6);
            c.pit.read(channel);
            let others = c.pit.c;
            c.pit.write(3, ((channel as u8) << 6) | 0x30 | (mode << 1));
            assert!(!c.pit.c[channel].enabled);
            assert!(!c.advance(19));
            c.pit.write(channel, 4);
            assert!(!c.advance(19));
            assert!(!c.pit.c[channel].enabled);
            c.pit.write(channel, 0);
            for (i, other) in others.iter().enumerate() {
                if i != channel {
                    assert_eq!(&c.pit.c[i], other);
                }
            }
            assert_eq!(c.pit.c[channel].output, mode != 0);
            assert!(!c.advance(1));
            assert_eq!(
                (c.pit.c[channel].remaining, c.pit.c[channel].output),
                (4, mode != 0)
            );
            assert_eq!(c.pit.read(channel), 4);
            assert_eq!(c.pit.read(channel), 0);
            assert_eq!(c.advance(4), channel == 0);
            assert!(c.pit.c[channel].output);
        }
    }
}

#[test]
fn mode0_single_clocks_batches_fraction_and_maximum_cycles_agree() {
    for n in [1, 2, 4, 65535, 0] {
        let mut single = Clock::default();
        for channel in 0..3 {
            program(&mut single.pit, channel, 3, 0, n);
        }
        let mut batch = Clock {
            pit: single.pit.clone(),
            ..Clock::default()
        };
        // Include the pending load, terminal count and several full wraps.
        for clocks in [0, 1, 1, 3, 17, 0, 65536, 7, 134437] {
            let mut edges = false;
            for _ in 0..clocks {
                edges |= single.advance(1);
            }
            assert_eq!(edges, batch.advance(clocks));
            assert_eq!(single.pit, batch.pit);
        }
        // Also cover MAX advancement starting before load and after terminal.
        for mut whole in [single.pit, {
            let mut p = Pit::default();
            for channel in 0..3 {
                program(&mut p, channel, 3, 0, n);
            }
            p
        }] {
            let mut split = whole.clone();
            let edge = whole.tick(u32::MAX);
            let split_edge = split.tick(2_000_000_000) | split.tick(2_294_967_295);
            assert_eq!(edge, split_edge);
            assert_eq!(whole, split);
        }
    }
    let mut a = Pit::default();
    program(&mut a, 0, 3, 0, 4);
    let mut b = a.clone();
    assert!(!a.tick(4)); // fraction 0.9984, still pending load
    let saved = a.clone();
    assert!(!a.tick(0));
    assert_eq!(a, saved);
    assert!(!a.tick(1));
    assert!(!b.tick(5));
    assert_eq!(a, b);
    assert_eq!((a.c[0].remaining, a.c[0].output), (4, false));
    let edges = (0..101).fold(false, |edge, _| a.tick(1) | edge);
    assert_eq!(edges, b.tick(101));
    assert_eq!(a, b);
}

#[test]
fn mode0_both_profiles_banks_irq_latch_mask_isr_eoi_and_rearming() {
    for profile in PROFILES {
        for base in [0x71, 0x3fd9] {
            let mut b = Bus::new(profile);
            let (mut clocks, mut cycles) = (0u64, 0u64);
            let mut advance = |b: &mut Bus, n: u32| {
                clocks += n as u64;
                let next = (clocks * 625).div_ceil(156);
                b.tick((next - cycles) as u32);
                cycles = next;
            };
            b.output(2, 0xff);
            for mode in [2, 3] {
                b.output(base + 6, 0x30 | (mode << 1));
                assert_eq!(b.pics[0].irr & 1, 1); // existing programming rise
                b.output(base + 6, 0x30); // falling OUT must retain setup IRR
                assert!(!b.pit.c[0].output);
                assert_eq!(b.pics[0].irr & 1, 1);
                assert_eq!(b.irq(), None);
                b.output(2, 0xfe);
                assert_eq!(b.irq(), Some(8));
                assert_eq!(b.pics[0].irr & 1, 0);
                assert_eq!(b.pics[0].isr & 1, 1);
                b.output(0, 0x20);
                b.output(2, 0xff);
                // All channels load together; only channel zero requests IRQ0.
                for channel in 0..3 {
                    b.output(base + 6, (channel << 6) | 0x30);
                    b.output(base + 2 * channel as u16, if channel == 0 { 4 } else { 1 });
                    b.output(base + 2 * channel as u16, 0);
                }
                advance(&mut b, 1);
                assert_eq!(b.pics[0].irr & 1, 0);
                advance(&mut b, 1); // other channels reach terminal
                assert_eq!(b.pics[0].irr & 1, 0);
                advance(&mut b, 3);
                assert_eq!(b.pics[0].irr & 1, 1);
                assert_eq!(b.irq(), None); // masked terminal retained
                advance(&mut b, 200000); // high OUT across repeated wraps
                b.output(base + 6, 0); // latch and read post-terminal through bank
                let count = b.input(base) as u16 | ((b.input(base) as u16) << 8);
                assert_eq!(count, 0u16.wrapping_sub(200000u32 as u16));
                assert_eq!(b.pics[0].irr & 1, 1);
                b.output(2, 0xfe);
                assert_eq!(b.irq(), Some(8));
                b.output(0, 0x0b);
                assert_eq!(b.input(0) & 1, 1);
                advance(&mut b, 200000);
                assert_eq!(b.pics[0].irr & 1, 0);
                b.output(0, 0x20);
                assert_eq!(b.input(0) & 1, 0);
                assert_eq!(b.irq(), None);
                advance(&mut b, 200000);
                assert_eq!(b.irq(), None);
                let others = [b.pit.c[1], b.pit.c[2]];
                b.output(base + 6, 0x30); // fresh mode word rearms independently
                assert_eq!([b.pit.c[1], b.pit.c[2]], others);
                b.output(base, 2);
                advance(&mut b, 20); // partial pair cannot count or request
                assert!(!b.pit.c[0].output);
                assert_eq!(b.irq(), None);
                b.output(base, 0);
                advance(&mut b, 3); // load, 2, 1, terminal
                b.output(2, 0xff);
                b.output(base + 6, 0x30); // falling OUT retains terminal request
                assert!(!b.pit.c[0].output);
                assert_eq!(b.pics[0].irr & 1, 1);
                assert_eq!(b.irq(), None);
                b.output(2, 0xfe);
                assert_eq!(b.irq(), Some(8));
                b.output(0, 0x20);
                b.output(2, 0xff);
            }
        }
    }
}

#[test]
fn mode0_if_clear_hlt_delivery_eoi_iret_and_no_repeat() {
    for profile in PROFILES {
        for base in [0x71, 0x3fd9] {
            let mut m = Machine::new(profile);
            m.cpu.s = [0; 4];
            m.cpu.ip = 0x100;
            m.cpu.r[4] = 0x7c00;
            m.cpu.flags = 2;
            m.cpu.halted = true;
            m.bus.mem[0x100] = 0xf4;
            m.bus.mem[0x200..0x20b]
                .copy_from_slice(&[0x50, 0xfe, 0x06, 0, 0x81, 0xb0, 0x20, 0xe6, 0, 0x58, 0xcf]);
            m.bus.write_seg(0, 8 * 4, 0x200, true);
            m.bus.write_seg(0, 8 * 4 + 2, 0, true);
            m.bus.output(2, 0xfe);
            m.bus.output(base + 6, 0x30);
            m.bus.output(base, 0xe8);
            m.bus.output(base, 3);
            m.step(600);
            assert!(m.cpu.halted);
            assert_eq!(m.bus.mem[0x8100], 0);
            assert_eq!(m.bus.pics[0].irr & 1, 1);
            m.cpu.flags |= IF;
            m.step(20);
            assert!(m.cpu.halted);
            assert_eq!(m.bus.mem[0x8100], 1);
            assert_eq!(m.bus.pics[0].isr & 1, 0);
            m.step(100000); // continued counting, including wraps, cannot wake HLT
            assert_eq!(m.bus.mem[0x8100], 1);
            assert_eq!(m.bus.pics[0].irr & 1, 0);
            assert!(m.cpu.halted);
            assert!(m.error.is_none());
        }
    }
}

#[test]
fn mode0_reset_partial_pending_load_and_post_terminal_all_channels() {
    for profile in PROFILES {
        for phase in 0..3 {
            let mut m = Machine::new(profile);
            for channel in 0..3 {
                m.bus.output(0x77, (channel << 6) | 0x30);
                m.bus.output(0x71 + 2 * channel as u16, 4);
                if phase != 0 {
                    m.bus.output(0x71 + 2 * channel as u16, 0);
                }
            }
            if phase == 2 {
                m.bus.tick(1001);
            }
            for channel in 0..3 {
                assert_eq!(m.bus.pit.c[channel].enabled, phase != 0);
                assert_eq!(m.bus.pit.c[channel].output, phase == 2);
                m.bus.output(0x77, (channel as u8) << 6);
                m.bus.input(0x71 + 2 * channel as u16);
            }
            m.reset();
            assert_eq!(m.bus.pit, Pit::default());
            assert_eq!(m.bus.pics[0].irr, 0);
            assert_eq!(m.bus.pics[0].isr, 0);
            // No partial write/load/latch/fraction can leak into a fresh lifecycle.
            m.bus.output(0x77, 0x30);
            m.bus.output(0x71, 2);
            m.bus.output(0x71, 0);
            m.bus.tick(5);
            assert_eq!(
                (m.bus.pit.c[0].remaining, m.bus.pit.c[0].output),
                (2, false)
            );
        }
    }
}

#[test]
fn original_mode0_boot_probe_before_and_after_reset_in_both_profiles() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/pit_mode0/pit_mode0.hdm"
    ))
    .expect("run python3 tools/prepare_pit_mode0_fixture.py");
    let mut expected = [0xa5; 32];
    expected[..10].copy_from_slice(&[0xa5, 2, 1, 1, 0, 0, 1, 1, 1, 1]);
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "pit_mode0.hdm").unwrap();
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
                    "{text}; {:02x?}",
                    &m.bus.mem[0x8000..0x8020]
                );
                if text.contains("PIT MODE0 PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(m.bus.text().contains("PIT MODE0 PASS"), "{}", m.bus.text());
            assert_eq!(&m.bus.mem[0x8000..0x8020], &expected);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

#[test]
fn mode0_rewrite_partial_pair_stops_old_count_reproduction() {
    let mut c = Clock::default();
    program(&mut c.pit, 0, 3, 0, 4);
    assert!(!c.advance(2));
    state(&c, 3, false);
    c.pit.write(0, 2);
    assert!(
        !c.advance(4),
        "unfinished replacement must suppress old terminal rise"
    );
    state(&c, 3, false);
    let stopped = c.pit.c[0];
    for cycles in [0, 1, u32::MAX, u32::MAX] {
        assert!(!c.pit.tick(cycles));
        assert_eq!(c.pit.c[0], stopped);
    }
}

#[test]
fn mode0_rewrite_after_terminal_rearms_reproduction() {
    let mut c = Clock::default();
    program(&mut c.pit, 0, 3, 0, 4);
    assert!(c.advance(5));
    state(&c, 0, true);
    write_count(&mut c.pit, 0, 3, 2);
    assert!(!c.pit.c[0].output, "count-only rewrite must lower OUT");
    for (count, high, edge) in [(2, false, false), (1, false, false), (0, true, true)] {
        assert_eq!(c.advance(1), edge);
        state(&c, count, high);
    }
}

#[test]
fn mode0_rewrite_all_channels_access_values_and_lifecycle_boundaries() {
    for channel in 0..3 {
        for (access, values) in [
            (1, &[1u16, 2, 0][..]),
            (2, &[256, 512, 65280, 0][..]),
            (3, &[1, 2, 65535, 0][..]),
        ] {
            let old = if access == 2 { 1024 } else { 4 };
            // Mid-count, immediately before terminal, terminal, later wraps.
            for elapsed in [2, old as u32, old as u32 + 1, old as u32 + 131074] {
                for &encoded in values {
                    let n = if encoded == 0 { 65536 } else { encoded as u32 };
                    let mut c = Clock::default();
                    program(&mut c.pit, channel, access, 0, old);
                    c.advance(elapsed);
                    let others = c.pit.c;
                    if access == 3 {
                        c.pit.write(channel, encoded as u8);
                        assert!(!c.pit.c[channel].output); // immediate first-byte fall
                        assert!(!c.pit.c[channel].enabled);
                        let stopped = c.pit.c[channel];
                        assert!(!c.advance(131077));
                        assert_eq!(c.pit.c[channel], stopped);
                        c.pit.write(channel, (encoded >> 8) as u8);
                    } else {
                        write_count(&mut c.pit, channel, access, encoded);
                        assert!(!c.pit.c[channel].output); // single-byte fall
                    }
                    let saved = c.pit.clone();
                    assert!(!c.advance(0));
                    assert_eq!(c.pit, saved);
                    // No assertions about undefined pre-load readback.
                    for (clocks, count, high, rise) in [
                        (1, n, false, false), // separate load clock
                        (n - 1, 1, false, false),
                        (1, 0, true, true),
                        (1, 65535, true, false),
                        (65535, 0, true, false),
                        (65536, 0, true, false),
                        (2, 65534, true, false),
                    ] {
                        assert_eq!(c.advance(clocks), channel == 0 && rise);
                        assert_eq!(
                            (c.pit.c[channel].remaining, c.pit.c[channel].output),
                            (count, high),
                            "channel={channel}, access={access}, value={encoded}, elapsed={elapsed}"
                        );
                        // Count-only replacement preserves the programmed read format.
                        assert_eq!(
                            c.pit.read(channel),
                            if access == 2 {
                                (count >> 8) as u8
                            } else {
                                count as u8
                            }
                        );
                        if access == 3 {
                            assert_eq!(c.pit.read(channel), (count >> 8) as u8);
                        }
                    }
                    for (i, other) in others.iter().enumerate() {
                        if i != channel {
                            assert_eq!(&c.pit.c[i], other);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mode0_rewrite_latest_complete_and_partial_supersede_pending_loads() {
    for channel in 0..3 {
        for access in 1..=3 {
            let mut c = Clock::default();
            let (a, b) = if access == 2 { (256, 512) } else { (1, 2) };
            program(&mut c.pit, channel, access, 0, a);
            c.advance(a as u32 + 1);
            write_count(&mut c.pit, channel, access, a);
            write_count(&mut c.pit, channel, access, b);
            assert!(!c.advance(1));
            assert_eq!(
                (c.pit.c[channel].remaining, c.pit.c[channel].output),
                (b as u32, false)
            );
            assert_eq!(c.advance(b as u32), channel == 0);
            if access == 3 {
                write_count(&mut c.pit, channel, access, 2); // pending replacement
                c.pit.write(channel, 1); // newer partial pair cancels that load
                let stopped = c.pit.c[channel];
                for clocks in [0, 1, 65536, 131077] {
                    assert!(!c.advance(clocks));
                    assert!(!c.pit.c[channel].enabled);
                    assert!(!c.pit.c[channel].output);
                    assert_eq!(c.pit.c[channel], stopped);
                }
                c.pit.write(channel, 0);
                assert!(!c.advance(1));
                assert_eq!(
                    (c.pit.c[channel].remaining, c.pit.c[channel].output),
                    (1, false)
                );
                assert_eq!(c.advance(1), channel == 0);
            }
        }
    }
}

#[test]
fn mode0_rewrite_preserves_completed_latch_snapshots() {
    for channel in 0..3 {
        for access in 1..=3 {
            for read_low in [false, true] {
                let mut c = Clock::default();
                let initial = match access {
                    1 => 0x82,
                    2 => 0x8200,
                    _ => 0x8202,
                };
                program(&mut c.pit, channel, access, 0, initial);
                c.advance(2);
                let snapshot = initial - 1;
                c.pit.write(3, (channel as u8) << 6);
                if read_low && access == 3 {
                    assert_eq!(c.pit.read(channel), snapshot as u8);
                }
                write_count(&mut c.pit, channel, access, 0);
                c.advance(1); // new 65536 count has loaded, old snapshot survives
                if access == 2 {
                    assert_eq!(c.pit.read(channel), (snapshot >> 8) as u8);
                } else if access == 1 {
                    assert_eq!(c.pit.read(channel), snapshot as u8);
                } else {
                    if !read_low {
                        assert_eq!(c.pit.read(channel), snapshot as u8);
                    }
                    assert_eq!(c.pit.read(channel), (snapshot >> 8) as u8);
                }
                c.advance(1);
                assert_eq!(c.pit.read(channel), 255);
                if access == 3 {
                    assert_eq!(c.pit.read(channel), 255);
                }
            }
        }
    }
}

#[test]
fn mode0_rewrite_fresh_mode_cancels_partial_and_pending_state() {
    for channel in 0..3 {
        for partial in [false, true] {
            for mode in [0, 2, 3] {
                let mut c = Clock::default();
                program(&mut c.pit, channel, 3, 0, 4);
                c.advance(2);
                write_count(&mut c.pit, channel, 3, 2);
                if partial {
                    c.pit.write(channel, 7);
                }
                c.pit.write(3, ((channel as u8) << 6) | 0x30 | (mode << 1));
                assert!(!c.advance(131077));
                assert!(!c.pit.c[channel].enabled);
                assert_eq!(c.pit.c[channel].output, mode != 0);
                c.pit.write(channel, 4); // this must be a fresh low byte
                assert!(!c.advance(10));
                assert!(!c.pit.c[channel].enabled);
                c.pit.write(channel, 0);
                assert!(!c.advance(1));
                assert_eq!(
                    (c.pit.c[channel].remaining, c.pit.c[channel].output),
                    (4, mode != 0)
                );
                assert_eq!(c.advance(4), channel == 0);
                assert!(c.pit.c[channel].output);
            }
        }
    }
}

#[test]
fn mode0_rewrite_single_clocks_batches_fraction_and_maximum_cycles_agree() {
    for encoded in [1, 2, 65535, 0] {
        let mut single = Clock::default();
        for channel in 0..3 {
            program(&mut single.pit, channel, 3, 0, 4);
        }
        single.advance(5);
        let mut batch = Clock {
            pit: single.pit.clone(),
            clocks: single.clocks,
            cycles: single.cycles,
        };
        for channel in 0..3 {
            write_count(&mut single.pit, channel, 3, 2);
            write_count(&mut batch.pit, channel, 3, 2);
            single.pit.write(channel, encoded as u8);
            batch.pit.write(channel, encoded as u8);
        }
        // Include partial, completed-pending, loaded and post-terminal states.
        for phase in 0..4 {
            let mut whole = batch.pit.clone();
            let mut split = whole.clone();
            let rise = whole.tick(u32::MAX);
            let split_rise = split.tick(2_000_000_000) | split.tick(2_294_967_295);
            assert_eq!(whole, split);
            assert_eq!(rise, split_rise);
            for clocks in [0, 1, 3, 17, 65536, 7, 134437] {
                let mut edges = false;
                for _ in 0..clocks {
                    edges |= single.advance(1);
                }
                assert_eq!(batch.advance(clocks), edges);
                assert_eq!(single.pit, batch.pit);
            }
            for channel in 0..3 {
                if phase == 0 {
                    single.pit.write(channel, (encoded >> 8) as u8);
                    batch.pit.write(channel, (encoded >> 8) as u8);
                } else {
                    write_count(&mut single.pit, channel, 3, encoded);
                    write_count(&mut batch.pit, channel, 3, encoded);
                }
            }
            if phase == 1 || phase == 2 {
                let clocks = if phase == 1 { 1 } else { 65537 };
                single.advance(clocks);
                batch.advance(clocks);
            }
        }
    }
    let mut a = Pit::default();
    program(&mut a, 0, 3, 0, 4);
    a.tick(24); // terminal reached, nonzero fractional remainder
    write_count(&mut a, 0, 3, 2);
    let mut b = a.clone();
    let saved = a.clone();
    assert!(!a.tick(0));
    assert_eq!(a, saved);
    let edges = (0..101).fold(false, |edge, _| a.tick(1) | edge);
    assert_eq!(edges, b.tick(101));
    assert_eq!(a, b);
}

#[test]
fn mode0_rewrite_both_profiles_banks_channels_access_and_irq_lifecycle() {
    for profile in PROFILES {
        for base in [0x71, 0x3fd9] {
            for channel in 0..3 {
                for access in 1..=3 {
                    let mut b = Bus::new(profile);
                    let (mut clocks, mut cycles) = (0u64, 0u64);
                    let mut advance = |b: &mut Bus, n: u32| {
                        clocks += n as u64;
                        let next = (clocks * 625).div_ceil(156);
                        b.tick((next - cycles) as u32);
                        cycles = next;
                    };
                    let port = base + 2 * channel as u16;
                    let (old, n) = if access == 2 {
                        (1024u16, 512u16)
                    } else {
                        (4, 2)
                    };
                    b.output(2, 0xff);
                    b.output(base + 6, ((channel as u8) << 6) | (access << 4));
                    if access != 2 {
                        b.output(port, old as u8);
                    }
                    if access != 1 {
                        b.output(port, (old >> 8) as u8);
                    }
                    advance(&mut b, old as u32 + 1);
                    assert!(b.pit.c[channel].output);
                    assert_eq!(b.pics[0].irr & 1, u8::from(channel == 0));
                    b.output(2, 0xfe);
                    assert_eq!(b.irq(), if channel == 0 { Some(8) } else { None });
                    b.output(0, 0x20);
                    b.output(2, 0xff);
                    if access != 2 {
                        b.output(port, n as u8);
                    }
                    if access == 3 {
                        assert!(!b.pit.c[channel].output); // first-byte fall
                        advance(&mut b, 131077); // incomplete pair, many old wraps
                        assert!(!b.pit.c[channel].enabled);
                        assert_eq!(b.pics[0].irr & 1, 0);
                    }
                    if access != 1 {
                        b.output(port, (n >> 8) as u8);
                    }
                    assert!(!b.pit.c[channel].output);
                    assert_eq!(b.pics[0].irr & 1, 0); // falling OUT cannot request
                    advance(&mut b, 0);
                    advance(&mut b, 1);
                    assert_eq!(
                        (b.pit.c[channel].remaining, b.pit.c[channel].output),
                        (n as u32, false)
                    );
                    // Read through the same bank, proving access preservation.
                    b.output(base + 6, (channel as u8) << 6);
                    assert_eq!(
                        b.input(port),
                        if access == 2 { (n >> 8) as u8 } else { n as u8 }
                    );
                    if access == 3 {
                        assert_eq!(b.input(port), 0);
                    }
                    advance(&mut b, n as u32 - 1);
                    assert_eq!(
                        (b.pit.c[channel].remaining, b.pit.c[channel].output),
                        (1, false)
                    );
                    assert_eq!(b.pics[0].irr & 1, 0);
                    advance(&mut b, 1);
                    assert_eq!(
                        (b.pit.c[channel].remaining, b.pit.c[channel].output),
                        (0, true)
                    );
                    assert_eq!(b.pics[0].irr & 1, u8::from(channel == 0));
                    assert_eq!(b.irq(), None); // masked terminal stays pending
                    advance(&mut b, 131072);
                    // Falling OUT from another replacement retains that request.
                    if access != 2 {
                        b.output(port, n as u8);
                    }
                    if access == 2 {
                        b.output(port, (n >> 8) as u8);
                    }
                    assert!(!b.pit.c[channel].output);
                    assert_eq!(b.pics[0].irr & 1, u8::from(channel == 0));
                    b.output(2, 0xfe);
                    assert_eq!(b.irq(), if channel == 0 { Some(8) } else { None });
                    assert_eq!(b.pics[0].isr & 1, u8::from(channel == 0));
                    assert_eq!(b.pics[0].irr & 1, 0);
                    if access == 3 {
                        advance(&mut b, 131077);
                        assert_eq!(b.pics[0].irr & 1, 0);
                        b.output(port, (n >> 8) as u8);
                    }
                    advance(&mut b, n as u32 + 1);
                    assert_eq!(b.pics[0].irr & 1, u8::from(channel == 0));
                    assert_eq!(b.irq(), None); // normal ISR blocking still applies
                    b.output(0, 0x20);
                    assert_eq!(b.irq(), if channel == 0 { Some(8) } else { None });
                    b.output(0, 0x20);
                    for clocks in [0, 1, 65535, 131072] {
                        advance(&mut b, clocks);
                        assert!(b.pit.c[channel].output);
                        assert_eq!(b.pics[0].irr & 1, 0);
                        assert_eq!(b.pics[0].isr & 1, 0);
                        assert_eq!(b.irq(), None); // held high never repeats
                    }
                }
            }
        }
    }
}

#[test]
fn mode0_rewrite_active_channel_isolation() {
    for channel in 0..3 {
        let mut c = Clock::default();
        for i in 0..3 {
            program(&mut c.pit, i, 3, 0, 100);
        }
        c.advance(2);
        let mut control = Clock {
            pit: c.pit.clone(),
            clocks: c.clocks,
            cycles: c.cycles,
        };
        c.pit.write(channel, 2); // only this counter stops
        for clocks in [1, 17, 100, 65536] {
            c.advance(clocks);
            control.advance(clocks);
            for i in 0..3 {
                if i != channel {
                    assert_eq!(c.pit.c[i], control.pit.c[i]);
                }
            }
        }
        c.pit.write(channel, 0);
        c.advance(1);
        control.advance(1);
        assert_eq!(
            (c.pit.c[channel].remaining, c.pit.c[channel].output),
            (2, false)
        );
        for i in 0..3 {
            if i != channel {
                assert_eq!(c.pit.c[i], control.pit.c[i]);
            }
        }
    }
}

#[test]
fn mode0_rewrite_if_hlt_delivery_eoi_iret_and_no_repeat() {
    for profile in PROFILES {
        for base in [0x71, 0x3fd9] {
            let mut m = Machine::new(profile);
            m.cpu.s = [0; 4];
            m.cpu.ip = 0x100;
            m.cpu.r[4] = 0x7c00;
            m.cpu.flags = 2;
            m.cpu.halted = true;
            m.bus.mem[0x100..0x104].fill(0xf4);
            m.bus.mem[0x200..0x20b]
                .copy_from_slice(&[0x50, 0xfe, 0x06, 0, 0x81, 0xb0, 0x20, 0xe6, 0, 0x58, 0xcf]);
            m.bus.write_seg(0, 8 * 4, 0x200, true);
            m.bus.write_seg(0, 8 * 4 + 2, 0, true);
            m.bus.output(2, 0xfe);
            m.bus.output(base + 6, 0x30); // only mode word in this test
            m.bus.output(base, 0xe8);
            m.bus.output(base, 3);
            m.bus.tick(5);
            for entries in 1..=2 {
                m.cpu.flags &= !IF;
                m.bus.output(base, 0xe8); // interrupt active count, then post-terminal
                m.step(100000);
                assert_eq!(m.bus.mem[0x8100], entries - 1);
                assert_eq!(m.bus.pics[0].irr & 1, 0);
                assert!(m.cpu.halted);
                m.bus.output(base, 3);
                m.step(600); // IF clear retains request while CPU stays halted
                assert_eq!(m.bus.mem[0x8100], entries - 1);
                assert_eq!(m.bus.pics[0].irr & 1, 1);
                assert!(m.cpu.halted);
                m.cpu.flags |= IF;
                m.step(20);
                assert_eq!(m.bus.mem[0x8100], entries);
                assert_eq!(m.bus.pics[0].isr & 1, 0);
                assert!(m.cpu.halted); // handler EOI/IRET followed by next HLT
                m.step(100000);
                assert_eq!(m.bus.mem[0x8100], entries);
                assert_eq!(m.bus.pics[0].irr & 1, 0);
                assert!(m.cpu.halted);
                assert!(m.error.is_none(), "{:?}", m.error);
            }
        }
    }
}

#[test]
fn mode0_rewrite_reset_clears_partial_pending_latches_and_fraction() {
    for profile in PROFILES {
        for base in [0x71, 0x3fd9] {
            for partial in [false, true] {
                let mut m = Machine::new(profile);
                for channel in 0..3 {
                    m.bus.output(base + 6, (channel << 6) | 0x30);
                    m.bus.output(base + 2 * channel as u16, 4);
                    m.bus.output(base + 2 * channel as u16, 0);
                }
                m.bus.tick(25); // OUT high, latched IRQ and fractional remainder
                for channel in 0..3 {
                    let port = base + 2 * channel as u16;
                    m.bus.output(base + 6, (channel as u8) << 6);
                    m.bus.input(port); // leave half-consumed snapshot
                    m.bus.output(port, 2);
                    if !partial {
                        m.bus.output(port, 0);
                    }
                    assert!(!m.bus.pit.c[channel].output);
                }
                assert_eq!(m.bus.pics[0].irr & 1, 1);
                m.reset();
                assert_eq!(m.bus.pit, Pit::default());
                assert_eq!(m.bus.pics[0].irr, 0);
                assert_eq!(m.bus.pics[0].isr, 0);
                for channel in 0..3 {
                    m.bus.output(base + 6, (channel << 6) | 0x30);
                    m.bus.output(base + 2 * channel as u16, 1);
                    m.bus.output(base + 2 * channel as u16, 0);
                }
                m.bus.tick(5);
                for channel in 0..3 {
                    assert_eq!(
                        (m.bus.pit.c[channel].remaining, m.bus.pit.c[channel].output),
                        (1, false)
                    );
                }
                m.bus.tick(4);
                assert_eq!(m.bus.pics[0].irr & 1, 1);
                for channel in 0..3 {
                    assert!(m.bus.pit.c[channel].output);
                }
            }
        }
    }
}

#[test]
fn mode0_rewrite_original_boot_probe_before_and_after_reset_in_both_profiles() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/pit_mode0_rewrite/pit_mode0_rewrite.hdm"
    ))
    .expect("run python3 tools/prepare_pit_mode0_rewrite_fixture.py");
    let manifest = include_str!("../fixtures/pit-mode0-rewrite-manifest.json");
    let hex = manifest
        .split_once("\"result_hex\": \"")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap();
    let expected: Vec<u8> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(expected.len(), 32);
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "pit_mode0_rewrite.hdm").unwrap();
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
                    "{profile:?} boot {run}: {slots} slots; {text}; {:02x?}",
                    &m.bus.mem[0x8000..0x8020]
                );
                if text.contains("PIT MODE0 REWRITE PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(
                m.bus.text().contains("PIT MODE0 REWRITE PASS"),
                "{}",
                m.bus.text()
            );
            assert_eq!(&m.bus.mem[0x8000..0x8020], expected.as_slice());
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            println!("{profile:?} boot {run}: {slots} slots");
        }
    }
}

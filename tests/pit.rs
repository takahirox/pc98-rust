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

use pc98_rust::{bus::Bus, devices::Pic, Machine, Profile};

const PROFILES: [Profile; 2] = [Profile::Pc9801Vm, Profile::Pc9821ApDos];

fn pic(aeoi: bool) -> Pic {
    let mut p = Pic::new(8);
    p.write(false, 0x11);
    p.write(true, 8);
    p.write(true, 0x80);
    p.write(true, if aeoi { 3 } else { 1 });
    p.write(true, 0);
    p
}

fn status(p: &mut Pic, irr: u8, isr: u8, mask: u8) {
    p.write(false, 0x0a);
    assert_eq!(p.read(false), irr, "IRR");
    p.write(false, 0x0b);
    assert_eq!(p.read(false), isr, "ISR");
    assert_eq!(p.read(true), mask, "IMR");
}

#[test]
fn set_priority_minimal_reproduction() {
    let mut p = Pic::new(8);
    p.write(true, 0);
    p.write(false, 0xc0);
    p.request(0);
    p.request(1);
    assert_eq!(p.ack(), Some(9));
}

#[test]
fn every_programmed_lowest_line_wraps_and_retains_masked_requests() {
    for lowest in 0..8 {
        let mut p = pic(false);
        p.write(false, 0xc0 | lowest);
        for n in 0..8 {
            p.request(n);
        }
        let first = (lowest + 1) & 7;
        p.write(true, 1 << first);
        status(&mut p, 255, 0, 1 << first);
        assert_eq!(p.pending(), Some((first + 1) & 7));
        p.write(true, 0);
        let mut irr = 255;
        for offset in 1..=8 {
            let n = (lowest + offset) & 7;
            assert_eq!(p.pending(), Some(n));
            assert_eq!(p.ack(), Some(8 + n));
            irr &= !(1 << n);
            status(&mut p, irr, 1 << n, 0);
            assert_eq!(p.ack(), None); // every remaining line is lower
            p.write(false, 0x20);
            status(&mut p, irr, 0, 0);
        }
        assert_eq!(p.ack(), None);
    }
}

#[test]
fn nested_isr_and_nonspecific_eoi_follow_reprogrammed_priority_even_when_masked() {
    let mut p = pic(false);
    p.write(false, 0xc3); // 4,5,6,7,0,1,2,3
    for n in [1, 6, 4] {
        p.request(n);
        assert_eq!(p.ack(), Some(8 + n));
    }
    p.request(5);
    p.request(4);
    p.write(true, 1 << 4);
    status(&mut p, 0x30, 0x52, 0x10);
    assert_eq!(p.pending(), None); // masking ISR4 cannot bypass it
    p.write(false, 0xc5); // 6 highest: set priority must preserve ISR
    status(&mut p, 0x30, 0x52, 0x10);
    p.write(false, 0x20); // clears ISR6, not numerically lowest ISR1
    status(&mut p, 0x30, 0x12, 0x10);
    assert_eq!(p.pending(), None); // ISR1 still above request5
    p.write(false, 0x20);
    status(&mut p, 0x30, 0x10, 0x10);
    assert_eq!(p.pending(), None); // masked ISR4 still above request5
    p.write(false, 0x64);
    assert_eq!(p.pending(), Some(5));
    p.write(true, 0);
    assert_eq!(p.pending(), Some(4));
}

#[test]
fn rotating_eois_and_ordinary_eois_have_distinct_effects() {
    for command in [0x20, 0x60, 0xa0, 0xe0] {
        let mut p = pic(false);
        p.request(0);
        assert_eq!(p.ack(), Some(8));
        p.request(0);
        p.request(1);
        p.write(false, command);
        status(&mut p, 3, 0, 0);
        assert_eq!(p.pending(), Some(if command & 0x80 != 0 { 1 } else { 0 }));
    }
    let mut p = pic(false);
    p.request(5);
    assert_eq!(p.ack(), Some(13));
    p.request(2);
    assert_eq!(p.ack(), Some(10));
    p.write(false, 0xe5); // specific rotation must clear 5, preserving 2
    status(&mut p, 0, 4, 0);
    p.request(7);
    p.request(1);
    assert_eq!(p.pending(), Some(7));
    p.write(false, 0xa0); // reordered highest ISR is 2; make it lowest
    status(&mut p, 0x82, 0, 0);
    assert_eq!(p.pending(), Some(7));
}

#[test]
fn no_op_and_empty_isr_commands_preserve_requests_masks_and_priority() {
    let mut p = pic(false);
    p.write(false, 0xc4);
    p.write(true, 2);
    for n in [0, 1, 6] {
        p.request(n);
    }
    for command in [0x40, 0x47, 0x20, 0xa0, 0x63, 0x00, 0x80] {
        p.write(false, command);
        status(&mut p, 0x43, 0, 2);
        assert_eq!(p.pending(), Some(6));
    }
    assert_eq!(p.ack(), Some(14));
    for command in [0x40, 0x47] {
        p.write(false, command);
        status(&mut p, 3, 0x40, 2);
    }
    p.write(false, 0x66);
    p.write(false, 0xe7); // even absent ISR7, explicit line becomes lowest
    status(&mut p, 3, 0, 2);
    assert_eq!(p.pending(), Some(0));
    p.write(true, 255);
    assert_eq!(p.ack(), None);
    status(&mut p, 3, 0, 255);
}

#[test]
fn aeoi_rotation_only_on_successful_ack_and_disable_preserves_order() {
    let mut p = pic(true);
    p.write(false, 0x80);
    for round in 0..3 {
        for n in 0..8 {
            p.request(n);
        }
        for n in 0..8 {
            for _ in 0..3 {
                assert_eq!(p.pending(), Some(n), "round {round}");
                status(&mut p, 255u8 << n, 0, 0);
            }
            assert_eq!(p.ack(), Some(8 + n));
            assert_eq!(p.isr, 0);
        }
        assert_eq!(p.ack(), None);
    }
    p.request(0);
    assert_eq!(p.ack(), Some(8)); // IR0 becomes lowest
    p.write(false, 0x00);
    for _ in 0..3 {
        p.request(0);
        p.request(1);
        assert_eq!(p.ack(), Some(9)); // disabling does not restore IR0 highest
        p.request(1);
        assert_eq!(p.ack(), Some(9)); // no further rotation
        status(&mut p, 1, 0, 0);
    }
    assert_eq!(p.ack(), Some(8));
}

#[test]
fn rotation_enable_without_aeoi_does_not_rotate_on_ack_or_ordinary_eoi() {
    let mut p = pic(false);
    p.write(false, 0x80);
    for _ in 0..3 {
        p.request(0);
        p.request(1);
        assert_eq!(p.ack(), Some(8));
        status(&mut p, 2, 1, 0);
        p.write(false, 0x20);
        assert_eq!(p.pending(), Some(1));
    }
}

#[test]
fn icw_initialization_restores_priority_and_disables_automatic_rotation() {
    for icw4 in [false, true] {
        let mut p = pic(true);
        p.write(false, 0x80);
        p.write(false, 0xc3);
        p.write(false, if icw4 { 0x11 } else { 0x10 });
        status(&mut p, 0, 0, 0);
        p.write(true, 0x20);
        p.write(true, 0x80);
        if icw4 {
            p.write(true, 3); // AEOI on again, rotation remains disabled
        }
        assert_eq!(p.auto_eoi, icw4);
        for _ in 0..2 {
            p.request(0);
            p.request(1);
            assert_eq!(p.ack(), Some(0x20));
            p.write(false, 0x20);
        }
    }
}

#[test]
fn machine_reset_and_controller_instances_start_with_independent_fixed_priority() {
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        let mut other = Machine::new(profile);
        for p in &mut m.bus.pics {
            *p = pic(true);
            p.write(false, 0x80);
            p.write(false, 0xc0);
        }
        for p in &mut other.bus.pics {
            p.write(true, 0);
            p.request(0);
            p.request(1);
            assert_eq!(p.ack(), Some(p.base));
        }
        m.reset();
        for p in &mut m.bus.pics {
            assert!(!p.auto_eoi);
            p.write(true, 0);
            p.request(0);
            p.request(1);
            assert_eq!(p.ack(), Some(p.base));
            p.write(false, 0x20);
            // Re-enable AEOI through ICW4 is covered separately. Construction
            // rotation defaults are observed here without reinitializing state.
            p.auto_eoi = true;
            p.request(0);
            assert_eq!(p.ack(), Some(p.base));
            p.request(0);
            assert_eq!(p.ack(), Some(p.base));
        }
    }
}

#[test]
fn bus_ports_independent_priorities_cascade_and_separate_eois_in_both_profiles() {
    for profile in PROFILES {
        for slave_first in [false, true] {
            let mut b = Bus::new(profile);
            b.output(2, 0);
            b.output(10, 0);
            b.output(0, 0xc6); // master IR7 highest
            b.output(8, 0xc3); // slave IR4 highest
            b.pics[0].request(0);
            b.pics[1].request(2);
            b.pics[1].request(4);
            assert_eq!(b.irq(), Some(0x14));
            b.output(0, 0x0b);
            b.output(8, 0x0b);
            assert_eq!(b.input(0), 0x80);
            assert_eq!(b.input(8), 0x10);
            assert_eq!(b.input(2), 0);
            assert_eq!(b.input(10), 0);
            assert_eq!(b.irq(), None); // master0 and slave2 deferred
            if slave_first {
                b.output(8, 0xa0); // slave4 lowest; master still in service
                assert_eq!(b.irq(), None);
                assert_eq!(b.input(0), 0x80);
                assert_eq!(b.input(8), 0);
                b.output(0, 0x20);
            } else {
                b.output(0, 0x20);
                assert_eq!(b.irq(), Some(8)); // slave still blocked by ISR4
                assert_eq!(b.input(8), 0x10);
                b.output(8, 0xa0);
            }
            assert_eq!(b.irq(), Some(0x12)); // master7 can nest above master0
            assert_eq!(b.input(0), if slave_first { 0x80 } else { 0x81 });
            assert_eq!(b.input(8), 4);
            b.output(8, 0x62);
            assert_eq!(b.irq(), None); // master EOI still needed
            b.output(0, 0xe7); // rotate master; slave order unaffected
            if slave_first {
                assert_eq!(b.irq(), Some(8));
            }
            b.output(0, 0x20);
            assert_eq!(b.irq(), None);
            for port in [0, 8] {
                b.output(port, 0x0a);
                assert_eq!(b.input(port), 0);
                b.output(port, 0x0b);
                assert_eq!(b.input(port), 0);
            }
            // Slave priority did not change when rotating the master.
            b.pics[1].request(0);
            b.pics[1].request(5);
            assert_eq!(b.irq(), Some(0x15));
        }
    }
}

#[test]
fn original_hardware_irq_boot_probe_before_and_after_reset_in_both_profiles() {
    let image = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/pic/pic.hdm"))
        .expect("run python3 tools/prepare_pic_fixture.py");
    let expected = [
        0xa5, 4, 6, 0xa5, 0x1c, 0x9c, 0x1c, 0xa5, 1, 0, 0, 1, 1, 0, 0xa5, 0xa5, 3, 3, 3, 0xa5, 2,
        1, 1, 2, 2, 1, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    ];
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "pic.hdm").unwrap();
        for run in 0..2 {
            if run != 0 {
                m.reset();
            }
            let mut slots = 0;
            let mut next_phase = 1;
            while slots < 2_000_000 {
                slots += m.step(1000);
                assert!(m.error.is_none(), "{:?}", m.error);
                let phase = m.bus.read(0x8001);
                if phase == next_phase && phase <= 3 {
                    m.bus.key(0x1c, phase != 2); // existing physical input only
                    next_phase += 1;
                }
                let text = m.bus.text();
                assert!(!text.contains("FAIL"), "{text}");
                if text.contains("PIC PASS") {
                    break;
                }
            }
            assert_eq!(next_phase, 4);
            assert!(slots <= 2_000_000);
            assert!(
                m.bus.text().contains("PIC PASS"),
                "{} phase={} count={} ip={:x}",
                m.bus.text(),
                m.bus.read(0x8001),
                m.bus.read(0x8002),
                m.cpu.ip
            );
            assert_eq!(&m.bus.mem[0x8000..0x8020], &expected);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            m.bus.key(0x1c, false);
        }
    }
}

#[test]
fn bus_aeoi_rotation_is_local_to_each_controller_and_status_reads_are_passive() {
    for profile in PROFILES {
        let mut b = Bus::new(profile);
        for (command, data, base, cascade) in [(0, 2, 8, 0x80), (8, 10, 0x10, 7)] {
            b.output(command, 0x11);
            b.output(data, base);
            b.output(data, cascade);
            b.output(data, 3);
            b.output(data, 0);
        }
        b.output(0, 0xc6); // cascade first
        b.output(8, 0xc0); // slave1 first
        b.output(8, 0x80); // rotation only on slave
        b.pics[0].request(0);
        b.pics[1].request(0);
        b.pics[1].request(1);
        for _ in 0..4 {
            b.output(8, 0x0a);
            assert_eq!(b.input(8), 3);
            assert_eq!(b.pics[1].pending(), Some(1));
        }
        assert_eq!(b.irq(), Some(0x11));
        b.pics[1].request(1);
        assert_eq!(b.irq(), Some(0x10)); // slave rotated, master did not
        b.pics[1].request(0);
        b.output(0, 0x80); // now master also rotates at successful ack
        assert_eq!(b.irq(), Some(0x11)); // master IR7 becomes lowest
        b.pics[1].request(1);
        assert_eq!(b.irq(), Some(8)); // pending master IR0 now beats cascade
        assert_eq!(b.irq(), Some(0x10));
        for port in [0, 8] {
            b.output(port, 0x0b);
            assert_eq!(b.input(port), 0); // both AEOI, no ISR remains
        }
        b.output(8, 0x00); // disable slave rotation, preserving IR0 lowest
        b.pics[1].request(0);
        for _ in 0..2 {
            b.pics[1].request(1);
            assert_eq!(b.irq(), Some(0x11));
        }
        b.output(8, 0x0a);
        assert_eq!(b.input(8), 1); // deferred slave0 retained
    }
}

#[test]
fn failed_aeoi_acknowledgements_do_not_rotate_or_discard_masked_requests() {
    let mut p = pic(true);
    p.write(false, 0x80);
    p.write(false, 0xc3); // 4 highest, 3 lowest
    p.write(true, 255);
    p.request(1);
    p.request(6);
    for _ in 0..3 {
        assert_eq!(p.ack(), None);
        status(&mut p, 0x42, 0, 255);
    }
    p.write(true, 0);
    assert_eq!(p.ack(), Some(14)); // no failed ack changed the order
    assert_eq!(p.ack(), Some(9));
    status(&mut p, 0, 0, 0);
}

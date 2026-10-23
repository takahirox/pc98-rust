use pc98_rust::{bus::Bus, devices::Pic, Machine, Profile};

const PROFILES: [Profile; 2] = [Profile::Pc9801Vm, Profile::Pc9821ApDos];

#[test]
fn smm_minimal_reproduction() {
    let mut p = pc98_rust::devices::Pic::new(8);
    p.write(true, 0);
    p.request(0);
    assert_eq!(p.ack(), Some(8));
    p.write(true, 1);
    p.request(1);
    assert_eq!(p.pending(), None);
    p.write(false, 0x68);
    assert_eq!(p.pending(), Some(1));
    assert_eq!(p.ack(), Some(9));
    p.write(false, 0x20);
    assert_eq!(p.isr, 1);
}

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
fn smm_arbitration_and_mask_transitions_at_every_priority_origin() {
    for lowest in 0..8 {
        let mut p = pic(false);
        p.write(false, 0xc0 | lowest);
        let [a, b, c, d] = [1, 2, 3, 4].map(|offset| (lowest + offset) & 7);
        let bits = |lines: &[u8]| lines.iter().fold(0, |mask, n| mask | (1 << n));
        for n in [c, a] {
            p.request(n);
            assert_eq!(p.ack(), Some(8 + n));
        }
        for n in [a, b, c, d] {
            p.request(n);
        }
        p.write(true, 1 << a);
        let irr = bits(&[a, b, c, d]);
        status(&mut p, irr, bits(&[a, c]), 1 << a);
        assert_eq!(p.ack(), None);
        p.write(false, 0x68);
        status(&mut p, irr, bits(&[a, c]), 1 << a);
        assert_eq!(p.ack(), Some(8 + b));
        status(&mut p, bits(&[a, c, d]), bits(&[a, b, c]), 1 << a);
        assert_eq!(p.pending(), None); // unmasked ISR b still blocks
        p.write(false, 0x20);
        status(&mut p, bits(&[a, c, d]), bits(&[a, c]), 1 << a);
        assert_eq!(p.pending(), None); // unmasked ISR c blocks itself and d
        p.write(true, bits(&[a, c]));
        assert_eq!(p.pending(), Some(d));
        p.write(false, 0x48);
        status(&mut p, bits(&[a, c, d]), bits(&[a, c]), bits(&[a, c]));
        assert_eq!(p.pending(), None);
        p.write(false, 0x68);
        assert_eq!(p.pending(), Some(d));
        p.write(true, 1 << c); // unmasking an ISR restores its blocking
        assert_eq!(p.pending(), None);
        p.write(true, bits(&[a, c]));
        assert_eq!(p.ack(), Some(8 + d));
        status(&mut p, bits(&[a, c]), bits(&[a, c, d]), bits(&[a, c]));
        p.write(false, 0x20);
        p.write(false, 0x60 | a);
        p.write(false, 0x60 | c);
        status(&mut p, bits(&[a, c]), 0, bits(&[a, c]));
        assert_eq!(p.ack(), None); // masks still inhibit requests, retaining IRR
        p.write(true, 0);
        assert_eq!(p.ack(), Some(8 + a));
        status(&mut p, 1 << c, 1 << a, 0);
    }
}

#[test]
fn smm_eoi_selection_rotation_and_empty_eligible_sets_at_every_origin() {
    for lowest in 0..8 {
        let [a, b, c, d, e, f] = [1, 2, 3, 4, 5, 6].map(|n| (lowest + n) & 7);
        let service = (1 << a) | (1 << c) | (1 << e);
        let mask = (1 << a) | (1 << e);
        for command in [0x20, 0xa0, 0x60 | e, 0xe0 | e] {
            let mut p = pic(false);
            p.write(false, 0xc0 | lowest);
            for n in [e, c, a] {
                p.request(n);
                assert_eq!(p.ack(), Some(8 + n));
            }
            for n in 0..8 {
                p.request(n);
            }
            p.write(true, mask);
            p.write(false, 0x68);
            let specific = command & 0x40 != 0;
            p.write(false, command);
            status(
                &mut p,
                255,
                service & !(1 << if specific { e } else { c }),
                mask,
            );
            let next = match command {
                0xa0 => d,
                v if v & 0xe0 == 0xe0 => f,
                _ => b,
            };
            assert_eq!(p.ack(), Some(8 + next));
            status(
                &mut p,
                !(1 << next),
                (service & !(1 << if specific { e } else { c })) | (1 << next),
                mask,
            );
        }
        for command in [0x20, 0xa0] {
            let mut p = pic(false);
            p.write(false, 0xc0 | lowest);
            for n in [e, c, a] {
                p.request(n);
                assert_eq!(p.ack(), Some(8 + n));
            }
            p.write(true, service);
            p.write(false, 0x68);
            for n in 0..8 {
                p.request(n);
            }
            for _ in 0..3 {
                p.write(false, command);
                status(&mut p, 255, service, service);
                assert_eq!(p.pending(), Some(b));
            }
            // Unmask after nonrotating specific EOIs so even rotation to a
            // masked line would be visible in the next delivered vector.
            let mut order_check = p.clone();
            for n in [a, c, e] {
                order_check.write(false, 0x60 | n);
            }
            order_check.write(true, 0);
            status(&mut order_check, 255, 0, 0);
            assert_eq!(order_check.ack(), Some(8 + a));
            p.write(false, 0x48);
            assert_eq!(p.pending(), None);
            p.write(false, 0x20); // normal mode again clears masked ISR a
            status(&mut p, 255, service & !(1 << a), service);
            assert_eq!(p.ack(), Some(8 + b));
        }
    }
}

#[test]
fn smm_combined_status_commands_and_esmm_qualification_are_independent() {
    let mut p = pic(false);
    p.write(false, 0xc3); // 4 highest
    p.request(4);
    assert_eq!(p.ack(), Some(12));
    p.write(true, 0x10);
    p.request(4);
    p.request(5);
    p.request(0);
    // command, selected register value, resulting arbitration
    for (command, read, pending) in [
        (0x2a, 0x31, None), // ESMM=0 cannot enable SMM
        (0x6b, 0x10, Some(5)),
        (0x08, 0x10, Some(5)), // RR=0 preserves status, ESMM=0 preserves mode
        (0x28, 0x10, Some(5)),
        (0x6a, 0x31, Some(5)),
        (0x4b, 0x10, None),
        (0x2b, 0x10, None),
        (0x68, 0x10, Some(5)), // mode-only command preserves ISR selection
        (0x0a, 0x31, Some(5)), // status-only command cannot disable SMM
        (0x4a, 0x31, None),
        (0x48, 0x31, None),
    ] {
        p.write(false, command);
        for _ in 0..3 {
            assert_eq!(p.read(false), read);
            assert_eq!(p.read(true), 0x10);
            assert_eq!((p.irr, p.isr, p.mask), (0x31, 0x10, 0x10));
            assert_eq!(p.pending(), pending);
        }
    }
    p.write(false, 0x68);
    assert_eq!(p.ack(), Some(13)); // priority survived every transition/read
}

fn assert_normal_mode(p: &mut Pic) {
    p.write(true, 0);
    p.request(0);
    assert_eq!(p.ack(), Some(p.base));
    p.write(true, 1);
    p.request(1);
    status(p, 2, 1, 1);
    assert_eq!(p.pending(), None);
    p.write(false, 0x20); // normal nonspecific EOI clears masked ISR
    status(p, 2, 0, 1);
    assert_eq!(p.ack(), Some(p.base + 1));
}

#[test]
fn smm_construction_icw1_machine_reset_and_instance_isolation() {
    assert_normal_mode(&mut Pic::new(8));
    for icw4 in [false, true] {
        let mut p = pic(false);
        p.write(false, 0x68);
        p.write(false, if icw4 { 0x11 } else { 0x10 });
        status(&mut p, 0, 0, 0);
        p.write(true, 8);
        p.write(true, 0x80);
        if icw4 {
            p.write(true, 1);
        }
        assert_normal_mode(&mut p);
    }
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        let mut other = Machine::new(profile);
        for p in &mut m.bus.pics {
            p.write(false, 0x68);
        }
        for p in &mut other.bus.pics {
            assert_normal_mode(p);
        }
        m.reset();
        for p in &mut m.bus.pics {
            status(p, 0, 0, 255);
            assert_normal_mode(p);
        }
    }
}

#[test]
fn smm_master_slave_ports_and_cascade_keep_modes_and_eois_independent() {
    for profile in PROFILES {
        let mut b = Bus::new(profile);
        b.output(2, 0x7f); // only cascade IR7
        b.output(10, 0);
        b.pics[1].request(0);
        assert_eq!(b.irq(), Some(0x10));
        b.output(10, 1); // slave ISR0 masked
        b.pics[1].request(1);
        assert_eq!(b.irq(), None);
        b.output(0, 0x68); // enabling master does not enable slave
        assert_eq!(b.pics[1].pending(), None);
        b.output(8, 0x6b);
        assert_eq!(b.pics[1].pending(), Some(1));
        assert_eq!(b.irq(), None); // unmasked master ISR7 still blocks cascade
        b.output(0, 0x20);
        assert_eq!(b.irq(), Some(0x11));
        for (port, value) in [(0, 0x80), (8, 3), (2, 0x7f), (10, 1)] {
            if port == 0 {
                b.output(port, 0x0b);
            }
            assert_eq!(b.input(port), value);
        }
        b.output(8, 0x20); // skip masked slave ISR0, clear ISR1
        assert_eq!(b.input(8), 1);
        b.pics[1].request(2);
        assert_eq!(b.irq(), None); // slave EOI did not complete master ISR7
        b.output(0, 0x20);
        assert_eq!(b.irq(), Some(0x12));
        b.output(8, 0x20);
        b.output(0, 0x20);
        b.output(8, 0x4b); // disable slave; master remains special-mask
        b.pics[1].request(3);
        assert_eq!(b.irq(), None);
        b.pics[0].request(0);
        b.output(2, 0x7e);
        assert_eq!(b.irq(), Some(8));
        b.output(2, 0x7f); // mask master ISR0
        b.pics[0].request(1);
        b.output(2, 0x7d); // unmask IR1, retain mask on IR0
        assert_eq!(b.irq(), Some(9)); // master mode survived slave disable
        b.output(0, 0x20);
        b.output(0, 0x60);
        b.output(8, 0x60); // specific EOI can clear masked slave ISR0
        assert_eq!(b.irq(), Some(0x13));
        b.output(8, 0x20);
        b.output(0, 0x20);
        for port in [0, 8] {
            b.output(port, 0x0a);
            assert_eq!(b.input(port), 0);
            b.output(port, 0x0b);
            assert_eq!(b.input(port), 0);
        }
        assert_eq!(b.input(2), 0x7d);
        assert_eq!(b.input(10), 1);
    }
}

#[test]
fn smm_hardware_irq_nesting_probe_before_and_after_reset_in_both_profiles() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/pic_smm/pic_smm.hdm"
    ))
    .expect("run python3 tools/prepare_pic_smm_fixture.py");
    let expected = [
        0xa5, 2, 3, 1, 0x1c, 3, 0, 1, 1, 0, 1, 0xa5, 2, 3, 2, 0xa5, 1, 0xfe, 0, 0, 0xa5, 0xa5,
        0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    ];
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "pic_smm.hdm").unwrap();
        for run in 0..2 {
            if run != 0 {
                m.reset();
            }
            let mut slots = 0;
            let mut sent_key = false;
            while slots < 2_000_000 {
                slots += m.step(1000);
                assert!(m.error.is_none(), "{:?}", m.error);
                if m.bus.read(0x8001) == 1 && !sent_key {
                    m.bus.key(0x1c, true); // physical input; guest owns all results
                    sent_key = true;
                }
                let text = m.bus.text();
                assert!(!text.contains("FAIL"), "{text}");
                if text.contains("PIC SMM PASS") {
                    break;
                }
            }
            assert!(sent_key);
            assert!(slots <= 2_000_000);
            assert!(
                m.bus.text().contains("PIC SMM PASS"),
                "{} phase={} count={} timer={} ip={:x}",
                m.bus.text(),
                m.bus.read(0x8001),
                m.bus.read(0x8002),
                m.bus.read(0x8003),
                m.cpu.ip
            );
            assert_eq!(&m.bus.mem[0x8000..0x8020], &expected);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            m.bus.key(0x1c, false);
        }
    }
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

#[test]
fn poll_minimal_reproduction() {
    let mut p = Pic::new(8);
    p.write(true, 0);
    p.request(1);
    p.write(false, 0x0c);
    assert_eq!(p.read(false), 0x81);
    assert_eq!((p.irr, p.isr), (0, 2));
}

fn poll(p: &mut Pic) -> u8 {
    p.write(false, 0x0c);
    p.read(false)
}

#[test]
fn polling_every_level_and_priority_origin_with_exact_eoi_effects() {
    for base in [8, 0x70] {
        for lowest in 0..8 {
            let mut p = pic(false);
            p.base = base;
            p.write(false, 0xc0 | lowest);
            for n in 0..8 {
                p.request(n);
            }
            let mut irr = 255;
            for offset in 1..=8 {
                let n = (lowest + offset) & 7;
                assert_eq!(poll(&mut p), 0x80 | n);
                irr &= !(1 << n);
                status(&mut p, irr, 1 << n, 0);
                assert_eq!(poll(&mut p), 0); // ISR blocks every lower request
                status(&mut p, irr, 1 << n, 0);
                p.write(false, if offset & 1 == 0 { 0x60 | n } else { 0x20 });
                status(&mut p, irr, 0, 0);
            }
            assert_eq!(poll(&mut p), 0);
            status(&mut p, 0, 0, 0);
        }
    }
}

#[test]
fn polling_masks_smm_and_combined_commands_preserve_passive_status() {
    for lowest in 0..8 {
        let a = (lowest + 1) & 7;
        let b = (lowest + 2) & 7;
        let mut p = pic(false);
        p.write(false, 0xc0 | lowest);
        p.request(a);
        p.write(true, 1 << a);
        assert_eq!(poll(&mut p), 0);
        status(&mut p, 1 << a, 0, 1 << a);
        p.write(true, 0);
        assert_eq!(poll(&mut p), 0x80 | a);
        p.request(b);
        p.write(true, 1 << a);
        assert_eq!(poll(&mut p), 0); // normal mode masked ISR still blocks
        p.write(false, 0x6f); // SMM + P + RR + RIS
        assert_eq!(p.read(false), 0x80 | b);
        for _ in 0..3 {
            assert_eq!(p.read(false), (1 << a) | (1 << b));
            assert_eq!(p.read(true), 1 << a);
        }
        p.write(false, 0x20); // SMM skips masked ISR a
        status(&mut p, 0, 1 << a, 1 << a);
        p.request(b);
        p.write(false, 0x4e); // disable SMM, poll IRR: frozen empty
        assert_eq!(p.read(false), 0);
        assert_eq!(p.read(false), 1 << b);
        p.write(false, 0x6e); // enable SMM, combined IRR poll
        assert_eq!(p.read(false), 0x80 | b);
        assert_eq!(p.read(false), 0);
        p.write(false, 0x20);
        p.write(false, 0x60 | a);
        status(&mut p, 0, 0, 1 << a);
    }
}

#[test]
fn polling_freezes_selection_and_empty_transactions_retaining_later_requests() {
    for command in [0x0c, 0x0e, 0x0f] {
        for data_read in [false, true] {
            let mut p = pic(false);
            p.request(5);
            p.write(false, command);
            p.request(0);
            assert_eq!(p.pending(), Some(0)); // poll does not suppress INT
            assert_eq!((p.irr, p.isr), (0x21, 0));
            assert_eq!(p.read(data_read), 0x85);
            status(&mut p, 1, 0x20, 0);
            assert_eq!(poll(&mut p), 0x80);
            p.write(false, 0x20);
            p.write(false, 0x20);
            p.write(false, command); // initially empty
            p.request(1);
            assert_eq!(p.read(data_read), 0);
            status(&mut p, 2, 0, 0);
            assert_eq!(poll(&mut p), 0x81);
            p.write(false, 0x20);
            p.request(2);
            p.write(false, command);
            p.request(2); // a later request at the frozen level is retained too
            assert_eq!(p.read(data_read), 0x82);
            status(&mut p, 4, 4, 0);
            assert_eq!(poll(&mut p), 0);
            p.write(false, 0x20);
            assert_eq!(poll(&mut p), 0x82);
        }
    }
}

#[test]
fn polling_isolation_initialization_and_machine_reset_cancel_transactions() {
    for icw1 in [0x10, 0x11] {
        let mut p = pic(false);
        p.request(1);
        p.write(false, 0x0f);
        p.write(false, icw1);
        p.request(2);
        assert_eq!(p.read(false), 4); // reset selects IRR, cancels acknowledgement
        assert_eq!(p.isr, 0);
    }
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        let mut other = Machine::new(profile);
        for p in &mut m.bus.pics {
            p.write(true, 0);
            p.request(2);
            p.write(false, 0x0c);
        }
        for p in &mut other.bus.pics {
            p.request(3);
            assert_eq!(p.read(false), 8);
            assert_eq!(p.isr, 0);
        }
        assert_eq!(m.bus.pics[0].read(false), 0x82);
        assert_eq!(m.bus.pics[1].isr, 0);
        m.bus.pics[0].write(false, 0x0c); // frozen empty also canceled
        m.reset();
        for p in &mut m.bus.pics {
            p.request(4);
            assert_eq!(p.read(false), 16);
            assert_eq!(p.isr, 0);
        }
    }
}

fn bus_poll(b: &mut Bus, port: u16) -> u8 {
    b.output(port, 0x0c);
    b.input(port)
}

#[test]
fn polling_cascade_with_if_clear_separate_acknowledgements_and_both_eoi_orders() {
    for profile in PROFILES {
        for slave_first in [false, true] {
            let mut m = Machine::new(profile);
            m.cpu.flags &= !0x200;
            let b = &mut m.bus;
            b.output(2, 0x7f);
            b.output(10, 0);
            b.pics[1].request(3);
            b.pics[1].request(4);
            b.tick(1); // no CPU acknowledgement
            assert_eq!(b.pics[0].irr, 0x80);
            assert_eq!(bus_poll(b, 0), 0x87);
            assert_eq!((b.pics[1].irr, b.pics[1].isr), (0x18, 0));
            for _ in 0..3 {
                b.tick(1);
            }
            assert_eq!(b.pics[0].irr, 0); // no repeated edge before slave read
            assert_eq!(bus_poll(b, 8), 0x83);
            assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 8));
            assert_eq!(bus_poll(b, 0), 0);
            assert_eq!(bus_poll(b, 8), 0);
            let first = if slave_first { 8 } else { 0 };
            let second = 8 - first;
            b.output(first, 0x20);
            assert_eq!(bus_poll(b, 0), 0);
            // The deferred slave request survives either EOI ordering.
            b.output(second, 0x20);
            assert_eq!(bus_poll(b, 0), 0x87);
            assert_eq!(b.pics[1].isr, 0);
            assert_eq!(bus_poll(b, 8), 0x84);
            b.output(first, 0x20);
            b.output(second, 0x20);
            for _ in 0..4 {
                b.tick(100);
                assert_eq!(bus_poll(b, 0), 0);
                assert_eq!(bus_poll(b, 8), 0);
                assert_eq!(b.irq(), None);
                assert_eq!(
                    (b.pics[0].irr, b.pics[0].isr, b.pics[1].irr, b.pics[1].isr),
                    (0, 0, 0, 0)
                );
            }
        }
    }
}

#[test]
fn polling_hardware_boot_probe_before_and_after_reset_in_both_profiles() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/pic_poll/pic_poll.hdm"
    ))
    .expect("run python3 tools/prepare_pic_poll_fixture.py");
    let expected = [
        0xa5, 0x80, 1, 0, 0x87, 0, 0x80, 0x83, 8, 0x80, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 1, 3,
        0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    ];
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "pic_poll.hdm").unwrap();
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
                    "{text}; results={:02x?}",
                    &m.bus.mem[0x8000..0x8020]
                );
                if text.contains("PIC POLL PASS") {
                    break;
                }
            }
            assert!(slots <= 2_000_000);
            assert!(
                m.bus.text().contains("PIC POLL PASS"),
                "ip={:x} results={:02x?}",
                m.cpu.ip,
                &m.bus.mem[0x8000..0x8020]
            );
            assert_eq!(&m.bus.mem[0x8000..0x8020], &expected);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        }
    }
}

#[test]
fn polling_slave_freeze_retains_later_higher_priority_cascade_work() {
    for profile in PROFILES {
        let mut b = Bus::new(profile);
        b.output(2, 0x7f);
        b.output(10, 0);
        b.pics[1].request(5);
        b.tick(1);
        assert_eq!(bus_poll(&mut b, 0), 0x87);
        b.output(8, 0x0c); // freeze slave5
        b.pics[1].request(0); // higher priority, after OUT and before IN
        b.tick(1);
        assert_eq!(b.input(8), 0x85);
        assert_eq!((b.pics[1].irr, b.pics[1].isr), (1, 0x20));
        assert_eq!(b.pics[0].irr, 0x80); // new INT after slave acknowledgement
        assert_eq!(bus_poll(&mut b, 0), 0); // master ISR7 still blocks
        b.output(0, 0x20);
        assert_eq!(bus_poll(&mut b, 0), 0x87);
        assert_eq!(bus_poll(&mut b, 8), 0x80);
        b.output(8, 0x20);
        b.output(8, 0x20);
        b.output(0, 0x20);
        b.tick(1);
        assert_eq!(bus_poll(&mut b, 0), 0);
        assert_eq!(bus_poll(&mut b, 8), 0);
        assert_eq!(b.irq(), None);
    }
}

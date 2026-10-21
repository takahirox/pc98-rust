use pc98_rust::{cpu::*, Machine, Profile};

const PROFILES: [Profile; 2] = [Profile::Pc9801Vm, Profile::Pc9821ApDos];

fn run(m: &mut Machine, slots: u32) {
    assert_eq!(m.step(slots), slots);
    assert!(m.error.is_none(), "{:?}", m.error);
}

fn machine(profile: Profile) -> Machine {
    let mut m = Machine::new(profile);
    m.cpu.s = [0; 4];
    m.cpu.r[SP] = 0x9000;
    m.cpu.ip = 0x1000;
    m.cpu.flags = IF | 2;
    m.bus.pics[0].mask = 0xfd; // Only IRQ1, through the actual CPU/PIC/ROM route.
    m.bus.mem[0x1000..0x1003].copy_from_slice(&[0xf4, 0xeb, 0xfd]); // HLT loop
    m
}

fn bios(m: &mut Machine, ax: u16) -> u16 {
    m.cpu.ip = 0x1100;
    m.cpu.halted = false;
    m.cpu.r[AX] = ax;
    m.bus.mem[0x1100..0x1103].copy_from_slice(&[0xcd, 0x18, 0xf4]);
    run(m, 3); // INT 18, ROM OUT, IRET
    assert_eq!(m.cpu.ip, 0x1102);
    m.cpu.ip = 0x1000;
    m.cpu.halted = true;
    m.cpu.r[AX]
}

fn assert_groups(m: &mut Machine, expected: &[u8; 16]) {
    assert_eq!(&m.bus.mem[0x52a..0x53a], expected);
    for group in 0..16u16 {
        assert_eq!(
            bios(m, 0x0400 | group) >> 8,
            expected[group as usize] as u16
        );
    }
    for group in [16, 127, 255] {
        assert_eq!(bios(m, 0x0400 | group) >> 8, 0);
    }
}

fn assert_ring(m: &Machine, expected: &[u16]) {
    assert_eq!(
        m.bus.keyboard.bios.iter().copied().collect::<Vec<_>>(),
        expected
    );
    assert_eq!(m.bus.read16(0x524), 0x502);
    assert_eq!(
        m.bus.read16(0x526),
        0x502 + (expected.len() as u16 * 2) % 32
    );
    assert_eq!(m.bus.read(0x528), expected.len() as u8);
    for (i, key) in expected.iter().enumerate() {
        assert_eq!(m.bus.read16(0x502 + i * 2), *key);
    }
}

#[test]
fn queued_make_break_and_repeat_follow_serviced_irq_order() {
    for profile in PROFILES {
        let mut m = machine(profile);
        // Physical state has already returned to released before BIOS sees make.
        m.bus.key(0x1c, true);
        m.bus.key(0x1c, true);
        m.bus.key(0x1c, false);
        assert_eq!(m.bus.keyboard.pressed[3], 0);
        m.cpu.flags &= !IF;
        run(&mut m, 100);
        assert_groups(&mut m, &[0; 16]);
        assert_ring(&m, &[]);
        assert_eq!(m.bus.keyboard.fifo.len(), 3);

        m.cpu.flags |= IF;
        for (count, held) in [(1, 0x10), (2, 0x10), (2, 0)] {
            run(&mut m, 1); // IRQ entry executes the firmware OUT instruction.
            assert_eq!(m.bus.read(0x52d), held);
            assert_ring(&m, &vec![0x1c0d; count]);
            assert_eq!(m.bus.pics[0].isr & 2, 0); // Firmware EOI.
            run(&mut m, 1); // IRET; the next byte needs another serviced IRQ.
        }
        assert_groups(&mut m, &[0; 16]);
        assert_eq!(bios(&mut m, 0x0000), 0x1c0d);
        assert_ring(&m, &[0x1c0d]);
        assert_eq!(bios(&mut m, 0x0500), 0x1c0d);
        assert_ring(&m, &[]);
    }
}

#[test]
fn simultaneous_groups_modifiers_and_ring_operations_preserve_held_keys() {
    for profile in PROFILES {
        let mut m = machine(profile);
        let mut held = [0; 16];
        for scan in [0x00, 0x1c, 0x70, 0x1d, 0x3b, 0x7f] {
            m.bus.key(scan, true);
            run(&mut m, 20);
            held[scan as usize / 8] |= 1 << (scan % 8);
            assert_groups(&mut m, &held);
        }
        assert_ring(&m, &[0x001b, 0x1c0d, 0x1d41, 0x3b00, 0x7f00]);
        assert_eq!(bios(&mut m, 0x0200) & 0xff, 1);
        assert_eq!(m.bus.read(0x53a), 1);
        assert_eq!(bios(&mut m, 0x0100), 0x001b); // Peek leaves ring and holds.
        assert_ring(&m, &[0x001b, 0x1c0d, 0x1d41, 0x3b00, 0x7f00]);
        bios(&mut m, 0x0300); // Flush characters while keys remain held.
        assert_ring(&m, &[]);
        assert_groups(&mut m, &held);
        for scan in [0x1c, 0x7f, 0x00, 0x1d, 0x70, 0x3b] {
            m.bus.key(scan, false);
            run(&mut m, 20);
            held[scan as usize / 8] &= !(1 << (scan % 8));
            assert_groups(&mut m, &held);
            assert_ring(&m, &[]); // Breaks never enqueue characters.
        }
        assert_eq!(m.bus.read(0x53a), 0);
    }
}

#[test]
fn every_group_and_full_ring_still_track_releases_and_repeated_presses() {
    for profile in PROFILES {
        let mut m = machine(profile);
        for group in 0..16 {
            m.bus.key(group * 8 + 7, true);
        }
        run(&mut m, 100);
        assert_groups(&mut m, &[0x80; 16]);
        assert_eq!(m.bus.read(0x528), 16);
        let full_ring = m.bus.keyboard.bios.clone();
        m.bus.key(0x1c, true); // Full ring must not prevent bitmap updates.
        run(&mut m, 20);
        assert_eq!(m.bus.read(0x52d), 0x90);
        m.bus.key(0x1c, false);
        for group in 0..16 {
            m.bus.key(group * 8 + 7, false);
        }
        run(&mut m, 100);
        assert_groups(&mut m, &[0; 16]);
        assert_eq!(m.bus.keyboard.bios, full_ring);
        bios(&mut m, 0x0300);
        for _ in 0..3 {
            m.bus.key(0x1c, true);
            m.bus.key(0x1c, false);
            run(&mut m, 20);
        }
        assert_ring(&m, &[0x1c0d; 3]);
        assert_groups(&mut m, &[0; 16]);
    }
}

#[test]
fn pic_mask_defers_bios_updates_and_only_keyboard_work_area_and_stack_change() {
    for profile in PROFILES {
        let mut m = machine(profile);
        m.bus.mem[0x529] = 0xa5;
        m.bus.mem[0x53b] = 0x5a;
        m.bus.pics[0].mask = 255;
        m.bus.key(0x1c, true);
        run(&mut m, 100);
        assert_groups(&mut m, &[0; 16]);
        let before = m.bus.mem.clone();
        m.bus.pics[0].mask = 0xfd;
        run(&mut m, 20);
        for (p, (&old, &new)) in before.iter().zip(&m.bus.mem).enumerate() {
            if old != new {
                assert!(
                    (0x502..0x522).contains(&p)
                        || (0x524..0x529).contains(&p)
                        || p == 0x52d
                        || (0x8ffa..0x9000).contains(&p),
                    "unexpected write at {p:05x}: {old:02x} -> {new:02x}"
                );
            }
        }
        assert_eq!(m.bus.read(0x52d), 0x10);
        assert_ring(&m, &[0x1c0d]);
        // A spurious INT 09 with no ready byte must not fabricate Escape.
        m.bus.pics[0].request(1);
        run(&mut m, 20);
        assert_ring(&m, &[0x1c0d]);
        assert_eq!(m.bus.read(0x52a), 0);
    }
}

#[test]
fn guest_irq_hooks_can_chain_or_consume_input_without_host_bitmap_writes() {
    for profile in PROFILES {
        for chain in [false, true] {
            let mut m = machine(profile);
            // INC byte [2000]; then JMP FAR to the original BIOS vector, or
            // IN AL,41h; MOV AL,20h; OUT 0,AL; IRET to consume input in guest.
            let mut hook = vec![0xfe, 0x06, 0x00, 0x20];
            hook.extend(if chain {
                vec![0xea, 0x24, 0x10, 0x00, 0xf0]
            } else {
                vec![0xe4, 0x41, 0xb0, 0x20, 0xe6, 0x00, 0xcf]
            });
            m.bus.mem[0x1200..0x1200 + hook.len()].copy_from_slice(&hook);
            m.bus.write_seg(0, 9 * 4, 0x1200, true);
            m.bus.write_seg(0, 9 * 4 + 2, 0, true);
            for down in [true, false] {
                m.bus.key(0x1c, down);
                run(&mut m, 30);
                let mut expected = [0; 16];
                expected[3] = if chain && down { 0x10 } else { 0 };
                assert_groups(&mut m, &expected);
                assert_ring(&m, if chain { &[0x1c0d] } else { &[] });
                assert!(m.bus.keyboard.fifo.is_empty());
            }
            // FIFO ready can latch another IRQ during the guest prologue.
            // Both entries reach the hook; an empty BIOS entry adds no key.
            assert_eq!(m.bus.read(0x2000), 4);
        }
    }
}

#[test]
fn command_replies_preserve_holds_while_identical_break_bytes_release_keys() {
    for profile in PROFILES {
        let mut m = machine(profile);
        let mut held = [0; 16];
        for scan in [0x1c, 0x20, 0x7a, 0x70] {
            m.bus.key(scan, true);
            held[scan as usize / 8] |= 1 << (scan % 8);
        }
        run(&mut m, 40);
        let ring = [0x1c0d, 0x2066, 0x7a00];
        assert_groups(&mut m, &held);
        assert_ring(&m, &ring);
        // Execute real guest OUTs: reset replies FA/A0, command ACK, parameter ACK.
        for command in [0xff, 0x9d, 0x00] {
            m.cpu.ip = 0x1200;
            m.cpu.halted = false;
            m.bus.mem[0x1200..0x1207]
                .copy_from_slice(&[0xb0, command, 0xe6, 0x41, 0xf4, 0xeb, 0xfd]);
            run(&mut m, 40);
            assert!(m.bus.keyboard.fifo.is_empty());
            assert_eq!(m.bus.keyboard.pressed, held);
            assert_groups(&mut m, &held);
            assert_ring(&m, &ring);
            assert_eq!(bios(&mut m, 0x0200) & 0xff, 1);
            assert_eq!(m.bus.read(0x53a), 1);
        }
        // Interleave a reply with real A0/FA breaks. Byte-value filtering would
        // preserve these keys incorrectly; IRQ service must retain FIFO order.
        m.bus.output(0x41, 0x9d);
        for scan in [0x20, 0x7a] {
            m.bus.key(scan, false);
            held[scan as usize / 8] &= !(1 << (scan % 8));
        }
        run(&mut m, 40);
        assert_eq!(m.bus.keyboard.pressed, held);
        assert_groups(&mut m, &held);
        assert_ring(&m, &ring);
        for scan in [0x1c, 0x70] {
            m.bus.key(scan, false);
        }
        run(&mut m, 40);
        assert_groups(&mut m, &[0; 16]);
        assert_eq!(m.bus.keyboard.pressed, [0; 16]);
        assert_eq!(bios(&mut m, 0x0200) & 0xff, 0);
        assert_ring(&m, &ring);
    }
}

#[test]
fn command_replies_and_scan_bytes_remain_readable_in_port_order() {
    for profile in PROFILES {
        let mut m = machine(profile);
        m.bus.key(0x1c, true);
        m.bus.output(0x41, 0xff); // Reset discards the queued make, not the hold.
        m.bus.key(0x20, false);
        m.bus.key(0x7a, false);
        for byte in [0xfa, 0xa0, 0xa0, 0xfa] {
            assert_ne!(m.bus.input(0x43) & 2, 0);
            assert_eq!(m.bus.input(0x41), byte);
        }
        assert_eq!(m.bus.input(0x43) & 2, 0);
        run(&mut m, 20); // A latched IRQ after guest reads must find an empty FIFO.
        assert_eq!(m.bus.keyboard.pressed[3], 0x10);
        assert_ring(&m, &[]);
        assert_groups(&mut m, &[0; 16]); // Guest consumed all bytes itself.
    }
}

#[test]
fn machine_reset_discards_serviced_and_pending_keys_and_clears_the_bitmap() {
    for profile in PROFILES {
        let mut m = machine(profile);
        m.bus.key(0x1c, true);
        run(&mut m, 20);
        m.bus.key(0x1d, true); // Still pending at reset.
        assert_eq!(m.bus.read(0x52d), 0x10);
        m.reset();
        assert_eq!(&m.bus.mem[0x52a..0x53b], &[0; 17]);
        assert_eq!(m.bus.keyboard.pressed, [0; 16]);
        assert!(m.bus.keyboard.fifo.is_empty());
        assert_ring(&m, &[]);
        // Re-enter the synthetic BIOS after reset, without booting media.
        m.cpu.s = [0; 4];
        m.cpu.r[SP] = 0x9000;
        assert_groups(&mut m, &[0; 16]);
    }
}

#[test]
fn original_keymap_control_passes_hold_release_through_real_dos() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/freedos/keymap.hdm"
    ))
    .expect("run python3 tools/prepare_freedos.py before cargo test --test keyboard");
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "keymap.hdm").unwrap();
        m.reset();
        for _ in 0..50 {
            run(&mut m, 100_000);
        }
        assert!(m.bus.text().contains("KEYMAP READY"), "{}", m.bus.text());
        m.bus.key(0x1c, true);
        for _ in 0..10 {
            run(&mut m, 100_000);
        }
        assert!(
            m.bus.text().contains("KEYMAP DOWN PASS"),
            "{}",
            m.bus.text()
        );
        assert_eq!(m.bus.read(0x52d), 0x10);
        m.bus.key(0x1c, false);
        for _ in 0..10 {
            run(&mut m, 100_000);
        }
        let text = m.bus.text();
        for expected in [
            "KEYMAP DOWN PASS",
            "KEYMAP UP PASS",
            "PROBE SHELL RETURN",
            "A:\\>",
        ] {
            assert!(text.contains(expected), "{profile:?}: {expected}: {text}");
        }
        assert!(!text.contains("KEYMAP BWA STALE"), "{text}");
        assert_eq!(&m.bus.mem[0x52a..0x53a], &[0; 16]);
        assert!(m.bus.fdc.reads > 100);
    }
}

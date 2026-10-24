use pc98_rust::{bus::Bus, disk::Disk, Machine, Profile};

const PROFILES: [Profile; 2] = [Profile::Pc9801Vm, Profile::Pc9821ApDos];

fn command(b: &mut Bus, bytes: &[u8]) {
    for &byte in bytes {
        b.output(0x92, byte);
    }
    b.tick(160);
}

fn patterned_raw() -> Vec<u8> {
    (0..1_261_568).map(|i| (i ^ (i >> 8)) as u8).collect()
}

fn setup(profile: Profile) -> Bus {
    let mut b = Bus::new(profile);
    b.disks[0] = Some(Disk::parse(patterned_raw(), "original.hdm").unwrap());
    b.output(0x94, 0x18);
    b.output(0x15, 6); // mask DMA channel 2
    command(&mut b, &[3, 0xdf, 3]);
    b
}

fn nd_read_exposes_clocked_payload(profile: Profile) {
    let mut b = setup(profile);
    assert_eq!(b.input(0x90), 0x80);
    for (i, byte) in [0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff].into_iter().enumerate() {
        b.output(0x92, byte);
        assert_eq!(b.input(0x90), if i == 8 { 0x70 } else { 0x90 });
        assert_eq!(b.input(0x92), 0x80); // command/early reads preserve parameters
    }
    b.tick(159);
    assert_eq!(b.input(0x90), 0x70);
    b.output(0x92, 3); // extra byte while dispatch is pending is ignored
    b.tick(1);
    assert_eq!(b.input(0x90), 0x70); // EXM, DIO, CB; not ready yet
    b.tick(128);
    assert_eq!(b.input(0x90), 0xf0);
    assert_eq!(b.input(0x92), 0);
    assert_eq!(b.input(0x90), 0x70);
}

#[test]
fn nd_read_exposes_clocked_payload_vm() {
    nd_read_exposes_clocked_payload(Profile::Pc9801Vm);
}

#[test]
fn nd_read_exposes_clocked_payload_ap() {
    nd_read_exposes_clocked_payload(Profile::Pc9821ApDos);
}

fn results(b: &mut Bus) -> Vec<u8> {
    let mut out = Vec::new();
    for _ in 0..7 {
        assert_eq!(b.input(0x90), 0xd0);
        b.output(0x92, 3); // wrong direction cannot start SPECIFY
        out.push(b.input(0x92));
    }
    assert_eq!(b.input(0x90), 0x80);
    assert_eq!(b.input(0x92), 0x80);
    out
}

fn read_payload(b: &mut Bus, expected: &[u8]) {
    let mut guarded = vec![0xa5; expected.len() + 2];
    for (i, &byte) in expected.iter().enumerate() {
        assert_eq!(b.input(0x90), 0x70);
        // Reads and writes do not invent elapsed guest time.
        for _ in 0..3 {
            assert_eq!(b.input(0x92), 0x80);
            b.output(0x92, 0xff);
            assert_eq!(b.input(0x90), 0x70);
        }
        b.tick(127);
        assert_eq!(b.input(0x90), 0x70);
        b.tick(1);
        for _ in 0..3 {
            assert_eq!(b.input(0x90), 0xf0);
        }
        b.output(0x92, 0x45);
        guarded[i + 1] = b.input(0x92);
        assert_eq!(guarded[i + 1], byte, "payload byte {i}");
    }
    assert_eq!(guarded[0], 0xa5);
    assert_eq!(guarded[expected.len() + 1], 0xa5);
    assert_eq!(&guarded[1..expected.len() + 1], expected);
    b.tick(128); // terminate after the last byte, with a separate result request
}

// Original normal MFM D88 tracks. Parser, commands and clocks are the only
// entry points; no execution state or PIC requests are injected.
fn patterned_d88(n: u8) -> Vec<u8> {
    let mut image = vec![0; 0x2a0];
    image[0x1b] = 0x20;
    for h in 0..2u8 {
        let offset = image.len() as u32;
        image[0x20 + h as usize * 4..0x24 + h as usize * 4].copy_from_slice(&offset.to_le_bytes());
        for r in 1..=3u8 {
            let mut header = [0; 16];
            header[..6].copy_from_slice(&[0, h, r, n, 3, 0]);
            let len = 128u16 << n;
            header[14..].copy_from_slice(&len.to_le_bytes());
            image.extend(header);
            image.extend((0..len).map(|i| (i as u8).wrapping_add(r * 17 + h * 61)));
        }
    }
    let len = image.len() as u32;
    image[0x1c..0x20].copy_from_slice(&len.to_le_bytes());
    image
}

#[test]
fn pio_single_and_multi_sector_raw_and_d88_both_heads_all_sizes() {
    for profile in PROFILES {
        for n in 1..=3 {
            for raw in [false, true] {
                if raw && n == 1 {
                    continue;
                } // supported raw geometries start at N=2
                let image = if raw {
                    let len = if n == 2 { 655_360 } else { 1_261_568 };
                    (0..len).map(|i| (i ^ (i >> 8)) as u8).collect()
                } else {
                    patterned_d88(n)
                };
                for h in 0..2 {
                    for start in [2, 3] {
                        let mut b = setup(profile);
                        b.disks[0] = Some(
                            Disk::parse(
                                image.clone(),
                                if raw { "original.hdm" } else { "original.d88" },
                            )
                            .unwrap(),
                        );
                        let mut expected = Vec::new();
                        for r in start..=3 {
                            let disk = b.disks[0].as_ref().unwrap();
                            let s = disk.sector(0, h, r, n).unwrap();
                            expected.extend_from_slice(&disk.bytes[s.offset..s.offset + s.len]);
                        }
                        command(&mut b, &[0x46, h << 2, 0, h, start, n, 3, 0x1b, 0xff]);
                        read_payload(&mut b, &expected);
                        // NEC EOT without TC retains the final ID; Table 2 is
                        // explicitly for processor termination, not this case.
                        assert_eq!(results(&mut b), [0x40 | h << 2, 0x80, 0, 0, h, 3, n]);
                        assert_eq!(b.fdc.reads, u64::from(4 - start));
                        assert_eq!(b.fdc.writes, 0);
                        assert_eq!(b.disks[0].as_ref().unwrap().writes, 0);
                        assert_eq!(b.disks[0].as_ref().unwrap().bytes, image);
                    }
                }
            }
        }
    }
}

#[test]
fn byte_deadlines_are_partition_invariant_and_reads_do_not_create_time() {
    for profile in PROFILES {
        for chunks in [vec![287], vec![160, 127], vec![1; 287]] {
            let mut b = setup(profile);
            for byte in [0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff] {
                b.output(0x92, byte);
            }
            for clocks in chunks {
                b.tick(clocks);
            }
            assert_eq!(b.input(0x90), 0x70);
            b.tick(1);
            assert_eq!(b.input(0x90), 0xf0);
            // Service late, but before deadline. The next arrival remains at
            // 160+256, rather than being postponed by this delayed read.
            b.tick(103);
            assert_eq!(b.input(0x92), 0);
            b.tick(24);
            assert_eq!(b.input(0x90), 0x70);
            b.tick(1);
            assert_eq!(b.input(0x90), 0xf0);
            assert_eq!(b.input(0x92), 1);
        }
        for chunks in [vec![392], vec![160, 128, 104], vec![1; 392], vec![1000]] {
            let mut b = setup(profile);
            for byte in [0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff] {
                b.output(0x92, byte);
            }
            for clocks in chunks {
                b.tick(clocks);
            }
            assert_eq!(results(&mut b), [0x40, 0x10, 0, 0, 0, 1, 3]);
            assert_eq!(b.fdc.reads, 0);
        }
    }
}

fn eoi(b: &mut Bus) {
    b.output(8, 0x20);
    b.output(0, 0x20);
}

#[test]
fn pio_requests_acknowledge_each_byte_and_result_without_held_retrigger() {
    for profile in PROFILES {
        let mut b = setup(profile);
        b.output(2, 0x7f);
        b.output(10, 0xf7);
        command(&mut b, &[0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff]);
        assert_eq!(b.irq(), None);
        for i in 0..1024 {
            b.tick(if i == 0 { 128 } else { 127 });
            assert_eq!(b.irq(), Some(0x13));
            assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 8));
            if i % 2 == 0 {
                eoi(&mut b);
                b.tick(1); // held request remains silent after acknowledgement/EOI
                assert_eq!(b.irq(), None);
                assert_eq!(b.input(0x90), 0xf0);
                assert_eq!(b.input(0x92), (i ^ (i >> 8)) as u8);
            } else {
                assert_eq!(b.input(0x92), (i ^ (i >> 8)) as u8);
                // Reading data acknowledges the FDC, but does not clear PIC ISR.
                assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 8));
                eoi(&mut b);
                b.tick(1);
                assert_eq!(b.irq(), None);
            }
        }
        b.tick(127);
        assert_eq!(b.irq(), Some(0x13));
        eoi(&mut b);
        b.tick(2000);
        assert_eq!(b.irq(), None); // held result does not retrigger
        assert_eq!(results(&mut b), [0x40, 0x80, 0, 0, 0, 1, 3]);
        assert_eq!(b.irq(), None);
    }
}

#[test]
fn errors_unsupported_commands_and_reset_recover_without_stale_payload() {
    for profile in PROFILES {
        let mut b = setup(profile);
        for bytes in [
            [0x46, 0, 0, 0, 9, 3, 9, 0x1b, 0xff],
            [0x46, 1, 0, 0, 1, 3, 1, 0x1b, 0xff],
            [0x46, 0, 1, 0, 1, 3, 1, 0x1b, 0xff],
        ] {
            command(&mut b, &bytes);
            let st0 = if bytes[1] == 1 { 0x49 } else { 0x40 };
            assert_eq!(results(&mut b), [st0, 4, 0, bytes[2], 0, bytes[4], 3]);
        }
        for (op, n) in [
            (0x45, 3),
            (0x49, 3), // deleted write
            (0x4c, 3), // deleted read
            (0x4d, 3), // format
            (0x51, 3), // scan
            (0xc6, 3),
            (0x66, 3),
            (0x06, 3),
            (0x46, 0),
            (0x46, 4),
        ] {
            command(&mut b, &[op, 0, 0, 0, 1, n, 1, 0x1b, 0xff]);
            assert_eq!(b.input(0x90), 0xd0);
            assert_eq!(b.input(0x92), 0x80);
            assert_eq!(b.input(0x90), 0x80);
            assert_eq!(b.fdc.reads, 0);
            assert_eq!(b.fdc.writes, 0);
        }
        // Reset a ready byte and, separately, an overrun result still undrained.
        for overrun in [false, true] {
            command(&mut b, &[0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff]);
            b.tick(if overrun { 232 } else { 128 });
            b.output(0x94, 0x98);
            b.output(0x94, 0x18);
            assert_eq!(b.input(0x90), 0x80);
            assert_eq!(b.input(0x92), 0x80);
            command(&mut b, &[8]);
            assert_eq!((b.input(0x92), b.input(0x92)), (0xc0, 0));
            command(&mut b, &[3, 0xdf, 3]);
        }
        command(&mut b, &[0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff]);
        read_payload(&mut b, &patterned_raw()[..1024]);
        assert_eq!(results(&mut b), [0x40, 0x80, 0, 0, 0, 1, 3]);
    }
}

fn dma_word(b: &mut Bus, port: u16, word: u16) {
    b.output(0x19, 0);
    for byte in word.to_le_bytes() {
        b.output(port, byte);
    }
}

#[test]
fn pio_preserves_dma_programming_and_nd_clear_restores_dma_reads_and_writes() {
    for profile in PROFILES {
        for disabled in [false, true] {
            let mut b = setup(profile);
            dma_word(&mut b, 9, 0x9000);
            dma_word(&mut b, 11, 3);
            b.output(0x17, 0x56); // auto-init, read
            b.output(0x23, 2);
            b.dma.advance(2); // distinct current/base, public DMA operation only
            b.dma.advance(0); // retain an unrelated sticky TC indication
            b.output(0x11, if disabled { 4 } else { 0 });
            b.input(9); // shared byte pointer left high
            let mut expected = b.dma.clone();
            b.mem[0x28fff..0x29010].fill(0xa5);
            command(&mut b, &[0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff]);
            read_payload(&mut b, &patterned_raw()[..1024]);
            assert_eq!(results(&mut b), [0x40, 0x80, 0, 0, 0, 1, 3]);
            assert_eq!(&b.mem[0x28fff..0x29010], &[0xa5; 17]);
            assert_eq!(b.dma.addr, expected.addr);
            assert_eq!(b.dma.count, expected.count);
            assert_eq!(b.dma.page, expected.page);
            assert_eq!(b.dma.mode, expected.mode);
            assert_eq!(b.dma.mask, expected.mask);
            assert_eq!(b.dma.status, expected.status);
            assert_eq!(b.dma.can_service(0), expected.can_service(0));
            assert_eq!(b.input(9), expected.read(9)); // pointer still high
                                                      // Advance cloned programming through TC: proves both retained bases.
            let mut observed = b.dma.clone();
            for _ in 0..4 {
                assert_eq!(observed.advance(2), expected.advance(2));
            }
            assert_eq!(observed.addr, expected.addr);
            assert_eq!(observed.count, expected.count);
            // Observe controller disable after unmasking channel 2.
            b.output(0x15, 2);
            assert_eq!(b.dma.can_service(2), !disabled);
            b.output(0x11, 0);
            b.output(0x23, 0);
            command(&mut b, &[3, 0xdf, 2]);
            for write in [false, true] {
                dma_word(&mut b, 9, 0x9000);
                dma_word(&mut b, 11, 1023);
                b.output(0x17, if write { 0x5a } else { 0x56 });
                command(
                    &mut b,
                    &[
                        if write { 0x45 } else { 0x46 },
                        0,
                        0,
                        0,
                        2,
                        3,
                        2,
                        0x1b,
                        0xff,
                    ],
                );
                assert_eq!(results(&mut b), [0, 0, 0, 0, 0, 2, 3]);
                assert_eq!(&b.mem[0x9000..0x9400], &patterned_raw()[1024..2048]);
            }
            assert_eq!(b.fdc.writes, 1);
        }
    }
}

#[test]
fn original_interrupt_driven_pio_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/fdc_pio/fdc_pio.hdm"
    ))
    .expect("run python3 tools/prepare_fdc_pio_fixture.py");
    let expected = [
        0xa5, 1, 0, 8, 8, 0x80, 0, 0x98, 1, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0x40, 0x80,
        0, 0, 0, 3, 3, 0xa5, 0xd0, 0xd0, 0x80, 0, 0, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
        0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
        0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    ];
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "fdc_pio.hdm").unwrap();
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
                    "{text}; ip={:x} result={:02x?}",
                    m.cpu.ip,
                    &m.bus.mem[0x8000..0x8040]
                );
                if text.contains("FDC PIO PASS") {
                    break;
                }
            }
            assert!(
                m.bus.text().contains("FDC PIO PASS"),
                "ip={:x} result={:02x?}",
                m.cpu.ip,
                &m.bus.mem[0x8000..0x8040]
            );
            assert!(slots <= 2_000_000);
            assert_eq!(&m.bus.mem[0x8000..0x8040], &expected);
            assert!(m.bus.mem[0x8040..0x9000].iter().all(|&byte| byte == 0xa5));
            assert_eq!(&m.bus.mem[0x9000..0x9800], &image[1024..3072]);
            assert_eq!(&m.bus.mem[0x9800..0x9810], &[0xa5; 16]);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
            assert_eq!(m.bus.fdc.writes, 0);
            println!(
                "{profile:?} FDC PIO boot {run}: {slots} slots, 2048 data IRQs and 1 result IRQ"
            );
        }
    }
}

#[test]
fn overrun_raises_result_irq_after_acknowledged_held_data_request() {
    for profile in PROFILES {
        let mut b = setup(profile);
        b.output(2, 0x7f);
        b.output(10, 0xf7);
        command(&mut b, &[0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff]);
        b.tick(128);
        assert_eq!(b.irq(), Some(0x13));
        eoi(&mut b);
        b.tick(103);
        assert_eq!(b.irq(), None);
        assert_eq!(b.input(0x90), 0xf0); // held byte has not been serviced
        b.tick(1);
        assert_eq!(b.irq(), Some(0x13)); // distinct overrun result event
        assert_eq!(b.input(0x90), 0xd0);
        assert_eq!(b.input(0x92), 0x40); // first result acknowledges this event
        eoi(&mut b);
        b.tick(1000);
        assert_eq!(b.irq(), None);
        for byte in [0x10, 0, 0, 0, 1, 3] {
            assert_eq!(b.input(0x90), 0xd0);
            assert_eq!(b.input(0x92), byte);
        }
        assert_eq!(b.input(0x90), 0x80);
        command(&mut b, &[3, 0xdf, 3]);
        command(&mut b, &[0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff]);
        b.tick(128);
        assert_eq!(b.irq(), Some(0x13));
        assert_eq!(b.input(0x90), 0xf0);
        assert_eq!(b.input(0x92), 0); // recovery begins at the first payload byte
        eoi(&mut b);
    }
}

#[test]
fn unavailable_media_and_missing_later_sector_end_in_result_phase() {
    for profile in PROFILES {
        let mut b = setup(profile);
        b.disks[0] = None;
        command(&mut b, &[0x46, 0, 0, 0, 1, 3, 1, 0x1b, 0xff]);
        assert_eq!(results(&mut b), [0x48, 4, 0, 0, 0, 1, 3]);
        b.disks[0] = Some(Disk::parse(patterned_d88(1), "original.d88").unwrap());
        command(&mut b, &[0x46, 0, 0, 0, 3, 1, 4, 0x1b, 0xff]);
        let expected: Vec<_> = (0..256).map(|i| (i as u8).wrapping_add(51)).collect();
        read_payload(&mut b, &expected);
        assert_eq!(results(&mut b), [0x40, 4, 0, 0, 0, 4, 1]);
        // A host eject between availability and the data read cannot panic or
        // leak an old payload byte into the next command.
        command(&mut b, &[0x46, 0, 0, 0, 1, 1, 1, 0x1b, 0xff]);
        b.tick(128);
        b.disks[0] = None;
        assert_eq!(b.input(0x92), 0x80);
        assert_eq!(results(&mut b), [0x40, 4, 0, 0, 0, 1, 1]);
    }
}

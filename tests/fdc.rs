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

#[test]
fn bios_takes_idle_nd_for_dma_and_preserves_errors_and_pio_reentry() {
    for profile in PROFILES {
        for cylinder in [0, 2] {
            for (write, missing, protected) in [
                (false, false, false),
                (true, false, false),
                (false, true, false),
                (true, true, false),
                (true, false, true),
            ] {
                let mut b = setup(profile);
                b.output(0x11, 0);
                b.disks[0].as_mut().unwrap().protected = protected;
                let mut expected_media = patterned_raw();
                let sector = if missing { 9 } else { 2 };
                let offset = cylinder as usize * 16384 + 8192 + 1024;
                let payload: Vec<_> = (0..1024)
                    .map(|i| (i as u8).wrapping_mul(29).wrapping_add(71))
                    .collect();
                b.mem[0x28ff0..0x29410].fill(0xa5);
                if write {
                    b.mem[0x29000..0x29400].copy_from_slice(&payload);
                }
                let success = !missing && !protected;
                if write && success {
                    expected_media[offset..offset + 1024].copy_from_slice(&payload);
                }
                assert_eq!(b.input(0x90), 0x80);
                assert_eq!(
                    pc98_rust::firmware::disk_transfer(
                        &mut b,
                        0,
                        [cylinder, 1, sector, 3],
                        1024,
                        0x29000,
                        write,
                    ),
                    success,
                    "{profile:?} C={cylinder} write={write} missing={missing} protected={protected}"
                );
                let expected_buffer = if write {
                    payload
                } else if success {
                    expected_media[offset..offset + 1024].to_vec()
                } else {
                    vec![0xa5; 1024]
                };
                assert_eq!(&b.mem[0x29000..0x29400], &expected_buffer);
                assert_eq!(&b.mem[0x28ff0..0x29000], &[0xa5; 16]);
                assert_eq!(&b.mem[0x29400..0x29410], &[0xa5; 16]);
                assert_eq!(b.disks[0].as_ref().unwrap().bytes, expected_media);
                assert_eq!(
                    b.disks[0].as_ref().unwrap().writes,
                    u64::from(write && success)
                );
                assert_eq!(b.fdc.writes, u64::from(write && success));
                assert_eq!(b.fdc.reads, u64::from(!write && success));
                assert_eq!(b.dma.addr[2], if success { 0x9400 } else { 0x9000 });
                assert_eq!(b.dma.page[2], 2);
                assert_eq!(b.dma.count[2], if success { 0xffff } else { 1023 });
                assert_eq!(b.dma.status & 4, if success { 4 } else { 0 });
                assert_eq!(b.dma.mask & 4, if success { 4 } else { 0 });
                assert_eq!(b.fdc.cylinder[0], cylinder);
                // All seven results were drained; the controller is idle.
                assert_eq!(b.input(0x90), 0x80);
                assert_eq!(b.input(0x92), 0x80);
                // An ordinary command works immediately after success or error.
                command(&mut b, &[4, 0]);
                assert_eq!(
                    b.input(0x92),
                    0x28 | if cylinder == 0 { 16 } else { 0 } | if protected { 64 } else { 0 }
                );
                assert_eq!(b.input(0x90), 0x80);
                // BIOS selecting DMA must not remove raw clients' PIO capability.
                command(&mut b, &[3, 0xdf, 3]);
                command(&mut b, &[0x46, 0, cylinder, 0, 1, 3, 1, 0x1b, 0xff]);
                let start = cylinder as usize * 16384;
                read_payload(&mut b, &expected_media[start..start + 1024]);
                assert_eq!(results(&mut b), [0x40, 0x80, 0, cylinder, 0, 1, 3]);
            }
        }
    }
}

#[test]
fn original_bios_handoff_guest_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/fdc_bios/fdc_bios.hdm"
    ))
    .expect("run python3 tools/prepare_fdc_bios_fixture.py");
    let mut expected_media = image.clone();
    let write0: Vec<_> = (0..1024).map(|i| (29 * i + 71) as u8).collect();
    let write2: Vec<_> = (0..1024).map(|i| (13 * i + 0xb3) as u8).collect();
    expected_media[2048..3072].copy_from_slice(&write0);
    expected_media[34816..35840].copy_from_slice(&write2);
    let mut expected_ram = vec![0xa5; 0x4410];
    expected_ram[0] = 1;
    expected_ram[1..15].fill(0); // seven successful AH/CF pairs
    for (address, payload) in [
        (0x9000, &image[1024..2048]),
        (0x9800, &image[33792..34816]),
        (0xa000, &write0[..]),
        (0xa800, &write2[..]),
        (0xb000, &write2[..]),
        (0xb800, &image[33792..34816]),
        (0xc000, &image[1024..2048]),
    ] {
        expected_ram[address - 0x8000..address - 0x8000 + 1024].copy_from_slice(payload);
    }
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "fdc_bios.hdm").unwrap();
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
                    "{profile:?}: {text}; ip={:x} record={:02x?}",
                    m.cpu.ip,
                    &m.bus.mem[0x8000..0x8020]
                );
                if text.contains("FDC BIOS PASS") {
                    break;
                }
            }
            assert!(
                m.bus.text().contains("FDC BIOS PASS"),
                "{profile:?}: ip={:x} record={:02x?}",
                m.cpu.ip,
                &m.bus.mem[0x8000..0x8020]
            );
            assert!(slots <= 2_000_000);
            assert_eq!(&m.bus.mem[0x8000..0xc410], &expected_ram);
            assert_eq!(m.cpu.r[pc98_rust::cpu::SP], 0x7c00);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, expected_media);
            assert_eq!(m.bus.fdc.writes, 2);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 2 * (run + 1));
            assert_eq!(m.bus.dma.addr[2], 0x9400);
            assert_eq!(m.bus.dma.count[2], 0xffff);
            assert_eq!(m.bus.dma.status & 4, 4);
            assert_eq!(m.bus.input(0x90), 0x80);
            println!("{profile:?} FDC BIOS boot {run}: {slots} slots; seven INT 1Bh calls, two PIO reads, two writes");
        }
    }
}

fn write_command(drive: u8, h: u8, r: u8, n: u8) -> [u8; 9] {
    [0x45, drive | h << 2, 0, h, r, n, r, 0x1b, 1] // DTL must not shorten N>0
}

fn write_payload(b: &mut Bus, payload: &[u8]) {
    for &byte in payload {
        assert_eq!(b.input(0x90), 0x30);
        b.output(0x92, !byte); // early/duplicate writes cannot become parameters
        assert_eq!(b.input(0x92), 0x80);
        b.tick(127);
        assert_eq!(b.input(0x90), 0x30);
        b.tick(1);
        assert_eq!(b.input(0x90), 0xb0);
        assert_eq!(b.input(0x92), 0x80); // wrong direction preserves the request
        assert_eq!(b.input(0x90), 0xb0);
        b.output(0x92, byte);
        assert_eq!(b.input(0x90), 0x30);
        b.output(0x92, !byte);
    }
    b.tick(128);
}

#[test]
fn nd_write_clocked_full_images_both_drives_heads_sizes_and_geometries() {
    for profile in PROFILES {
        // All supported normal D88 sizes and all three raw geometries.
        for (n, raw_len, spt) in [
            (1, 0, 3),
            (2, 0, 3),
            (3, 0, 3),
            (2, 655_360, 8),
            (2, 1_228_800, 15),
            (3, 1_261_568, 8),
        ] {
            let len = 128usize << n;
            let image = if raw_len == 0 {
                patterned_d88(n)
            } else {
                (0..raw_len).map(|i| (i ^ (i >> 8)) as u8).collect()
            };
            for drive in 0..2 {
                for h in 0..2 {
                    let mut b = setup(profile);
                    let name = if raw_len == 0 {
                        "original.d88"
                    } else {
                        "original.hdm"
                    };
                    for d in 0..2 {
                        b.disks[d] = Some(Disk::parse(image.clone(), name).unwrap());
                    }
                    let payload: Vec<_> = (0..len).map(|i| (i * 29 + 71) as u8).collect();
                    // Independent fixture layout calculation, never a sector lookup.
                    let offset = if raw_len == 0 {
                        0x2a0 + h as usize * 3 * (16 + len) + (16 + len) + 16
                    } else {
                        (h as usize * spt + 1) * len
                    };
                    let mut expected = image.clone();
                    expected[offset..offset + len].copy_from_slice(&payload);
                    for (i, byte) in write_command(drive, h, 2, n).into_iter().enumerate() {
                        b.output(0x92, byte);
                        assert_eq!(b.input(0x90), if i == 8 { 0x30 } else { 0x90 });
                        assert_eq!(b.input(0x92), 0x80);
                    }
                    b.tick(159);
                    b.output(0x92, 0xff); // pending dispatch cannot accept data
                    assert_eq!(b.input(0x90), 0x30);
                    b.tick(1);
                    write_payload(&mut b, &payload);
                    assert_eq!(
                        results(&mut b),
                        [0x40 | h << 2 | drive, 0x80, 0, 0, h, 2, n]
                    );
                    for d in 0..2 {
                        let disk = b.disks[d].as_ref().unwrap();
                        assert_eq!(
                            disk.bytes,
                            if d == drive as usize {
                                &expected
                            } else {
                                &image
                            }[..]
                        );
                        assert_eq!(disk.writes, u64::from(d == drive as usize));
                    }
                    assert_eq!(b.fdc.writes, 1);
                    assert_eq!(b.fdc.reads, 0);
                }
            }
        }
    }
}

#[test]
fn nd_write_deadlines_prefixes_result_irqs_and_recovery() {
    for profile in PROFILES {
        for chunks in [vec![287], vec![160, 127], vec![1; 287]] {
            let mut b = setup(profile);
            for byte in write_command(0, 0, 2, 3) {
                b.output(0x92, byte);
            }
            for clocks in chunks {
                b.tick(clocks);
            }
            assert_eq!(b.input(0x90), 0x30);
            b.tick(1);
            b.tick(103);
            b.output(0x92, 0x59);
            b.tick(24);
            assert_eq!(b.input(0x90), 0x30);
            b.tick(1);
            assert_eq!(b.input(0x90), 0xb0); // original boundary, independent of service
            b.output(0x92, 0x6a);
            let mut expected = patterned_raw();
            expected[1024..1026].copy_from_slice(&[0x59, 0x6a]);
            assert_eq!(b.disks[0].as_ref().unwrap().bytes, expected);
        }
        for d88 in [false, true] {
            for prefix in [0, 7] {
                let dispatch = if prefix == 0 { 160 } else { 0 };
                for chunks in [
                    vec![dispatch + 232],
                    vec![dispatch, 128, 104],
                    vec![1; (dispatch + 232) as usize],
                    vec![1000],
                ] {
                    let mut b = setup(profile);
                    b.output(2, 0x7f);
                    b.output(10, 0xf7);
                    let image = if d88 {
                        patterned_d88(3)
                    } else {
                        patterned_raw()
                    };
                    let offset = if d88 { 0x2a0 + 1040 + 16 } else { 1024 };
                    b.disks[0] = Some(
                        Disk::parse(
                            image.clone(),
                            if d88 { "original.d88" } else { "original.hdm" },
                        )
                        .unwrap(),
                    );
                    for byte in write_command(0, 0, 2, 3) {
                        b.output(0x92, byte);
                    }
                    if prefix != 0 {
                        b.tick(160);
                    }
                    for i in 0..prefix {
                        b.tick(128);
                        assert_eq!(b.irq(), Some(0x13));
                        b.output(0x92, i as u8 + 0x53);
                        eoi(&mut b);
                    }
                    for clocks in chunks {
                        b.tick(clocks);
                    }
                    assert_eq!(b.input(0x90), 0xd0);
                    b.output(0x92, 0xff); // deadline write must fail
                    assert_eq!(b.irq(), Some(0x13));
                    assert_eq!(results(&mut b), [0x40, 0x10, 0, 0, 0, 2, 3]);
                    assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 8));
                    eoi(&mut b);
                    b.tick(1000);
                    assert_eq!(b.irq(), None);
                    let mut expected = image;
                    for i in 0..prefix {
                        expected[offset + i] = i as u8 + 0x53;
                    }
                    assert_eq!(b.disks[0].as_ref().unwrap().bytes, expected);
                    assert_eq!(b.fdc.writes, u64::from(prefix != 0));
                    assert_eq!(b.disks[0].as_ref().unwrap().writes, u64::from(prefix != 0));
                    command(&mut b, &write_command(0, 0, 2, 3));
                    write_payload(&mut b, &[0x68; 1024]);
                    assert_eq!(results(&mut b), [0x40, 0x80, 0, 0, 0, 2, 3]);
                }
            }
        }
    }
}

#[test]
fn nd_write_each_byte_and_distinct_result_use_real_pic_and_eoi() {
    for profile in PROFILES {
        let mut b = setup(profile);
        b.output(2, 0x7f);
        b.output(10, 0xf7);
        command(&mut b, &write_command(0, 0, 2, 3));
        assert_eq!(b.irq(), None);
        for i in 0..1024 {
            b.tick(if i == 0 { 128 } else { 127 });
            assert_eq!(b.irq(), Some(0x13));
            assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 8));
            assert_eq!(b.input(0x92), 0x80);
            if i % 2 == 0 {
                eoi(&mut b);
                b.tick(1);
                assert_eq!(b.irq(), None); // held write request does not retrigger
                assert_eq!(b.input(0x90), 0xb0);
                b.output(0x92, i as u8);
            } else {
                b.output(0x92, i as u8);
                // FIFO service acknowledges the FDC, but retains both PIC ISRs.
                assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 8));
                eoi(&mut b);
                b.tick(1);
                assert_eq!(b.irq(), None);
            }
            b.output(0x92, 0xcc); // cannot acknowledge a new request
        }
        b.tick(127);
        assert_eq!(b.irq(), Some(0x13));
        assert_eq!(b.input(0x90), 0xd0);
        assert_eq!(b.input(0x92), 0x40);
        assert_eq!((b.pics[0].isr, b.pics[1].isr), (0x80, 8));
        eoi(&mut b);
        b.tick(1000);
        assert_eq!(b.irq(), None);
        for byte in [0x80, 0, 0, 0, 2, 3] {
            assert_eq!(b.input(0x92), byte);
        }
        assert_eq!(b.input(0x90), 0x80);
        // A missed held data request must also generate a distinct result edge.
        command(&mut b, &write_command(0, 0, 2, 3));
        b.tick(128);
        assert_eq!(b.irq(), Some(0x13));
        eoi(&mut b);
        b.tick(103);
        assert_eq!(b.irq(), None);
        b.tick(1);
        assert_eq!(b.irq(), Some(0x13));
        assert_eq!(results(&mut b), [0x40, 0x10, 0, 0, 0, 2, 3]);
    }
}

#[test]
fn nd_write_errors_unsupported_variants_reset_and_eject() {
    for profile in PROFILES {
        let mut b = setup(profile);
        let mut image = patterned_d88(3);
        image[0x1a] = 0x10;
        b.disks[0] = Some(Disk::parse(image.clone(), "protected.d88").unwrap());
        command(&mut b, &write_command(0, 1, 2, 3));
        assert_eq!(results(&mut b), [0x44, 2, 0, 0, 1, 2, 3]);
        assert_eq!(b.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(b.disks[0].as_ref().unwrap().writes, 0);
        b.disks[0] = None;
        command(&mut b, &write_command(0, 0, 2, 3));
        assert_eq!(results(&mut b), [0x48, 4, 0, 0, 0, 2, 3]);
        b.disks[0] = Some(Disk::parse(patterned_raw(), "original.hdm").unwrap());
        command(&mut b, &write_command(0, 0, 9, 3));
        assert_eq!(results(&mut b), [0x40, 4, 0, 0, 0, 9, 3]);
        let mut variants = Vec::new();
        for (index, value) in [
            (0, 0xc5),
            (0, 0x65),
            (0, 5),
            (0, 0x49),
            (0, 0x4d),
            (0, 0x51),
            (5, 0),
            (5, 4),
            (3, 1),
            (4, 0),
            (6, 3),
        ] {
            let mut bytes = write_command(0, 0, 2, 3);
            bytes[index] = value;
            variants.push(bytes);
        }
        for bytes in variants {
            let mut rejected = setup(profile);
            dma_word(&mut rejected, 9, 0x9000);
            dma_word(&mut rejected, 11, 1023);
            rejected.output(0x17, 0x5a);
            rejected.output(0x15, 2);
            rejected.output(0x11, 0);
            rejected.mem[0x8ff0..0x9410].fill(0xa5);
            let expected_dma = rejected.dma.clone();
            command(&mut rejected, &bytes);
            assert_eq!(rejected.input(0x90), 0xd0);
            assert_eq!(rejected.input(0x92), 0x80);
            assert_eq!(rejected.input(0x90), 0x80);
            assert_eq!(rejected.fdc.writes, 0);
            assert_eq!(rejected.fdc.reads, 0);
            assert_eq!(rejected.disks[0].as_ref().unwrap().writes, 0);
            assert_eq!(rejected.disks[0].as_ref().unwrap().bytes, patterned_raw());
            assert_eq!(rejected.dma.addr, expected_dma.addr);
            assert_eq!(rejected.dma.count, expected_dma.count);
            assert_eq!(rejected.dma.status, expected_dma.status);
            assert!(rejected.mem[0x8ff0..0x9410].iter().all(|&v| v == 0xa5));
        }
        for ready in [false, true] {
            command(&mut b, &write_command(0, 0, 2, 3));
            b.tick(128);
            b.output(0x92, 0x59);
            if ready {
                b.tick(128);
            }
            b.output(0x94, 0x98);
            b.output(0x94, 0x18);
            b.output(0x92, 3);
            b.output(0x92, 0xdf);
            b.output(0x92, 3);
            b.tick(160);
            assert_eq!(b.input(0x90), 0x80);
            let mut expected = patterned_raw();
            expected[1024] = 0x59;
            assert_eq!(b.disks[0].as_ref().unwrap().bytes, expected);
        }
        for ready in [false, true] {
            command(&mut b, &write_command(0, 0, 2, 3));
            b.tick(128);
            b.output(0x92, 0x6a);
            if ready {
                b.tick(128);
            }
            let disk = b.disks[0].take().unwrap();
            if ready {
                b.output(0x92, 0xcc);
            } else {
                b.tick(128);
            }
            assert_eq!(results(&mut b), [0x40, 4, 0, 0, 0, 2, 3]);
            let mut expected = patterned_raw();
            expected[1024] = 0x6a;
            assert_eq!(disk.bytes, expected);
            b.disks[0] = Some(disk);
        }
    }
}

#[test]
fn nd_write_media_changes_cancel_waiting_and_ready_requests() {
    for profile in PROFILES {
        for drive in 0..2 {
            for ready in [false, true] {
                for eject in [false, true] {
                    for replacement in ["raw", "d88", "protected.d88"] {
                        let mut m = Machine::new(profile);
                        m.insert(drive, patterned_raw(), "original.hdm").unwrap();
                        m.bus.output(0x94, 0x18);
                        command(&mut m.bus, &[3, 0xdf, 3]);
                        command(&mut m.bus, &write_command(drive as u8, 0, 1, 3));
                        m.bus.tick(128);
                        m.bus.output(0x92, 0x59);
                        if ready {
                            m.bus.tick(128);
                        }
                        // Loading the other drive must leave this request intact.
                        let other = patterned_d88(3);
                        m.insert(1 - drive, other.clone(), "other.d88").unwrap();
                        assert_eq!(m.bus.input(0x90), if ready { 0xb0 } else { 0x30 });
                        let mut original = patterned_raw();
                        original[0] = 0x59;
                        assert_eq!(m.bus.disks[drive].as_ref().unwrap().bytes, original);
                        if eject {
                            let disk = m.bus.mount_floppy(drive, None).unwrap();
                            assert_eq!(disk.bytes, original);
                            assert_eq!(disk.writes, 1);
                            assert_eq!(m.bus.input(0x90), 0xd0);
                        }
                        let image = if replacement == "raw" {
                            patterned_raw()
                        } else {
                            let mut image = patterned_d88(3);
                            if replacement == "protected.d88" {
                                image[0x1a] = 0x10;
                            }
                            image
                        };
                        let name = if replacement == "raw" {
                            "new.hdm"
                        } else {
                            "new.d88"
                        };
                        m.insert(drive, image.clone(), name).unwrap();
                        // A stale service byte must not reach the replacement,
                        // even when no guest clocks separate eject and insert.
                        m.bus.output(0x92, 0xcc);
                        m.bus.tick(128);
                        assert_eq!(m.bus.disks[drive].as_ref().unwrap().bytes, image);
                        assert_eq!(m.bus.disks[drive].as_ref().unwrap().writes, 0);
                        assert_eq!(m.bus.fdc.writes, 1);
                        assert_eq!(results(&mut m.bus), [0x40 | drive as u8, 4, 0, 0, 0, 1, 3]);
                        // A fresh command resolves offsets/protection on the new disk.
                        command(&mut m.bus, &write_command(drive as u8, 0, 1, 3));
                        if replacement == "protected.d88" {
                            assert_eq!(results(&mut m.bus), [0x40 | drive as u8, 2, 0, 0, 0, 1, 3]);
                            assert_eq!(m.bus.disks[drive].as_ref().unwrap().bytes, image);
                        } else {
                            write_payload(&mut m.bus, &[0xa6; 1024]);
                            assert_eq!(
                                results(&mut m.bus),
                                [0x40 | drive as u8, 0x80, 0, 0, 0, 1, 3]
                            );
                            let mut expected = image;
                            let offset = if replacement == "raw" { 0 } else { 0x2b0 };
                            expected[offset..offset + 1024].fill(0xa6);
                            assert_eq!(m.bus.disks[drive].as_ref().unwrap().bytes, expected);
                            assert_eq!(m.bus.disks[drive].as_ref().unwrap().writes, 1);
                        }
                        assert_eq!(m.bus.disks[1 - drive].as_ref().unwrap().bytes, other);
                        assert_eq!(m.bus.disks[1 - drive].as_ref().unwrap().writes, 0);
                    }
                }
            }
        }
    }
}

#[test]
fn nd_write_isolates_all_dma_state_then_bios_readback_and_pio_reentry() {
    for profile in PROFILES {
        for disabled in [false, true] {
            let mut b = setup(profile);
            dma_word(&mut b, 9, 0x9000);
            dma_word(&mut b, 11, 3);
            b.output(0x17, 0x5a);
            b.output(0x23, 2);
            b.dma.advance(2);
            b.dma.advance(0);
            b.output(0x15, if disabled { 6 } else { 2 });
            b.output(0x11, if disabled { 4 } else { 0 });
            b.input(9);
            let mut expected = b.dma.clone();
            b.mem[0x28ff0..0x29410].fill(0xa5);
            let payload: Vec<_> = (0..1024).map(|i| (i * 29 + 71) as u8).collect();
            command(&mut b, &write_command(0, 0, 2, 3));
            write_payload(&mut b, &payload);
            assert_eq!(results(&mut b), [0x40, 0x80, 0, 0, 0, 2, 3]);
            assert!(b.mem[0x28ff0..0x29410].iter().all(|&v| v == 0xa5));
            assert_eq!(b.dma.addr, expected.addr);
            assert_eq!(b.dma.count, expected.count);
            assert_eq!(b.dma.page, expected.page);
            assert_eq!(b.dma.mode, expected.mode);
            assert_eq!(b.dma.mask, expected.mask);
            assert_eq!(b.dma.status, expected.status);
            assert_eq!(b.input(9), expected.read(9));
            let mut observed = b.dma.clone();
            for _ in 0..4 {
                assert_eq!(observed.advance(2), expected.advance(2));
            }
            assert_eq!(observed.addr, expected.addr);
            assert_eq!(observed.count, expected.count);
            b.output(0x15, 2);
            assert_eq!(b.dma.can_service(2), !disabled);
            b.output(0x11, 0);
            assert!(pc98_rust::firmware::disk_transfer(
                &mut b,
                0,
                [0, 0, 2, 3],
                1024,
                0x29000,
                false
            ));
            assert_eq!(&b.mem[0x29000..0x29400], &payload);
            assert_eq!(&b.mem[0x28ff0..0x29000], &[0xa5; 16]);
            assert_eq!(&b.mem[0x29400..0x29410], &[0xa5; 16]);
            command(&mut b, &[3, 0xdf, 3]);
            command(&mut b, &write_command(0, 0, 2, 3));
            write_payload(&mut b, &payload);
            assert_eq!(results(&mut b), [0x40, 0x80, 0, 0, 0, 2, 3]);
        }
    }
}

#[test]
fn original_interrupt_driven_write_probe_before_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/fdc_write/fdc_write.hdm"
    ))
    .expect("run python3 tools/prepare_fdc_write_fixture.py");
    let payload: Vec<_> = (0..1024).map(|i| (29 * i + 71) as u8).collect();
    let mut expected_media = image.clone();
    expected_media[1024..2048].copy_from_slice(&payload);
    let mut expected = [0xa5; 128];
    for (base, end) in [(0, 0x94), (64, 0x98)] {
        expected[base + 1..base + 9].copy_from_slice(&[1, 0, 4, 8, 0x80, 0, end, 1]);
        expected[base + 16..base + 23].copy_from_slice(&[0x40, 0x80, 0, 0, 0, 2, 3]);
        expected[base + 24..base + 29].copy_from_slice(&[0xd0, 0xd0, 0x80, 0, 0]);
    }
    for profile in PROFILES {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "fdc_write.hdm").unwrap();
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
                    "{profile:?}: {text}; ip={:x} record={:02x?}",
                    m.cpu.ip,
                    &m.bus.mem[0x8000..0x8080]
                );
                if text.contains("FDC WRITE PASS") {
                    break;
                }
            }
            assert!(
                m.bus.text().contains("FDC WRITE PASS"),
                "{profile:?}: ip={:x} record={:02x?}",
                m.cpu.ip,
                &m.bus.mem[0x8000..0x8080]
            );
            assert!(slots <= 2_000_000);
            assert_eq!(&m.bus.mem[0x8000..0x8080], &expected);
            assert!(m.bus.mem[0x8080..0x9000].iter().all(|&v| v == 0xa5));
            assert_eq!(&m.bus.mem[0x9000..0x9400], &payload);
            assert_eq!(&m.bus.mem[0x9400..0x9800], &payload);
            assert_eq!(&m.bus.mem[0x9800..0x9810], &[0xa5; 16]);
            assert_eq!(m.cpu.r[pc98_rust::cpu::SP], 0x7c00);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, expected_media);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, run + 1);
            assert_eq!(m.bus.fdc.writes, 1);
            println!("{profile:?} FDC WRITE boot {run}: {slots} slots; 1024 write + 1024 read byte IRQs, two result IRQs");
        }
    }
}

#[test]
fn issue_83_write_data_requests_payload_instead_of_invalid_command() {
    for profile in PROFILES {
        let mut b = setup(profile);
        // The reported reproduction, entirely through the processor interface.
        command(&mut b, &[0x45, 0, 0, 0, 2, 3, 2, 0x1b, 0xff]);
        b.tick(128);
        assert_eq!(
            b.input(0x90),
            0xb0,
            "WRITE DATA must request its first byte"
        );
        b.output(0x92, 0x59);
        assert_eq!(b.input(0x90), 0x30);
        assert_eq!(b.disks[0].as_ref().unwrap().bytes[1024], 0x59);
    }
}

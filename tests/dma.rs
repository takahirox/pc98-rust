use pc98_rust::{devices::Dma, Machine, Profile};

fn write_word(dma: &mut Dma, port: u16, word: u16) {
    dma.write(0x19, 0);
    for byte in word.to_le_bytes() {
        dma.write(port, byte);
    }
}
fn read_word(dma: &mut Dma, port: u16) -> u16 {
    dma.write(0x19, 0);
    u16::from_le_bytes([dma.read(port), dma.read(port)])
}

#[test]
fn terminal_count_matrix() {
    for channel in 0..4 {
        for decrement in [false, true] {
            for auto in [false, true] {
                for initial_count in [0u16, 1, 0x03ff, 0xffff] {
                    // Both directions cross the 16-bit boundary; banks never carry.
                    let initial_addr = if decrement { 0 } else { 0xffff };
                    let mut d = Dma::default();
                    for other in 0..4 {
                        write_word(&mut d, 1 + other * 4, 0x1234 + other);
                        write_word(&mut d, 3 + other * 4, 0x5678 + other);
                    }
                    let port = 1 + channel as u16 * 4;
                    let mode = channel as u8
                        | if decrement { 0x20 } else { 0 }
                        | if auto { 0x10 } else { 0 };
                    d.write(0x17, mode);
                    // PC-98 bank registers have a non-linear channel mapping.
                    d.write([0x27, 0x21, 0x23, 0x25][channel], 0xfa);
                    d.write(0x1f, 15);
                    d.write(0x15, channel as u8);
                    write_word(&mut d, port, initial_addr);
                    write_word(&mut d, port + 2, initial_count);
                    let neighbors_addr = d.addr;
                    let neighbors_count = d.count;
                    let length = u32::from(initial_count) + 1;
                    for round in 0..2 {
                        if round == 1 && !auto {
                            // A masked, non-auto-init channel needs guest rearming.
                            write_word(&mut d, port, initial_addr);
                            write_word(&mut d, port + 2, initial_count);
                            d.write(0x15, channel as u8);
                        }
                        for done in 1..=length {
                            assert_eq!(d.advance(channel), done == length);
                            let reload = auto && done == length;
                            let displacement = if decrement {
                                -(done as i64)
                            } else {
                                done as i64
                            };
                            let address = if reload {
                                initial_addr
                            } else {
                                (i64::from(initial_addr) + displacement).rem_euclid(65536) as u16
                            };
                            let count = if reload {
                                initial_count
                            } else {
                                (i64::from(initial_count) - i64::from(done)).rem_euclid(65536)
                                    as u16
                            };
                            assert_eq!(d.address(channel), 0xa0000 + usize::from(address));
                            assert_eq!(d.count[channel], count);
                            assert_eq!(
                                d.mask,
                                if !auto && done == length {
                                    15
                                } else {
                                    15 ^ (1 << channel)
                                }
                            );
                            assert_eq!(
                                d.status,
                                if round == 1 || done == length {
                                    1 << channel
                                } else {
                                    0
                                }
                            );
                            // Read current registers while active, after wrap and after reload.
                            if done == 1 || done == length / 2 || done == length {
                                assert_eq!(read_word(&mut d, port), address);
                                assert_eq!(read_word(&mut d, port + 2), count);
                            }
                        }
                        for other in 0..4 {
                            if other != channel {
                                assert_eq!(d.addr[other], neighbors_addr[other]);
                                assert_eq!(d.count[other], neighbors_count[other]);
                                assert_eq!(d.page[other], 0);
                                assert_eq!(d.mode[other], 0);
                            }
                        }
                        // Keep sticky TC through the second service: it must not
                        // turn any non-terminal advance into a new event.
                    }
                    assert_eq!(d.read(0x11), 1 << channel);
                    assert_eq!(d.read(0x11), 0);
                }
            }
        }
    }
}

#[test]
fn shared_byte_pointer_and_partial_byte_reprogramming() {
    let mut d = Dma::default();
    write_word(&mut d, 1, 0x1234);
    write_word(&mut d, 7, 0x5678);
    assert_eq!(d.read(1), 0x34); // low address of channel 0
    assert_eq!(d.read(7), 0x56); // high count of channel 1
    d.write(1, 0xab); // low address of channel 0
    d.write(0x17, 0x11); // mode/status accesses do not change byte pointer
    assert_eq!(d.read(0x11), 0);
    d.write(7, 0xcd); // high count of channel 1
    assert_eq!(read_word(&mut d, 1), 0x12ab);
    assert_eq!(read_word(&mut d, 7), 0xcd78);
    d.read(1); // leave pointer high, then explicitly reset
    d.write(0x19, 0);
    assert_eq!(d.read(7), 0x78);

    for channel in 0..4 {
        for address in [true, false] {
            for high in [false, true] {
                let mut d = Dma::default();
                let port = 1 + channel as u16 * 4;
                write_word(&mut d, port, 0x12ff);
                write_word(&mut d, port + 2, 0x0200);
                d.write(0x17, 0x10 | channel as u8);
                d.write(0x1f, 0b1010); // auto-init preserves even a set mask bit
                assert!(!d.advance(channel)); // current 1300/01ff, base 12ff/0200
                d.write(0x19, 0);
                let target = port + if address { 0 } else { 2 };
                if high {
                    d.read(target); // select high byte without modifying either base
                }
                d.write(target, 3);
                let replace = |word: u16| {
                    if high {
                        (word & 255) | 0x0300
                    } else {
                        (word & 0xff00) | 3
                    }
                };
                let base_addr = if address { replace(0x12ff) } else { 0x12ff };
                let base_count = if address { 0x0200 } else { replace(0x0200) };
                let current_addr = if address { replace(0x1300) } else { 0x1300 };
                let current_count = if address { 0x01ff } else { replace(0x01ff) };
                assert_eq!(read_word(&mut d, port), current_addr);
                assert_eq!(read_word(&mut d, port + 2), current_count);
                for remaining in (0..=current_count).rev() {
                    assert_eq!(d.advance(channel), remaining == 0);
                }
                assert_eq!(read_word(&mut d, port), base_addr);
                assert_eq!(read_word(&mut d, port + 2), base_count);
                assert_eq!(d.mask, 0b1010);
                assert_eq!(d.read(0x11), 1 << channel);
                assert_eq!(d.read(0x11), 0);
            }
        }
    }
}

#[test]
fn interleaved_channels_keep_bases_and_latched_status_independent() {
    let mut d = Dma::default();
    for channel in 0..4u16 {
        write_word(&mut d, 1 + channel * 4, 0x21ff + channel * 0x100);
        write_word(&mut d, 3 + channel * 4, channel);
        d.write(0x17, 0x10 | channel as u8);
    }
    d.write(0x1f, 0b0101);
    let mut latched = 0;
    // Interleave service while other channels are partly advanced or reloaded.
    for step in 0..4u16 {
        for channel in 0..4u16 {
            if step <= channel {
                assert_eq!(d.advance(channel as usize), step == channel);
                if step == channel {
                    latched |= 1 << channel;
                }
            }
        }
        assert_eq!(d.status, latched);
        assert_eq!(d.mask, 0b0101);
        for channel in 0..4u16 {
            let serviced = (step + 1).min(channel + 1);
            let progress = if serviced == channel + 1 { 0 } else { serviced };
            assert_eq!(
                read_word(&mut d, 1 + channel * 4),
                0x21ff + channel * 0x100 + progress
            );
            assert_eq!(read_word(&mut d, 3 + channel * 4), channel - progress);
        }
    }
    assert_eq!(d.read(0x11), 15);
    assert_eq!(d.read(0x11), 0);
    // The first byte of channel 3's next service must not see old TC.
    assert!(!d.advance(3));
    assert_eq!(d.status, 0);
}

#[test]
fn master_clear_retains_programming_matrix() {
    for decrement in [false, true] {
        for auto in [false, true] {
            for high in [false, true] {
                for data in [0, 0x55, 0xff] {
                    let mut d = Dma::default();
                    for channel in 0..4 {
                        let port = 1 + channel as u16 * 4;
                        // Latch all four TC bits before programming distinct bases.
                        assert!(d.advance(channel));
                        write_word(&mut d, port, 0x2340 + channel as u16 * 0x100);
                        write_word(&mut d, port + 2, 3 + channel as u16);
                        d.write(
                            0x17,
                            channel as u8
                                | 0x40
                                | if decrement { 0x20 } else { 0 }
                                | if auto { 0x10 } else { 0 },
                        );
                        assert!(!d.advance(channel));
                    }
                    assert_eq!(d.status, 15);
                    let addresses = d.addr;
                    let counts = d.count;
                    let modes = d.mode;
                    d.write(0x1f, 0b1010);
                    for _ in 0..2 {
                        d.write(0x19, 0);
                        if high {
                            d.read(7);
                        }
                        d.write(0x1b, data);
                        // FIRST register access: no helper/19h can hide a bad reset.
                        assert_eq!(d.read(9), addresses[2] as u8);
                        assert_eq!(d.read(9), (addresses[2] >> 8) as u8);
                        assert_eq!(d.read(0x11), 0);
                        assert_eq!(d.mask, 15);
                        assert_eq!(d.mode, modes);
                        for channel in 0..4 {
                            let port = 1 + channel as u16 * 4;
                            assert_eq!(read_word(&mut d, port), addresses[channel]);
                            assert_eq!(read_word(&mut d, port + 2), counts[channel]);
                        }
                    }
                    for channel in 0..4 {
                        d.write(0x15, channel as u8);
                        for remaining in (0..=counts[channel]).rev() {
                            assert_eq!(d.advance(channel), remaining == 0);
                            if remaining != 0 {
                                let done = counts[channel] - remaining + 1;
                                let expected = if decrement {
                                    addresses[channel].wrapping_sub(done)
                                } else {
                                    addresses[channel].wrapping_add(done)
                                };
                                assert_eq!(read_word(&mut d, 1 + channel as u16 * 4), expected);
                                assert_eq!(
                                    read_word(&mut d, 3 + channel as u16 * 4),
                                    remaining - 1
                                );
                            }
                        }
                        let base = 0x2340 + channel as u16 * 0x100;
                        let end = if decrement {
                            base - 4 - channel as u16
                        } else {
                            base + 4 + channel as u16
                        };
                        assert_eq!(
                            read_word(&mut d, 1 + channel as u16 * 4),
                            if auto { base } else { end }
                        );
                        assert_eq!(
                            read_word(&mut d, 3 + channel as u16 * 4),
                            if auto { 3 + channel as u16 } else { 0xffff }
                        );
                        assert_eq!(d.mask & (1 << channel), if auto { 0 } else { 1 << channel });
                        assert_eq!(d.read(0x11), 1 << channel);
                    }
                }
            }
        }
    }
}

#[test]
fn master_clear_preserves_partial_address_and_count_bytes() {
    for channel in 0..4 {
        for address in [false, true] {
            for high in [false, true] {
                let mut d = Dma::default();
                let port = 1 + channel as u16 * 4;
                write_word(&mut d, port, 0x12ff);
                write_word(&mut d, port + 2, 0x0200);
                d.write(0x17, 0x10 | channel as u8);
                assert!(!d.advance(channel)); // current 1300/01ff, base 12ff/0200
                let target = port + if address { 0 } else { 2 };
                if high {
                    d.read(target);
                }
                d.write(target, 3); // only one programmed byte before master clear
                d.write(0x1b, 0x55);
                let replace = |v: u16| {
                    if high {
                        (v & 255) | 0x300
                    } else {
                        (v & 0xff00) | 3
                    }
                };
                let current_addr = if address { replace(0x1300) } else { 0x1300 };
                let current_count = if address { 0x01ff } else { replace(0x01ff) };
                assert_eq!(d.read(port), current_addr as u8);
                assert_eq!(d.read(port), (current_addr >> 8) as u8);
                assert_eq!(read_word(&mut d, port + 2), current_count);
                d.write(0x15, channel as u8);
                for remaining in (0..=current_count).rev() {
                    assert_eq!(d.advance(channel), remaining == 0);
                }
                assert_eq!(
                    read_word(&mut d, port),
                    if address { replace(0x12ff) } else { 0x12ff }
                );
                assert_eq!(
                    read_word(&mut d, port + 2),
                    if address { 0x0200 } else { replace(0x0200) }
                );
            }
        }
    }
}

fn program_fdc_dma(m: &mut Machine, len: usize, write: bool) {
    for (port, word) in [(9, 0x2000), (11, (len - 1) as u16)] {
        m.bus.output(0x19, 0);
        for byte in word.to_le_bytes() {
            m.bus.output(port, byte);
        }
    }
    m.bus.output(0x23, 0);
    m.bus.output(0x17, if write { 0x5a } else { 0x56 });
    m.bus.output(0x15, 2);
}
fn raw_command(m: &mut Machine, write: bool, sector: u8) -> Vec<u8> {
    raw_command_at_service(m, write, sector, |_| {})
}
fn raw_command_at_service(
    m: &mut Machine,
    write: bool,
    sector: u8,
    before_service: impl FnOnce(&mut pc98_rust::bus::Bus),
) -> Vec<u8> {
    let b = &mut m.bus;
    assert_eq!(b.input(0x90), 0x80);
    assert_eq!(b.irq(), None);
    for byte in [
        if write { 0x45 } else { 0x46 },
        0,
        0,
        0,
        sector,
        3,
        8,
        0x1b,
        0xff,
    ] {
        b.output(0x92, byte);
    }
    b.tick(159);
    assert_eq!(b.irq(), None);
    assert_eq!(b.input(0x90), 0x90);
    before_service(b);
    b.tick(1);
    assert_eq!(b.irq(), Some(0x13));
    b.output(8, 0x20);
    b.output(0, 0x20);
    let mut result = Vec::new();
    for _ in 0..7 {
        assert_eq!(b.input(0x90), 0xd0);
        result.push(b.input(0x92));
    }
    assert_eq!(b.input(0x90), 0x80);
    assert_eq!(b.input(0x92), 0x80); // exactly seven result bytes
    result
}
fn command(m: &mut Machine, write: bool, sector: u8, len: usize) {
    let result = raw_command(m, write, sector);
    let b = &mut m.bus;
    assert_eq!(result[0], 0, "{result:?}");
    assert_eq!(&result[1..3], &[0, 0]);
    assert_eq!(&result[3..], &[0, 0, sector + ((len - 1) / 1024) as u8, 3]);
    assert_eq!(read_word(&mut b.dma, 9), 0x2000);
    assert_eq!(read_word(&mut b.dma, 11), (len - 1) as u16);
    assert_eq!(b.dma.mask & 4, 0);
}

#[test]
fn fdc_dma_remains_physical_while_cpu_uses_a_nonidentity_page() {
    use pc98_rust::cpu::{Cpu, SegmentCache, AX, CS};
    let image: Vec<u8> = (0..1_261_568).map(|i| (i * 13 + 7) as u8).collect();
    for (write, vm86) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.insert(0, image.clone(), "original.hdm").unwrap();
        m.bus.output(2, 0x7f);
        m.bus.output(10, 0xf7);
        m.bus.output(0x94, 0x18);
        for (address, value) in [(0x10000, 0x11003u32), (0x11004, 0x1003), (0x11008, 0x3003)] {
            let value = value | if vm86 { 4 } else { 0 };
            m.bus.mem[address..address + 4].copy_from_slice(&value.to_le_bytes());
        }
        let code = if vm86 {
            [0x66, 0xa1, 0, 0x20, 0x90]
        } else {
            [0xa1, 0, 0x20, 0, 0]
        };
        m.bus.mem[0x1000..0x1005].copy_from_slice(&code);
        m.bus.mem[0x2000..0x2400].fill(0x5a);
        m.bus.mem[0x3000..0x3400].fill(0xa5);
        m.cpu = Cpu {
            cr0: 0x80000011,
            cr3: 0x10000,
            s: [0; 4],
            ip: 0x1000,
            flags: 2,
            flags_upper: if vm86 { 2 } else { 0 },
            cpl: if vm86 { 3 } else { 0 },
            ..Cpu::default()
        };
        m.cpu.segment_caches[CS] = Some(SegmentCache {
            base: 0,
            limit: 0xffff,
            access: if vm86 { 0xfb } else { 0x9b },
            attributes: if vm86 { 0 } else { 0x40 },
        });
        m.cpu.step(&mut m.bus).unwrap();
        assert_eq!(m.cpu.reg32(AX), 0xa5a5a5a5);
        program_fdc_dma(&mut m, 1024, write);
        command(&mut m, write, 2, 1024);
        if write {
            assert_eq!(
                &m.bus.disks[0].as_ref().unwrap().bytes[1024..2048],
                &[0x5a; 1024]
            );
        } else {
            assert_eq!(&m.bus.mem[0x2000..0x2400], &image[1024..2048]);
        }
        assert_eq!(&m.bus.mem[0x3000..0x3400], &[0xa5; 1024]);
        m.cpu.ip = 0x1000;
        m.cpu.step(&mut m.bus).unwrap();
        assert_eq!(m.cpu.reg32(AX), 0xa5a5a5a5);
    }
}

#[test]
fn raw_fdc_reuses_auto_init_for_consecutive_reads_and_writes() {
    let image: Vec<u8> = (0..1_261_568)
        .map(|i| ((i / 1024) * 37 + i % 1024 * 13) as u8)
        .collect();
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for write in [false, true] {
            // Before EOT, multi-sector, partial second sector, partial first sector.
            for len in [1024, 2048, 1301, 37] {
                let mut m = Machine::new(profile);
                m.insert(0, image.clone(), "original.hdm").unwrap();
                m.bus.output(2, 0x7f); // only cascade IRQ enabled
                m.bus.output(10, 0xf7); // only FDC IRQ enabled
                m.bus.output(0x94, 0x18);
                m.bus.mem[0x1fe0..0x2020 + len].fill(0xcc);
                program_fdc_dma(&mut m, len, write);
                let mut expected_disk = image.clone();
                for sector in [2, 5] {
                    let offset = (sector as usize - 1) * 1024;
                    let payload: Vec<u8> = (0..len)
                        .map(|i| (i * 19 + sector as usize * 41) as u8)
                        .collect();
                    m.bus.mem[0x2000..0x2000 + len].fill(0xa5);
                    if write {
                        m.bus.mem[0x2000..0x2000 + len].copy_from_slice(&payload);
                        expected_disk[offset..offset + len].copy_from_slice(&payload);
                    }
                    command(&mut m, write, sector, len);
                    assert_eq!(&m.bus.mem[0x1fe0..0x2000], &[0xcc; 32]);
                    assert_eq!(&m.bus.mem[0x2000 + len..0x2020 + len], &[0xcc; 32]);
                    assert_eq!(
                        &m.bus.mem[0x2000..0x2000 + len],
                        if write {
                            &payload
                        } else {
                            &image[offset..offset + len]
                        }
                    );
                    assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, expected_disk);
                    // Deliberately retain the status latch between commands.
                    assert_eq!(m.bus.dma.status & 4, 4);
                }
                assert_eq!(m.bus.input(0x11) & 4, 4);
                assert_eq!(m.bus.input(0x11) & 4, 0);
                let sectors = 2 * len.div_ceil(1024) as u64;
                assert_eq!(
                    if write {
                        m.bus.fdc.writes
                    } else {
                        m.bus.fdc.reads
                    },
                    sectors
                );
            }
        }
    }
}

#[test]
fn raw_fdc_master_clear_masks_then_resumes_retained_read_and_write() {
    let image: Vec<u8> = (0..1_261_568)
        .map(|i| (i * 13 + i / 1024 * 37) as u8)
        .collect();
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for write in [false, true] {
            let mut m = Machine::new(profile);
            m.insert(0, image.clone(), "master-clear.hdm").unwrap();
            m.bus.output(2, 0x7f);
            m.bus.output(10, 0xf7);
            m.bus.output(0x94, 0x18);
            m.bus.mem[0x1fe0..0x2420].fill(0xcc);
            let payload: Vec<u8> = (0..1024).map(|i| (i * 19 + 41) as u8).collect();
            m.bus.mem[0x2000..0x2400].copy_from_slice(&payload);
            program_fdc_dma(&mut m, 1024, write);
            m.bus.output(0x1b, 0xff);
            assert_eq!(m.bus.input(9), 0);
            assert_eq!(m.bus.input(9), 0x20);
            let before = m.bus.mem[0x1fe0..0x2420].to_vec();
            assert_eq!(raw_command(&mut m, write, 2), [0x40, 0x10, 0, 0, 0, 2, 3]);
            assert_eq!(&m.bus.mem[0x1fe0..0x2420], &before);
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
            assert_eq!((m.bus.fdc.reads, m.bus.fdc.writes), (0, 0));
            assert_eq!(m.bus.dma.mask, 15);
            assert_eq!(m.bus.input(0x11), 0);
            // No address/count/mode rewrite after master clear or masked service.
            m.bus.output(0x15, 2);
            command(&mut m, write, 2, 1024);
            let mut expected_disk = image.clone();
            if write {
                expected_disk[1024..2048].copy_from_slice(&payload);
            }
            assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, expected_disk);
            assert_eq!(
                &m.bus.mem[0x2000..0x2400],
                if write { &payload } else { &image[1024..2048] }
            );
            assert_eq!(&m.bus.mem[0x1fe0..0x2000], &[0xcc; 32]);
            assert_eq!(&m.bus.mem[0x2400..0x2420], &[0xcc; 32]);
            assert_eq!(
                (m.bus.fdc.reads, m.bus.fdc.writes),
                if write { (0, 1) } else { (1, 0) }
            );
            assert_eq!(m.bus.input(0x11), 4);
            assert_eq!(m.bus.input(0x11), 0);
        }
    }
}

#[test]
fn original_boot_probe_runs_on_both_profiles() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/dma/autoinit.hdm"
    ))
    .expect("run python3 tools/prepare_dma_fixture.py");
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "autoinit.hdm").unwrap();
        m.reset();
        for _ in 0..20 {
            m.step(100_000);
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("DMA DISABLE PASS") {
                break;
            }
        }
        assert!(
            m.bus.text().contains("DMA AUTO INIT PASS"),
            "{}",
            m.bus.text()
        );
        assert!(m.bus.text().contains("DMA MASTER CLEAR PASS"));
        assert!(
            m.bus.text().contains("DMA DISABLE PASS"),
            "{}",
            m.bus.text()
        );
        assert!(!m.bus.text().contains("FAIL"));
        assert_eq!(m.bus.fdc.reads, 5); // IPL, auto-init, retained and enabled reads
        assert_eq!(m.bus.fdc.writes, 4);
        let disk = &m.bus.disks[0].as_ref().unwrap().bytes;
        assert_eq!(&disk[4 * 1024..5 * 1024], &vec![0x69; 1024]);
        assert_eq!(&disk[5 * 1024..6 * 1024], &vec![0xb6; 1024]);
        assert_eq!(&disk[..3 * 1024], &image[..3 * 1024]);
        assert_eq!(&disk[3 * 1024..4 * 1024], &[0xd2; 1024]);
        assert_eq!(&disk[6 * 1024..7 * 1024], &image[6 * 1024..7 * 1024]);
        assert_eq!(&disk[7 * 1024..8 * 1024], &[0x4d; 1024]);
        assert_eq!(&disk[8 * 1024..], &image[8 * 1024..]);
        assert_eq!(&m.bus.mem[0x8000..0x8400], &[0xd2; 1024]);
        assert_eq!(m.bus.mem[0x7fff], 0xa5);
        assert_eq!(m.bus.mem[0x8400], 0xa5);
    }
}

#[test]
fn command_writes_preserve_programming_pointer_and_tc() {
    for channel in 0..4 {
        for address in [false, true] {
            for high in [false, true] {
                let mut d = Dma::default();
                for n in 0..4 {
                    assert!(d.advance(n)); // latch TC independently of programming
                    write_word(&mut d, 1 + n as u16 * 4, 0x12ff + n as u16 * 0x100);
                    write_word(&mut d, 3 + n as u16 * 4, 0x0200 + n as u16);
                    d.write(0x17, 0x50 | n as u8);
                    d.write([0x27, 0x21, 0x23, 0x25][n], n as u8 + 1);
                    assert!(!d.advance(n)); // current and base now differ
                }
                d.write(0x1f, 0b1010);
                let (addresses, counts, modes, pages) = (d.addr, d.count, d.mode, d.page);
                let port = 1 + channel as u16 * 4 + if address { 0 } else { 2 };
                if high {
                    d.read(port);
                }
                for value in [4, 4, 0, 0, 0xff, 0xfb, 4] {
                    d.write(0x11, value);
                    assert_eq!(d.addr, addresses);
                    assert_eq!(d.count, counts);
                    assert_eq!(d.mode, modes);
                    assert_eq!(d.page, pages);
                    assert_eq!(d.mask, 0b1010);
                    assert_eq!(d.status, 15);
                }
                // Status, not command readback; neither read changes the pointer.
                assert_eq!(d.read(0x11), 15);
                assert_eq!(d.read(0x11), 0);
                d.write(port, 3); // programming remains possible while disabled
                let replace = |word: u16| {
                    if high {
                        (word & 255) | 0x0300
                    } else {
                        (word & 0xff00) | 3
                    }
                };
                let current_addr = if address {
                    replace(addresses[channel])
                } else {
                    addresses[channel]
                };
                let current_count = if address {
                    counts[channel]
                } else {
                    replace(counts[channel])
                };
                assert_eq!(read_word(&mut d, 1 + channel as u16 * 4), current_addr);
                assert_eq!(read_word(&mut d, 3 + channel as u16 * 4), current_count);
                // Enable, then service to TC: retained bases must still reload.
                d.write(0x11, 0);
                for remaining in (0..=current_count).rev() {
                    assert_eq!(d.advance(channel), remaining == 0);
                }
                let base_addr = 0x12ff + channel as u16 * 0x100;
                let base_count = 0x0200 + channel as u16;
                assert_eq!(
                    d.addr[channel],
                    if address {
                        replace(base_addr)
                    } else {
                        base_addr
                    }
                );
                assert_eq!(
                    d.count[channel],
                    if address {
                        base_count
                    } else {
                        replace(base_count)
                    }
                );
                assert_eq!(d.mask, 0b1010);
                assert_eq!(d.read(0x11), 1 << channel);
            }
        }
    }
}

#[test]
fn raw_fdc_controller_disable_samples_service_and_retains_programming() {
    let image: Vec<u8> = (0..1_261_568)
        .map(|i| (i * 13 + i / 1024 * 37) as u8)
        .collect();
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for write in [false, true] {
            for auto in [false, true] {
                for late_disable in [false, true] {
                    let mut m = Machine::new(profile);
                    m.insert(0, image.clone(), "disable.hdm").unwrap();
                    m.bus.output(2, 0x7f);
                    m.bus.output(10, 0xf7);
                    m.bus.output(0x94, 0x18);
                    m.bus.mem[0x1fe0..0x2420].fill(0xcc);
                    let payload: Vec<u8> = (0..1024).map(|i| (i * 19 + 41) as u8).collect();
                    m.bus.mem[0x2000..0x2400].copy_from_slice(&payload);
                    // Keep an unrelated TC latch and mask through all commands.
                    assert!(m.bus.dma.advance(0));
                    program_fdc_dma(&mut m, 1024, write);
                    m.bus.output(
                        0x17,
                        if write { 0x4a } else { 0x46 } | if auto { 0x10 } else { 0 },
                    );
                    let before = m.bus.mem[0x1fe0..0x2420].to_vec();
                    let (addresses, counts, modes, pages, mask) = (
                        m.bus.dma.addr,
                        m.bus.dma.count,
                        m.bus.dma.mode,
                        m.bus.dma.page,
                        m.bus.dma.mask,
                    );
                    if !late_disable {
                        m.bus.output(0x11, 4);
                    }
                    assert_eq!(
                        raw_command_at_service(&mut m, write, 2, |b| {
                            if late_disable {
                                b.output(0x11, 4);
                            }
                        }),
                        [0x40, 0x10, 0, 0, 0, 2, 3]
                    );
                    // Clearing disable cannot replay the completed command.
                    m.bus.output(0x11, 0);
                    m.bus.tick(1000);
                    assert_eq!(m.bus.irq(), None);
                    assert_eq!(m.bus.input(0x90), 0x80);
                    // Re-enabling must also respect a channel mask.
                    m.bus.output(0x15, 6);
                    m.bus.output(0x11, 4);
                    m.bus.output(0x11, 0);
                    assert_eq!(raw_command(&mut m, write, 2), [0x40, 0x10, 0, 0, 0, 2, 3]);
                    m.bus.output(0x15, 2);
                    assert_eq!(&m.bus.mem[0x1fe0..0x2420], &before);
                    assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
                    assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
                    assert_eq!((m.bus.fdc.reads, m.bus.fdc.writes), (0, 0));
                    assert_eq!(m.bus.dma.addr, addresses);
                    assert_eq!(m.bus.dma.count, counts);
                    assert_eq!(m.bus.dma.mode, modes);
                    assert_eq!(m.bus.dma.page, pages);
                    assert_eq!(m.bus.dma.mask, mask);
                    assert_eq!(m.bus.dma.status, 1);
                    // Disable and fail again, then clear ONLY disable before a
                    // fresh command, without any address/count/mode/mask writes.
                    m.bus.output(0x11, 4);
                    assert_eq!(raw_command(&mut m, write, 2), [0x40, 0x10, 0, 0, 0, 2, 3]);
                    m.bus.output(0x11, 0);
                    assert_eq!(raw_command(&mut m, write, 2), [0, 0, 0, 0, 0, 2, 3]);
                    let mut expected_disk = image.clone();
                    if write {
                        expected_disk[1024..2048].copy_from_slice(&payload);
                    }
                    assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, expected_disk);
                    assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, u64::from(write));
                    assert_eq!(
                        &m.bus.mem[0x2000..0x2400],
                        if write { &payload } else { &image[1024..2048] }
                    );
                    assert_eq!(&m.bus.mem[0x1fe0..0x2000], &[0xcc; 32]);
                    assert_eq!(&m.bus.mem[0x2400..0x2420], &[0xcc; 32]);
                    assert_eq!(
                        (m.bus.fdc.reads, m.bus.fdc.writes),
                        if write { (0, 1) } else { (1, 0) }
                    );
                    assert_eq!(
                        read_word(&mut m.bus.dma, 9),
                        if auto { 0x2000 } else { 0x2400 }
                    );
                    assert_eq!(
                        read_word(&mut m.bus.dma, 11),
                        if auto { 1023 } else { 0xffff }
                    );
                    assert_eq!(m.bus.dma.mask, if auto { mask } else { mask | 4 });
                    assert_eq!(m.bus.input(0x11), 5);
                    assert_eq!(m.bus.input(0x11), 0);
                }
            }
        }
    }
}

#[test]
fn machine_reset_and_master_clear_enable_controller_without_bypassing_masks() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for master_clear in [false, true] {
            let mut m = Machine::new(profile);
            m.insert(0, vec![0x39; 1_261_568], "reset.hdm").unwrap();
            program_fdc_dma(&mut m, 1024, false);
            m.bus.output(0x11, 4);
            if master_clear {
                m.bus.output(0x1b, 0xff);
                assert_eq!(m.bus.dma.mask, 15);
            } else {
                m.reset();
                assert_eq!(m.bus.dma.addr, [0; 4]);
                assert_eq!(m.bus.dma.count, [0; 4]);
                assert_eq!(m.bus.dma.mode, [0; 4]);
                assert_eq!(m.bus.dma.status, 0);
                program_fdc_dma(&mut m, 1024, false);
                m.bus.output(0x15, 6);
            }
            m.bus.output(2, 0x7f);
            m.bus.output(10, 0xf7);
            m.bus.output(0x94, 0x18);
            assert_eq!(raw_command(&mut m, false, 2), [0x40, 0x10, 0, 0, 0, 2, 3]);
            m.bus.output(0x15, 2); // no command-register write after reset
            command(&mut m, false, 2, 1024);
            assert_eq!(&m.bus.mem[0x2000..0x2400], &[0x39; 1024]);
        }
    }
}

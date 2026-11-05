use pc98_rust::{cpu::*, firmware, Machine, Profile};
#[test]
fn reproduce_idle_nd_bios_handoff() {
    let mut failures = 0;
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for guest in [false, true] {
            for write in [false, true] {
                for cylinder in [0u8, 2] {
                    let mut m = Machine::new(profile);
                    let image: Vec<_> = (0..1_261_568).map(|i| (i ^ (i >> 8)) as u8).collect();
                    m.insert(0, image.clone(), "original.hdm").unwrap();
                    m.bus.output(0x11, 0);
                    for byte in [3, 0xdf, 3] {
                        m.bus.output(0x92, byte);
                    }
                    m.bus.tick(160);
                    assert_eq!(m.bus.input(0x90), 0x80);
                    m.bus.mem[0x8ff0..0x9410].fill(0xa5);
                    let payload: Vec<_> = (0..1024)
                        .map(|i| (i as u8).wrapping_mul(29).wrapping_add(71))
                        .collect();
                    if write {
                        m.bus.mem[0x9000..0x9400].copy_from_slice(&payload);
                    }
                    let offset = cylinder as usize * 16384 + 1024;
                    let mut expected_media = image.clone();
                    if write {
                        expected_media[offset..offset + 1024].copy_from_slice(&payload);
                    }
                    let ok = if guest {
                        m.cpu.s = [0; 4];
                        m.cpu.ip = 0x100;
                        m.cpu.flags = 2;
                        m.cpu.r[SP] = 0x7c00;
                        m.cpu.r[AX] = if write { 0x0590 } else { 0x0690 };
                        m.cpu.r[CX] = 0x0300 | cylinder as u16;
                        m.cpu.r[DX] = 2;
                        m.cpu.r[BX] = 1024;
                        m.cpu.r[BP] = 0x9000;
                        m.bus.mem[0x100..0x103].copy_from_slice(&[0xcd, 0x1b, 0xf4]);
                        m.step(8);
                        assert!(m.error.is_none(), "{:?}", m.error);
                        m.cpu.flags & CF == 0 && m.cpu.byte(4) == 0
                    } else {
                        firmware::disk_transfer(
                            &mut m.bus,
                            0,
                            [cylinder, 0, 2, 3],
                            1024,
                            0x9000,
                            write,
                        )
                    };
                    let data_ok = &m.bus.mem[0x9000..0x9400]
                        == if write {
                            &payload[..]
                        } else {
                            &image[offset..offset + 1024]
                        };
                    let media_ok = m.bus.disks[0].as_ref().unwrap().bytes == expected_media;
                    let msr = m.bus.input(0x90);
                    println!("{profile:?} guest={guest} write={write} C={cylinder} ok={ok} CF={} AH={:02x} payload={data_ok} media={media_ok} DMA={:04x}/{:04x} TC={} MSR={msr:02x}",m.cpu.flags & CF,m.cpu.byte(4),m.bus.dma.addr[2],m.bus.dma.count[2],m.bus.dma.status & 4);
                    if !(ok
                        && data_ok
                        && media_ok
                        && m.bus.dma.addr[2] == 0x9400
                        && m.bus.dma.count[2] == 0xffff
                        && m.bus.dma.status & 4 != 0
                        && msr == 0x80)
                    {
                        failures += 1;
                    }
                    assert_eq!(&m.bus.mem[0x8ff0..0x9000], &[0xa5; 16]);
                    assert_eq!(&m.bus.mem[0x9400..0x9410], &[0xa5; 16]);
                }
            }
        }
    }
    assert_eq!(failures, 0);
}

#[test]
fn installed_units_are_independent_of_media_and_empty_reads_fail() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for mask in 0..4 {
            let mut m = Machine::new(profile);
            for drive in 0..2 {
                if mask & (1 << drive) != 0 {
                    m.insert(drive, vec![0x5a; 1_261_568], "data.hdm").unwrap();
                }
            }
            m.reset();
            assert_eq!(m.bus.mem[0x55c], 3);
            assert_eq!(m.bus.mem[0x55d], 0); // no IDE advertised by floppy equipment
            for drive in 0..2 {
                m.cpu.s = [0; 4];
                m.cpu.ip = 0x100;
                m.cpu.r[SP] = 0x7c00;
                m.cpu.flags = 2;
                m.cpu.halted = false;
                m.cpu.r[AX] = 0x0690 | drive as u16;
                m.cpu.r[CX] = 0x0300;
                m.cpu.r[DX] = 1;
                m.cpu.r[BX] = 1024;
                m.cpu.r[BP] = 0x9000;
                m.bus.mem[0x9000..0x9400].fill(0xa5);
                m.bus.mem[0x100..0x103].copy_from_slice(&[0xcd, 0x1b, 0xf4]);
                m.step(8);
                assert!(m.error.is_none(), "{:?}", m.error);
                if mask & (1 << drive) == 0 {
                    assert_ne!(m.cpu.flags & CF, 0);
                    assert_eq!(m.cpu.byte(4), 0x80); // not ready
                    assert_eq!(&m.bus.mem[0x9000..0x9400], &[0xa5; 1024]);
                } else {
                    assert_eq!(m.cpu.flags & CF, 0);
                    assert_eq!(m.cpu.byte(4), 0);
                    assert_eq!(&m.bus.mem[0x9000..0x9400], &[0x5a; 1024]);
                    assert_eq!(m.bus.mem[0x564 + drive * 8] & 0xc0, 0);
                }
            }
        }
    }
}

#[test]
fn media_ready_transitions_invalidate_only_the_selected_bios_unit() {
    let mut m = Machine::default();
    for drive in 0..2 {
        m.insert(drive, vec![0x5a; 1_261_568], "data.hdm").unwrap();
        assert!(firmware::disk_transfer(
            &mut m.bus,
            drive as u8,
            [0, 0, 1, 3],
            1024,
            0x9000,
            false
        ));
    }
    let first_status = m.bus.mem[0x564..0x56b].to_vec();
    assert_eq!(m.bus.mem[0x56c] & 0xc0, 0);
    m.bus.mount_floppy(1, None);
    assert_eq!(m.bus.mem[0x56c], 0xc1);
    assert_eq!(&m.bus.mem[0x564..0x56b], first_status);
    assert!(!firmware::disk_transfer(
        &mut m.bus,
        1,
        [0, 0, 1, 3],
        1024,
        0x9000,
        false
    ));
    assert_eq!(m.bus.mem[0x56c] & 0xc0, 0x40);
    m.insert(1, vec![0x3c; 1_261_568], "replacement.hdm")
        .unwrap();
    assert_eq!(m.bus.mem[0x56c], 0xc1);
    assert!(firmware::disk_transfer(
        &mut m.bus,
        1,
        [0, 0, 1, 3],
        1024,
        0x9000,
        false
    ));
    assert_eq!(&m.bus.mem[0x9000..0x9400], &[0x3c; 1024]);
    assert_eq!(m.bus.mem[0x56c] & 0xc0, 0);
    assert_eq!(&m.bus.mem[0x564..0x56b], first_status);
    assert_eq!(m.bus.mem[0x55c], 3);
}

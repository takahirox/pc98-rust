//! Original MIT sparse-fixture tests for #127. No large files in Git/CI artifacts.
mod common;
use pc98_rust::{
    hdd::{HardDisk, HdiImport, Ide, PAGE_BYTES, TRANSFER_BYTES},
    Machine, Profile,
};

fn header(cylinders: u32, offset: u32) -> Vec<u8> {
    let mut bytes = vec![0; 32];
    for (at, v) in [
        (8, offset),
        (12, cylinders * 16 * 32 * 512),
        (16, 512),
        (20, 32),
        (24, 16),
        (28, cylinders),
    ] {
        bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    bytes
}
fn sparse(cylinders: u32) -> HardDisk {
    let header = header(cylinders, 4096);
    let len = 4096 + cylinders as usize * 16 * 32 * 512;
    let mut import = HdiImport::new(&header, len as u64).unwrap();
    let mut chunk = vec![0; TRANSFER_BYTES];
    chunk[..32].copy_from_slice(&header);
    let probe =
        std::fs::read("fixtures/large_hdi/probe.bin").expect("prepare_large_hdi_fixture.py");
    chunk[4096..4096 + probe.len()].copy_from_slice(&probe);
    for at in (0..len).step_by(TRANSFER_BYTES) {
        import
            .append(&chunk[..TRANSFER_BYTES.min(len - at)])
            .unwrap();
        if at == 0 {
            chunk.fill(0);
        }
    }
    import.finish().unwrap()
}
fn address(ide: &mut Ide, lba: usize, count: u8, command: u8) {
    for (port, byte) in [
        (0x644, count),
        (0x646, lba as u8),
        (0x648, (lba >> 8) as u8),
        (0x64a, (lba >> 16) as u8),
        (0x64c, 0xe0 | ((lba >> 24) as u8 & 15)),
        (0x64e, command),
    ] {
        ide.output(port, byte);
    }
}
fn probe(m: &mut Machine, persisted: bool) {
    for _ in 0..50 {
        common::advance(m, 10_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
        let text = m.bus.text();
        assert!(!text.contains("LARGE HDI FAIL"), "{text}");
        if text.contains("LARGE HDI PASS") {
            assert_eq!(text.contains("LARGE HDI PERSIST"), persisted);
            return;
        }
    }
    panic!("guest timed out: {}", m.bus.text());
}
#[test]
fn sparse_512mib_and_1gib_real_guest_pio_bios_and_reset() {
    for cylinders in [2048, 4096] {
        let disk = sparse(cylinders);
        assert_eq!(disk.resident_bytes(), PAGE_BYTES);
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.bus.ide.mount(Some(disk));
        m.bus.boot_hdd = true;
        m.reset();
        probe(&mut m, false);
        assert_eq!(m.bus.ide.writes, 9);
        let d = m.bus.ide.disk.as_ref().unwrap();
        assert!(d.resident_bytes() <= 5 * PAGE_BYTES);
        let last = d.sector_count() - 1;
        let mut bytes = [0; 512];
        d.read_sector(last, &mut bytes).unwrap();
        assert_eq!(bytes, [0x5a; 512]);
        // Unchanged header/boot/zero sectors and changed tail are inspected
        // in bounded buffers, without materializing the logical capacity.
        d.read_bytes(0, &mut bytes[..32]).unwrap();
        assert_eq!(&bytes[..32], header(cylinders, 4096));
        d.read_sector(last - 2, &mut bytes).unwrap();
        assert_eq!(bytes, [0; 512]);
        m.reset();
        probe(&mut m, true);
        // Large end-of-capacity errors retain IRQ/residual and cannot modify data.
        m.bus.ide.output(0x74c, 0);
        address(&mut m.bus.ide, last, 2, 0x30);
        assert!(m.bus.ide.irq_enabled());
        assert_eq!(m.bus.ide.input(0x74c), 0x51);
        assert_eq!(m.bus.ide.input(0x644), 2);
        assert_eq!(m.bus.ide.input(0x642), 0x10);
        assert_eq!(m.bus.ide.input(0x64e), 0x51);
        assert!(!m.bus.ide.irq_enabled());
        m.bus.ide.output(0x640, 0);
        m.bus
            .ide
            .disk
            .as_ref()
            .unwrap()
            .read_sector(last, &mut bytes)
            .unwrap();
        assert_eq!(bytes, [0x5a; 512]);
        for lba in [0, 65536, last / 2, last] {
            address(&mut m.bus.ide, lba, 1, 0x20);
            assert_eq!(m.bus.ide.input(0x74c), 0x58);
            for _ in 0..512 {
                m.bus.ide.input(0x640);
            }
            assert_eq!(m.bus.ide.input(0x644), 0);
        }
        address(&mut m.bus.ide, last + 1, 1, 0x20);
        assert_eq!(m.bus.ide.input(0x642), 0x10);
        m.bus.ide.mount(None);
        assert!(m.bus.ide.disk.is_none());
        assert!(!m.bus.ide.irq_enabled());
    }
}
#[test]
fn import_validation_extent_overflow_addressing_and_atomic_replacement() {
    let good = header(2048, 4096);
    let len = 512 * 1024 * 1024 + 4096;
    assert!(HdiImport::new(&good, len).is_ok());
    assert!(HdiImport::new(&good, len - 1).is_err());
    assert!(HdiImport::new(&good, len + 1).is_err());
    assert!(HdiImport::new(&good, u32::MAX as u64 + 1).is_err());
    for (at, v) in [
        (8, 0),
        (8, u32::MAX),
        (12, u32::MAX),
        (16, 1024),
        (20, 0),
        (20, 64),
        (24, 17),
        (28, u32::MAX),
    ] {
        let mut bad = good.clone();
        bad[at..at + 4].copy_from_slice(&v.to_le_bytes());
        assert!(HdiImport::new(&bad, len).is_err());
    }
    // Above 28-bit IDE or 32-bit HDI payload, even with valid CHS field widths.
    let mut bad = good.clone();
    bad[28..32].copy_from_slice(&65535u32.to_le_bytes());
    bad[20..24].copy_from_slice(&63u32.to_le_bytes());
    assert!(HdiImport::new(&bad, len).is_err());
    assert!(HdiImport::new(&good[..31], len).is_err());
    assert!(HdiImport::new(&good, len).unwrap().finish().is_err());
    let mut import = HdiImport::new(&good, len).unwrap();
    assert!(import.append(&[0; 1]).is_err());
    assert!(import.append(&vec![0; TRANSFER_BYTES + 1]).is_err());
    // No allocation based on header alone: only metadata, not the full extent.
    // Also cover a non-sector-aligned HDI header and sector straddling pages.
    let mut bytes = vec![0; 32 + 512 * 512];
    bytes[..32].copy_from_slice(&header(1, 32));
    let mut importer = HdiImport::new(&bytes, bytes.len() as u64).unwrap();
    importer.append(&bytes).unwrap();
    let mut disk = importer.finish().unwrap();
    disk.write_sector(127, &[0xab; 512]).unwrap();
    let mut out = [0; 512];
    disk.read_sector(127, &mut out).unwrap();
    assert_eq!(out, [0xab; 512]);
    assert_eq!(disk.dirty_page(0), Some(0));
    assert_eq!(disk.dirty_page(1), Some(1));
    assert!(disk.write_sector(512, &[0; 512]).is_err());
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.bus.ide.mount(Some(disk));
    assert!(m.insert(2, vec![0; 32], "bad.hdi").is_err());
    m.bus
        .ide
        .disk
        .as_ref()
        .unwrap()
        .read_sector(127, &mut out)
        .unwrap();
    assert_eq!(out, [0xab; 512]);
}

#[test]
fn large_fat16_freedos_reads_and_creates_file_above_128mib() {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open("fixtures/freedos/large-fat16.hdi")
        .expect("prepare_large_dos_fixture.py after prepare_freedos.py");
    let mut header = [0; 32];
    file.read_exact(&mut header).unwrap();
    let mut import = HdiImport::new(&header, file.metadata().unwrap().len()).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut chunk = vec![0; TRANSFER_BYTES];
    let len = file.metadata().unwrap().len() as usize;
    for at in (0..len).step_by(TRANSFER_BYTES) {
        let n = TRANSFER_BYTES.min(len - at);
        file.read_exact(&mut chunk[..n]).unwrap();
        import.append(&chunk[..n]).unwrap();
    }
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.bus.ide.mount(Some(import.finish().unwrap()));
    m.bus.boot_hdd = true;
    for _ in 0..2 {
        m.reset();
        for _ in 0..800 {
            common::advance(&mut m, 10_000);
            m.bus.audio.clear();
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("LARGE DOS READY") && m.bus.text().contains("A:\\>") {
                break;
            }
        }
        let text = m.bus.text();
        assert!(text.contains("HIGH FAT16 SOURCE"), "{text}");
        assert!(text.contains("LARGE DOS WRITE"), "{text}");
        assert!(
            text.contains("LARGE DOS READY") && text.contains("A:\\>"),
            "{text}"
        );
        let disk = m.bus.ide.disk.as_ref().unwrap();
        let mut root = vec![0; 512 * 32];
        disk.read_bytes(4096 + (512 + 258) * 512, &mut root)
            .unwrap();
        let entry = root
            .as_chunks::<32>()
            .0
            .iter()
            .find(|entry| &entry[..11] == b"NEW     TXT")
            .unwrap();
        let cluster = u16::from_le_bytes([entry[26], entry[27]]) as usize;
        assert!(
            cluster >= 8193,
            "DOS allocated unexpected low cluster {cluster}"
        );
        let lba = 802 + (cluster - 2) * 32;
        assert!(lba * 512 > 128 * 1024 * 1024);
        let mut data = [0; 512];
        disk.read_sector(lba, &mut data).unwrap();
        assert_eq!(&data[..17], b"LARGE DOS WRITE\r\n");
        assert!(disk.writes > 0);
    }
}

#[test]
fn padding_over_64k_is_preserved_and_changed_header_is_rejected() {
    let header = header(1, 1024 * 1024);
    let mut bytes = vec![0; 1024 * 1024 + 512 * 512];
    bytes[..32].copy_from_slice(&header);
    bytes[900000..900016].fill(0xce);
    let owned = HardDisk::parse(bytes.clone()).unwrap();
    let mut importer = HdiImport::new(&header, bytes.len() as u64).unwrap();
    for chunk in bytes.chunks(TRANSFER_BYTES) {
        importer.append(chunk).unwrap();
    }
    let paged = importer.finish().unwrap();
    assert_eq!(owned.to_vec(), paged.to_vec());
    // Another internally valid geometry with identical extent cannot replace
    // the header already used to validate/import the staged disk.
    let mut importer = HdiImport::new(&header, bytes.len() as u64).unwrap();
    bytes[24..28].copy_from_slice(&8u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&2u32.to_le_bytes());
    for chunk in bytes.chunks(TRANSFER_BYTES) {
        importer.append(chunk).unwrap();
    }
    assert!(importer.finish().is_err());
}

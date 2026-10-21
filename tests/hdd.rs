use pc98_rust::{hdd::HardDisk, Machine, Profile};

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/dos-apps/adv98.hdi"
    ))
    .expect("run python3 tools/prepare_hdd_fixture.py before HDD tests")
}

fn file(image: &[u8], name: &[u8; 11]) -> Vec<u8> {
    let volume = &image[4096 + 128 * 512..];
    let root = &volume[14 * 512..26 * 512];
    let e = root
        .chunks_exact(32)
        .find(|e| &e[..11] == name)
        .expect("guest-created file");
    let size = u32::from_le_bytes(e[28..32].try_into().unwrap()) as usize;
    let mut cluster = u16::from_le_bytes(e[26..28].try_into().unwrap()) as usize;
    let mut result = Vec::new();
    for _ in 0..2010 {
        if cluster >= 0xff8 {
            break;
        }
        assert!((2..2010).contains(&cluster));
        let at = (26 + (cluster - 2) * 4) * 512;
        result.extend_from_slice(&volume[at..at + 2048]);
        let off = 2 * 512 + cluster * 3 / 2;
        cluster = (u16::from_le_bytes(volume[off..off + 2].try_into().unwrap()) as usize
            >> if cluster & 1 != 0 { 4 } else { 0 })
            & 0xfff;
    }
    result.truncate(size);
    result
}

fn advance(m: &mut Machine, count: usize) {
    for _ in 0..count {
        m.step(100_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
    }
}
fn key(m: &mut Machine, scan: u8) {
    m.bus.key(scan, true);
    advance(m, 1);
    m.bus.key(scan, false);
    advance(m, 20);
}
fn title(m: &mut Machine) {
    let frame = m.render();
    for y in 0..16 {
        for x in 0..16 {
            let ink = pc98_rust::font::jis_byte(0x24, 0x4f, y * 2 + x / 8) & (128 >> (x & 7)) != 0;
            let at = ((317 + y) * 640 + 160 + x) * 4;
            assert_eq!(frame[at..at + 3] == [255, 255, 255], ink);
        }
    }
}

#[test]
fn adv98_consumes_mouse_through_guest_driver_and_irq13() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(2, fixture(), "adv98.hdi").unwrap();
    m.reset();
    advance(&mut m, 160);
    title(&mut m);
    let state = m.bus.mem[..0xa0000]
        .windows(8)
        .position(|b| b == b"M98STATE")
        .expect("resident guest driver");
    let before = m.render().to_vec();
    m.bus.mouse.motion(32, -16, 1);
    advance(&mut m, 1);
    assert_eq!(m.bus.read16(state + 8), 352);
    assert_eq!(m.bus.read16(state + 10), 184);
    assert_eq!(m.bus.read16(state + 12), 1);
    assert_ne!(m.bus.read16(state + 14), 0);
    m.bus.mouse.motion(0, 0, 0);
    advance(&mut m, 30);
    assert!(
        before != m.render(),
        "independent game's mouse input changes its menu"
    );
    assert_eq!(m.bus.read16(state + 12), 0);
    assert!(m.bus.mouse.latches > 0);
}

#[test]
fn hdd_boots_freedos_and_independent_adv98_with_durable_writes() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(2, fixture(), "adv98.hdi").unwrap();
    m.reset();
    assert_eq!(m.bus.mem[0x584], 0x80);
    assert_eq!(m.bus.mem[0x55d], 1);
    advance(&mut m, 160);
    assert!(
        m.bus.ide.writes > 0,
        "{} reads={} equip={:x} boot={:x}",
        m.bus.text(),
        m.bus.ide.reads,
        m.bus.read16(0x55c),
        m.bus.mem[0x584]
    );
    assert_eq!(
        file(&m.bus.ide.disk.as_ref().unwrap().bytes, b"HDDSAVE TXT"),
        b"DURABLE DOS WRITE\r\n"
    );
    assert_eq!(m.bus.fdc.reads, 0);
    title(&mut m);
    key(&mut m, 0x1c); // Start, choose Talk, then save slot 1 through guest DOS.
    key(&mut m, 0x1c);
    key(&mut m, 0x3c);
    key(&mut m, 0x1c);
    key(&mut m, 0);
    key(&mut m, 0x1c);
    key(&mut m, 0x1c);
    let saved = file(&m.bus.ide.disk.as_ref().unwrap().bytes, b"SAVE1   DAT");
    assert_eq!(&saved[..8], b"ADV98SAV");
    let bytes = m.bus.ide.disk.as_ref().unwrap().bytes.clone();
    let writes = m.bus.ide.writes;
    m.reset();
    assert_eq!(m.bus.ide.writes, 0);
    assert_eq!(m.bus.ide.disk.as_ref().unwrap().writes, writes);
    m.insert(2, bytes, "export.hdi").unwrap();
    m.reset();
    advance(&mut m, 160);
    assert!(m.bus.ide.writes > 0);
    title(&mut m);
    key(&mut m, 0x3c);
    key(&mut m, 0x1c);
    key(&mut m, 0x1c);
    assert_eq!(
        file(&m.bus.ide.disk.as_ref().unwrap().bytes, b"SAVE1   DAT"),
        saved
    );
}

#[test]
fn hdi_rejects_inconsistent_geometry_offsets_truncation_and_overflow() {
    let bytes = fixture();
    assert!(HardDisk::parse(bytes.clone()).is_ok());
    for (offset, value) in [
        (8, 0),
        (8, u32::MAX),
        (12, u32::MAX),
        (16, 1024),
        (20, 0),
        (20, 64),
        (24, 17),
        (28, 0),
        (28, u32::MAX),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            HardDisk::parse(bad).is_err(),
            "offset={offset} value={value}"
        );
    }
    assert!(HardDisk::parse(bytes[..bytes.len() - 1].to_vec()).is_err());
    assert!(HardDisk::parse(vec![0; 31]).is_err());
    assert!(Machine::default().insert(2, bytes, "disk.hdi").is_err());
}

#[test]
fn ide_pio_chs_multisector_identify_irq_errors_and_media_replacement() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(2, fixture(), "test.hdi").unwrap();
    let b = &mut m.bus;
    b.output(0x64e, 0xec);
    assert!(b.ide.irq_enabled());
    assert_eq!(b.input(0x74c) & 8, 8);
    assert!(b.ide.irq_enabled());
    assert_eq!(b.input(0x64e) & 8, 8);
    assert!(!b.ide.irq_enabled());
    let id: Vec<_> = (0..512).map(|i| b.input(0x640 + (i & 1))).collect();
    assert_eq!(u16::from_le_bytes([id[2], id[3]]), 64);
    assert_eq!(u16::from_le_bytes([id[6], id[7]]), 8);
    b.output(0x644, 2);
    b.output(0x646, 15);
    b.output(0x648, 63);
    b.output(0x64a, 0);
    b.output(0x64c, 0xa7);
    b.output(0x64e, 0x30);
    for i in 0..1024 {
        b.output(0x640 + (i & 1), (i / 512 + 0x5a) as u8);
    }
    assert_eq!(b.ide.writes, 2);
    b.output(0x644, 2);
    b.output(0x646, 15);
    b.output(0x648, 63);
    b.output(0x64c, 0xa7);
    b.output(0x64e, 0x20);
    for i in 0..1024 {
        assert_eq!(b.input(0x640 + (i & 1)), (i / 512 + 0x5a) as u8);
    }
    assert_eq!(b.input(0x64e) & 9, 0);
    b.output(0x644, 2);
    b.output(0x646, 16);
    b.output(0x648, 63);
    b.output(0x64c, 0xa7);
    b.output(0x64e, 0x30);
    assert_eq!(b.input(0x64e) & 1, 1);
    assert_eq!(b.input(0x642), 0x10);
    let count = b.ide.writes;
    b.output(0x640, 0xff);
    assert_eq!(b.ide.writes, count);
    b.output(0x64e, 0xff);
    assert_eq!(b.input(0x642), 4);
    b.output(0x74c, 4);
    assert_eq!(b.input(0x74c), 0x80);
    b.output(0x74c, 0);
    assert_eq!(b.input(0x64e), 0x50);
    b.output(0x644, 0);
    b.output(0x64e, 0x20); // zero count requests 256 sectors
    assert_eq!(b.input(0x74c) & 8, 8);
    b.ide.mount(None);
    assert_eq!(b.input(0x640), 255); // cancels in-flight transfer
}

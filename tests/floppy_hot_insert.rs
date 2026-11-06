//! Issue #126: real GPL FreeDOS/FreeCOM, independent MIT data, real keyboard I/O.
use pc98_rust::{Machine, Profile};

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(format!("{}/fixtures/{path}", env!("CARGO_MANIFEST_DIR")))
        .expect("prepare_freedos.py and prepare_floppy_data.py must run first")
}
// Observe actual BIOS reads of DA/UA 91h, without modifying guest state.
fn step(m: &mut Machine, budget: u32) -> usize {
    let mut reads = 0;
    let end = m.bus.cycles + u64::from(budget) * 8;
    while m.bus.cycles < end && m.error.is_none() {
        m.step_scheduler_with_observer((end - m.bus.cycles).min(80_000) as u32, 100_000, |c, _| {
            if c.s[1] == 0xf000 && c.ip == 0x106c && c.r[0] & 0x0fff == 0x0691 {
                reads += 1;
            }
        });
    }
    m.bus.audio.clear();
    assert!(m.error.is_none(), "{:?}", m.error);
    reads
}
fn until(m: &mut Machine, expected: &str) -> usize {
    let mut reads = 0;
    for _ in 0..80 {
        if m.bus.text().contains(expected) {
            return reads;
        }
        reads += step(m, 100_000);
    }
    panic!("missing {expected:?}: {}", m.bus.text());
}
fn keys(m: &mut Machine, text: &str) -> usize {
    let mut reads = 0;
    for ch in text.bytes() {
        let scan = [(0x10, "qwertyuiop"), (0x1d, "asdfghjkl"), (0x29, "zxcvbnm")]
            .iter()
            .find_map(|(base, row)| row.bytes().position(|c| c == ch).map(|i| base + i as u8))
            .unwrap_or_else(|| match ch {
                b'.' => 0x31,
                b':' => 0x27,
                b' ' => 0x34,
                b'\r' => 0x1c,
                _ => panic!("unsupported {ch}"),
            });
        m.bus.key(scan, true);
        reads += step(m, 50_000);
        m.bus.key(scan, false);
        reads += step(m, 10_000);
    }
    reads
}
fn command(m: &mut Machine, text: &str) -> usize {
    keys(m, &format!("{text}\r"))
}
fn read_file(m: &mut Machine, name: &str, contents: &str) -> usize {
    command(m, "cls");
    let reads = command(m, &format!("type {name}")) + until(m, contents);
    until(m, "B:\\>");
    reads
}
fn hot_insert(profile: Profile) {
    let boot = fixture("freedos/minimal.hdm");
    let data = fixture("floppy_data/second.d88");
    let replacement = fixture("floppy_data/swapped.hdm");
    let mut m = Machine::new(profile);
    m.insert(0, boot.clone(), "minimal.hdm").unwrap();
    m.reset();
    until(&mut m, "A:\\>");
    println!(
        "{profile:?} equipment={:02x} DOS DA/UA={:02x?}",
        m.bus.mem[0x55c],
        &m.bus.mem[0x66c..0x66e]
    );
    let retired = m.cpu.retired;
    m.insert(1, data.clone(), "second.d88").unwrap();
    assert_eq!(m.cpu.retired, retired);
    command(&mut m, "b:");
    until(&mut m, "B:\\>");
    assert_eq!(m.bus.mem[0x55c], 3);
    assert_eq!(&m.bus.mem[0x66c..0x66e], &[0x90, 0x91]);
    command(&mut m, "dir");
    until(&mut m, "SECOND");
    assert!(read_file(&mut m, "second.txt", "ORIGINAL FDD2 FILE CONTENTS") > 0);
    read_file(&mut m, "second.txt", "ORIGINAL FDD2 FILE CONTENTS");
    // Direct swap, without an intervening no-media operation: catches stale FAT.
    m.insert(1, replacement.clone(), "swapped.hdm").unwrap();
    command(&mut m, "cls");
    command(&mut m, "dir");
    until(&mut m, "SWAPPED");
    assert!(!m.bus.text().contains("SECOND"));
    read_file(&mut m, "swapped.txt", "REPLACEMENT FDD2 FILE CONTENTS");
    m.bus.mount_floppy(1, None);
    assert_eq!(m.bus.mem[0x55c], 3);
    m.insert(1, data.clone(), "second.d88").unwrap();
    command(&mut m, "cls");
    command(&mut m, "dir");
    until(&mut m, "SECOND");
    read_file(&mut m, "second.txt", "ORIGINAL FDD2 FILE CONTENTS");
    assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, boot);
    assert_eq!(m.bus.disks[1].as_ref().unwrap().bytes, data);
    assert_eq!(m.bus.fdc.writes, 0);
    assert_eq!(m.cpu.fault_count, 0);
    println!(
        "{profile:?}: live insertion/swap/eject/reinsert {} slots",
        m.cpu.retired
    );
    assert!(m.bus.cycles < 12_000_000 * 8);
    // Same D88 mounted at reset is the reported working control.
    m.reset();
    until(&mut m, "A:\\>");
    command(&mut m, "b:");
    until(&mut m, "B:\\>");
    command(&mut m, "dir");
    until(&mut m, "SECOND");
    read_file(&mut m, "second.txt", "ORIGINAL FDD2 FILE CONTENTS");
    assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, boot);
    assert_eq!(m.bus.disks[1].as_ref().unwrap().bytes, data);
    assert_eq!(m.bus.fdc.writes, 0);
    println!("{profile:?}: reset control {} slots", m.cpu.retired);
    assert!(m.bus.cycles < 8_000_000 * 8);
    // No cached directory can turn an actually empty drive into a successful DIR.
    m.bus.mount_floppy(1, None);
    command(&mut m, "cls");
    command(&mut m, "dir");
    until(&mut m, "(R)"); // genuine DOS critical-error Retry choice
    assert!(!m.bus.text().contains("SECOND"));
    assert_eq!(m.bus.mem[0x55c], 3);
}
#[test]
fn vm_dos_reads_hot_inserted_fdd2_and_replacements() {
    hot_insert(Profile::Pc9801Vm);
}
#[test]
fn ap_dos_reads_hot_inserted_fdd2_and_replacements() {
    hot_insert(Profile::Pc9821ApDos);
}

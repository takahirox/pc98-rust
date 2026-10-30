//! Original MIT guest: no private media or guest-derived bytes.
use pc98_rust::{Machine, Profile};

#[test]
fn real_iret_boundary_guest_boots_cold_and_after_reset() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/iret_wrap/iret_wrap.hdm"
    ))
    .expect("run python3 tools/prepare_iret_wrap_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "iret_wrap.hdm").unwrap();
    for _ in 0..2 {
        m.reset();
        for _ in 0..100 {
            m.step(1000);
            m.bus.audio.clear();
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.text().contains("IRET WRAP PASS") {
                break;
            }
        }
        assert!(m.bus.text().contains("IRET WRAP PASS"));
        assert_eq!(&m.bus.mem[0x8000..0x8003], &[0xa5, 5, 0xa5]);
        assert_eq!(m.cpu.fault_count, 0);
        assert_eq!(m.cpu.r[4], 0x7c00);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().writes, 0);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
    }
}

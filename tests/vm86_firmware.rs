//! Original MIT firmware guest, never private DOS/game media.
use pc98_rust::{Machine, Profile};
#[test]
fn paged_vm86_bios_guest_recovers_faults_and_accepts_enter_cold_reset() {
    let image = std::fs::read("fixtures/vm86_firmware/vm86_firmware.hdm")
        .expect("run python3 tools/prepare_vm86_firmware_fixture.py");
    let mut m = Machine::new(Profile::Pc9821ApDos);
    m.insert(0, image.clone(), "probe.hdm").unwrap();
    for _ in 0..2 {
        m.reset();
        let mut input = false;
        for _ in 0..200 {
            m.step(10_000);
            m.bus.audio.clear();
            assert!(m.error.is_none(), "{:?}", m.error);
            if m.bus.mem[0x8000] == 1 && !input {
                m.bus.key(0x1c, true);
                input = true;
            }
            if m.cpu.halted {
                break;
            }
        }
        assert!(
            m.bus.text().contains("VM86 BIOS PASS"),
            "{} result={:?} ticks={}/{}",
            m.bus.text(),
            &m.bus.mem[0x8000..0x8008],
            m.bus.read16(0x58a),
            m.bus.read16(0x10058a)
        );
        assert!(input);
        assert_eq!(&m.bus.mem[0x8000..0x8008], &[2, 2, 0, 0, 1, 1, 0x0d, 0x1c]);
        assert_eq!(m.cpu.fault_count, 2);
        assert_eq!(m.bus.disks[0].as_ref().unwrap().bytes, image);
        assert_eq!(m.bus.fdc.writes, 0);
    }
}

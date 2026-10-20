use pc98_rust::Machine;
/// This test is deliberately required rather than ignored when media is absent.
/// CI prepares the pinned legal guest and source archives before cargo test.
#[test]
fn freedos_boot_and_execute_guest_com() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/freedos/minimal.hdm");
    let bytes = std::fs::read(path)
        .expect("run python3 tools/prepare_freedos.py before cargo test --test dos");
    let mut m = Machine::default();
    m.insert(0, bytes, "dos.hdm").unwrap();
    m.reset();
    for _ in 0..80 {
        m.step(100_000);
        assert!(m.error.is_none(), "{:?}", m.error);
        if m.bus.text().contains("A:\\>") {
            break;
        }
    }
    assert!(
        m.bus.text().contains("FreeCom ver 0.85a_DBCS"),
        "{}",
        m.bus.text()
    );
    assert!(m.bus.text().contains("A:\\>"));
    // Type PROBE through PC-98 scancodes. The shell loads and executes PROBE.COM
    // via the actual guest DOS kernel. There is no host implementation of INT 21.
    for scan in [0x19, 0x13, 0x18, 0x2d, 0x12, 0x1c] {
        m.bus.key(scan, true);
        m.step(50_000);
        m.bus.key(scan, false);
        m.step(10_000);
        assert!(m.error.is_none(), "{:?}", m.error);
    }
    for _ in 0..80 {
        m.step(100_000);
        assert!(m.error.is_none(), "{:?}", m.error);
        if m.bus.text().contains("PC98 DOS PROBE OK") {
            break;
        }
    }
    assert!(
        m.bus.text().contains("PC98 DOS PROBE OK"),
        "{}",
        m.bus.text()
    );
    assert!(m.bus.beep);
    assert!(m.bus.fdc.reads > 100);
    assert!(
        m.bus.text().matches("A:\\>").count() >= 2,
        "shell must regain control: {}",
        m.bus.text()
    );
    // ECHO through the guest shell must receive keypad characters via the BIOS.
    // Check the output line separately from the echoed command at the prompt.
    for scan in [0x12, 0x2b, 0x22, 0x18, 0x34]
        .into_iter()
        .chain(0x40..=0x50)
        .chain([0x1c])
    {
        m.bus.key(scan, true);
        m.step(50_000);
        m.bus.key(scan, false);
        m.step(10_000);
        assert!(m.error.is_none(), "{:?}", m.error);
    }
    assert!(
        m.bus
            .text()
            .lines()
            .any(|line| line.trim() == "-/789*456+123=0,."),
        "keypad ECHO output missing: {}",
        m.bus.text()
    );
    // ECHO under_score|ECHO pipe_ok exercises both shifted punctuation keys
    // through the guest BIOS and FreeCOM's actual pipeline processing.
    let writes = m.bus.fdc.writes;
    for (scans, shifted) in [
        (
            &[0x12, 0x2b, 0x22, 0x18, 0x34, 0x16, 0x2e, 0x1f, 0x12, 0x13][..],
            false,
        ),
        (&[0x33][..], true),
        (&[0x1e, 0x2b, 0x18, 0x13, 0x12][..], false),
        (&[0x0d][..], true),
        (
            &[0x12, 0x2b, 0x22, 0x18, 0x34, 0x19, 0x17, 0x19, 0x12][..],
            false,
        ),
        (&[0x33][..], true),
        (&[0x18, 0x24, 0x1c][..], false),
    ] {
        m.bus.key(0x70, shifted);
        m.step(1000);
        for &scan in scans {
            m.bus.key(scan, true);
            m.step(50_000);
            m.bus.key(scan, false);
            m.step(10_000);
            assert!(m.error.is_none(), "{:?}", m.error);
        }
    }
    for _ in 0..80 {
        m.step(100_000);
        assert!(m.error.is_none(), "{:?}", m.error);
        if m.bus.text().lines().any(|line| line.trim() == "pipe_ok") {
            break;
        }
    }
    let text = m.bus.text();
    assert!(text.contains("echo under_score|echo pipe_ok"), "{text}");
    assert!(text.lines().any(|line| line.trim() == "pipe_ok"), "{text}");
    assert!(
        !text.lines().any(|line| line.trim() == "under_score"),
        "{text}"
    );
    assert!(
        m.bus.fdc.writes > writes,
        "pipeline must write its temporary file"
    );
}

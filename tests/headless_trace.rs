// Original synthetic failing guest; no third-party code/media. MIT, Issue #97.
#[test]
fn bounded_local_trace_retains_early_unsupported_control_write() {
    let path = std::env::temp_dir().join(format!("pc98-cr-trace-{}.hdm", std::process::id()));
    let mut image = vec![0; 1_261_568];
    image[..3].copy_from_slice(&[0x0f, 0x20, 0xc0]);
    image[3..203].fill(0x90); // Progress beyond the final 100-slot history.
    image[203..209].copy_from_slice(&[0x66, 0x67, 0x65, 0x0f, 0x22, 0xdc]);
    std::fs::write(&path, image).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_headless"))
        .arg(&path)
        .args(["1000000", "trace", "--ap"])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostic.contains("profile=Pc9821ApDos"), "{diagnostic}");
    assert!(diagnostic.contains("1fc0:000000cb opcode=0f 22 modrm=dc mod=3 cr=3 gpr=esp prefixes=[66, 67, 65] operand=32 address=32"), "{diagnostic}");
    assert!(diagnostic.contains("cr0=60000010"));
    assert!(
        diagnostic
            .lines()
            .any(|line| line.starts_with("control-slot=")
                && line.contains("1fc0:00000000 opcode=0f 20 modrm=c0 mod=3 cr=0 gpr=eax")),
        "{diagnostic}"
    );
    assert!(!diagnostic
        .lines()
        .any(|line| line.starts_with("slot=") && line.contains("opcode=0f 20")));
    assert!(
        diagnostic
            .lines()
            .filter(|line| line.starts_with("control-slot="))
            .count()
            <= 16
    );
    assert!(diagnostic.contains("unimplemented i486 opcode 0F 22"));
    assert!(
        diagnostic
            .lines()
            .filter(|line| line.starts_with("slot="))
            .count()
            <= 100
    );
}

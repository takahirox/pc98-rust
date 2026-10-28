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

// Original IRQ0 guest: install an IVT entry, program the PIC and a one-shot PIT,
// then wait at 1fc0:0040. No host-injected interrupt or third-party guest code.
fn trace_irq_handler(handler: &[u8], name: &str) -> String {
    let mut image = vec![0; 1_261_568];
    let setup = [
        0xfa, // cli
        0x31, 0xc0, // xor ax, ax
        0x8e, 0xc0, // mov es, ax
        0x8e, 0xd0, // mov ss, ax
        0xbc, 0x00, 0x7c, // mov sp, 7c00h
        0x26, 0xc7, 0x06, 0x20, 0x00, 0x80, 0x00, // mov word [es:20h], 80h
        0x8c, 0xc8, // mov ax, cs
        0x26, 0xa3, 0x22, 0x00, // mov [es:22h], ax
        0xb0, 0x11, 0xe6, 0x00, // PIC ICW1
        0xb0, 0x08, 0xe6, 0x02, // IRQ0 vector 8
        0xb0, 0x80, 0xe6, 0x02, // ICW3
        0xb0, 0x01, 0xe6, 0x02, // ICW4 (no AEOI)
        0xb0, 0xfe, 0xe6, 0x02, // unmask IRQ0 only
        0xb0, 0xff, 0xe6, 0x0a, // mask slave
        0xb0, 0x30, 0xe6, 0x77, // PIT channel 0, mode 0, low/high
        0xb0, 0xe8, 0xe6, 0x71, // divisor 1000 low
        0xb0, 0x03, 0xe6, 0x71, // divisor high
        0xfb, 0x90, // sti; nop (IRQ shadow)
        0xeb, 0x01, 0x90, // jmp 0040h; padding
        0xeb, 0xfe, // interrupted loop
    ];
    image[..setup.len()].copy_from_slice(&setup);
    image[0x80..0x80 + handler.len()].copy_from_slice(handler);
    let path = std::env::temp_dir().join(format!(
        "pc98-cr-irq-trace-{name}-{}.hdm",
        std::process::id()
    ));
    std::fs::write(&path, image).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_headless"))
        .arg(&path)
        .args(["1000000", "trace", "--ap"])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(diagnostic.contains("unimplemented i486 opcode 0F 22"));
    diagnostic
}

#[test]
fn trace_captures_unsupported_control_write_at_irq_handler_entry() {
    let diagnostic = trace_irq_handler(&[0x66, 0x67, 0x65, 0x0f, 0x22, 0xdc], "write");
    let expected = "1fc0:00000080 opcode=0f 22 modrm=dc mod=3 cr=3 gpr=esp prefixes=[66, 67, 65] operand=32 address=32";
    let last = diagnostic
        .lines()
        .rfind(|line| line.starts_with("slot="))
        .unwrap();
    assert!(last.contains(expected), "{diagnostic}");
    // IRQ entry has already pushed the six-byte frame and cleared IF.
    assert!(last.contains(", 00007bfa,"), "{last}");
    assert!(last.contains("flags=00000046"), "{last}");
    assert!(
        diagnostic
            .lines()
            .any(|line| line.starts_with("control-slot=") && line.contains(expected)),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("1fc0:00000040 opcode=eb"));
}

#[test]
fn trace_captures_supported_control_read_at_irq_handler_entry() {
    let diagnostic = trace_irq_handler(
        &[
            0x66, 0x67, 0x65, 0x0f, 0x20, 0xc3, // mov ebx, cr0
            0x0f, 0x22, 0xdc, // unsupported write stops execution
        ],
        "read",
    );
    let controls: Vec<_> = diagnostic
        .lines()
        .filter(|line| line.starts_with("control-slot="))
        .collect();
    assert_eq!(controls.len(), 2, "{diagnostic}");
    assert!(controls[0].contains("1fc0:00000080 opcode=0f 20 modrm=c3 mod=3 cr=0 gpr=ebx prefixes=[66, 67, 65] operand=32 address=32"), "{diagnostic}");
    assert!(
        controls[1].contains("1fc0:00000086 opcode=0f 22 modrm=dc mod=3 cr=3 gpr=esp"),
        "{diagnostic}"
    );
    // The next pre-instruction snapshot includes the actual CR0 read result.
    assert!(
        controls[1].contains(", 60000010, 00007bfa,"),
        "{diagnostic}"
    );
}

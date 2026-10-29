// Original synthetic failing guest; no third-party code/media. MIT, Issue #97.
#[test]
fn bounded_local_trace_retains_early_rejected_control_write() {
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
    assert!(diagnostic.contains("1fc0:000000cb opcode=0f 22 modrm=dc mod=3 cr=3 gpr=esp old=00000000 written=00007c00 prefixes=[66, 67, 65] operand=32 address=32"), "{diagnostic}");
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
    assert!(diagnostic.contains("undefined reserved-bit write"));
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
    assert!(diagnostic.contains("undefined reserved-bit write"));
    diagnostic
}

#[test]
fn trace_captures_rejected_control_write_at_irq_handler_entry() {
    let diagnostic = trace_irq_handler(&[0x66, 0x67, 0x65, 0x0f, 0x22, 0xdc], "write");
    let expected = "1fc0:00000080 opcode=0f 22 modrm=dc mod=3 cr=3 gpr=esp old=00000000 written=00007bfa prefixes=[66, 67, 65] operand=32 address=32";
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
            0x0f, 0x22, 0xdc, // reserved CR3 bits stop execution
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

#[test]
fn trace_distinguishes_real_mode_setup_from_a_requested_transition() {
    // Guest writes CR2, runs NOP, then requests PE+PG in CR0. Only original bytes.
    let mut image = vec![0; 1_261_568];
    let code = [
        0x66, 0xb8, 0xef, 0xbe, 0xad, 0xde, 0x0f, 0x22, 0xd0, 0x90, 0x66, 0xb8, 0x11, 0, 0, 0xe0,
        0x66, 0x67, 0x65, 0x0f, 0x22, 0xc0, 0xf4,
    ];
    image[..code.len()].copy_from_slice(&code);
    let path = std::env::temp_dir().join(format!("pc98-cr-mode-{}.hdm", std::process::id()));
    std::fs::write(&path, image).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_headless"))
        .arg(&path)
        .args(["10000", "trace", "--ap"])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic.contains("cr=2 gpr=eax old=00000000 written=deadbeef"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("next-attempt=1fc0:00000009 opcode=90"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains(
            "cr=0 gpr=eax old=60000010 written=e0000011 PE=0->1 PG=0->1 prefixes=[66, 67, 65]"
        ),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("next-attempt=1fc0:00000016 opcode=<fetch-limit>"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("protected page fault delivery failed"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("triple fault/shutdown"), "{diagnostic}");
}

#[test]
fn protected_round_trip_trace_retains_modes_caches_tables_and_fault() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_headless"))
        .args([
            "fixtures/protected_mode/protected_mode.hdm",
            "10000",
            "trace",
            "--ap",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("PROTECTED MODE PASS"));
    let trace = String::from_utf8(output.stderr).unwrap();
    assert!(trace.contains("PE=0->1 PG=0->0"));
    assert!(trace.contains("PE=1->0 PG=0->0"));
    assert!(trace.contains("mode=protected cpl=0"));
    assert!(trace.contains("mode=real cpl=0"));
    assert!(trace.contains("caches=[Some(SegmentCache"));
    assert!(trace.contains("fault=Some((13, 64))"));
    assert!(trace.contains("gdt=0001f"));
    assert!(trace.contains("idt=00009000/0187"));
    assert!(
        trace
            .lines()
            .filter(|s| s.starts_with("control-slot="))
            .count()
            <= 16
    );
    assert!(trace.lines().filter(|s| s.starts_with("slot=")).count() <= 100);
}

#[test]
fn bounded_trace_names_ltr_str_and_retains_loaded_tss_metadata() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_headless"))
        .args([
            "fixtures/task_register/task_register.hdm",
            "190",
            "trace",
            "--ap",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(
        diagnostic
            .contains("0008:00000036 opcode=0f 00 modrm=d8 LTR selector=0028 descriptor=Some"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("opcode=0f 00 modrm=cb STR"));
    assert!(diagnostic.contains("opcode=0f 00 modrm=1f LTR"));
    assert!(
        diagnostic.contains("tr=0028 tss=Some(SegmentCache { base: 33024, limit: 43, access: 131")
    );
    assert!(
        diagnostic.contains("tr=0030 tss=Some(SegmentCache { base: 33280, limit: 103, access: 235")
    );
    assert!(diagnostic.contains("next-attempt="));
    assert!(
        diagnostic
            .lines()
            .filter(|s| s.starts_with("control-slot="))
            .count()
            <= 16
    );
    assert!(
        diagnostic
            .lines()
            .filter(|s| s.starts_with("slot="))
            .count()
            <= 100
    );
}

#[test]
fn bounded_clts_trace_records_actual_ts_transition_and_fault_result() {
    // Two original raw boot guests: CLTS succeeds, or LOCK CLTS delivers #UD.
    for locked in [false, true] {
        let path = std::env::temp_dir().join(format!(
            "pc98-clts-trace-{}-{locked}.hdm",
            std::process::id()
        ));
        let mut image = vec![0; 1_261_568];
        let mut code = vec![
            0x66, 0xb8, 0x18, 0, 0, 0x60, // MOV EAX,60000018h
            0x0f, 0x22, 0xc0, // MOV CR0,EAX
        ];
        if locked {
            code.push(0xf0);
        }
        code.extend([0x0f, 0x06]);
        code.extend([0x90; 110]);
        code.extend([0x0f, 0x05]); // named unsupported stop
        image[..code.len()].copy_from_slice(&code);
        std::fs::write(&path, image).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_headless"))
            .arg(&path)
            .args(["300", "trace", "--ap"])
            .output()
            .unwrap();
        std::fs::remove_file(path).unwrap();
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        let record = diagnostic
            .lines()
            .find(|line| {
                line.starts_with("control-slot=")
                    && line
                        .split(" next-attempt=")
                        .next()
                        .unwrap()
                        .contains("opcode=0f 06 CLTS")
            })
            .unwrap_or_else(|| panic!("{diagnostic}"));
        assert!(record.contains("old=60000018 ts=1"), "{record}");
        if locked {
            assert!(
                record.contains("result=fault-delivered new-cr0=60000018 ts=1 fault=Some((6, 0))"),
                "{record}"
            );
        } else {
            assert!(
                record.contains("result=executed new-cr0=60000010 ts=0"),
                "{record}"
            );
            assert!(!output.status.success());
        }
        assert!(
            diagnostic
                .lines()
                .filter(|l| l.starts_with("control-slot="))
                .count()
                <= 16
        );
        assert!(
            diagnostic
                .lines()
                .filter(|l| l.starts_with("slot="))
                .count()
                <= 100
        );
    }
}

#[test]
fn vm86_trace_retains_cpl_vm_frame_tss_gate_and_fault_metadata_with_fixed_bounds() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_headless"))
        .args(["fixtures/vm86/vm86.hdm", "100000", "trace", "--ap"])
        .output()
        .unwrap();
    let diagnostic = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{diagnostic}");
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("VM86 PASS"));
    assert!(diagnostic.contains("cpl=3"));
    assert!(diagnostic.contains("pe=1 pg=1 vm=1"));
    assert!(diagnostic.contains("IRET frame(ss,sp,bytes,in-bounds)=(16, 98268, 36, true)"));
    assert!(diagnostic.contains("ss0_esp0=Some((16, 98304))"));
    assert!(diagnostic.contains("vm86_gate=Some((32, 14, 8,"));
    assert!(diagnostic.contains("fault=Some((14, 7))"));
    assert!(
        diagnostic
            .lines()
            .filter(|l| l.starts_with("control-slot="))
            .count()
            <= 16
    );
    assert!(
        diagnostic
            .lines()
            .filter(|l| l.starts_with("slot="))
            .count()
            <= 100
    );
}

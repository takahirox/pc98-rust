use pc98_rust::{bus::Bus, cpu::Cpu, Machine, Profile};
use std::collections::VecDeque;

// Read-only, bounded instruction metadata for local debugging. Never dump guest
// code beyond opcode/ModR/M or dereference a decoded memory operand.
fn instruction_diagnostic(c: &Cpu, b: &Bus) -> (String, bool) {
    let ap = b.profile == Profile::Pc9821ApDos;
    let mut offset = c.ip;
    let mut count = 0;
    let mut next = || {
        if count == 15 || (ap && offset > c.segment_cache(1).limit) {
            return None;
        }
        let value = b.read(c.segment_cache(1).base.wrapping_add(offset) as usize);
        offset = offset.wrapping_add(1);
        if !ap {
            offset &= 0xffff;
        }
        count += 1;
        Some(value)
    };
    let mut prefixes = Vec::new();
    let opcode = loop {
        match next() {
            Some(p)
                if matches!(
                    p,
                    0x26 | 0x2e | 0x36 | 0x3e | 0x64 | 0x65 | 0xf0 | 0xf2 | 0xf3
                ) || (ap && matches!(p, 0x66 | 0x67)) =>
            {
                prefixes.push(p)
            }
            value => break value,
        }
    };
    let mut operation = opcode.map_or_else(|| "<fetch-limit>".into(), |op| format!("{op:02x}"));
    let mut control = false;
    if opcode == Some(0x0f) {
        if let Some(ext) = next() {
            operation.push_str(&format!(" {ext:02x}"));
            if ap && ext == 0x06 {
                control = true;
                operation.push_str(&format!(" CLTS old={:08x} ts={}", c.cr0, (c.cr0 >> 3) & 1));
            }
            if ap && ext == 0x00 {
                if let Some(m) = next() {
                    let name = match (m >> 3) & 7 {
                        1 => "STR",
                        3 => "LTR",
                        _ => "group0",
                    };
                    operation.push_str(&format!(" modrm={m:02x} {name}"));
                    control = matches!((m >> 3) & 7, 1 | 3);
                    if name == "LTR" && m >= 0xc0 {
                        let selector = c.r[(m & 7) as usize];
                        operation.push_str(&format!(
                            " selector={selector:04x} descriptor={:?}",
                            c.task_descriptor_metadata(b, selector)
                        ));
                    }
                } else {
                    operation.push_str(" modrm=<fetch-limit>");
                }
            }
            if ap && matches!(ext, 0x20 | 0x22) {
                control = true;
                if let Some(m) = next() {
                    let names = ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"];
                    operation.push_str(&format!(
                        " modrm={m:02x} mod={} cr={} gpr={}",
                        m >> 6,
                        (m >> 3) & 7,
                        names[(m & 7) as usize]
                    ));
                    if ext == 0x22 {
                        let old = match (m >> 3) & 7 {
                            0 => Some(c.cr0),
                            2 => Some(c.cr2),
                            3 => Some(c.cr3),
                            _ => None,
                        };
                        let written = c.reg32((m & 7) as usize);
                        operation.push_str(&format!(
                            " old={} written={written:08x}",
                            old.map_or_else(|| "<undefined>".into(), |v| format!("{v:08x}"))
                        ));
                        if (m >> 3) & 7 == 0 {
                            operation.push_str(&format!(
                                " PE={}->{} PG={}->{}",
                                c.cr0 & 1,
                                written & 1,
                                c.cr0 >> 31,
                                written >> 31
                            ));
                        }
                    }
                } else {
                    operation.push_str(" modrm=<fetch-limit>");
                }
            }
        }
    }
    let registers: Vec<_> = (0..8).map(|r| c.reg32(r)).collect();
    (format!(
        "{:04x}:{:08x} opcode={} prefixes={:02x?} operand={} address={} gpr={:08x?} seg={:04x?} fs_gs={:04x?} flags={:04x}{:04x} cr0={:08x} cr2={:08x} cr3={:08x} mode={} cpl={} gdt={:08x}/{:04x} idt={:08x}/{:04x} caches={:?} ldtr={:04x} tr={:04x} tss={:?} fault={:?}",
        c.s[1], c.ip, operation, prefixes,
        if ap && (c.segment_cache(1).big() ^ prefixes.contains(&0x66)) {32} else {16},
        if ap && (c.segment_cache(1).big() ^ prefixes.contains(&0x67)) {32} else {16},
        registers, c.s, c.extra_segments, c.flags_upper, c.flags, c.cr0, c.cr2, c.cr3, if c.protected_mode() {"protected"} else {"real"}, c.cpl, c.gdtr_base, c.gdtr_limit, c.idtr_base, c.idtr_limit, c.segment_caches, c.ldtr, c.tr, c.tr_cache, c.last_fault
    ), control)
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("usage: headless IMAGE [STEP_BUDGET] [trace] [--ap]");
    let mut m = Machine::new(if args.iter().any(|a| a == "--ap") {
        pc98_rust::Profile::Pc9821ApDos
    } else {
        pc98_rust::Profile::Pc9801Vm
    });
    m.insert(
        if path.to_ascii_lowercase().ends_with(".hdi") {
            2
        } else {
            0
        },
        std::fs::read(path).expect("read image"),
        path,
    )
    .expect("parse image");
    m.reset();
    let steps = args
        .get(2)
        .map(|v| v.parse().unwrap())
        .unwrap_or(1_000_000u64);
    let trace = args.iter().any(|a| a == "trace");
    let mut history = VecDeque::with_capacity(100);
    let mut control_history: VecDeque<String> = VecDeque::with_capacity(16);
    let mut pending_control = false;
    if trace {
        eprintln!(
            "profile={:?} instruction-history=100 control-history=16 (opcode/ModR/M only)",
            m.bus.profile
        );
        if let Some(disk) = &m.bus.ide.disk {
            eprintln!(
                "format=HDI bytes={} geometry={}x{}x{} sector=512",
                disk.bytes.len(),
                disk.cylinders,
                disk.heads,
                disk.sectors
            );
        }
    }
    for i in 0..steps {
        let previous_retired = m.cpu.retired;
        let mut observed_control = false;
        if trace {
            m.step_with_observer(1, |cpu, bus| {
                if history.len() == 100 {
                    history.pop_front();
                }
                let (diagnostic, control) = instruction_diagnostic(cpu, bus);
                if pending_control {
                    if let Some(last) = control_history.back_mut() {
                        last.push_str(&format!(" next-attempt={diagnostic}"));
                    }
                }
                pending_control = control;
                observed_control = control;
                if control {
                    if control_history.len() == 16 {
                        control_history.pop_front();
                    }
                    control_history.push_back(format!("control-slot={i} {diagnostic}"));
                }
                history.push_back(format!("slot={i} {diagnostic}"));
            });
        } else {
            m.step(1);
        }
        if trace && observed_control {
            if let Some(last) = control_history.back_mut() {
                last.push_str(&format!(
                    " result={} new-cr0={:08x} ts={} fault={:?} next={:04x}:{:08x}",
                    if m.error.is_some() {
                        "stopped"
                    } else if m.cpu.retired == previous_retired {
                        "fault-delivered"
                    } else {
                        "executed"
                    },
                    m.cpu.cr0,
                    (m.cpu.cr0 >> 3) & 1,
                    m.cpu.last_fault,
                    m.cpu.s[1],
                    m.cpu.ip
                ));
            }
        }
        if m.error.is_some() {
            break;
        }
    }
    if trace {
        if pending_control {
            if let Some(last) = control_history.back_mut() {
                last.push_str(if m.error.is_some() {
                    " next-attempt=<stopped>"
                } else {
                    " next-attempt=<budget-end>"
                });
            }
        }
        for record in control_history {
            eprintln!("{record}");
        }
        for record in history {
            eprintln!("{record}");
        }
    }
    println!("{}", m.bus.text());
    println!("instructions={} cycles={} fdc_reads={} fdc_writes={} beep={} edges={} pc={:04x}:{:04x} ports={:x?}",m.cpu.retired,m.bus.cycles,m.bus.fdc.reads,m.bus.fdc.writes,m.bus.beep,m.bus.beep_edges,m.cpu.s[1],m.cpu.ip,m.bus.unknown_ports);
    if let Some(e) = m.error {
        eprintln!("{e}");
        std::process::exit(1)
    }
}

use pc98_rust::Machine;
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
    m.insert(0, std::fs::read(path).expect("read image"), path)
        .expect("parse image");
    m.reset();
    let steps = args
        .get(2)
        .map(|v| v.parse().unwrap())
        .unwrap_or(1_000_000u64);
    let trace = args.iter().any(|a| a == "trace");
    let mut old_text = String::new();
    for i in 0..steps {
        if trace && i + 100 > steps {
            eprintln!(
                "{:04x}:{:04x} {:02x?} r={:04x?} s={:04x?} f={:04x}",
                m.cpu.s[1],
                m.cpu.ip,
                (0..8)
                    .map(|n| m.bus.read(pc98_rust::cpu::Cpu::phys(
                        m.cpu.s[1],
                        m.cpu.ip.wrapping_add(n)
                    )))
                    .collect::<Vec<_>>(),
                m.cpu.r,
                m.cpu.s,
                m.cpu.flags
            );
        }
        m.step(1);
        if i % 100_000 == 0 {
            let text = m.bus.text();
            if trace && text != old_text {
                eprintln!("{text}");
                old_text = text;
            }
        }
        if m.error.is_some() {
            break;
        }
    }
    println!("{}", m.bus.text());
    println!("instructions={} cycles={} fdc_reads={} fdc_writes={} beep={} edges={} pc={:04x}:{:04x} ports={:x?}",m.cpu.retired,m.bus.cycles,m.bus.fdc.reads,m.bus.fdc.writes,m.bus.beep,m.bus.beep_edges,m.cpu.s[1],m.cpu.ip,m.bus.unknown_ports);
    if let Some(e) = m.error {
        eprintln!("{e}");
        std::process::exit(1)
    }
}

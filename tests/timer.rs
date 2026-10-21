use pc98_rust::{cpu::*, Machine, Profile};

fn run(m: &mut Machine, slots: u32) {
    assert_eq!(m.step(slots), slots);
    assert!(m.error.is_none(), "{:?}", m.error);
}

fn request(m: &mut Machine, count: u16, target: u16) -> u64 {
    m.cpu.s = [0x1200, 0, 0x3000, 0x4567];
    m.cpu.ip = 0x100;
    m.cpu.r = [
        0x02ab, count, 0x9876, target, 0x8000, 0x4567, 0x6789, 0x789a,
    ];
    m.cpu.flags = IF | DF | CF | 2;
    m.cpu.halted = false;
    m.bus.mem[0x100..0x103].copy_from_slice(&[0xcd, 0x1c, 0xf4]);
    let regs = m.cpu.r;
    run(m, 3); // INT, ROM OUT, IRET
    assert_eq!(m.cpu.r, regs);
    assert_eq!(m.cpu.flags & 0x0fff, IF | DF | CF | 2);
    assert_eq!(m.cpu.ip, 0x102);
    m.bus.cycles - 16 // OUT services the request before that slot's tick
}

fn machine(profile: Profile) -> Machine {
    let mut m = Machine::new(profile);
    m.bus.pics[0].mask = 0xfe;
    // INC byte [CS:0210]; IRET. The host never executes this handler.
    m.bus.mem[0x12200..0x12206].copy_from_slice(&[0x2e, 0xfe, 0x06, 0x10, 0x02, 0xcf]);
    m
}

fn advance_to(m: &mut Machine, cycles: u64) {
    while m.bus.cycles < cycles {
        m.bus
            .tick((cycles - m.bus.cycles).min(u32::MAX as u64) as u32);
    }
}

#[test]
fn interval_units_one_shot_frame_registers_and_iret_in_both_profiles() {
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        for count in [1, 2, 17] {
            let mut m = machine(profile);
            m.bus.tick(1234); // no dependency on a periodic PIT boundary
            let start = request(&mut m, count, 0x200);
            run(&mut m, 1); // HLT, resume address 0103
            let regs = m.cpu.r;
            let segments = m.cpu.s;
            let flags = m.cpu.flags;
            let deadline = start + count as u64 * 80_000;
            advance_to(&mut m, deadline - 1);
            assert_eq!(m.bus.pics[0].irr & 1, 0);
            run(&mut m, 1); // crosses deadline, still no early entry
            assert_eq!(m.cpu.s, segments);
            assert_eq!(m.bus.read(0x12210), 0);
            run(&mut m, 1); // IRQ0 ROM OUT transfers to callback
            assert_eq!((m.cpu.s[CS], m.cpu.ip), (0x1200, 0x200));
            assert_eq!(m.cpu.r[SP], regs[SP] - 6);
            assert_eq!(m.cpu.flags & (IF | TF), 0);
            assert_eq!(m.bus.read_seg(segments[SS], regs[SP] - 6, true), 0x103);
            assert_eq!(
                m.bus.read_seg(segments[SS], regs[SP] - 4, true),
                segments[CS]
            );
            assert_eq!(m.bus.read_seg(segments[SS], regs[SP] - 2, true), flags);
            assert_eq!(m.bus.pics[0].isr & 1, 0); // BIOS acknowledged IRQ
            assert_eq!(m.bus.read(0x12210), 0);
            run(&mut m, 2); // guest INC then guest IRET
            assert_eq!(m.bus.read(0x12210), 1);
            assert_eq!(m.cpu.r, regs);
            assert_eq!(m.cpu.s, segments);
            assert_eq!(m.cpu.flags, flags);
            assert_eq!(m.cpu.ip, 0x103);
            m.cpu.halted = true;
            for _ in 0..10 {
                run(&mut m, 100_000);
            }
            assert_eq!(m.bus.read(0x12210), 1);
        }
    }
}

#[test]
fn zero_means_65536_intervals_and_large_ticks_do_not_repeat() {
    let mut m = machine(Profile::Pc9821ApDos);
    let start = request(&mut m, 0, 0x200);
    m.cpu.halted = true;
    advance_to(&mut m, start + 65_536 * 80_000 - 1);
    assert_eq!(m.bus.pics[0].irr & 1, 0);
    assert_eq!(m.bus.read(0x12210), 0);
    m.bus.tick(1);
    run(&mut m, 3);
    assert_eq!(m.bus.read(0x12210), 1);
    m.cpu.halted = true;
    m.bus.tick(u32::MAX);
    run(&mut m, 10);
    assert_eq!(m.bus.read(0x12210), 1);
}

#[test]
fn expiry_obeys_if_pic_mask_priority_and_sti_shadow() {
    let mut m = machine(Profile::Pc9821ApDos);
    let start = request(&mut m, 1, 0x200);
    m.cpu.flags &= !IF;
    m.cpu.halted = true;
    advance_to(&mut m, start + 80_000);
    run(&mut m, 100);
    assert_eq!(m.bus.read(0x12210), 0);
    assert_eq!(m.bus.pics[0].irr & 1, 1);
    m.bus.pics[0].mask = 255;
    m.cpu.flags |= IF;
    run(&mut m, 100);
    assert_eq!(m.bus.read(0x12210), 0);
    m.bus.pics[0].mask = 0xfe;
    m.bus.pics[0].isr = 1; // equal-priority IRQ still in service
    run(&mut m, 100);
    assert_eq!(m.bus.read(0x12210), 0);
    m.bus.output(0, 0x20);
    m.cpu.flags &= !IF;
    m.cpu.halted = false;
    m.cpu.ip = 0x400;
    m.bus.mem[0x400..0x403].copy_from_slice(&[0xfb, 0x90, 0xf4]);
    run(&mut m, 1); // STI
    run(&mut m, 1); // protected NOP
    assert_eq!(m.cpu.ip, 0x402);
    run(&mut m, 3); // callback then IRET
    assert_eq!(m.cpu.ip, 0x402);
    assert_eq!(m.bus.read(0x12210), 1);
}

#[test]
fn subsequent_request_replaces_active_or_expired_request_and_reset_clears_it() {
    for expired in [false, true] {
        let mut m = machine(Profile::Pc9821ApDos);
        let first = request(&mut m, 1, 0x250);
        m.cpu.flags &= !IF;
        if expired {
            advance_to(&mut m, first + 80_000);
        }
        // A pending IRQ0 may still run, but must never invoke the old target.
        m.bus.pics[0].mask = 255;
        let second = request(&mut m, 2, 0x200);
        m.cpu.halted = true;
        m.bus.pics[0].mask = 0xfe;
        run(&mut m, 2);
        assert_eq!(m.bus.read(0x12210), 0);
        advance_to(&mut m, second + 160_000);
        run(&mut m, 3);
        assert_eq!(m.bus.read(0x12210), 1);
        m.bus.pics[0].mask = 255;
        let third = request(&mut m, 1, 0x200);
        advance_to(&mut m, third + 80_000); // expired but masked
        m.reset();
        m.cpu.s = [0; 4];
        m.cpu.ip = 0x100;
        m.cpu.halted = true;
        m.cpu.flags |= IF;
        m.bus.pics[0].mask = 0xfe;
        m.bus.tick(160_000);
        run(&mut m, 100);
        assert_eq!(m.cpu.s[CS], 0);
        assert_eq!(m.cpu.ip, 0x100);
        assert_eq!(m.bus.pics[0].irr & 1, 0);
    }
}

#[test]
fn original_timer_guest_boots_through_freedos_and_checks_its_irq_frame() {
    let image = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/freedos/timer.hdm"
    ))
    .expect("run python3 tools/prepare_freedos.py");
    for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
        let mut m = Machine::new(profile);
        m.insert(0, image.clone(), "timer.hdm").unwrap();
        m.reset();
        let mut passed = false;
        for _ in 0..160 {
            run(&mut m, 100_000);
            if m.bus.text().contains("TIMER CALLBACK PASS") {
                passed = true;
                break;
            }
            assert!(!m.bus.text().contains("TIMER CALLBACK TIMEOUT"));
        }
        assert!(passed, "{}", m.bus.text());
        // Match the initialized resident state, not another copy in DOS buffers.
        let state = (0x600..0x9ffe0)
            .find(|&p| &m.bus.mem[p..p + 8] == b"T98STATE" && m.bus.read(p + 9) == 1)
            .expect("guest must validate callback frame and set result=1");
        assert_eq!(m.bus.read(state + 8), 1);
        let word = |off| m.bus.read16(state + off);
        assert_eq!(word(12), word(10) - 6);
        assert!(word(14) >= word(22) && word(14) < word(24));
        assert_eq!(word(16), word(20));
        assert_ne!(word(18) & IF, 0);
        assert_eq!(state, word(20) as usize * 16 + 0x300);
        assert!(m.bus.fdc.reads > 100);
        for _ in 0..10 {
            run(&mut m, 100_000);
        }
        assert_eq!(m.bus.read(state + 8), 1);
        assert_eq!(m.bus.read(state + 9), 1);
    }
}

#[test]
fn callback_can_rearm_after_the_first_request_is_disarmed() {
    let mut m = machine(Profile::Pc9821ApDos);
    // Preserve the interrupted registers, request a second callback, then IRET.
    let code = [
        0x50, 0x51, 0x53, // PUSH AX,CX,BX
        0xb4, 2, 0xb9, 1, 0, 0xbb, 0, 2, // AH=2, CX=1, BX=0200
        0xcd, 0x1c, // same ES:0200, from inside callback
        0x5b, 0x59, 0x58, 0xcf, // POP BX,CX,AX; IRET
    ];
    m.bus.mem[0x12250..0x12250 + code.len()].copy_from_slice(&code);
    let start = request(&mut m, 1, 0x250);
    run(&mut m, 1); // HLT
    let regs = m.cpu.r;
    advance_to(&mut m, start + 80_000);
    run(&mut m, 14); // IRQ thunk, guest handler including nested BIOS INT, IRET
    assert_eq!(m.cpu.r, regs);
    assert_eq!(m.cpu.s[CS], 0);
    assert_eq!(m.bus.read(0x12210), 0);
    m.cpu.halted = true;
    m.bus.tick(80_000);
    run(&mut m, 3);
    assert_eq!(m.bus.read(0x12210), 1);
    assert_eq!(m.cpu.r, regs);
}

#[test]
fn reset_clears_an_unexpired_request() {
    let mut m = machine(Profile::Pc9821ApDos);
    request(&mut m, 0, 0x200);
    m.reset();
    m.cpu.s = [0; 4];
    m.cpu.ip = 0x100;
    m.cpu.flags |= IF;
    m.cpu.halted = true;
    m.bus.pics[0].mask = 0xfe;
    advance_to(&mut m, 65_536 * 80_000 + 1);
    run(&mut m, 10);
    assert_eq!(m.cpu.ip, 0x100);
    assert_eq!(m.bus.pics[0].irr & 1, 0);
}

#[test]
fn periodic_pit_irqs_cannot_expire_early_and_step_slices_are_deterministic() {
    let mut snapshots = Vec::new();
    for slice in [1, 37, 100_000] {
        let mut m = machine(Profile::Pc9821ApDos);
        request(&mut m, 1, 0x200);
        // HLT; JMP back to HLT, so periodic IRQs always resume valid guest code.
        m.bus.mem[0x102..0x105].copy_from_slice(&[0xf4, 0xeb, 0xfd]);
        m.bus.pit.write(3, 0x36);
        m.bus.pit.write(0, 0xe8); // PIT divisor 1000, much shorter than 10 ms
        m.bus.pit.write(0, 3);
        let mut left = 9997; // cycles=80000, eight cycles before expiry
        while left > 0 {
            let n = left.min(slice);
            run(&mut m, n);
            left -= n;
        }
        assert_eq!(m.bus.read(0x12210), 0);
        assert!(m.bus.read16(0x58a) > 0); // intervening BIOS IRQ0s were serviced
        let mut left = 20_000;
        while left > 0 {
            let n = left.min(slice);
            run(&mut m, n);
            left -= n;
        }
        assert_eq!(m.bus.read(0x12210), 1);
        snapshots.push((
            m.cpu.r,
            m.cpu.s,
            m.cpu.ip,
            m.cpu.flags,
            m.bus.cycles,
            m.bus.read16(0x58a),
        ));
    }
    assert!(snapshots.windows(2).all(|w| w[0] == w[1]));
}

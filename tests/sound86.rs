use pc98_rust::{bus::Bus, pcm::Pcm86, Profile};

fn register(b: &mut Bus, bank: bool, a: u8, value: u8) {
    b.output(if bank { 0x18c } else { 0x188 }, a);
    b.output(if bank { 0x18e } else { 0x18a }, value);
}

#[test]
fn opna_extended_bank_mask_sixth_channel_and_timer_irq() {
    let mut b = Bus::new(Profile::Pc9821ApDos);
    assert_eq!(b.input(0xa460), 0x40);
    register(&mut b, true, 0x30, 3);
    assert_eq!(b.opn.registers[0x130], 0);
    b.output(0xa460, 1);
    assert_eq!(b.input(0xa460), 0x41);
    register(&mut b, false, 0x29, 0x83);
    for slot in [0, 4, 8, 12] {
        for (reg, value) in [(0x32, 1), (0x42, 24), (0x52, 31), (0x82, 15)] {
            register(&mut b, true, reg + slot, value);
        }
    }
    for (reg, value) in [(0xb2, 7), (0xa6, 0x22), (0xa2, 0x69)] {
        register(&mut b, true, reg, value);
    }
    assert_eq!(b.opn.registers[0xa2], 0);
    register(&mut b, false, 0x28, 0xf6); // FM6, bank 1 channel 2
    b.tick(100_000);
    assert!(b.audio.iter().any(|v| v.abs() > 0.01));
    assert_eq!(b.opn.key_edges, 4);
    b.output(0xa460, 3);
    let writes = b.opn.writes;
    register(&mut b, false, 0x28, 0);
    assert_eq!(writes, b.opn.writes);
    b.output(0xa460, 1);
    for (reg, value) in [(0x24, 255), (0x25, 3), (0x27, 5)] {
        register(&mut b, false, reg, value);
    }
    b.tick(145);
    assert_ne!(b.pics[1].irr & 16, 0);
}

#[test]
fn pcm_fifo_signed_samples_endianness_stereo_irq_reset_and_bounds() {
    let mut p = Pcm86::default();
    assert_eq!(p.input(0xa466) & 0xc0, 0x40);
    p.output(0xa46a, 0xd2); // 8-bit right channel
    p.output(0xa468, 0x28);
    p.output(0xa468, 0x20);
    p.output(0xa46a, 1); // interrupt at 128 bytes
    for _ in 0..256 {
        p.output(0xa46c, 0x40);
    }
    p.output(0xa468, 0xa0);
    let mut peak: f32 = 0.0;
    for _ in 0..150 {
        peak = peak.max(p.sample());
    }
    assert_eq!(peak, 0.15);
    assert!(p.irq);
    assert_ne!(p.input(0xa468) & 16, 0);
    p.output(0xa468, 0x80);
    assert!(!p.irq);
    p.output(0xa468, 8);
    assert!(p.fifo.is_empty());
    assert_eq!(p.sample(), 0.0);
    p.output(0xa468, 0);
    p.output(0xa46a, 0xb2); // signed 16-bit stereo, high byte first
    for v in [0x40, 0x00, 0xc0, 0x00] {
        p.output(0xa46c, v);
    }
    p.output(0xa468, 0x80);
    for _ in 0..2 {
        assert_eq!(p.sample(), 0.0);
    }
    p.output(0xa468, 8);
    p.output(0xa468, 0);
    p.output(0xa46a, 0x92);
    for v in [0x80, 0x00] {
        p.output(0xa46c, v);
    }
    p.output(0xa468, 0x80);
    p.sample();
    assert_eq!(p.sample(), -0.3);
    p.output(0xa468, 8);
    p.output(0xa468, 0);
    for _ in 0..40000 {
        p.output(0xa46c, 0);
    }
    assert_eq!(p.fifo.len(), 32768);
    assert_ne!(p.input(0xa466) & 128, 0);
}

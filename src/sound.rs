//! Original, sample-clocked OPN subset. No chip code or ROM tables are imported.
//! Register contract: Yamaha YM2203 datasheet. Analog/envelope timing is approximate.
use std::f64::consts::TAU;

const CLOCK: u64 = 3_993_600;

#[derive(Clone, Copy, Default)]
struct Operator {
    phase: f64,
    level: f64,
    stage: u8, // 0 off, 1 attack, 2 decay, 3 sustain, 4 release
    key: bool,
}

pub struct Opn {
    pub registers: [u8; 512],
    pub address: u8,
    pub bank_address: u8,
    pub writes: u64,
    pub key_edges: u64,
    operators: [[Operator; 4]; 6],
    feedback: [f64; 6],
    timer: [u64; 2],
    fraction: u64,
    busy: u64,
    status: u8,
    ssg_phase: [f64; 3],
    noise_phase: f64,
    noise: u32,
    envelope_phase: f64,
    envelope_step: u32,
}

impl Default for Opn {
    fn default() -> Self {
        let mut registers = [0; 512];
        registers[7] = 0xff;
        Self {
            registers,
            address: 0,
            bank_address: 0,
            writes: 0,
            key_edges: 0,
            operators: [[Operator::default(); 4]; 6],
            feedback: [0.0; 6],
            timer: [0; 2],
            fraction: 0,
            busy: 0,
            status: 0,
            ssg_phase: [0.0; 3],
            noise_phase: 0.0,
            noise: 1,
            envelope_phase: 0.0,
            envelope_step: 0,
        }
    }
}

impl Opn {
    pub fn status(&self) -> u8 {
        self.status | if self.busy > 0 { 128 } else { 0 }
    }
    pub fn irq(&self) -> bool {
        self.status & 3 != 0
    }
    pub fn write(&mut self, value: u8) {
        self.write_at(self.address as usize, value);
    }
    pub fn write_bank(&mut self, value: u8) {
        self.write_at(256 + self.bank_address as usize, value);
    }
    fn write_at(&mut self, a: usize, value: u8) {
        self.writes += 1;
        self.busy = 32;
        let old = self.registers[a];
        self.registers[a] = value;
        match a {
            0x0d => {
                self.envelope_phase = 0.0;
                self.envelope_step = 0;
            }
            0x27 => {
                self.status &= !((value >> 4) & 3);
                for t in 0..2 {
                    if old & (1 << t) == 0 && value & (1 << t) != 0 {
                        self.timer[t] = 0;
                    }
                }
            }
            0x28 => {
                let channel = (value & 3) as usize;
                if channel == 3 || (value & 4 != 0 && self.registers[0x29] & 128 == 0) {
                    return;
                }
                let channel = channel + if value & 4 != 0 { 3 } else { 0 };
                for slot in 0..4 {
                    let op = &mut self.operators[channel][slot];
                    let key = value & (0x10 << slot) != 0;
                    if key != op.key {
                        self.key_edges += 1;
                        if key {
                            op.phase = 0.0;
                            op.level = 0.0;
                            op.stage = 1;
                        } else if op.stage != 0 {
                            op.stage = 4;
                        }
                    }
                    op.key = key;
                }
            }
            _ => {}
        }
    }
    pub fn tick(&mut self, cycles: u32) {
        self.fraction += cycles as u64 * CLOCK;
        let clocks = self.fraction / 8_000_000;
        self.fraction %= 8_000_000;
        self.busy = self.busy.saturating_sub(clocks);
        let a = ((self.registers[0x24] as u64) << 2) | (self.registers[0x25] as u64 & 3);
        let periods = [72 * (1024 - a), 1152 * (256 - self.registers[0x26] as u64)];
        let control = self.registers[0x27];
        for (t, period) in periods.into_iter().enumerate() {
            if control & (1 << t) != 0 {
                self.timer[t] += clocks;
                if self.timer[t] >= period {
                    self.timer[t] %= period;
                    if control & (4 << t) != 0 {
                        self.status |= 1 << t;
                    }
                }
            }
        }
    }
    fn operator(&mut self, ch: usize, slot: usize, hz: f64, modulation: f64) -> f64 {
        if self.operators[ch][slot].stage == 0 {
            return 0.0;
        }
        // Yamaha's physical slot register order is 1,3,2,4.
        let offset = ch % 3 + (ch / 3) * 256 + [0, 8, 4, 12][slot];
        let dtmul = self.registers[0x30 + offset];
        let tl = self.registers[0x40 + offset] & 127;
        let ar = self.registers[0x50 + offset] & 31;
        let dr = self.registers[0x60 + offset] & 31;
        let sr = self.registers[0x70 + offset] & 31;
        let slrr = self.registers[0x80 + offset];
        let sustain = 10f64.powf(-f64::from(slrr >> 4) * 3.0 / 20.0);
        let rate = |r: u8| {
            if r == 0 {
                0.0
            } else {
                2f64.powf(f64::from(r) / 2.0) / (48_000.0 * 16.0)
            }
        };
        let op = &mut self.operators[ch][slot];
        match op.stage {
            1 => {
                op.level += if ar == 31 {
                    1.0
                } else {
                    (1.01 - op.level) * rate(ar) * 16.0
                };
                if op.level >= 1.0 {
                    op.level = 1.0;
                    op.stage = 2;
                }
            }
            2 => {
                op.level *= 1.0 - rate(dr).min(1.0);
                if op.level <= sustain {
                    op.stage = 3;
                }
            }
            3 => op.level *= 1.0 - rate(sr).min(1.0),
            4 => {
                op.level *= 1.0 - rate((slrr & 15) * 2 + 1).min(1.0);
                if op.level < 0.00001 {
                    op.level = 0.0;
                    op.stage = 0;
                }
            }
            _ => {}
        }
        let multiple = if dtmul & 15 == 0 {
            0.5
        } else {
            f64::from(dtmul & 15)
        };
        let detune = f64::from((dtmul >> 4) & 3) * if dtmul & 64 == 0 { 0.1 } else { -0.1 };
        op.phase = (op.phase + (hz * multiple + detune) / 48_000.0).fract();
        (op.phase * TAU + modulation * 4.0).sin()
            * op.level
            * 10f64.powf(-f64::from(tl) * 0.75 / 20.0)
    }
    pub fn sample(&mut self) -> f32 {
        let mut sum = 0.0;
        for ch in 0..6 {
            if ch >= 3 && self.registers[0x29] & 128 == 0 {
                continue;
            }
            let register_ch = ch % 3 + ch / 3 * 256;
            let high = self.registers[0xa4 + register_ch];
            let fnum = u16::from(self.registers[0xa0 + register_ch]) | (u16::from(high & 7) << 8);
            let hz = f64::from(fnum) * f64::from(1u16 << ((high >> 3) & 7)) * CLOCK as f64
                / (144.0 * 1_048_576.0);
            let alg = self.registers[0xb0 + register_ch] & 7;
            let fb = (self.registers[0xb0 + register_ch] >> 3) & 7;
            let first = self.operator(
                ch,
                0,
                hz,
                if fb == 0 {
                    0.0
                } else {
                    self.feedback[ch] * f64::from(fb) / 8.0
                },
            );
            self.feedback[ch] = first;
            let second = self.operator(
                ch,
                1,
                hz,
                if matches!(alg, 0 | 3 | 4 | 5 | 6) {
                    first
                } else {
                    0.0
                },
            );
            let third = self.operator(
                ch,
                2,
                hz,
                match alg {
                    0 | 2 => second,
                    1 => first + second,
                    5 => first,
                    _ => 0.0,
                },
            );
            let fourth = self.operator(
                ch,
                3,
                hz,
                match alg {
                    0..=1 | 4 => third,
                    2 => first + third,
                    3 => second + third,
                    5 => first,
                    _ => 0.0,
                },
            );
            sum += match alg {
                0..=3 => fourth,
                4 => second + fourth,
                5..=6 => second + third + fourth,
                _ => first + second + third + fourth,
            } * 0.08;
        }
        // SSG tone/noise mixer and programmable envelope, with bounded state.
        self.noise_phase +=
            CLOCK as f64 / (64.0 * f64::from((self.registers[6] & 31).max(1)) * 48_000.0);
        while self.noise_phase >= 1.0 {
            self.noise_phase -= 1.0;
            let bit = (self.noise ^ (self.noise >> 3)) & 1;
            self.noise = (self.noise >> 1) | (bit << 16);
        }
        let period = u16::from_le_bytes([self.registers[11], self.registers[12]]).max(1);
        self.envelope_phase += CLOCK as f64 / (1024.0 * f64::from(period) * 48_000.0);
        while self.envelope_phase >= 1.0 {
            self.envelope_phase -= 1.0;
            self.envelope_step = self.envelope_step.wrapping_add(1);
        }
        let shape = self.registers[13];
        let mut step = self.envelope_step;
        let mut up = shape & 4 != 0;
        let held = step >= 32 && (shape & 8 == 0 || shape & 1 != 0);
        let level = if held {
            if shape & 8 != 0 && (up ^ (shape & 2 != 0)) {
                31
            } else {
                0
            }
        } else {
            if shape & 2 != 0 && (step / 32) & 1 != 0 {
                up = !up;
            }
            step &= 31;
            if up {
                step
            } else {
                31 - step
            }
        };
        for ch in 0..3 {
            let period = (u16::from(self.registers[ch * 2])
                | (u16::from(self.registers[ch * 2 + 1] & 15) << 8))
                .max(1);
            self.ssg_phase[ch] =
                (self.ssg_phase[ch] + CLOCK as f64 / (64.0 * f64::from(period) * 48_000.0)).fract();
            let volume = self.registers[8 + ch];
            let level = if volume & 16 != 0 {
                level as u8
            } else {
                (volume & 15) * 2 + u8::from(volume & 15 != 0)
            };
            if level != 0 {
                let gate = (self.registers[7] & (1 << ch) != 0 || self.ssg_phase[ch] < 0.5)
                    && (self.registers[7] & (8 << ch) != 0 || self.noise & 1 != 0);
                sum += if gate { 1.0 } else { -1.0 }
                    * 0.08
                    * 10f64.powf((f64::from(level) - 31.0) * 1.5 / 20.0);
            }
        }
        sum.clamp(-1.0, 1.0) as f32
    }
}

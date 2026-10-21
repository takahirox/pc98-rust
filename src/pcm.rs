//! Original PC-9801-86 playback FIFO; io_sound.txt A460/A466–A46C.
use std::collections::VecDeque;

pub struct Pcm86 {
    pub fifo: VecDeque<u8>,
    pub control: u8,
    pub format: u8,
    pub mask: u8,
    pub writes: u64,
    pub samples: u64,
    pub irq: bool,
    threshold: usize,
    phase: u64,
    output: f32,
    clock: bool,
    attenuation: u8,
}
impl Default for Pcm86 {
    fn default() -> Self {
        Self {
            fifo: VecDeque::with_capacity(32768),
            control: 0,
            format: 0,
            mask: 0,
            writes: 0,
            samples: 0,
            irq: false,
            threshold: 128,
            phase: 0,
            output: 0.0,
            clock: false,
            attenuation: 0,
        }
    }
}
impl Pcm86 {
    pub fn input(&self, p: u16) -> u8 {
        match p {
            0xa460 => 0x40 | self.mask,
            0xa466 => {
                u8::from(self.clock)
                    | if self.fifo.is_empty() { 64 } else { 0 }
                    | if self.fifo.len() == 32768 { 128 } else { 0 }
            }
            0xa468 => (self.control & !16) | if self.irq { 16 } else { 0 },
            0xa46a => self.format,
            _ => 255,
        }
    }
    pub fn output(&mut self, p: u16, value: u8) {
        match p {
            0xa460 => self.mask = value & 3,
            0xa466 if value >> 5 == 5 => self.attenuation = value & 15,
            0xa468 => {
                self.control = value;
                if value & 16 == 0 {
                    self.irq = false;
                }
                if value & 8 != 0 {
                    self.fifo.clear();
                    self.phase = 0;
                    self.output = 0.0;
                }
            }
            0xa46a if self.control & 32 != 0 => self.threshold = value as usize * 128,
            0xa46a => self.format = value,
            0xa46c if self.fifo.len() < 32768 && self.control & 64 == 0 => {
                self.fifo.push_back(value);
                self.writes += 1;
            }
            _ => {}
        }
    }
    /// Sample-and-hold resampling to the same 48 kHz stream as FM and beeper.
    pub fn sample(&mut self) -> f32 {
        if self.control & 0xc8 != 128 {
            return 0.0;
        }
        let divisor = [12, 16, 24, 32, 48, 64, 96, 128][self.control as usize & 7];
        self.phase += 529_200;
        while self.phase >= 48_000 * divisor {
            self.phase -= 48_000 * divisor;
            self.clock = !self.clock;
            let channels = if self.format & 0x30 == 0x30 { 2 } else { 1 };
            let bytes = if self.format & 64 != 0 { 1 } else { 2 };
            let old = self.fifo.len();
            if old < channels * bytes {
                self.output = 0.0;
                continue;
            }
            self.output = 0.0;
            for _ in 0..channels {
                let high = self.fifo.pop_front().unwrap();
                self.output += if bytes == 1 {
                    (high as i8) as f32 / 128.0
                } else {
                    i16::from_be_bytes([high, self.fifo.pop_front().unwrap()]) as f32 / 32768.0
                };
            }
            self.output /= channels as f32;
            if self.format & 0x30 == 0 {
                self.output = 0.0;
            }
            self.samples += 1;
            if self.control & 32 != 0 && old > self.threshold && self.fifo.len() <= self.threshold {
                self.irq = true;
            }
        }
        self.output * (15 - self.attenuation) as f32 / 15.0 * 0.3
    }
}

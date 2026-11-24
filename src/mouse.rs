//! Original PC-98 8255 bus mouse. UNDOCUMENTED 9801/9821, io_mouse.txt.
use std::collections::VecDeque;

const HOST_QUEUE_CAPACITY: usize = 64;
const HOST_QUEUE_TIMEOUT: u32 = 2_000_000; // 250 ms at 8 MHz

struct HostSample {
    motion: [i64; 2],
    buttons: u8,
}

impl HostSample {
    fn add_motion(&mut self, motion: [i64; 2]) {
        for (pending, delta) in self.motion.iter_mut().zip(motion) {
            *pending = pending.saturating_add(delta);
        }
    }
}

pub struct Mouse {
    counter: [u8; 2],
    latch: [u8; 2],
    pending: VecDeque<HostSample>,
    pending_clocks: u32,
    sampled_buttons: Option<u8>,
    pub control: u8,
    pub rate: u8,
    mode: u8,
    buttons: u8,
    fraction: u64,
    pub events: u64,
    pub latches: u64,
}
impl Default for Mouse {
    fn default() -> Self {
        Self {
            counter: [0; 2],
            latch: [0; 2],
            pending: VecDeque::new(),
            pending_clocks: 0,
            sampled_buttons: None,
            control: 0x10,
            rate: 0,
            mode: 0x93,
            buttons: 0,
            fraction: 0,
            events: 0,
            latches: 0,
        }
    }
}
impl Mouse {
    pub(crate) fn clocks_until_irq(&self) -> u64 {
        (8_000_000 - self.fraction).div_ceil(120 >> (self.rate & 3))
    }
    /// Apply movement directly to the hardware counters.
    pub fn motion(&mut self, dx: i32, dy: i32, buttons: u8) {
        self.counter[0] = self.counter[0].wrapping_add(dx as u8);
        self.counter[1] = self.counter[1].wrapping_add(dy as u8);
        self.buttons = buttons & 7;
        self.sampled_buttons = None;
        self.events += 1;
    }
    /// Keep live button levels for direct reads and queue samples for drivers
    /// that latch movement. Motion between button changes shares one sample.
    pub fn host_motion(&mut self, dx: i32, dy: i32, buttons: u8) {
        let buttons = buttons & 7;
        if buttons != self.buttons {
            self.buttons = buttons;
            self.sampled_buttons = None;
        }
        if self.pending.is_empty() {
            self.pending_clocks = HOST_QUEUE_TIMEOUT;
        }
        let motion = [i64::from(dx), i64::from(dy)];
        if let Some(sample) = self
            .pending
            .back_mut()
            .filter(|sample| sample.buttons == buttons)
        {
            sample.add_motion(motion);
        } else {
            if self.pending.len() == HOST_QUEUE_CAPACITY {
                // Drop the oldest button state under pressure, carrying its
                // remaining motion into the next sample.
                let oldest = self.pending.pop_front().unwrap();
                self.pending.front_mut().unwrap().add_motion(oldest.motion);
            }
            self.pending.push_back(HostSample { motion, buttons });
        }
        self.events += 1;
    }
    pub fn release(&mut self) {
        self.buttons = 0;
        self.pending.clear();
        self.pending_clocks = 0;
        self.sampled_buttons = None;
    }
    pub fn tick(&mut self, cycles: u32) -> bool {
        if self.pending_clocks != 0 {
            self.pending_clocks = self.pending_clocks.saturating_sub(cycles);
            if self.pending_clocks == 0 {
                self.pending.clear();
                self.sampled_buttons = None;
            }
        }
        self.fraction += cycles as u64 * (120 >> (self.rate & 3));
        if self.fraction < 8_000_000 {
            return false;
        }
        self.fraction %= 8_000_000;
        self.control & 0x10 == 0
    }
    pub fn input(&mut self, port: u16, ap: bool) -> u8 {
        match port {
            0x7fd9 => {
                let axis = usize::from(self.control & 0x40 != 0);
                let count = if self.control & 0x80 == 0 {
                    let pending = self.pending.front().map_or(0, |sample| sample.motion[axis]);
                    self.counter[axis].wrapping_add(pending as u8)
                } else {
                    self.latch[axis]
                };
                // The first status read of a new sample can report a queued
                // tap. Reads with HC held steady see the live button levels.
                let buttons = self.sampled_buttons.take().unwrap_or(self.buttons);
                let bits = (if buttons & 1 != 0 { 0x80 } else { 0 })
                    | (if buttons & 2 != 0 { 0x20 } else { 0 })
                    | (if buttons & 4 != 0 { 0x40 } else { 0 });
                (0xe0 & !bits) | ((count >> if self.control & 0x20 != 0 { 4 } else { 0 }) & 15)
            }
            0x7fdb => 0xff,
            0x7fdd => (self.control & 0xf0) | if ap { 8 } else { 0 },
            0xbfdb => self.rate,
            _ => 0xff,
        }
    }
    fn set_control(&mut self, value: u8) {
        if value & 0x80 == 0 {
            self.sampled_buttons = None;
        } else if self.control & 0x80 == 0 {
            self.latch = self.counter;
            self.counter = [0; 2];
            if let Some(sample) = self.pending.front_mut() {
                self.sampled_buttons = Some(sample.buttons);
                for (latched, pending) in self.latch.iter_mut().zip(&mut sample.motion) {
                    let count = i64::from(*latched as i8);
                    let total = count.saturating_add(*pending).clamp(-128, 127);
                    *latched = total as u8;
                    *pending -= total - count;
                }
                if sample.motion == [0; 2] {
                    self.pending.pop_front();
                }
            }
            // Extend the lifetime of the backlog only while the guest samples.
            self.pending_clocks = if self.pending.is_empty() && self.sampled_buttons.is_none() {
                0
            } else {
                HOST_QUEUE_TIMEOUT
            };
            self.latches += 1;
        }
        self.control = value & 0xf0;
    }
    pub fn output(&mut self, port: u16, value: u8) {
        match port {
            0x7fdd => self.set_control(value),
            0x7fdf if value & 128 == 0 => {
                let bit = 1 << ((value >> 1) & 7);
                self.set_control(if value & 1 != 0 {
                    self.control | bit
                } else {
                    self.control & !bit
                });
            }
            0x7fdf => {
                self.mode = value;
                self.set_control(0);
            }
            0xbfdb => self.rate = value & 3,
            _ => {}
        }
    }
}

//! Original PC-98 8255 bus mouse. UNDOCUMENTED 9801/9821, io_mouse.txt.
use std::collections::VecDeque;

const HOST_QUEUE_CAPACITY: usize = 64;
const HOST_QUEUE_TIMEOUT: u64 = 2_000_000; // 250 ms at 8 MHz

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct ButtonState {
    bits: u8,
    revision: u64,
}

struct HostSample {
    motion: [i64; 2],
    buttons: ButtonState,
    received_at: u64,
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
    host_clock: u64,
    last_sample_at: u64,
    sampled_buttons: Option<ButtonState>,
    reported_buttons: ButtonState,
    pub control: u8,
    pub rate: u8,
    mode: u8,
    live_buttons: ButtonState,
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
            host_clock: 0,
            last_sample_at: 0,
            sampled_buttons: None,
            reported_buttons: ButtonState::default(),
            control: 0x10,
            rate: 0,
            mode: 0x93,
            live_buttons: ButtonState::default(),
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
    fn update_buttons(&mut self, buttons: u8) {
        let bits = buttons & 7;
        if bits != self.live_buttons.bits {
            self.live_buttons.bits = bits;
            self.live_buttons.revision += 1;
            // Latching can remove a transition from the queue before the
            // guest reads it. Only let live input supersede a delivered sample.
            if self
                .sampled_buttons
                .is_some_and(|sample| sample.revision <= self.reported_buttons.revision)
            {
                self.sampled_buttons = None;
            }
        }
    }
    /// Apply movement directly to the hardware counters.
    pub fn motion(&mut self, dx: i32, dy: i32, buttons: u8) {
        self.counter[0] = self.counter[0].wrapping_add(dx as u8);
        self.counter[1] = self.counter[1].wrapping_add(dy as u8);
        self.update_buttons(buttons);
        self.sampled_buttons = None;
        self.events += 1;
    }
    /// Keep live button levels for direct reads and queue samples for drivers
    /// that latch movement. Keep distinct arrival times so old motion can expire.
    pub fn host_motion(&mut self, dx: i32, dy: i32, buttons: u8) {
        self.update_buttons(buttons);
        let buttons = self.live_buttons;
        let motion = [i64::from(dx), i64::from(dy)];
        let received_at = self.host_clock;
        if let Some(sample) = self
            .pending
            .back_mut()
            .filter(|sample| sample.buttons == buttons && sample.received_at == received_at)
        {
            sample.add_motion(motion);
        } else {
            if self.pending.len() == HOST_QUEUE_CAPACITY {
                // Drop the oldest button state under pressure, carrying its
                // remaining motion into the next sample.
                let oldest = self.pending.pop_front().unwrap();
                self.pending.front_mut().unwrap().add_motion(oldest.motion);
            }
            self.pending.push_back(HostSample {
                motion,
                buttons,
                received_at,
            });
        }
        self.events += 1;
    }
    pub fn release(&mut self) {
        self.update_buttons(0);
        self.pending.clear();
        self.sampled_buttons = None;
        self.reported_buttons = self.live_buttons;
    }
    pub fn tick(&mut self, cycles: u32) -> bool {
        if !self.pending.is_empty() || self.sampled_buttons.is_some() {
            self.host_clock += u64::from(cycles);
            // Keep motion available to both relatching and unlatched polling
            // drivers. After sampling stops, expire each arrival separately.
            while self.pending.front().is_some_and(|sample| {
                self.host_clock - sample.received_at.max(self.last_sample_at) >= HOST_QUEUE_TIMEOUT
            }) {
                self.pending.pop_front();
            }
            // A driver holding HC high can still see the live button levels
            // after the queued sample has had time to be read.
            if self.host_clock - self.last_sample_at >= HOST_QUEUE_TIMEOUT {
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
                    self.last_sample_at = self.host_clock;
                    let mut count = self.counter[axis];
                    for sample in &self.pending {
                        count = count.wrapping_add(sample.motion[axis] as u8);
                    }
                    count
                } else {
                    self.latch[axis]
                };
                let buttons = if let Some(sampled) = self.sampled_buttons {
                    // Older motion can remain queued after its buttons were read.
                    if sampled.revision > self.reported_buttons.revision {
                        self.reported_buttons = sampled;
                    }
                    self.reported_buttons.bits
                } else {
                    // Show the live levels, but only record them as delivered if
                    // doing so leaves no unread transition behind in the queue.
                    let live = self.live_buttons;
                    if live.revision > self.reported_buttons.revision
                        && !self.pending.iter().any(|sample| {
                            sample.buttons.revision > self.reported_buttons.revision
                                && sample.buttons.revision < live.revision
                        })
                    {
                        self.reported_buttons = live;
                    }
                    live.bits
                };
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
            self.sampled_buttons = self.pending.front().map(|sample| sample.buttons);
            // Consecutive arrivals with the same buttons share a latch up to
            // the signed counter limit. Leave later button edges queued.
            while let Some(sample) = self
                .pending
                .front_mut()
                .filter(|sample| Some(sample.buttons) == self.sampled_buttons)
            {
                for (latched, pending) in self.latch.iter_mut().zip(&mut sample.motion) {
                    let count = i64::from(*latched as i8);
                    let total = count.saturating_add(*pending).clamp(-128, 127);
                    *latched = total as u8;
                    *pending -= total - count;
                }
                if sample.motion != [0; 2] {
                    break;
                }
                self.pending.pop_front();
            }
            self.last_sample_at = self.host_clock;
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

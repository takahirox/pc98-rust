//! Original PC-98 8255 bus mouse. UNDOCUMENTED 9801/9821, io_mouse.txt.
use std::collections::VecDeque;

const HOST_QUEUE_CAPACITY: usize = 64;
const HOST_QUEUE_TIMEOUT: u64 = 2_000_000; // 250 ms at 8 MHz
const BUTTON_LATCH_WINDOW: u64 = 8_000; // 1 ms at 8 MHz
const ALL_MOTION_NIBBLES: u8 = 0b1111;

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
    latched_buttons: Option<ButtonState>,
    unread_motion_nibbles: u8,
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
            latched_buttons: None,
            unread_motion_nibbles: 0,
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
        }
    }
    fn unread_latch(&self) -> Option<ButtonState> {
        self.latched_buttons
            .filter(|buttons| buttons.revision > self.reported_buttons.revision)
    }
    fn read_buttons(&mut self, latched: bool) -> ButtonState {
        // Each unread motion nibble keeps the original buttons even when
        // reads are spaced out. A driver polling an already-read nibble can
        // see later edges with HC still high after the short latch window.
        if let Some(buttons) = self.latched_buttons.filter(|_| latched) {
            let nibble = 1 << ((self.control >> 5) & 3);
            if self.unread_motion_nibbles & nibble != 0 {
                if self.unread_motion_nibbles == ALL_MOTION_NIBBLES {
                    self.last_sample_at = self.host_clock;
                }
                self.unread_motion_nibbles &= !nibble;
                return buttons;
            }
            if self.host_clock - self.last_sample_at < BUTTON_LATCH_WINDOW {
                return buttons;
            }
        }
        let buttons = if let Some(buttons) = self.unread_latch() {
            buttons
        } else if let Some(sample) = self.pending.iter().find(|sample| {
            sample.buttons.revision > self.reported_buttons.revision
                || (latched && sample.motion != [0; 2])
        }) {
            if sample.buttons.revision <= self.reported_buttons.revision {
                // HC high only exposes the latched count. Wait for a relatch
                // to drain older motion without letting button-only polls
                // keep that motion alive indefinitely.
                return self.reported_buttons;
            }
            sample.buttons
        } else {
            self.live_buttons
        };
        let latched_buttons = latched.then_some(buttons);
        if self.latched_buttons != latched_buttons {
            self.unread_motion_nibbles = 0;
        }
        self.latched_buttons = latched_buttons;
        self.last_sample_at = self.host_clock;
        buttons
    }
    /// Apply movement directly to the hardware counters.
    pub fn motion(&mut self, dx: i32, dy: i32, buttons: u8) {
        self.counter[0] = self.counter[0].wrapping_add(dx as u8);
        self.counter[1] = self.counter[1].wrapping_add(dy as u8);
        self.update_buttons(buttons);
        self.events += 1;
    }
    /// Queue input for both polling and latching drivers. Keep distinct
    /// arrival times so old motion can expire.
    pub fn host_motion(&mut self, dx: i32, dy: i32, buttons: u8) {
        self.update_buttons(buttons);
        let buttons = self.live_buttons;
        let motion = [i64::from(dx), i64::from(dy)];
        let received_at = self.host_clock;
        let full = self.pending.len() == HOST_QUEUE_CAPACITY;
        if let Some(sample) = self.pending.back_mut().filter(|sample| {
            sample.buttons == buttons && (sample.received_at == received_at || full)
        }) {
            sample.add_motion(motion);
            sample.received_at = received_at;
        } else {
            if full {
                // Compact motion within a button state first. If the queue
                // contains only distinct edges, fold the oldest into its successor.
                let merge_at = self
                    .pending
                    .iter()
                    .zip(self.pending.iter().skip(1))
                    .position(|(older, newer)| older.buttons == newer.buttons)
                    .unwrap_or(0);
                let older = self.pending.remove(merge_at).unwrap();
                self.pending[merge_at].add_motion(older.motion);
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
        self.latch = [0; 2];
        self.latched_buttons = None;
        self.unread_motion_nibbles = 0;
        self.reported_buttons = self.live_buttons;
    }
    pub fn tick(&mut self, cycles: u32) -> bool {
        if !self.pending.is_empty() || self.latched_buttons.is_some() {
            self.host_clock += u64::from(cycles);
            // Keep motion available to both relatching and unlatched polling
            // drivers. After sampling stops, expire each arrival separately.
            while self.pending.front().is_some_and(|sample| {
                self.host_clock - sample.received_at.max(self.last_sample_at) >= HOST_QUEUE_TIMEOUT
            }) {
                self.pending.pop_front();
            }
            // An inactive driver need not keep an old button snapshot.
            if self.host_clock - self.last_sample_at >= HOST_QUEUE_TIMEOUT {
                self.latched_buttons = None;
                self.unread_motion_nibbles = 0;
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
                let latched = self.control & 0x80 != 0;
                let count = if latched {
                    self.latch[axis]
                } else {
                    let mut count = self.counter[axis];
                    for sample in &self.pending {
                        count = count.wrapping_add(sample.motion[axis] as u8);
                    }
                    count
                };
                let buttons = self.read_buttons(latched);
                self.reported_buttons = buttons;
                let buttons = buttons.bits;
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
        if value & 0x80 != 0 && self.control & 0x80 == 0 {
            self.latch = self.counter;
            self.counter = [0; 2];
            // A previous latch may have consumed a sample before the guest
            // read its buttons. Deliver that edge before consuming a later one.
            let sample_buttons = self
                .unread_latch()
                .or_else(|| self.pending.front().map(|sample| sample.buttons));
            let buttons = sample_buttons.unwrap_or(self.live_buttons);
            // Live reads can report edges while older motion is still queued.
            self.latched_buttons = Some(if buttons.revision < self.reported_buttons.revision {
                self.reported_buttons
            } else {
                buttons
            });
            // Consecutive arrivals with the same buttons share a latch up to
            // the signed counter limit. Leave later button edges queued.
            while let Some(sample) = self
                .pending
                .front_mut()
                .filter(|sample| Some(sample.buttons) == sample_buttons)
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
            self.unread_motion_nibbles = if self.latch != [0; 2] {
                ALL_MOTION_NIBBLES
            } else {
                0
            };
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

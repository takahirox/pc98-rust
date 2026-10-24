use std::collections::VecDeque;
#[derive(Clone)]
pub struct Pic {
    pub mask: u8,
    pub irr: u8,
    pub isr: u8,
    pub base: u8,
    init: u8,
    icw4: bool,
    single: bool,
    read_isr: bool,
    special_mask: bool,
    // Outer None: no transaction; Some(None): frozen empty poll.
    poll: Option<Option<u8>>,
    poll_retrigger: bool,
    pub auto_eoi: bool,
    lowest_priority: u8,
    rotate_auto_eoi: bool,
}
impl Pic {
    pub fn new(base: u8) -> Self {
        Self {
            mask: 255,
            irr: 0,
            isr: 0,
            base,
            init: 0,
            icw4: false,
            single: false,
            read_isr: false,
            special_mask: false,
            poll: None,
            poll_retrigger: false,
            auto_eoi: false,
            // Deterministic emulator power-on/reset policy: IR0 highest.
            lowest_priority: 7,
            rotate_auto_eoi: false,
        }
    }
    pub fn request(&mut self, n: u8) {
        self.irr |= 1 << n;
        if self.poll == Some(Some(n)) {
            self.poll_retrigger = true;
        }
    }
    pub fn pending(&self) -> Option<u8> {
        let available = self.irr & !self.mask;
        let blocking = self.eligible_isr();
        for n in self.priority_order() {
            if blocking & (1 << n) != 0 {
                break;
            }
            if available & (1 << n) != 0 {
                return Some(n);
            }
        }
        None
    }
    // Special-mask mode excludes masked service levels from both arbitration
    // and nonspecific EOI selection. Specific EOI still addresses the raw ISR.
    fn eligible_isr(&self) -> u8 {
        self.isr & if self.special_mask { !self.mask } else { 255 }
    }
    fn priority_order(&self) -> impl Iterator<Item = u8> {
        let lowest = self.lowest_priority;
        (1..=8).map(move |offset| (lowest + offset) & 7)
    }
    pub fn ack(&mut self) -> Option<u8> {
        let n = self.pending()?;
        self.ack_level(n);
        Some(self.base + n)
    }
    fn ack_level(&mut self, n: u8) {
        self.irr &= !(1 << n);
        if !self.auto_eoi {
            self.isr |= 1 << n
        } else if self.rotate_auto_eoi {
            self.lowest_priority = n;
        }
    }
    pub(crate) fn single(&self) -> bool {
        self.single
    }
    pub(crate) fn poll_armed(&self) -> bool {
        self.poll.is_some()
    }
    pub fn read(&mut self, data: bool) -> u8 {
        // The next controller RD acknowledges, including an A0=1 read.
        // Poll overrides RR/RIS; unspecified result bits deterministically read 0.
        if let Some(selected) = self.poll.take() {
            return if let Some(n) = selected {
                self.ack_level(n);
                if self.poll_retrigger {
                    self.irr |= 1 << n;
                }
                self.poll_retrigger = false;
                0x80 | n
            } else {
                0
            };
        }
        if data {
            self.mask
        } else if self.read_isr {
            self.isr
        } else {
            self.irr
        }
    }
    pub fn write(&mut self, data: bool, v: u8) {
        if data {
            if self.init != 0 {
                match self.init {
                    1 => self.base = v & 0xf8,
                    2 => {}
                    3 => self.auto_eoi = v & 2 != 0,
                    _ => {}
                }
                // SNGL skips ICW3; IC4 alone determines whether ICW4 follows.
                self.init = if self.init == 1 && !self.single {
                    2
                } else if self.init <= 2 && self.icw4 {
                    3
                } else {
                    0
                };
            } else {
                self.mask = v
            }
        } else if v & 16 != 0 {
            self.init = 1;
            self.icw4 = v & 1 != 0;
            self.single = v & 2 != 0;
            self.isr = 0;
            self.irr = 0;
            self.mask = 0;
            self.lowest_priority = 7;
            self.rotate_auto_eoi = false;
            self.auto_eoi = false;
            self.read_isr = false;
            self.special_mask = false;
            self.poll = None;
            self.poll_retrigger = false;
        } else if v & 0x18 == 8 {
            if v & 0x40 != 0 {
                self.special_mask = v & 0x20 != 0;
            }
            if v & 2 != 0 {
                self.read_isr = v & 1 != 0
            }
            if v & 4 != 0 {
                self.poll = Some(self.pending());
                self.poll_retrigger = false;
            }
        } else {
            match v & 0xe0 {
                0x00 => self.rotate_auto_eoi = false,
                0x80 => self.rotate_auto_eoi = true,
                0x40 => {} // OCW2 no operation
                0xc0 => self.lowest_priority = v & 7,
                _ => {
                    let n = if v & 0x40 != 0 {
                        Some(v & 7)
                    } else {
                        self.priority_order()
                            .find(|n| self.eligible_isr() & (1 << n) != 0)
                    };
                    if let Some(n) = n {
                        self.isr &= !(1 << n);
                        if v & 0x80 != 0 {
                            self.lowest_priority = n;
                        }
                    }
                }
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counter {
    pub reload: u32,
    pub remaining: u32,
    mode: u8,
    access: u8,
    write_hi: bool,
    read_hi: bool,
    low: u8,
    latch: Option<u16>,
    load_pending: bool,
    mode3_phase: u32,
    mode3_reload: u32, // Active half-cycle divisor; reload may already differ.
    mode4_armed: bool,
    pub enabled: bool,
    pub output: bool,
}
impl Default for Counter {
    fn default() -> Self {
        Self {
            reload: 65536,
            remaining: 65536,
            mode: 3,
            access: 3,
            write_hi: false,
            read_hi: false,
            low: 0,
            latch: None,
            load_pending: false,
            mode3_phase: 0,
            mode3_reload: 65536,
            mode4_armed: false,
            enabled: false,
            output: false,
        }
    }
}
impl Counter {
    fn write(&mut self, v: u8) {
        if self.mode == 0 {
            // A count write starts a new mode-0 lifecycle immediately. The
            // first byte of a pair also stops the old count and cancels any
            // earlier complete value still waiting for its load clock.
            self.output = false;
            self.enabled = false;
            self.load_pending = false;
        }
        let value = match self.access {
            1 => Some(v as u16),
            2 => Some((v as u16) << 8),
            _ => {
                if !self.write_hi {
                    self.low = v;
                    self.write_hi = true;
                    None
                } else {
                    self.write_hi = false;
                    Some(self.low as u16 | ((v as u16) << 8))
                }
            }
        };
        if let Some(v) = value {
            self.reload = if v == 0 { 65536 } else { v as u32 };
            if matches!(self.mode, 2 | 3) {
                // The first count transfers on the next clock. Later complete
                // writes replace only the reload register, until period end
                // (mode 2) or the next half-cycle boundary (mode 3).
                if !self.enabled {
                    self.load_pending = true;
                }
            } else if matches!(self.mode, 0 | 4) {
                // Modes 0/4 transfer each replacement on a separate clock.
                self.load_pending = true;
            } else {
                self.remaining = self.reload;
            }
            self.enabled = true;
        }
    }
    fn read(&mut self) -> u8 {
        let v = self.latch.unwrap_or(self.remaining as u16);
        match self.access {
            2 => {
                self.latch = None;
                (v >> 8) as u8
            }
            1 => {
                self.latch = None;
                v as u8
            }
            _ => {
                let out = if self.read_hi {
                    (v >> 8) as u8
                } else {
                    v as u8
                };
                self.read_hi = !self.read_hi;
                if !self.read_hi {
                    self.latch = None
                }
                out
            }
        }
    }
    fn tick(&mut self, ticks: u32) -> bool {
        if !self.enabled {
            return false;
        }
        if self.mode == 0 {
            return self.tick_mode0(ticks);
        }
        if self.mode == 2 {
            return self.tick_mode2(ticks);
        }
        if self.mode == 3 {
            return self.tick_mode3(ticks);
        }
        if self.mode == 4 {
            return self.tick_mode4(ticks);
        }
        if ticks >= self.remaining {
            self.remaining = self.reload - (ticks - self.remaining) % self.reload;
            self.output = !self.output;
            true
        } else {
            self.remaining -= ticks;
            false
        }
    }
    fn tick_mode0(&mut self, mut ticks: u32) -> bool {
        if ticks == 0 {
            return false;
        }
        if self.load_pending {
            self.load_pending = false;
            self.remaining = self.reload;
            ticks -= 1; // Loading consumes a clock without decrementing.
        }
        // Only the first terminal count raises OUT. Subsequent 16-bit wraps
        // keep counting with OUT high and cannot request another interrupt.
        let rising = !self.output && ticks >= self.remaining;
        self.output |= rising;
        if ticks != 0 {
            self.remaining = self.remaining.wrapping_sub(ticks) & 0xffff;
        }
        rising
    }
    fn tick_mode2(&mut self, mut ticks: u32) -> bool {
        if ticks == 0 {
            return false;
        }
        if self.load_pending {
            self.load_pending = false;
            self.remaining = self.reload;
            ticks -= 1; // Loading does not also decrement the counter.
        }
        if ticks < self.remaining {
            self.remaining -= ticks;
            self.output = self.remaining != 1;
            return false;
        }
        // Count 1 is the single low clock; its successor reloads and rises.
        // Finish the old period first, then skip whole new periods in O(1).
        ticks -= self.remaining;
        self.remaining = self.reload - ticks % self.reload;
        self.output = self.remaining != 1;
        true // At least one rising edge, even if the final output is low.
    }
    fn tick_mode3(&mut self, mut ticks: u32) -> bool {
        if ticks == 0 {
            return false;
        }
        if self.load_pending {
            self.load_pending = false;
            self.mode3_phase = 0;
            self.mode3_reload = self.reload;
            ticks -= 1;
        }
        let mut rising = false;
        if self.mode3_reload != self.reload {
            // Finish the active half with its original divisor. A complete
            // replacement transfers at the toggle, without another load clock.
            let boundary = if self.output {
                self.mode3_reload.div_ceil(2)
            } else {
                self.mode3_reload
            };
            let until_boundary = boundary - self.mode3_phase;
            if ticks >= until_boundary {
                ticks -= until_boundary;
                rising = !self.output;
                self.mode3_reload = self.reload;
                self.mode3_phase = if self.output {
                    self.mode3_reload.div_ceil(2)
                } else {
                    0
                };
            }
        }
        let n = self.mode3_reload;
        // A full period ends with the low-to-high transition. Detect it before
        // reducing modulo N, so even hidden transitions reach the PIC latch.
        rising |= ticks >= n - self.mode3_phase;
        self.mode3_phase = (self.mode3_phase + ticks) % n;
        let high_clocks = n.div_ceil(2);
        self.output = self.mode3_phase < high_clocks;
        let elapsed = if self.output {
            self.mode3_phase
        } else {
            self.mode3_phase - high_clocks
        };
        self.remaining = if elapsed == 0 {
            n
        } else if n & 1 == 0 {
            n - 2 * elapsed
        } else {
            // Odd counts subtract one on the first high clock, three on the
            // first low clock, then two on every subsequent clock (Intel 8253).
            let adjusted = if self.output { n + 1 } else { n - 1 };
            adjusted - 2 * elapsed
        };
        rising
    }
    fn tick_mode4(&mut self, mut ticks: u32) -> bool {
        if ticks == 0 {
            return false;
        }
        let mut rising = false;
        if self.load_pending {
            self.load_pending = false;
            self.remaining = self.reload;
            self.mode4_armed = true;
            // A write alone leaves OUT alone. On the load clock an old low
            // strobe finishes, even when a replacement restarts the count.
            rising = !self.output;
            self.output = true;
            ticks -= 1;
        }
        if ticks != 0 && !self.output {
            self.output = true;
            rising = true;
        }
        if self.mode4_armed && ticks >= self.remaining {
            self.mode4_armed = false;
            self.output = ticks > self.remaining;
            // Preserve a rise hidden inside a batch crossing the low pulse.
            rising |= self.output;
        }
        if ticks != 0 {
            // Counting continues through wrap, without another strobe.
            self.remaining = self.remaining.wrapping_sub(ticks) & 0xffff;
        }
        rising
    }
    pub(crate) fn beep_high(&self) -> bool {
        if matches!(self.mode, 3 | 4) {
            self.output
        } else {
            self.remaining > self.reload / 2
        }
    }
}
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Pit {
    pub c: [Counter; 3],
    fraction: u64,
}
impl Pit {
    pub fn write(&mut self, channel: usize, v: u8) {
        if channel == 3 {
            let n = (v >> 6) as usize;
            if n < 3 {
                let c = &mut self.c[n];
                if v & 0x30 == 0 {
                    // Ignore duplicates until the programmed read completes,
                    // including the high byte of a half-consumed paired latch.
                    if c.latch.is_none() {
                        c.latch = Some(c.remaining as u16);
                        c.read_hi = false;
                    }
                } else {
                    c.access = (v >> 4) & 3;
                    c.mode = (v >> 1) & 7;
                    if c.mode > 5 {
                        c.mode -= 4
                    }
                    c.write_hi = false;
                    c.read_hi = false;
                    c.load_pending = false;
                    if matches!(c.mode, 0 | 2 | 3 | 4) {
                        c.enabled = false;
                        c.output = c.mode != 0;
                        c.latch = None;
                        c.mode3_phase = 0;
                        c.mode3_reload = 65536;
                        c.mode4_armed = false;
                    }
                }
            }
        } else {
            self.c[channel].write(v)
        }
    }
    pub fn read(&mut self, n: usize) -> u8 {
        self.c[n].read()
    }
    pub fn tick(&mut self, cycles: u32) -> bool {
        self.fraction += cycles as u64 * 1_996_800;
        let ticks = (self.fraction / 8_000_000) as u32;
        self.fraction %= 8_000_000;
        let irq = self.c[0].tick(ticks);
        self.c[1].tick(ticks);
        self.c[2].tick(ticks);
        irq
    }
    pub fn beep_hz(&self) -> u32 {
        1_996_800 / self.c[1].reload
    }
}
#[derive(Clone, Default)]
pub struct Dma {
    pub addr: [u16; 4],
    pub count: [u16; 4],
    pub(crate) base_addr: [u16; 4],
    pub(crate) base_count: [u16; 4],
    pub page: [u8; 4],
    pub mode: [u8; 4],
    pub mask: u8,
    pub status: u8,
    pub(crate) command: u8,
    pub(crate) high: bool,
}
impl Dma {
    pub fn read(&mut self, p: u16) -> u8 {
        if p < 0x10 {
            let i = ((p - 1) / 4) as usize;
            let v = if p & 2 == 0 {
                self.addr[i]
            } else {
                self.count[i]
            };
            let out = if self.high { (v >> 8) as u8 } else { v as u8 };
            self.high = !self.high;
            out
        } else {
            let v = self.status;
            self.status = 0;
            v
        }
    }
    pub fn write(&mut self, p: u16, v: u8) {
        match p {
            1..=15 => {
                let i = ((p - 1) / 4) as usize;
                let (reg, base) = if p & 2 == 0 {
                    (&mut self.addr[i], &mut self.base_addr[i])
                } else {
                    (&mut self.count[i], &mut self.base_count[i])
                };
                // Each byte updates both registers, preserving each one's other
                // byte even when the current register has progressed since setup.
                for word in [reg, base] {
                    *word = if self.high {
                        (*word & 255) | ((v as u16) << 8)
                    } else {
                        (*word & 0xff00) | v as u16
                    };
                }
                self.high = !self.high;
            }
            // Only bit 2 (controller disable) is modeled. Other command
            // controls remain unsupported; writes do not disturb programming.
            0x11 => self.command = v & 4,
            0x15 => {
                let bit = 1 << (v & 3);
                if v & 4 != 0 {
                    self.mask |= bit
                } else {
                    self.mask &= !bit
                }
            }
            0x17 => self.mode[(v & 3) as usize] = v,
            0x19 => self.high = false,
            0x1b => {
                // 8237 software reset retains current/base programming and modes.
                self.status = 0;
                self.command = 0;
                self.high = false;
                self.mask = 15;
                // Preserve the existing external PC-98 bank-reset policy. Its
                // board-level reset wiring is outside the modeled core contract.
                self.page = [0; 4]
            }
            0x1d => self.mask = 0,
            0x1f => self.mask = v & 15,
            0x21 | 0x23 | 0x25 | 0x27 => {
                self.page[match p {
                    0x21 => 1,
                    0x23 => 2,
                    0x25 => 3,
                    _ => 0,
                }] = v & 15
            }
            _ => {}
        }
    }
    pub fn address(&self, n: usize) -> usize {
        ((self.page[n] as usize) << 16) | self.addr[n] as usize
    }
    /// Consumers sample controller disable and channel masking at service time.
    pub fn can_service(&self, n: usize) -> bool {
        self.command & 4 == 0 && self.mask & (1 << n) == 0
    }
    /// Advance one serviced byte and report this transfer's terminal-count pulse.
    /// The pulse is independent of the sticky status and auto-init readback.
    pub fn advance(&mut self, n: usize) -> bool {
        self.addr[n] = if self.mode[n] & 0x20 != 0 {
            self.addr[n].wrapping_sub(1)
        } else {
            self.addr[n].wrapping_add(1)
        };
        self.count[n] = self.count[n].wrapping_sub(1);
        if self.count[n] == 65535 {
            self.status |= 1 << n;
            if self.mode[n] & 16 == 0 {
                self.mask |= 1 << n;
            } else {
                self.addr[n] = self.base_addr[n];
                self.count[n] = self.base_count[n];
            }
            return true;
        }
        false
    }
}
#[derive(Clone)]
pub enum KeyboardByte {
    Scan(u8),
    Reply(u8),
}
#[derive(Clone, Default)]
pub struct Keyboard {
    pub fifo: VecDeque<KeyboardByte>,
    pub bios: VecDeque<u16>,
    pub modifiers: u8,
    pub pressed: [u8; 16],
    command: u8,
}
impl Keyboard {
    pub fn inject(&mut self, scan: u8, down: bool) {
        if scan >= 128 {
            return;
        }
        let group = scan as usize / 8;
        let mask = 1 << (scan % 8);
        if down {
            self.pressed[group] |= mask
        } else {
            self.pressed[group] &= !mask
        }
        if self.fifo.len() < 64 {
            self.fifo
                .push_back(KeyboardByte::Scan(scan | if down { 0 } else { 0x80 }));
        }
    }
    pub(crate) fn scan_pending(&self) -> bool {
        matches!(self.fifo.front(), Some(KeyboardByte::Scan(_)))
    }
    pub fn receive(&mut self) -> u8 {
        match self.fifo.pop_front() {
            Some(KeyboardByte::Scan(byte) | KeyboardByte::Reply(byte)) => byte,
            None => 0,
        }
    }
    pub fn write(&mut self, v: u8) {
        if self.fifo.len() > 61 {
            return;
        }
        if self.command != 0 {
            self.command = 0;
            self.fifo.push_back(KeyboardByte::Reply(0xfa))
        } else if v == 0xff {
            // Reset the wire FIFO; physical holds and BIOS state are preserved.
            self.fifo.clear();
            self.fifo
                .extend([KeyboardByte::Reply(0xfa), KeyboardByte::Reply(0xa0)]);
        } else {
            self.command = v;
            self.fifo.push_back(KeyboardByte::Reply(0xfa))
        }
    }
    pub fn service(&mut self) -> Option<u16> {
        match self.fifo.pop_front() {
            Some(KeyboardByte::Scan(s)) => self.service_scan(s),
            _ => None,
        }
    }
    /// Translate a scan byte already consumed by the firmware's port read.
    pub(crate) fn service_scan(&mut self, s: u8) -> Option<u16> {
        let scan = s & 127;
        if [0x70, 0x74, 0x73].contains(&scan) {
            let bit = match scan {
                0x70 => 1,
                0x74 => 16,
                _ => 8,
            };
            if s & 128 == 0 {
                self.modifiers |= bit
            } else {
                self.modifiers &= !bit
            }
            return None;
        }
        if scan == 0x71 || scan == 0x72 {
            if s & 128 == 0 {
                self.modifiers ^= if scan == 0x71 { 2 } else { 4 }
            }
            return None;
        }
        if s & 128 != 0 {
            return None;
        }
        let normal = b"\x1b1234567890-^\\\x08\tqwertyuiop@[\r";
        let ch = match scan {
            0x00..=0x1c => normal.get(scan as usize).copied().unwrap_or(0),
            0x1d..=0x25 => b"asdfghjkl"[(scan - 0x1d) as usize],
            0x26 => b';',
            0x27 => b':',
            0x28 => b']',
            0x29..=0x2f => b"zxcvbnm"[(scan - 0x29) as usize],
            0x30 => b',',
            0x31 => b'.',
            0x32 => b'/',
            0x33 => b'\\',
            0x34 => b' ',
            0x40..=0x50 => b"-/789*456+123=0,."[(scan - 0x40) as usize],
            _ => 0,
        };
        let ch = if ch.is_ascii_alphabetic() {
            if (self.modifiers & 1 != 0) ^ (self.modifiers & 2 != 0) {
                ch.to_ascii_uppercase()
            } else {
                ch
            }
        } else if self.modifiers & 1 != 0 && !(0x40..=0x50).contains(&scan) {
            match ch {
                b'1'..=b'9' => b"!\"#$%&'()"[(ch - b'1') as usize],
                b'-' => b'=',
                b'^' => b'~',
                b'\\' if scan == 0x33 => b'_',
                b'\\' => b'|',
                b'@' => b'`',
                b'[' => b'{',
                b']' => b'}',
                b';' => b'+',
                b':' => b'*',
                b',' => b'<',
                b'.' => b'>',
                b'/' => b'?',
                _ => ch,
            }
        } else {
            ch
        };
        let ch = if self.modifiers & 16 != 0 && ch.is_ascii_alphabetic() {
            ch.to_ascii_uppercase() & 31
        } else {
            ch
        };
        let key = ((scan as u16) << 8) | ch as u16;
        if self.bios.len() < 16 {
            self.bios.push_back(key)
        }
        Some(key)
    }
}
#[derive(Clone, Default)]
pub struct Gdc {
    pub enabled: bool,
    /// Requested drawing timing: true permits drawing during active display.
    /// The fixed scheduler does not yet execute GDC drawing commands.
    pub draw_during_display: bool,
    pub command: u8,
    pub params: Vec<u8>,
    pub cursor: u32,
    pub pitch: u8,
    pub start: u32,
    pub cursor_visible: bool,
    pub cursor_top: u8,
    pub cursor_bottom: u8,
    pub fifo: VecDeque<u8>,
}
impl Gdc {
    pub fn command(&mut self, c: u8) {
        self.command = c;
        self.params.clear();
        match c {
            0x00 | 0x0c | 0x0e => self.enabled = false,
            0x0d | 0x6b => self.enabled = true,
            0xe0 if self.fifo.len() <= 11 => {
                self.fifo.extend([
                    self.cursor as u8,
                    (self.cursor >> 8) as u8,
                    (self.cursor >> 16) as u8,
                    0,
                    0,
                ]);
            }
            _ => {}
        }
    }
    pub fn data(&mut self, v: u8) {
        if self.params.len() < 16 {
            self.params.push(v)
        }
        let p = &self.params;
        match self.command {
            0x47 if p.len() == 1 => self.pitch = v,
            0x49 if p.len() >= 3 => {
                self.cursor = p[0] as u32 | ((p[1] as u32) << 8) | (((p[2] & 3) as u32) << 16)
            }
            0x4b if p.len() >= 3 => {
                self.cursor_visible = p[0] & 128 != 0;
                self.cursor_top = p[1] & 31;
                self.cursor_bottom = (p[2] >> 3) & 31
            }
            0x70..=0x7f if p.len() >= 4 => {
                self.start = p[0] as u32 | ((p[1] as u32) << 8) | (((p[2] & 3) as u32) << 16)
            }
            _ => {}
        }
    }
}
#[derive(Clone, Default)]
pub struct Fdc {
    pub command: Vec<u8>,
    pub result: VecDeque<u8>,
    pub cylinder: [u8; 2],
    pub sense: VecDeque<(u8, u8)>,
    pub control: u8,
    pub reads: u64,
    pub writes: u64,
    pub pending: bool,
    pub delay: u32,
    pub(crate) non_dma: bool,
    pub(crate) pio: Option<FdcPio>,
    pub(crate) irq: bool,
}
// One sector at a time; the bus resolves the next CHRN against mounted media.
#[derive(Clone)]
pub(crate) struct FdcPio {
    pub drive: u8,
    pub c: u8,
    pub h: u8,
    pub r: u8,
    pub n: u8,
    pub eot: u8,
    pub write: bool,
    pub offset: usize,
    pub len: usize,
    pub pos: usize,
    pub ready: bool,
    pub delay: u32,
}
impl Fdc {
    // Deterministic 500 kbit/s MFM approximation at the 8 MHz guest clock:
    // byte arrivals every 16 us, service required within 13 us of arrival.
    pub(crate) const BYTE_CLOCKS: u32 = 128;
    pub(crate) const SERVICE_CLOCKS: u32 = 104;
    pub fn seek_result(&mut self, status: u8, cylinder: u8) {
        self.sense.retain(|(s, _)| s & 3 != status & 3);
        self.sense.push_back((status, cylinder));
    }
    pub fn status(&self) -> u8 {
        if let Some(pio) = &self.pio {
            return (if pio.write { 0x30 } else { 0x70 }) | if pio.ready { 0x80 } else { 0 };
        }
        if self.pending {
            return if self.non_dma && matches!(self.command[0] & 31, 5 | 6) {
                if self.command[0] & 31 == 5 {
                    0x30
                } else {
                    0x70
                }
            } else {
                0x90
            };
        }
        0x80 | if !self.result.is_empty() {
            0x50
        } else if !self.command.is_empty() {
            0x10
        } else {
            0
        }
    }
    pub fn param_count(op: u8) -> usize {
        match op & 31 {
            3 => 3,
            4 | 7 => 2,
            8 => 1,
            15 => 3,
            5 | 6 | 9 | 12 => 9,
            10 => 2,
            13 => 6,
            _ => 1,
        }
    }
}

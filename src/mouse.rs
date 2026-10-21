//! Original PC-98 8255 bus mouse. UNDOCUMENTED 9801/9821, io_mouse.txt.
pub struct Mouse {
    counter: [u8; 2],
    latch: [u8; 2],
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
    /// Host supplies relative movement and DOM button bits; all consumption is
    /// through the guest PPI and optional periodic IRQ13, never application RAM.
    pub fn motion(&mut self, dx: i32, dy: i32, buttons: u8) {
        self.counter[0] = self.counter[0].wrapping_add(dx as u8);
        self.counter[1] = self.counter[1].wrapping_add(dy as u8);
        self.buttons = buttons & 7;
        self.events += 1;
    }
    pub fn release(&mut self) {
        self.buttons = 0;
    }
    pub fn tick(&mut self, cycles: u32) -> bool {
        self.fraction += cycles as u64 * (120 >> (self.rate & 3));
        let interrupt = self.fraction >= 8_000_000;
        self.fraction %= 8_000_000;
        interrupt && self.control & 0x10 == 0
    }
    pub fn input(&self, port: u16, ap: bool) -> u8 {
        match port {
            0x7fd9 => {
                let axis = usize::from(self.control & 0x40 != 0);
                let count = if self.control & 0x80 == 0 {
                    self.counter[axis]
                } else {
                    self.latch[axis]
                };
                let bits = (if self.buttons & 1 != 0 { 0x80 } else { 0 })
                    | (if self.buttons & 2 != 0 { 0x20 } else { 0 })
                    | (if self.buttons & 4 != 0 { 0x40 } else { 0 });
                (0xe0 & !bits) | ((count >> if self.control & 0x20 != 0 { 4 } else { 0 }) & 15)
            }
            0x7fdb => 0xff,
            0x7fdd => (self.control & 0xf0) | if ap { 8 } else { 0 },
            0xbfdb => self.rate,
            _ => 0xff,
        }
    }
    fn set_control(&mut self, value: u8) {
        if self.control & 0x80 == 0 && value & 0x80 != 0 {
            self.latch = self.counter;
            self.counter = [0; 2];
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

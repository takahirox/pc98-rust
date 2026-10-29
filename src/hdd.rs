//! Original PC-9821 IDE PIO subset. PC-98 ports 640–64E/74C, INT 11h.
//! One master on bank zero; HDI geometry is explicit rather than guessed.
pub const MAX_HDI_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct HardDisk {
    pub bytes: Vec<u8>,
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
    pub writes: u64,
    offset: usize,
}

impl HardDisk {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() < 32 || bytes.len() > MAX_HDI_BYTES {
            return Err("HDI must contain a 32-byte header and fit within 32 MiB".into());
        }
        let field = |p| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
        let (offset, size, sector, sectors, heads, cylinders) = (
            field(8),
            field(12),
            field(16),
            field(20),
            field(24),
            field(28),
        );
        if !(32..=65536).contains(&offset)
            || sector != 512
            || !(1..=63).contains(&sectors)
            || !(1..=16).contains(&heads)
            || !(1..=65535).contains(&cylinders)
            || cylinders as u64 * heads as u64 * sectors as u64 * sector as u64 != size as u64
            || offset.checked_add(size) != Some(bytes.len())
        {
            return Err(
                "invalid HDI geometry, size or offset (requires 512-byte IDE sectors)".into(),
            );
        }
        Ok(Self {
            bytes,
            offset,
            cylinders: cylinders as u16,
            heads: heads as u8,
            sectors: sectors as u8,
            writes: 0,
        })
    }
    pub fn sector_count(&self) -> usize {
        self.cylinders as usize * self.heads as usize * self.sectors as usize
    }
    pub fn chs(&self, cylinder: u16, head: u8, sector: u8) -> Option<usize> {
        if cylinder >= self.cylinders || head >= self.heads || sector >= self.sectors {
            return None;
        }
        Some(
            (cylinder as usize * self.heads as usize + head as usize) * self.sectors as usize
                + sector as usize,
        )
    }
    fn range(&self, lba: usize) -> std::ops::Range<usize> {
        let start = self.offset + lba * 512;
        start..start + 512
    }
}

pub struct Ide {
    pub disk: Option<HardDisk>,
    pub reads: u64,
    pub writes: u64,
    pub irq: bool,
    bank: u8,
    count: u8,
    sector: u8,
    cylinder: u16,
    head: u8,
    status: u8,
    error: u8,
    control: u8,
    buffer: [u8; 512],
    cursor: usize,
    remaining: u16,
    lba: usize,
    writing: bool,
    identify: bool,
}
impl Default for Ide {
    fn default() -> Self {
        Self {
            disk: None,
            reads: 0,
            writes: 0,
            irq: false,
            bank: 0,
            count: 1,
            sector: 1,
            cylinder: 0,
            head: 0xa0,
            status: 0x50,
            error: 0,
            control: 0,
            buffer: [0; 512],
            cursor: 0,
            remaining: 0,
            lba: 0,
            writing: false,
            identify: false,
        }
    }
}
impl Ide {
    /// Read-only task-file metadata; never reads data or acknowledges INTRQ.
    pub fn diagnostic(&self) -> String {
        format!(
            "reads={} writes={} status={:02x} error={:02x} count={} sector={:02x} cylinder={:04x} head={:02x} remaining={} lba={} cursor={} irq={} control={:02x} bank={}",
            self.reads, self.writes, self.status, self.error, self.count,
            self.sector, self.cylinder, self.head, self.remaining, self.lba,
            self.cursor, self.irq, self.control, self.bank
        )
    }
    pub fn mount(&mut self, disk: Option<HardDisk>) {
        *self = Self {
            disk,
            ..Self::default()
        };
    }
    pub fn irq_enabled(&self) -> bool {
        self.irq && self.control & 2 == 0
    }
    fn selected(&self) -> bool {
        self.bank == 0 && self.head & 16 == 0 && self.disk.is_some()
    }
    fn fail(&mut self, error: u8) {
        self.error = error;
        self.status = 0x51;
        self.remaining = 0;
        self.irq = true;
    }
    fn load(&mut self) {
        let d = self.disk.as_ref().unwrap();
        self.buffer.copy_from_slice(&d.bytes[d.range(self.lba)]);
        self.reads += 1;
        self.cursor = 0;
        self.status = 0x58;
        self.irq = true;
    }
    fn advance(&mut self) {
        self.remaining -= 1;
        self.count = self.remaining as u8;
        self.lba += 1;
        let d = self.disk.as_ref().unwrap();
        if self.head & 64 != 0 {
            self.sector = self.lba as u8;
            self.cylinder = (self.lba >> 8) as u16;
            self.head = (self.head & 0xf0) | ((self.lba >> 24) as u8 & 15);
        } else {
            self.sector = (self.lba % d.sectors as usize + 1) as u8;
            self.head =
                (self.head & 0xf0) | ((self.lba / d.sectors as usize) % d.heads as usize) as u8;
            self.cylinder = (self.lba / (d.sectors as usize * d.heads as usize)) as u16;
        }
        self.cursor = 0;
        if self.remaining == 0 {
            self.status = 0x50;
        } else if !self.writing {
            self.load();
        }
    }
    pub fn input(&mut self, port: u16) -> u8 {
        if port == 0x430 || port == 0x432 {
            return self.bank;
        }
        if !self.selected() {
            return 0xff;
        }
        match port {
            // Cpu's width adapter accesses the two bytes of the 16-bit data port.
            0x640 | 0x641 => {
                if self.status & 8 == 0 || self.writing {
                    return 0xff;
                }
                let v = self.buffer[self.cursor];
                self.cursor += 1;
                if self.cursor == 512 {
                    if self.identify {
                        self.status = 0x50;
                    } else {
                        self.advance();
                    }
                }
                v
            }
            0x642 => self.error,
            0x644 => self.count,
            0x646 => self.sector,
            0x648 => self.cylinder as u8,
            0x64a => (self.cylinder >> 8) as u8,
            0x64c => self.head,
            0x64e => {
                self.irq = false;
                self.status
            }
            0x74c => self.status,
            0x74e => 0x80 | ((!self.head & 15) << 2),
            _ => 0xff,
        }
    }
    pub fn output(&mut self, port: u16, value: u8) {
        match port {
            0x430 | 0x432 => {
                if value != 0x80 {
                    self.bank = value;
                }
            }
            0x644 => self.count = value,
            0x646 => self.sector = value,
            0x648 => self.cylinder = (self.cylinder & 0xff00) | value as u16,
            0x64a => self.cylinder = (self.cylinder & 255) | ((value as u16) << 8),
            0x64c => self.head = value,
            0x74c => {
                let old = self.control;
                self.control = value;
                if value & 4 != 0 {
                    self.status = 0x80;
                    self.remaining = 0;
                    self.irq = false;
                } else if old & 4 != 0 {
                    self.status = 0x50;
                    self.error = 1;
                    self.count = 1;
                    self.sector = 1;
                    self.cylinder = 0;
                    self.head = 0xa0;
                }
            }
            0x640 | 0x641 if self.selected() && self.writing && self.status & 8 != 0 => {
                self.buffer[self.cursor] = value;
                self.cursor += 1;
                if self.cursor == 512 {
                    let d = self.disk.as_mut().unwrap();
                    let range = d.range(self.lba);
                    d.bytes[range].copy_from_slice(&self.buffer);
                    d.writes += 1;
                    self.writes += 1;
                    self.advance();
                    self.irq = true;
                }
            }
            0x64e => self.command(value),
            _ => {}
        }
    }
    fn command(&mut self, command: u8) {
        self.error = 0;
        self.irq = false;
        self.remaining = 0;
        self.cursor = 0;
        self.identify = false;
        if !self.selected() || self.control & 4 != 0 {
            self.fail(4);
            return;
        }
        let d = self.disk.as_ref().unwrap();
        if command == 0xec {
            self.buffer.fill(0);
            for (word, value) in [
                (0, 0x0040),
                (1, d.cylinders),
                (3, d.heads as u16),
                (6, d.sectors as u16),
                (49, 0x0200),
                (60, d.sector_count() as u16),
                (61, (d.sector_count() >> 16) as u16),
            ] {
                self.buffer[word * 2..word * 2 + 2].copy_from_slice(&value.to_le_bytes());
            }
            self.writing = false;
            self.identify = true;
            self.status = 0x58;
            self.irq = true;
            return;
        }
        if matches!(command, 0x10..=0x1f | 0x70..=0x7f | 0x90 | 0xe7) {
            self.status = 0x50;
            self.irq = true;
            return;
        }
        if !matches!(command, 0x20 | 0x21 | 0x30 | 0x31 | 0x40 | 0x41) {
            self.fail(4);
            return;
        }
        let lba = if self.head & 64 != 0 {
            Some(
                self.sector as usize
                    | ((self.cylinder as usize) << 8)
                    | ((self.head as usize & 15) << 24),
            )
        } else {
            self.sector
                .checked_sub(1)
                .and_then(|s| d.chs(self.cylinder, self.head & 15, s))
        };
        let count = if self.count == 0 {
            256
        } else {
            self.count as usize
        };
        if command & 0xf0 == 0x40 {
            // Selected legacy READ VERIFY contract: retain the last successful
            // address, or the first failing address with its inclusive residual.
            // No PIO load/advance: this command never transfers data or sets DRQ.
            let Some(lba) = lba else {
                self.fail(0x10);
                return;
            };
            let verified = count.min(d.sector_count().saturating_sub(lba));
            self.count = (count - verified) as u8;
            if verified != 0 {
                let address = lba + verified - usize::from(verified == count);
                if self.head & 64 != 0 {
                    self.sector = address as u8;
                    self.cylinder = (address >> 8) as u16;
                    self.head = (self.head & 0xf0) | ((address >> 24) as u8 & 15);
                } else {
                    self.sector = (address % d.sectors as usize + 1) as u8;
                    self.head = (self.head & 0xf0)
                        | ((address / d.sectors as usize) % d.heads as usize) as u8;
                    self.cylinder = (address / (d.sectors as usize * d.heads as usize)) as u16;
                }
            }
            if verified != count {
                self.fail(0x10);
            } else {
                self.status = 0x50;
                self.irq = true;
            }
            return;
        }
        let Some(lba) = lba.filter(|&l| l + count <= d.sector_count()) else {
            self.fail(0x10);
            return;
        };
        self.lba = lba;
        self.remaining = count as u16;
        self.writing = command & 0xf0 == 0x30;
        if self.writing {
            self.status = 0x58;
        } else {
            self.load();
        }
    }
}

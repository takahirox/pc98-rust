use crate::{cpu::Cpu, devices::*, disk::Disk};
pub struct Bus {
    pub mem: Vec<u8>,
    pub graphics: Vec<u8>,
    pub pics: [Pic; 2],
    pub pit: Pit,
    pub dma: Dma,
    pub keyboard: Keyboard,
    pub gdcs: [Gdc; 2],
    pub fdc: Fdc,
    pub disks: [Option<Disk>; 2],
    pub cycles: u64,
    pub beep: bool,
    pub beep_edges: u64,
    pub audio: Vec<f32>,
    audio_fraction: u64,
    pub system_c: u8,
    pub display_page: u8,
    pub cpu_page: u8,
    pub palette: [[u8; 3]; 8],
    pub grcg_mode: u8,
    pub analog: bool,
    palette_index: usize,
    tiles: [u8; 4],
    tile_index: usize,
    pub text_enabled: bool,
    pub unknown_ports: std::collections::BTreeSet<u16>,
    pub firmware_request: bool,
}
impl Default for Bus {
    fn default() -> Self {
        let mut palette = [[0; 3]; 8];
        for (i, p) in palette.iter_mut().enumerate() {
            *p = [
                if i & 2 != 0 { 255 } else { 0 },
                if i & 4 != 0 { 255 } else { 0 },
                if i & 1 != 0 { 255 } else { 0 },
            ];
        }
        Self {
            mem: vec![0; 1 << 20],
            graphics: vec![0; 192 * 1024],
            pics: [Pic::new(8), Pic::new(0x10)],
            pit: Pit::default(),
            dma: Dma {
                mask: 15,
                ..Dma::default()
            },
            keyboard: Keyboard::default(),
            gdcs: [Gdc::default(), Gdc::default()],
            fdc: Fdc::default(),
            disks: [None, None],
            cycles: 0,
            beep: false,
            beep_edges: 0,
            audio: Vec::with_capacity(4096),
            audio_fraction: 0,
            system_c: 8,
            display_page: 0,
            cpu_page: 0,
            palette,
            grcg_mode: 0,
            analog: false,
            palette_index: 0,
            tiles: [0; 4],
            tile_index: 0,
            text_enabled: true,
            unknown_ports: Default::default(),
            firmware_request: false,
        }
    }
}
impl Bus {
    pub fn read(&self, a: usize) -> u8 {
        let a = a & 0xfffff;
        match a {
            0..=0x9ffff | 0xa0000..=0xa2fff | 0xf0000..=0xfffff => self.mem[a],
            0xa8000..=0xbffff => {
                let off = a - 0xa8000;
                self.graphics[self.cpu_page as usize * 0x18000 + off]
            }
            _ => 255,
        }
    }
    pub fn read16(&self, a: usize) -> u16 {
        self.read(a) as u16 | ((self.read(a + 1) as u16) << 8)
    }
    pub fn write(&mut self, a: usize, v: u8) {
        let a = a & 0xfffff;
        match a {
            0..=0xa2fff => self.mem[a] = v,
            0xa8000..=0xbffff => {
                let off = a - 0xa8000;
                let base = self.cpu_page as usize * 0x18000;
                if self.grcg_mode & 128 == 0 {
                    self.graphics[base + off] = v
                } else {
                    let byte = off & 0x7fff;
                    for plane in 0..3 {
                        if self.grcg_mode & (1 << plane) == 0 {
                            let p = base + plane * 0x8000 + byte;
                            self.graphics[p] = if self.grcg_mode & 64 != 0 {
                                (self.graphics[p] & !v) | (self.tiles[plane] & v)
                            } else {
                                self.tiles[plane]
                            };
                        }
                    }
                }
            }
            _ => {}
        }
    }
    pub fn read_seg(&self, s: u16, o: u16, w: bool) -> u16 {
        self.read(Cpu::phys(s, o)) as u16
            | if w {
                (self.read(Cpu::phys(s, o.wrapping_add(1))) as u16) << 8
            } else {
                0
            }
    }
    pub fn write_seg(&mut self, s: u16, o: u16, v: u16, w: bool) {
        self.write(Cpu::phys(s, o), v as u8);
        if w {
            self.write(Cpu::phys(s, o.wrapping_add(1)), (v >> 8) as u8)
        }
    }
    pub fn tick(&mut self, cycles: u32) {
        self.cycles += cycles as u64;
        if self.pit.tick(cycles) {
            self.pics[0].request(0)
        }
        // Guest-clocked PCM, including short pulses between browser frames.
        self.audio_fraction += cycles as u64 * 48_000;
        while self.audio_fraction >= 8_000_000 {
            self.audio_fraction -= 8_000_000;
            if self.audio.len() < 4096 {
                let counter = &self.pit.c[1];
                self.audio.push(if !self.beep || !counter.enabled {
                    0.0
                } else if counter.remaining > counter.reload / 2 {
                    0.15
                } else {
                    -0.15
                });
            }
        }
        if !self.keyboard.fifo.is_empty() {
            self.pics[0].request(1)
        }
        if self.fdc.pending {
            if self.fdc.delay > cycles {
                self.fdc.delay -= cycles
            } else {
                self.fdc.pending = false;
                let irq = matches!(self.fdc.command[0] & 31, 5 | 6 | 7 | 15 | 10);
                self.finish_fdc();
                if irq {
                    self.pics[1].request(3);
                }
            }
        }
    }
    pub fn irq(&mut self) -> Option<u8> {
        if self.pics[1].pending().is_some() {
            self.pics[0].request(7)
        }
        let n = self.pics[0].ack()?;
        if n == self.pics[0].base + 7 {
            self.pics[1].ack()
        } else {
            Some(n)
        }
    }
    pub fn key(&mut self, scan: u8, down: bool) {
        self.keyboard.inject(scan, down);
        self.pics[0].request(1)
    }
    pub fn input(&mut self, p: u16) -> u8 {
        match p {
            0 | 2 => self.pics[0].read(p == 2),
            8 | 10 => self.pics[1].read(p == 10),
            1..=0x11 if p & 1 != 0 => self.dma.read(p),
            0x31 => 0x7b,
            0x33 => 0x38,
            0x35 => self.system_c,
            0x41 => self.keyboard.receive(),
            0x43 => {
                if self.keyboard.fifo.is_empty() {
                    5
                } else {
                    7
                }
            }
            0x60 | 0xa0 => {
                0x04 | if self.cycles % 133_333 > 125_000 {
                    0x20
                } else {
                    0
                } | if !self.gdcs[usize::from(p == 0xa0)].fifo.is_empty() {
                    1
                } else {
                    0
                }
            }
            0x62 | 0xa2 => self.gdcs[usize::from(p == 0xa2)]
                .fifo
                .pop_front()
                .unwrap_or(0),
            0x71 | 0x73 | 0x75 => self.pit.read(((p - 0x71) / 2) as usize),
            0x3fd9 | 0x3fdb | 0x3fdd => self.pit.read(((p - 0x3fd9) / 2) as usize),
            0x90 => self.fdc.status(),
            0x92 => self.fdc.result.pop_front().unwrap_or(0x80),
            0x94 => 0x40,
            0xbe => 1,
            0xa8 | 0xaa | 0xac | 0xae => {
                if self.analog {
                    if p == 0xa8 {
                        self.palette_index as u8
                    } else {
                        self.palette[self.palette_index][match p {
                            0xaa => 1,
                            0xac => 0,
                            _ => 2,
                        }] / 17
                    }
                } else {
                    let base = 3 - ((p - 0xa8) / 2) as usize;
                    let bits = |rgb: [u8; 3]| {
                        u8::from(rgb[0] != 0)
                            | (u8::from(rgb[1] != 0) << 1)
                            | (u8::from(rgb[2] != 0) << 2)
                    };
                    (bits(self.palette[base]) << 4) | bits(self.palette[base + 4])
                }
            }
            0xa4 => self.display_page,
            0xa6 => self.cpu_page,
            0x5c | 0x5e => (self.cycles / 8) as u8,
            0x40 => 0x20,
            0x64 => 0,
            0x6e => 0,
            0x188 => 255,
            _ => {
                self.unknown_ports.insert(p);
                255
            }
        }
    }
    pub fn output(&mut self, p: u16, v: u8) {
        match p {
            0 | 2 => self.pics[0].write(p == 2, v),
            8 | 10 => self.pics[1].write(p == 10, v),
            1..=0x29 if p & 1 != 0 => self.dma.write(p, v),
            0x35 => {
                self.system_c = v;
                self.set_beep(v & 8 == 0)
            }
            0x37 => {
                if v & 128 == 0 {
                    let bit = 1 << ((v >> 1) & 7);
                    if v & 1 != 0 {
                        self.system_c |= bit
                    } else {
                        self.system_c &= !bit
                    }
                    self.set_beep(self.system_c & 8 == 0)
                }
            }
            0x41 => self.keyboard.write(v),
            0x43 => {}
            0x60 | 0xa0 => self.gdcs[usize::from(p == 0xa0)].data(v),
            0x62 | 0xa2 => self.gdcs[usize::from(p == 0xa2)].command(v),
            0x68 => {
                if v & 0xfe == 0x0e {
                    self.text_enabled = v & 1 != 0
                }
            }
            0x6a => {
                if v <= 1 {
                    self.analog = v == 1
                } else {
                    self.unknown_ports.insert(p);
                }
            }
            0x6c => {}
            0x7c => {
                self.grcg_mode = v;
                self.tile_index = 0;
            }
            0x7e => {
                self.tiles[self.tile_index] = v;
                self.tile_index = (self.tile_index + 1) & 3;
            }
            0x71 | 0x73 | 0x75 | 0x77 => self.pit.write(((p - 0x71) / 2) as usize, v),
            0x3fd9 | 0x3fdb | 0x3fdd | 0x3fdf => self.pit.write(((p - 0x3fd9) / 2) as usize, v),
            0xdf => self.pit.write(3, v), // OUT imm8 truncation in the independent hello fixture.
            0x92 => {
                if !self.fdc.pending && self.fdc.result.is_empty() {
                    self.fdc.command.push(v);
                    let count = Fdc::param_count(self.fdc.command[0]);
                    if self.fdc.command.len() == count {
                        self.fdc.pending = true;
                        self.fdc.delay = 160;
                    }
                }
            }
            0x94 => {
                let old = self.fdc.control;
                self.fdc.control = v;
                if v & 128 != 0 && old & 128 == 0 {
                    self.fdc = Fdc {
                        control: v,
                        ..Default::default()
                    };
                    self.fdc.sense.extend([(0xc0, 0), (0xc1, 0)]);
                    self.pics[1].request(3);
                }
            }
            0xa4 => self.display_page = v & 1,
            0xa6 => self.cpu_page = v & 1,
            0xa8 | 0xaa | 0xac | 0xae => {
                if self.analog {
                    if p == 0xa8 {
                        self.palette_index = (v & 7) as usize
                    } else {
                        let channel = match p {
                            0xaa => 1,
                            0xac => 0,
                            _ => 2,
                        };
                        self.palette[self.palette_index][channel] = (v & 15) * 17;
                    }
                    return;
                }
                let base = 3 - ((p - 0xa8) / 2) as usize;
                for (i, color) in [v >> 4, v & 15].into_iter().enumerate() {
                    let idx = base + i * 4;
                    self.palette[idx] = [
                        if color & 1 != 0 { 255 } else { 0 },
                        if color & 2 != 0 { 255 } else { 0 },
                        if color & 4 != 0 { 255 } else { 0 },
                    ]
                }
            }
            0x40 | 0x50 | 0x52 | 0x64 | 0xbe | 0x188 | 0x18a => {}
            0xf0 => self.firmware_request = true,
            _ => {
                self.unknown_ports.insert(p);
            }
        }
    }
    pub fn set_beep(&mut self, on: bool) {
        if on != self.beep {
            self.beep_edges += 1
        }
        self.beep = on;
    }
    fn finish_fdc(&mut self) {
        let p = std::mem::take(&mut self.fdc.command);
        let op = p[0] & 31;
        let drive = p.get(1).copied().unwrap_or(0) & 3;
        let d = drive as usize;
        match op {
            3 => {}
            4 => self
                .fdc
                .result
                .push_back(if d < 2 && self.disks[d].is_some() {
                    0x28 | drive
                        | if self.fdc.cylinder[d] == 0 { 16 } else { 0 }
                        | if self.disks[d].as_ref().unwrap().protected {
                            64
                        } else {
                            0
                        }
                } else {
                    drive
                }),
            7 | 15 => {
                if d < 2 {
                    self.fdc.cylinder[d] = if op == 7 { 0 } else { p[2] };
                    self.fdc.seek_result(0x20 | drive, self.fdc.cylinder[d]);
                }
            }
            8 => {
                let (st, c) = self.fdc.sense.pop_front().unwrap_or((0x80, 0));
                self.fdc.result.extend([st, c]);
            }
            5 | 6 => {
                let (c, h, mut r, n, eot) = (p[2], p[3], p[4], p[5], p[6]);
                let mut st1 = 0;
                let mut st0 = drive | (h << 2);
                let write = op == 5;
                if d >= 2 || self.disks[d].is_none() || self.fdc.control & 8 == 0 {
                    st0 |= 0x48;
                    st1 = 4;
                } else if self.fdc.control & 16 == 0 {
                    st0 |= 0x40;
                    st1 = 0x10;
                } else if write && self.disks[d].as_ref().unwrap().protected {
                    st0 |= 0x40;
                    st1 = 2;
                } else if self.fdc.cylinder[d] != c {
                    st0 |= 64;
                    st1 = 4;
                } else if self.dma.mask & 4 != 0 {
                    st0 |= 0x40;
                    st1 = 0x10;
                } else {
                    loop {
                        let sector = self.disks[d].as_ref().unwrap().sector(c, h, r, n).cloned();
                        if let Some(s) = sector {
                            let transfer = self.dma.count[2] as usize + 1;
                            let count = s.len.min(transfer);
                            if self.dma.mode[2] & 12 != if write { 8 } else { 4 } {
                                st0 |= 64;
                                st1 = 16;
                                break;
                            }
                            for i in 0..count {
                                let addr = self.dma.address(2);
                                if write {
                                    let v = self.read(addr);
                                    self.disks[d].as_mut().unwrap().bytes[s.offset + i] = v;
                                } else {
                                    let v = self.disks[d].as_ref().unwrap().bytes[s.offset + i];
                                    self.write(addr, v)
                                }
                                self.dma.advance(2);
                            }
                            if write {
                                self.disks[d].as_mut().unwrap().writes += 1;
                                self.fdc.writes += 1
                            } else {
                                self.fdc.reads += 1
                            }
                            if self.dma.count[2] == 65535 {
                                break;
                            }
                            if r >= eot {
                                st0 |= 64;
                                st1 = 0x80;
                                break;
                            }
                            r += 1;
                        } else {
                            st0 |= 64;
                            st1 = 4;
                            break;
                        }
                    }
                }
                self.fdc.result.extend([st0, st1, 0, c, h, r, n]);
            }
            10 => {
                let h = (p[1] >> 2) & 1;
                let c = self.fdc.cylinder[d.min(1)];
                if let Some(s) = self
                    .disks
                    .get(d)
                    .and_then(|v| v.as_ref())
                    .and_then(|v| v.sectors.iter().find(|s| s.c == c && s.h == h))
                {
                    self.fdc
                        .result
                        .extend([drive | (h << 2), 0, 0, c, h, s.r, s.n])
                } else {
                    self.fdc.result.extend([0x48 | drive, 4, 0, c, h, 1, 3])
                }
            }
            _ => self.fdc.result.push_back(0x80),
        }
    }
    pub fn frame(&self, out: &mut [u8]) {
        let graphics = self.gdcs[1].enabled;
        let text = self.text_enabled && self.gdcs[0].enabled;
        let page = self.display_page as usize * 0x18000;
        for y in 0..400 {
            for x in 0..640 {
                let mut color = 0;
                if graphics {
                    let a = ((self.gdcs[1].start as usize * 2 + y * 80 + x / 8) & 0x7fff) + page;
                    let mask = 0x80 >> (x & 7);
                    for plane in 0..3 {
                        if self.graphics[a + plane * 0x8000] & mask != 0 {
                            color |= 1 << plane
                        }
                    }
                }
                if text {
                    let cell = (((y / 16) * 80 + x / 8) + self.gdcs[0].start as usize) % 2048;
                    let ch = self.mem[0xa0000 + cell * 2];
                    let attr = self.mem[0xa2000 + cell * 2];
                    let glyph = crate::font::row(ch, (y % 16) as u8);
                    let mut ink = glyph & (128 >> (x % 8)) != 0;
                    if attr & 4 != 0 {
                        ink = !ink
                    }
                    if attr & 8 != 0 && y % 16 == 15 {
                        ink = true
                    }
                    if self.gdcs[0].cursor_visible
                        && cell == self.gdcs[0].cursor as usize
                        && (self.gdcs[0].cursor_top..=self.gdcs[0].cursor_bottom)
                            .contains(&((y % 16) as u8))
                    {
                        ink = !ink
                    }
                    if attr & 1 != 0 && ink {
                        color = (attr >> 5) & 7;
                    }
                }
                let idx = (y * 640 + x) * 4;
                out[idx..idx + 3].copy_from_slice(&self.palette[color as usize]);
                out[idx + 3] = 255;
            }
        }
    }
    pub fn text(&self) -> String {
        (0..25)
            .map(|y| {
                let s: String = (0..80)
                    .map(|x| {
                        let v = self.mem[0xa0000 + (y * 80 + x) * 2];
                        if (32..127).contains(&v) {
                            v as char
                        } else {
                            ' '
                        }
                    })
                    .collect();
                s.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

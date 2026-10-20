//! Original 16-bit interpreter. Instructions never delegate to a host emulator.
use crate::bus::Bus;
pub const CF: u16 = 1;
pub const PF: u16 = 4;
pub const AF: u16 = 16;
pub const ZF: u16 = 64;
pub const SF: u16 = 128;
pub const TF: u16 = 256;
pub const IF: u16 = 512;
pub const DF: u16 = 1024;
pub const OF: u16 = 2048;
pub const AX: usize = 0;
pub const CX: usize = 1;
pub const DX: usize = 2;
pub const BX: usize = 3;
pub const SP: usize = 4;
pub const BP: usize = 5;
pub const SI: usize = 6;
pub const DI: usize = 7;
pub const ES: usize = 0;
pub const CS: usize = 1;
pub const SS: usize = 2;
pub const DS: usize = 3;
#[derive(Clone, Debug)]
pub struct Cpu {
    pub r: [u16; 8],
    pub s: [u16; 4],
    pub ip: u16,
    pub flags: u16,
    pub halted: bool,
    pub retired: u64,
    pub inhibit: u8,
}
#[derive(Clone, Copy)]
enum Operand {
    Reg(usize),
    Mem(u16, u16),
}
impl Default for Cpu {
    fn default() -> Self {
        Self {
            r: [0; 8],
            s: [0, 0xffff, 0, 0],
            ip: 0,
            flags: 0xf002,
            halted: false,
            retired: 0,
            inhibit: 0,
        }
    }
}
impl Cpu {
    pub fn phys(seg: u16, off: u16) -> usize {
        ((seg as usize) * 16 + off as usize) & 0xfffff
    }
    pub fn byte(&self, r: usize) -> u8 {
        if r < 4 {
            self.r[r] as u8
        } else {
            (self.r[r - 4] >> 8) as u8
        }
    }
    pub fn set_byte(&mut self, r: usize, v: u8) {
        let i = r & 3;
        self.r[i] = if r < 4 {
            (self.r[i] & 0xff00) | v as u16
        } else {
            (self.r[i] & 255) | ((v as u16) << 8)
        };
    }
    fn fetch(&mut self, b: &Bus) -> u8 {
        let v = b.read(Self::phys(self.s[CS], self.ip));
        self.ip = self.ip.wrapping_add(1);
        v
    }
    fn word(&mut self, b: &Bus) -> u16 {
        let l = self.fetch(b);
        l as u16 | ((self.fetch(b) as u16) << 8)
    }
    pub fn push(&mut self, b: &mut Bus, v: u16) {
        self.r[SP] = self.r[SP].wrapping_sub(2);
        b.write_seg(self.s[SS], self.r[SP], v, true);
    }
    pub fn pop(&mut self, b: &Bus) -> u16 {
        let v = b.read_seg(self.s[SS], self.r[SP], true);
        self.r[SP] = self.r[SP].wrapping_add(2);
        v
    }
    pub fn interrupt(&mut self, b: &mut Bus, n: u8) {
        self.push(b, self.flags);
        self.push(b, self.s[CS]);
        self.push(b, self.ip);
        self.flags &= !(IF | TF);
        self.ip = b.read16(n as usize * 4);
        self.s[CS] = b.read16(n as usize * 4 + 2);
        self.halted = false;
    }
    fn flag(&mut self, f: u16, on: bool) {
        if on {
            self.flags |= f
        } else {
            self.flags &= !f
        }
    }
    fn szp(&mut self, v: u16, w: bool) {
        let v = v & if w { 65535 } else { 255 };
        self.flag(ZF, v == 0);
        self.flag(SF, v & if w { 0x8000 } else { 0x80 } != 0);
        self.flag(PF, (v as u8).count_ones() & 1 == 0);
    }
    fn alu(&mut self, op: u8, a: u16, c: u16, w: bool) -> u16 {
        let mask = if w { 65535u32 } else { 255 };
        let sign = (mask + 1) >> 1;
        let a = a as u32 & mask;
        let c = c as u32 & mask;
        let carry = u32::from(self.flags & CF != 0);
        let (raw, sub) = match op {
            0 => (a + c, false),
            1 => (a | c, false),
            2 => (a + c + carry, false),
            3 => (a.wrapping_sub(c + carry), true),
            4 => (a & c, false),
            5 | 7 => (a.wrapping_sub(c), true),
            6 => (a ^ c, false),
            _ => unreachable!(),
        };
        let v = raw & mask;
        if matches!(op, 0 | 2 | 3 | 5 | 7) {
            self.flag(
                CF,
                if sub {
                    a < c + if op == 3 { carry } else { 0 }
                } else {
                    raw > mask
                },
            );
            self.flag(AF, (a ^ c ^ v) & 16 != 0);
            self.flag(
                OF,
                if sub {
                    (a ^ c) & (a ^ v) & sign != 0
                } else {
                    (!(a ^ c)) & (a ^ v) & sign != 0
                },
            );
        } else {
            self.flags &= !(CF | OF | AF);
        }
        self.szp(v as u16, w);
        v as u16
    }
    fn decode(&mut self, b: &Bus, m: u8, ov: Option<usize>) -> Operand {
        let mode = m >> 6;
        let rm = (m & 7) as usize;
        if mode == 3 {
            return Operand::Reg(rm);
        }
        let (base, stack) = match rm {
            0 => (self.r[BX].wrapping_add(self.r[SI]), false),
            1 => (self.r[BX].wrapping_add(self.r[DI]), false),
            2 => (self.r[BP].wrapping_add(self.r[SI]), true),
            3 => (self.r[BP].wrapping_add(self.r[DI]), true),
            4 => (self.r[SI], false),
            5 => (self.r[DI], false),
            6 if mode == 0 => (self.word(b), false),
            6 => (self.r[BP], true),
            _ => (self.r[BX], false),
        };
        let disp = match mode {
            1 => self.fetch(b) as i8 as u16,
            2 => self.word(b),
            _ => 0,
        };
        Operand::Mem(
            self.s[ov.unwrap_or(if stack { SS } else { DS })],
            base.wrapping_add(disp),
        )
    }
    fn get(&self, b: &Bus, o: Operand, w: bool) -> u16 {
        match o {
            Operand::Reg(r) => {
                if w {
                    self.r[r]
                } else {
                    self.byte(r) as u16
                }
            }
            Operand::Mem(s, a) => b.read_seg(s, a, w),
        }
    }
    fn put(&mut self, b: &mut Bus, o: Operand, v: u16, w: bool) {
        match o {
            Operand::Reg(r) => {
                if w {
                    self.r[r] = v
                } else {
                    self.set_byte(r, v as u8)
                }
            }
            Operand::Mem(s, a) => b.write_seg(s, a, v, w),
        }
    }
    fn cond(&self, c: u8) -> bool {
        let f = |x| self.flags & x != 0;
        match c {
            0 => f(OF),
            1 => !f(OF),
            2 => f(CF),
            3 => !f(CF),
            4 => f(ZF),
            5 => !f(ZF),
            6 => f(CF) || f(ZF),
            7 => !f(CF) && !f(ZF),
            8 => f(SF),
            9 => !f(SF),
            10 => f(PF),
            11 => !f(PF),
            12 => f(SF) != f(OF),
            13 => f(SF) == f(OF),
            14 => f(ZF) || f(SF) != f(OF),
            _ => !f(ZF) && f(SF) == f(OF),
        }
    }
    fn shift(&mut self, op: u8, mut v: u16, n: u8, w: bool) -> u16 {
        let mask = if w { 65535 } else { 255 };
        let sign = if w { 0x8000 } else { 0x80 };
        let old = v;
        for _ in 0..n {
            let c = self.flags & CF != 0;
            let next = match op {
                0 | 2 | 4 | 6 => v & sign != 0,
                _ => v & 1 != 0,
            };
            v = match op {
                0 => (v << 1) | u16::from(next),
                1 => (v >> 1) | if next { sign } else { 0 },
                2 => (v << 1) | u16::from(c),
                3 => (v >> 1) | if c { sign } else { 0 },
                4 | 6 => v << 1,
                5 => v >> 1,
                7 => (v >> 1) | (v & sign),
                _ => unreachable!(),
            } & mask;
            self.flag(CF, next);
        }
        if n > 0 {
            if op >= 4 {
                self.szp(v, w);
            }
            if n == 1 {
                self.flag(
                    OF,
                    match op {
                        0 | 2 | 4 | 6 => (v & sign != 0) != (self.flags & CF != 0),
                        1 | 3 => (v & sign != 0) != (v & (sign >> 1) != 0),
                        5 => old & sign != 0,
                        _ => false,
                    },
                );
            }
        }
        v
    }
    pub fn step(&mut self, b: &mut Bus) -> Result<(), String> {
        if self.flags & IF != 0 && self.inhibit == 0 {
            if let Some(n) = b.irq() {
                self.interrupt(b, n);
            }
        }
        if self.inhibit > 0 {
            self.inhibit -= 1;
        }
        if self.halted {
            return Ok(());
        }
        let start = self.ip;
        let mut ov = None;
        let mut rep = 0;
        let mut prefixes = 0;
        let op = loop {
            let c = self.fetch(b);
            match c {
                0x26 => ov = Some(ES),
                0x2e => ov = Some(CS),
                0x36 => ov = Some(SS),
                0x3e => ov = Some(DS),
                0xf2 | 0xf3 => rep = c,
                0xf0 => {}
                _ => break c,
            };
            prefixes += 1;
            if prefixes >= 15 {
                return Err("excessive instruction prefixes".into());
            }
        };
        self.retired += 1;
        match op {
            0x00..=0x3d if op & 7 <= 5 => {
                let kind = op >> 3;
                let w = op & 1 != 0;
                if op & 7 < 4 {
                    let m = self.fetch(b);
                    let rm = self.decode(b, m, ov);
                    let reg = Operand::Reg(((m >> 3) & 7) as usize);
                    let (dst, src) = if op & 2 == 0 { (rm, reg) } else { (reg, rm) };
                    let v = self.alu(kind, self.get(b, dst, w), self.get(b, src, w), w);
                    if kind != 7 {
                        self.put(b, dst, v, w);
                    }
                } else {
                    let v = if w {
                        self.word(b)
                    } else {
                        self.fetch(b) as u16
                    };
                    let out =
                        self.alu(kind, if w { self.r[AX] } else { self.byte(0) as u16 }, v, w);
                    if kind != 7 {
                        self.put(b, Operand::Reg(0), out, w);
                    }
                }
            }
            0x06 | 0x0e | 0x16 | 0x1e => self.push(b, self.s[(op >> 3) as usize]),
            0x07 | 0x17 | 0x1f => {
                let r = (op >> 3) as usize;
                self.s[r] = self.pop(b);
                if r == SS {
                    self.inhibit = 1;
                }
            }
            0x27 | 0x2f => {
                let old = self.byte(0);
                let sub = op == 0x2f;
                let cf = self.flags & CF != 0;
                let adjust = self.flags & AF != 0 || old & 15 > 9;
                let mut v = old;
                if adjust {
                    v = if sub {
                        v.wrapping_sub(6)
                    } else {
                        v.wrapping_add(6)
                    }
                }
                self.flag(AF, adjust);
                let carry = cf || old > 0x99;
                if carry {
                    v = if sub {
                        v.wrapping_sub(0x60)
                    } else {
                        v.wrapping_add(0x60)
                    }
                }
                self.flag(CF, carry);
                self.set_byte(0, v);
                self.szp(v as u16, false);
            }
            0x37 | 0x3f => {
                let adjust = self.byte(0) & 15 > 9 || self.flags & AF != 0;
                if adjust {
                    self.r[AX] = if op == 0x37 {
                        self.r[AX].wrapping_add(0x106)
                    } else {
                        self.r[AX].wrapping_sub(0x106)
                    }
                }
                self.set_byte(0, self.byte(0) & 15);
                self.flag(AF, adjust);
                self.flag(CF, adjust);
            }
            0x40..=0x4f => {
                let i = (op & 7) as usize;
                let cf = self.flags & CF;
                self.r[i] = self.alu(if op & 8 == 0 { 0 } else { 5 }, self.r[i], 1, true);
                self.flags = (self.flags & !CF) | cf;
            }
            0x50..=0x57 => self.push(b, self.r[(op & 7) as usize]),
            0x58..=0x5f => {
                let v = self.pop(b);
                self.r[(op & 7) as usize] = v;
            }
            0x60 => {
                let sp = self.r[SP];
                for i in [AX, CX, DX, BX, SP, BP, SI, DI] {
                    self.push(b, if i == SP { sp } else { self.r[i] });
                }
            }
            0x61 => {
                for i in [DI, SI, BP, SP, BX, DX, CX, AX] {
                    let v = self.pop(b);
                    if i != SP {
                        self.r[i] = v
                    }
                }
            }
            0x68 | 0x6a => {
                let v = if op == 0x68 {
                    self.word(b)
                } else {
                    self.fetch(b) as i8 as u16
                };
                self.push(b, v);
            }
            0x69 | 0x6b => {
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let a = self.get(b, o, true) as i16 as i32;
                let c = if op == 0x69 {
                    self.word(b) as i16 as i32
                } else {
                    self.fetch(b) as i8 as i32
                };
                let v = a * c;
                self.r[((m >> 3) & 7) as usize] = v as u16;
                self.flag(CF, v != v as i16 as i32);
                self.flag(OF, self.flags & CF != 0);
            }
            0x70..=0x7f => {
                let d = self.fetch(b) as i8 as u16;
                if self.cond(op & 15) {
                    self.ip = self.ip.wrapping_add(d)
                }
            }
            0x80..=0x83 => {
                let w = op & 1 != 0;
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let v = if op == 0x81 {
                    self.word(b)
                } else if op == 0x83 {
                    self.fetch(b) as i8 as u16
                } else {
                    self.fetch(b) as u16
                };
                let kind = (m >> 3) & 7;
                let out = self.alu(kind, self.get(b, o, w), v, w);
                if kind != 7 {
                    self.put(b, o, out, w);
                }
            }
            0x84 | 0x85 => {
                let w = op & 1 != 0;
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                self.alu(
                    4,
                    self.get(b, o, w),
                    self.get(b, Operand::Reg(((m >> 3) & 7) as usize), w),
                    w,
                );
            }
            0x86 | 0x87 => {
                let w = op & 1 != 0;
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let reg = Operand::Reg(((m >> 3) & 7) as usize);
                let a = self.get(b, o, w);
                let c = self.get(b, reg, w);
                self.put(b, o, c, w);
                self.put(b, reg, a, w);
            }
            0x88..=0x8b => {
                let w = op & 1 != 0;
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let reg = Operand::Reg(((m >> 3) & 7) as usize);
                let (dst, src) = if op & 2 == 0 { (o, reg) } else { (reg, o) };
                let v = self.get(b, src, w);
                self.put(b, dst, v, w);
            }
            0x8c | 0x8e => {
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let s = ((m >> 3) & 7) as usize;
                if s > 3 {
                    return Err("invalid segment register".into());
                }
                if op == 0x8c {
                    self.put(b, o, self.s[s], true)
                } else {
                    self.s[s] = self.get(b, o, true);
                    if s == SS {
                        self.inhibit = 1
                    }
                }
            }
            0x8d => {
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                if let Operand::Mem(_, a) = o {
                    self.r[((m >> 3) & 7) as usize] = a
                } else {
                    return Err("LEA requires memory".into());
                }
            }
            0x8f => {
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let v = self.pop(b);
                self.put(b, o, v, true);
            }
            0x90..=0x97 => self.r.swap(AX, (op & 7) as usize),
            0x98 => self.r[AX] = self.byte(0) as i8 as i16 as u16,
            0x99 => self.r[DX] = if self.r[AX] & 0x8000 != 0 { 65535 } else { 0 },
            0x9a => {
                let off = self.word(b);
                let seg = self.word(b);
                self.push(b, self.s[CS]);
                self.push(b, self.ip);
                self.s[CS] = seg;
                self.ip = off;
            }
            0x9b => {}
            0x9c => self.push(b, self.flags | 0xf002),
            0x9d => {
                let v = self.pop(b);
                self.flags = (v & 0x0fd5) | 0xf002;
            }
            0x9e => self.flags = (self.flags & !0xd5) | (self.byte(4) as u16 & 0xd5) | 2,
            0x9f => self.set_byte(4, self.flags as u8),
            0xa0..=0xa3 => {
                let w = op & 1 != 0;
                let a = self.word(b);
                let o = Operand::Mem(self.s[ov.unwrap_or(DS)], a);
                if op & 2 == 0 {
                    let v = self.get(b, o, w);
                    self.put(b, Operand::Reg(0), v, w)
                } else {
                    self.put(b, o, self.get(b, Operand::Reg(0), w), w)
                }
            }
            0xa4..=0xa7 | 0xaa..=0xaf | 0x6c..=0x6f => {
                let w = op & 1 != 0;
                if rep != 0 && self.r[CX] == 0 {
                    return Ok(());
                }
                let src = Operand::Mem(self.s[ov.unwrap_or(DS)], self.r[SI]);
                let dst = Operand::Mem(self.s[ES], self.r[DI]);
                let a = self.get(b, Operand::Reg(0), w);
                match op & 0xfe {
                    0xa4 => {
                        let v = self.get(b, src, w);
                        self.put(b, dst, v, w);
                    }
                    0xa6 => {
                        self.alu(7, self.get(b, src, w), self.get(b, dst, w), w);
                    }
                    0xaa => self.put(b, dst, a, w),
                    0xac => {
                        let v = self.get(b, src, w);
                        self.put(b, Operand::Reg(0), v, w);
                    }
                    0xae => {
                        self.alu(7, a, self.get(b, dst, w), w);
                    }
                    0x6c => {
                        let v = b.input(self.r[DX]) as u16
                            | if w {
                                (b.input(self.r[DX].wrapping_add(1)) as u16) << 8
                            } else {
                                0
                            };
                        self.put(b, dst, v, w);
                    }
                    0x6e => {
                        let v = self.get(b, src, w);
                        b.output(self.r[DX], v as u8);
                        if w {
                            b.output(self.r[DX].wrapping_add(1), (v >> 8) as u8)
                        }
                    }
                    _ => unreachable!(),
                }
                let d = if self.flags & DF == 0 {
                    if w {
                        2
                    } else {
                        1
                    }
                } else if w {
                    0xfffe
                } else {
                    0xffff
                };
                if matches!(op & 0xfe, 0xa4 | 0xa6 | 0xac | 0x6e) {
                    self.r[SI] = self.r[SI].wrapping_add(d)
                }
                if matches!(op & 0xfe, 0xa4 | 0xa6 | 0xaa | 0xae | 0x6c) {
                    self.r[DI] = self.r[DI].wrapping_add(d)
                }
                if rep != 0 {
                    self.r[CX] = self.r[CX].wrapping_sub(1);
                    let compare = matches!(op & 0xfe, 0xa6 | 0xae);
                    if self.r[CX] != 0 && (!compare || (self.flags & ZF != 0) == (rep == 0xf3)) {
                        self.ip = start;
                    }
                }
            }
            0xa8 | 0xa9 => {
                let w = op & 1 != 0;
                let v = if w {
                    self.word(b)
                } else {
                    self.fetch(b) as u16
                };
                self.alu(4, self.get(b, Operand::Reg(0), w), v, w);
            }
            0xb0..=0xb7 => {
                let v = self.fetch(b);
                self.set_byte((op & 7) as usize, v);
            }
            0xb8..=0xbf => {
                let v = self.word(b);
                self.r[(op & 7) as usize] = v;
            }
            0xc0 | 0xc1 | 0xd0..=0xd3 => {
                let w = op & 1 != 0;
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let n = if op < 0xd0 {
                    self.fetch(b)
                } else if op & 2 == 0 {
                    1
                } else {
                    self.byte(1)
                };
                let v = self.shift((m >> 3) & 7, self.get(b, o, w), n, w);
                self.put(b, o, v, w);
            }
            0xc2 | 0xc3 | 0xca | 0xcb => {
                let n = if op & 1 == 0 { self.word(b) } else { 0 };
                self.ip = self.pop(b);
                if op & 8 != 0 {
                    self.s[CS] = self.pop(b);
                }
                self.r[SP] = self.r[SP].wrapping_add(n);
            }
            0xc4 | 0xc5 => {
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                if let Operand::Mem(s, a) = o {
                    self.r[((m >> 3) & 7) as usize] = b.read_seg(s, a, true);
                    self.s[if op == 0xc4 { ES } else { DS }] =
                        b.read_seg(s, a.wrapping_add(2), true);
                } else {
                    return Err("far pointer requires memory".into());
                }
            }
            0xc6 | 0xc7 => {
                let w = op & 1 != 0;
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let v = if w {
                    self.word(b)
                } else {
                    self.fetch(b) as u16
                };
                self.put(b, o, v, w);
            }
            0xc8 => {
                let size = self.word(b);
                let level = self.fetch(b) & 31;
                self.push(b, self.r[BP]);
                let frame = self.r[SP];
                for _ in 1..level {
                    self.r[BP] = self.r[BP].wrapping_sub(2);
                    self.push(b, b.read_seg(self.s[SS], self.r[BP], true));
                }
                if level > 0 {
                    self.push(b, frame)
                }
                self.r[BP] = frame;
                self.r[SP] = self.r[SP].wrapping_sub(size);
            }
            0xc9 => {
                self.r[SP] = self.r[BP];
                self.r[BP] = self.pop(b);
            }
            0xcc => self.interrupt(b, 3),
            0xcd => {
                let n = self.fetch(b);
                self.interrupt(b, n);
            }
            0xce => {
                if self.flags & OF != 0 {
                    self.interrupt(b, 4)
                }
            }
            0xcf => {
                self.ip = self.pop(b);
                self.s[CS] = self.pop(b);
                self.flags = (self.pop(b) & 0x0fd5) | 0xf002;
            }
            0xd4 | 0xd5 => {
                let _encoded_radix = self.fetch(b);
                let radix = 10;
                if op == 0xd4 {
                    let a = self.byte(0);
                    self.set_byte(4, a / radix);
                    self.set_byte(0, a % radix)
                } else {
                    self.set_byte(
                        0,
                        self.byte(4).wrapping_mul(radix).wrapping_add(self.byte(0)),
                    );
                    self.set_byte(4, 0)
                }
                self.szp(self.byte(0) as u16, false);
            }
            0xd7 => self.set_byte(
                0,
                b.read(Self::phys(
                    self.s[ov.unwrap_or(DS)],
                    self.r[BX].wrapping_add(self.byte(0) as u16),
                )),
            ),
            0xe0..=0xe3 => {
                let d = self.fetch(b) as i8 as u16;
                let take = if op == 0xe3 {
                    self.r[CX] == 0
                } else {
                    self.r[CX] = self.r[CX].wrapping_sub(1);
                    self.r[CX] != 0 && (op == 0xe2 || (self.flags & ZF != 0) == (op == 0xe1))
                };
                if take {
                    self.ip = self.ip.wrapping_add(d)
                }
            }
            0xe4..=0xe7 | 0xec..=0xef => {
                let w = op & 1 != 0;
                let p = if op < 0xe8 {
                    self.fetch(b) as u16
                } else {
                    self.r[DX]
                };
                if op & 2 == 0 {
                    let v = b.input(p) as u16
                        | if w {
                            (b.input(p.wrapping_add(1)) as u16) << 8
                        } else {
                            0
                        };
                    self.put(b, Operand::Reg(0), v, w)
                } else {
                    b.output(p, self.byte(0));
                    if w {
                        b.output(p.wrapping_add(1), self.byte(4))
                    }
                }
            }
            0xe8 | 0xe9 => {
                let d = self.word(b);
                if op == 0xe8 {
                    self.push(b, self.ip)
                }
                self.ip = self.ip.wrapping_add(d);
            }
            0xea => {
                let off = self.word(b);
                self.s[CS] = self.word(b);
                self.ip = off;
            }
            0xeb => {
                let d = self.fetch(b) as i8 as u16;
                self.ip = self.ip.wrapping_add(d);
            }
            0xf4 => self.halted = true,
            0xf5 => self.flags ^= CF,
            0xf6 | 0xf7 => {
                let w = op & 1 != 0;
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let v = self.get(b, o, w);
                match (m >> 3) & 7 {
                    0 | 1 => {
                        let c = if w {
                            self.word(b)
                        } else {
                            self.fetch(b) as u16
                        };
                        self.alu(4, v, c, w);
                    }
                    2 => self.put(b, o, !v, w),
                    3 => {
                        let out = self.alu(5, 0, v, w);
                        self.put(b, o, out, w);
                    }
                    4 | 5 => {
                        let signed = m & 8 != 0;
                        let out = if signed {
                            if w {
                                (self.r[AX] as i16 as i32).wrapping_mul(v as i16 as i32) as u32
                            } else {
                                (self.byte(0) as i8 as i16).wrapping_mul(v as u8 as i8 as i16)
                                    as u16 as u32
                            }
                        } else if w {
                            self.r[AX] as u32 * v as u32
                        } else {
                            self.byte(0) as u32 * v as u32
                        };
                        let overflow = if signed {
                            if w {
                                out as i32 != out as i16 as i32
                            } else {
                                out as u16 as i16 != out as u8 as i8 as i16
                            }
                        } else if w {
                            out > 65535
                        } else {
                            out > 255
                        };
                        self.r[AX] = out as u16;
                        if w {
                            self.r[DX] = (out >> 16) as u16
                        }
                        self.flag(CF, overflow);
                        self.flag(OF, overflow);
                    }
                    6 | 7 => {
                        let signed = m & 8 != 0;
                        let a = if w {
                            ((self.r[DX] as u32) << 16) | self.r[AX] as u32
                        } else {
                            self.r[AX] as u32
                        };
                        let (q, r) = if v == 0 {
                            (i64::MAX, 0)
                        } else if signed {
                            let a = if w {
                                a as i32 as i64
                            } else {
                                a as u16 as i16 as i64
                            };
                            let c = if w {
                                v as i16 as i64
                            } else {
                                v as u8 as i8 as i64
                            };
                            (a / c, a % c)
                        } else {
                            (a as i64 / v as i64, a as i64 % v as i64)
                        };
                        let valid = if signed {
                            if w {
                                (-32768..=32767).contains(&q)
                            } else {
                                (-128..=127).contains(&q)
                            }
                        } else {
                            (0..=if w { 65535 } else { 255 }).contains(&q)
                        };
                        if !valid {
                            self.ip = start;
                            self.interrupt(b, 0)
                        } else if w {
                            self.r[AX] = q as u16;
                            self.r[DX] = r as u16;
                        } else {
                            self.set_byte(0, q as u8);
                            self.set_byte(4, r as u8);
                        }
                    }
                    _ => unreachable!(),
                }
            }
            0xf8 => self.flags &= !CF,
            0xf9 => self.flags |= CF,
            0xfa => self.flags &= !IF,
            0xfb => {
                self.flags |= IF;
                self.inhibit = 1;
            }
            0xfc => self.flags &= !DF,
            0xfd => self.flags |= DF,
            0xfe | 0xff => {
                let w = op & 1 != 0;
                let m = self.fetch(b);
                let o = self.decode(b, m, ov);
                let v = self.get(b, o, w);
                match (m >> 3) & 7 {
                    0 | 1 => {
                        let cf = self.flags & CF;
                        let out = self.alu(if m & 8 == 0 { 0 } else { 5 }, v, 1, w);
                        self.flags = (self.flags & !CF) | cf;
                        self.put(b, o, out, w);
                    }
                    2 => {
                        self.push(b, self.ip);
                        self.ip = v;
                    }
                    3 | 5 => {
                        if let Operand::Mem(s, a) = o {
                            let seg = b.read_seg(s, a.wrapping_add(2), true);
                            if m & 0x38 == 0x18 {
                                self.push(b, self.s[CS]);
                                self.push(b, self.ip)
                            }
                            self.s[CS] = seg;
                            self.ip = v;
                        } else {
                            return Err("far transfer requires memory".into());
                        }
                    }
                    4 => self.ip = v,
                    6 => self.push(b, v),
                    _ => return Err("invalid FF group".into()),
                }
            }
            // V30 TEST1/CLR1/SET1/NOT1, register bit index (10..17) or immediate (18..1f).
            0x0f => {
                let ext = self.fetch(b);
                if (0x10..=0x1f).contains(&ext) {
                    let w = ext & 1 != 0;
                    let m = self.fetch(b);
                    let o = self.decode(b, m, ov);
                    let bit = if ext & 8 != 0 {
                        self.fetch(b)
                    } else {
                        self.byte(1)
                    } & if w { 15 } else { 7 };
                    let v = self.get(b, o, w);
                    self.flag(ZF, v & (1 << bit) == 0);
                    let out = match (ext >> 1) & 3 {
                        0 => v,
                        1 => v & !(1 << bit),
                        2 => v | (1 << bit),
                        _ => v ^ (1 << bit),
                    };
                    if ext & 6 != 0 {
                        self.put(b, o, out, w)
                    }
                } else {
                    return Err(format!("unsupported V30 opcode 0F {ext:02x}"));
                }
            }
            _ => {
                return Err(format!(
                    "unsupported opcode {op:02x} at {:04x}:{start:04x}",
                    self.s[CS]
                ))
            }
        }
        Ok(())
    }
}

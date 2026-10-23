//! Original V30 and bounded i486 real-mode interpreter. Instructions never delegate to a host emulator.
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
    /// Low words stay public for the firmware and existing native callers.
    pub r: [u16; 8],
    pub s: [u16; 4],
    /// Upper words of EAX through EDI; byte/word writes preserve these.
    pub upper: [u16; 8],
    pub extra_segments: [u16; 2], // FS, GS
    /// CR0 low word for SMSW only. Reset: ET set, other/reserved bits zero.
    /// Guest status writers and protected execution are not implemented.
    pub machine_status: u16,
    /// Emulator reset policy: zero limit/base (hardware power-up GDTR is undefined).
    pub gdtr_limit: u16,
    /// Linear metadata; never masked by A20 or validated against installed RAM.
    pub gdtr_base: u32,
    /// Real-mode interrupt table; reset matches i486 architectural state.
    pub idtr_limit: u16,
    /// Unmasked linear metadata, independent of installed memory and A20.
    pub idtr_base: u32,
    /// Terminal IDT-limit shutdown; only reset resumes execution.
    pub shutdown: bool,
    pub ip: u32,
    pub flags_upper: u16,
    // Decoder state resets each instruction. Public to retain native struct-update
    // construction with ..Cpu::default(); callers should change architectural state.
    #[doc(hidden)]
    pub operand32: bool,
    #[doc(hidden)]
    pub address32: bool,
    #[doc(hidden)]
    pub lock_prefix: bool,
    #[doc(hidden)]
    pub trap_suppressed: bool,
    #[doc(hidden)]
    pub instruction_bytes: u8,
    pub flags: u16,
    pub halted: bool,
    pub retired: u64,
    pub extended_retired: u64,
    pub inhibit: u8,
}
#[derive(Clone, Copy)]
enum Operand {
    Reg(usize),
    Mem(usize, u32),
}
impl Default for Cpu {
    fn default() -> Self {
        Self {
            r: [0; 8],
            s: [0, 0xffff, 0, 0],
            upper: [0; 8],
            extra_segments: [0; 2],
            machine_status: 0x0010,
            gdtr_limit: 0,
            gdtr_base: 0,
            idtr_limit: 0x03ff,
            idtr_base: 0,
            shutdown: false,
            flags_upper: 0,
            operand32: false,
            address32: false,
            lock_prefix: false,
            trap_suppressed: false,
            instruction_bytes: 0,
            ip: 0,
            flags: 0xf002,
            halted: false,
            retired: 0,
            extended_retired: 0,
            inhibit: 0,
        }
    }
}
#[derive(Debug)]
enum ExecError {
    Fault(u8),
    Unsupported(String),
}
type ExecResult<T> = Result<T, ExecError>;
impl From<String> for ExecError {
    fn from(s: String) -> Self {
        Self::Unsupported(s)
    }
}
impl From<&str> for ExecError {
    fn from(s: &str) -> Self {
        Self::Unsupported(s.into())
    }
}
impl Cpu {
    pub fn phys(seg: u16, off: u16) -> usize {
        (seg as usize) * 16 + off as usize
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
    pub fn reg32(&self, r: usize) -> u32 {
        self.r[r] as u32 | ((self.upper[r] as u32) << 16)
    }
    pub fn set_reg32(&mut self, r: usize, v: u32) {
        self.r[r] = v as u16;
        self.upper[r] = (v >> 16) as u16;
    }
    fn segment(&self, s: usize) -> u16 {
        if s < 4 {
            self.s[s]
        } else {
            self.extra_segments[s - 4]
        }
    }
    fn set_segment(&mut self, s: usize, v: u16) {
        if s < 4 {
            self.s[s] = v;
        } else {
            self.extra_segments[s - 4] = v;
        }
    }
    fn bytes(&self, w: bool) -> u32 {
        if !w {
            1
        } else if self.operand32 {
            4
        } else {
            2
        }
    }
    fn mask(&self, w: bool) -> u32 {
        match self.bytes(w) {
            1 => 255,
            2 => 65535,
            _ => u32::MAX,
        }
    }
    fn fetch(&mut self, b: &Bus) -> ExecResult<u8> {
        if b.profile == crate::Profile::Pc9821ApDos
            && (self.ip > 0xffff || self.instruction_bytes >= 15)
        {
            return Err(ExecError::Fault(13));
        }
        let v = b.read(self.s[CS] as usize * 16 + self.ip as usize);
        self.ip = self.ip.wrapping_add(1);
        if b.profile == crate::Profile::Pc9801Vm {
            self.ip &= 0xffff;
        }
        self.instruction_bytes += 1;
        Ok(v)
    }
    fn word(&mut self, b: &Bus) -> ExecResult<u16> {
        let l = self.fetch(b)?;
        Ok(l as u16 | ((self.fetch(b)? as u16) << 8))
    }
    fn immediate(&mut self, b: &Bus, w: bool) -> ExecResult<u32> {
        if !w {
            return Ok(self.fetch(b)? as u32);
        }
        let l = self.word(b)? as u32;
        Ok(l | if self.operand32 {
            (self.word(b)? as u32) << 16
        } else {
            0
        })
    }
    fn check_mem(&self, b: &Bus, s: usize, off: u32, bytes: u32) -> ExecResult<()> {
        if b.profile == crate::Profile::Pc9821ApDos && (off > 0xffff || bytes - 1 > 0xffff - off) {
            return Err(ExecError::Fault(if s == SS { 12 } else { 13 }));
        }
        Ok(())
    }
    fn read_mem(&self, b: &Bus, s: usize, off: u32, bytes: u32) -> ExecResult<u32> {
        self.check_mem(b, s, off, bytes)?;
        let mut v = 0;
        for i in 0..bytes {
            let a = if b.profile == crate::Profile::Pc9801Vm {
                (off + i) & 0xffff
            } else {
                off + i
            };
            v |= (b.read(self.segment(s) as usize * 16 + a as usize) as u32) << (i * 8);
        }
        Ok(v)
    }
    fn write_mem(&self, b: &mut Bus, s: usize, off: u32, v: u32, bytes: u32) -> ExecResult<()> {
        self.check_mem(b, s, off, bytes)?;
        for i in 0..bytes {
            let a = if b.profile == crate::Profile::Pc9801Vm {
                (off + i) & 0xffff
            } else {
                off + i
            };
            b.write(
                self.segment(s) as usize * 16 + a as usize,
                (v >> (i * 8)) as u8,
            );
        }
        Ok(())
    }
    fn push_width(&mut self, b: &mut Bus, v: u32, bytes: u32) -> ExecResult<()> {
        let sp = self.r[SP].wrapping_sub(bytes as u16);
        self.write_mem(b, SS, sp as u32, v, bytes)?;
        self.r[SP] = sp;
        Ok(())
    }
    fn pop_width(&mut self, b: &Bus, bytes: u32) -> ExecResult<u32> {
        let v = self.read_mem(b, SS, self.r[SP] as u32, bytes)?;
        self.r[SP] = self.r[SP].wrapping_add(bytes as u16);
        Ok(v)
    }
    fn push(&mut self, b: &mut Bus, v: u32) -> ExecResult<()> {
        self.push_width(b, v, self.bytes(true))
    }
    fn push_segment(&mut self, b: &mut Bus, s: usize) -> ExecResult<()> {
        // i486 selector pushes reserve the operand size but only access a word.
        let sp = self.r[SP].wrapping_sub(self.bytes(true) as u16);
        self.write_mem(b, SS, sp as u32, self.segment(s) as u32, 2)?;
        self.r[SP] = sp;
        Ok(())
    }
    fn pop(&mut self, b: &Bus) -> ExecResult<u32> {
        self.pop_width(b, self.bytes(true))
    }
    fn deliver_interrupt(&mut self, b: &mut Bus, n: u8) -> ExecResult<()> {
        if self.shutdown {
            return Ok(());
        }
        let entry = n as u32 * 4;
        let base = if b.profile == crate::Profile::Pc9821ApDos {
            // Check the complete entry before touching the stack or reading a vector.
            if entry + 3 > self.idtr_limit as u32 {
                return Err(ExecError::Fault(8));
            }
            self.idtr_base
        } else {
            0 // V30 keeps its fixed IVT, irrespective of the public IDTR fields.
        };
        // Wrap each linear byte address at 32 bits before Bus mapping, on both
        // native and Wasm hosts (usize has different widths on those targets).
        let vector: [u8; 4] = std::array::from_fn(|i| {
            b.read(base.wrapping_add(entry).wrapping_add(i as u32) as usize)
        });
        // Real-mode entry always saves FLAGS, CS and IP as words, even for 66 INT.
        let bytes = 2;
        // Preflight each push: the 16-bit stack pointer may wrap between words.
        for i in 1..=3 {
            self.check_mem(
                b,
                SS,
                self.r[SP].wrapping_sub((i * bytes) as u16) as u32,
                bytes,
            )?;
        }
        self.push_width(b, self.flags as u32, bytes)?;
        self.push_width(b, self.s[CS] as u32, bytes)?;
        self.push_width(b, self.ip, bytes)?;
        self.flags &= !(IF | TF);
        self.flags_upper &= !4; // Real-mode interrupt/exception entry clears AC.
        self.trap_suppressed = true;
        self.ip = u16::from_le_bytes([vector[0], vector[1]]) as u32;
        self.s[CS] = u16::from_le_bytes([vector[2], vector[3]]);
        self.halted = false;
        Ok(())
    }
    // Events outside instruction execution keep their current resume IP. Software
    // INT uses deliver_interrupt directly so step restores the faulting prefix IP
    // before attempting #DF. This is a bounded IDT-limit path, not nested-fault emulation.
    fn deliver_event(&mut self, b: &mut Bus, n: u8) -> ExecResult<()> {
        match self.deliver_interrupt(b, n) {
            Err(ExecError::Fault(8)) => match self.deliver_interrupt(b, 8) {
                Err(ExecError::Fault(8)) => {
                    self.shutdown = true;
                    self.trap_suppressed = true;
                    Ok(())
                }
                result => result,
            },
            result => result,
        }
    }
    /// Inject a real-mode interrupt with a six-byte frame from a native caller.
    pub fn interrupt(&mut self, b: &mut Bus, n: u8) -> Result<(), String> {
        self.deliver_event(b, n)
            .map_err(|_| "invalid real-mode interrupt stack".to_string())
    }
    fn flag(&mut self, f: u16, on: bool) {
        if on {
            self.flags |= f
        } else {
            self.flags &= !f
        }
    }
    fn szp(&mut self, v: u32, w: bool) {
        let v = v & self.mask(w);
        self.flag(ZF, v == 0);
        self.flag(SF, v & (self.mask(w) ^ (self.mask(w) >> 1)) != 0);
        self.flag(PF, (v as u8).count_ones() & 1 == 0);
    }
    fn alu(&mut self, op: u8, a: u32, c: u32, w: bool) -> u32 {
        let mask = self.mask(w) as u64;
        let sign = (mask + 1) >> 1;
        let a = a as u64 & mask;
        let c = c as u64 & mask;
        let carry = u64::from(self.flags & CF != 0);
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
        self.szp(v as u32, w);
        v as u32
    }
    fn decode(&mut self, b: &Bus, m: u8, ov: Option<usize>) -> ExecResult<Operand> {
        let mode = m >> 6;
        let rm = (m & 7) as usize;
        if mode == 3 {
            if self.lock_prefix {
                return Err(ExecError::Fault(6));
            }
            return Ok(Operand::Reg(rm));
        }
        if self.address32 {
            let (base, stack) = if rm == 4 {
                let sib = self.fetch(b)?;
                let index = ((sib >> 3) & 7) as usize;
                let base = (sib & 7) as usize;
                let index = if index == SP {
                    0
                } else {
                    self.reg32(index).wrapping_shl((sib >> 6) as u32)
                };
                let (value, stack) = if base == BP && mode == 0 {
                    (self.dword(b)?, false)
                } else {
                    (self.reg32(base), base == BP || base == SP)
                };
                (value.wrapping_add(index), stack)
            } else if rm == BP && mode == 0 {
                (self.dword(b)?, false)
            } else {
                (self.reg32(rm), rm == BP)
            };
            let disp = match mode {
                1 => self.fetch(b)? as i8 as u32,
                2 => self.dword(b)?,
                _ => 0,
            };
            return Ok(Operand::Mem(
                ov.unwrap_or(if stack { SS } else { DS }),
                base.wrapping_add(disp),
            ));
        }
        let (base, stack) = match rm {
            0 => (self.r[BX].wrapping_add(self.r[SI]), false),
            1 => (self.r[BX].wrapping_add(self.r[DI]), false),
            2 => (self.r[BP].wrapping_add(self.r[SI]), true),
            3 => (self.r[BP].wrapping_add(self.r[DI]), true),
            4 => (self.r[SI], false),
            5 => (self.r[DI], false),
            6 if mode == 0 => (self.word(b)?, false),
            6 => (self.r[BP], true),
            _ => (self.r[BX], false),
        };
        let disp = match mode {
            1 => self.fetch(b)? as i8 as u16,
            2 => self.word(b)?,
            _ => 0,
        };
        Ok(Operand::Mem(
            ov.unwrap_or(if stack { SS } else { DS }),
            base.wrapping_add(disp) as u32,
        ))
    }
    fn dword(&mut self, b: &Bus) -> ExecResult<u32> {
        let l = self.word(b)? as u32;
        Ok(l | ((self.word(b)? as u32) << 16))
    }
    fn get(&self, b: &Bus, o: Operand, w: bool) -> ExecResult<u32> {
        match o {
            Operand::Reg(r) => Ok(if !w {
                self.byte(r) as u32
            } else if self.operand32 {
                self.reg32(r)
            } else {
                self.r[r] as u32
            }),
            Operand::Mem(s, a) => self.read_mem(b, s, a, self.bytes(w)),
        }
    }
    fn put(&mut self, b: &mut Bus, o: Operand, v: u32, w: bool) -> ExecResult<()> {
        match o {
            Operand::Reg(r) => {
                if !w {
                    self.set_byte(r, v as u8);
                } else if self.operand32 {
                    self.set_reg32(r, v);
                } else {
                    self.r[r] = v as u16;
                }
                Ok(())
            }
            Operand::Mem(s, a) => self.write_mem(b, s, a, v, self.bytes(w)),
        }
    }
    fn index(&self, r: usize) -> u32 {
        if self.address32 {
            self.reg32(r)
        } else {
            self.r[r] as u32
        }
    }
    fn set_index(&mut self, r: usize, v: u32) {
        if self.address32 {
            self.set_reg32(r, v);
        } else {
            self.r[r] = v as u16;
        }
    }
    fn branch(&mut self, target: u32) -> ExecResult<()> {
        let target = if self.operand32 {
            target
        } else {
            target & 0xffff
        };
        if target > 0xffff {
            return Err(ExecError::Fault(13));
        }
        self.ip = target;
        Ok(())
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
    fn shift(&mut self, op: u8, mut v: u32, n: u8, w: bool) -> u32 {
        let mask = self.mask(w);
        let sign = mask ^ (mask >> 1);
        let old = v;
        for _ in 0..n {
            let c = self.flags & CF != 0;
            let next = match op {
                0 | 2 | 4 | 6 => v & sign != 0,
                _ => v & 1 != 0,
            };
            v = match op {
                0 => (v << 1) | u32::from(next),
                1 => (v >> 1) | if next { sign } else { 0 },
                2 => (v << 1) | u32::from(c),
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
    fn signed(&self, value: u32, w: bool) -> i64 {
        match self.bytes(w) {
            1 => value as i8 as i64,
            2 => value as i16 as i64,
            _ => value as i32 as i64,
        }
    }
    fn check_stack_sequence(&self, b: &Bus, count: u32, push: bool) -> ExecResult<()> {
        let bytes = self.bytes(true);
        for i in 0..count {
            let delta = if push { (i + 1) * bytes } else { i * bytes } as u16;
            let off = if push {
                self.r[SP].wrapping_sub(delta)
            } else {
                self.r[SP].wrapping_add(delta)
            };
            self.check_mem(b, SS, off as u32, bytes)?;
        }
        Ok(())
    }
    fn load_flags(&mut self, b: &Bus, value: u32) {
        self.flags = if b.profile == crate::Profile::Pc9801Vm {
            (value as u16 & 0x0fd5) | 0xf002
        } else {
            (value as u16 & 0x7fd5) | 2
        };
        if self.operand32 {
            self.flags_upper = ((value >> 16) as u16) & 4;
        } // AC only; no VM86/CPUID.
    }
    fn input_width(&self, b: &mut Bus, port: u16, w: bool) -> u32 {
        let mut v = 0;
        for i in 0..self.bytes(w) {
            v |= (b.input(port.wrapping_add(i as u16)) as u32) << (i * 8);
        }
        v
    }
    fn output_width(&self, b: &mut Bus, port: u16, value: u32, w: bool) {
        for i in 0..self.bytes(w) {
            b.output(port.wrapping_add(i as u16), (value >> (i * 8)) as u8);
        }
    }
    fn load_pointer(&mut self, b: &Bus, o: Operand, reg: usize, seg: usize) -> ExecResult<()> {
        if let Operand::Mem(s, a) = o {
            self.check_mem(b, s, a, self.bytes(true) + 2)?;
            let off = self.get(b, o, true)?;
            let segment = self.read_mem(b, s, a + self.bytes(true), 2)?;
            self.put_register(reg, off);
            self.set_segment(seg, segment as u16);
            if seg == SS {
                self.inhibit = 1;
                self.trap_suppressed = true;
            }
            Ok(())
        } else {
            Err(ExecError::Fault(6))
        }
    }
    fn put_register(&mut self, r: usize, v: u32) {
        if self.operand32 {
            self.set_reg32(r, v);
        } else {
            self.r[r] = v as u16;
        }
    }
    pub fn step(&mut self, b: &mut Bus) -> Result<(), String> {
        if self.shutdown {
            return Ok(());
        }
        if self.flags & IF != 0 && self.inhibit == 0 {
            if let Some(n) = b.irq() {
                self.deliver_event(b, n)
                    .map_err(|_| "invalid real-mode interrupt stack".to_string())?;
            }
        }
        if self.shutdown {
            return Ok(());
        }
        if self.inhibit > 0 {
            self.inhibit -= 1;
        }
        // Restore architectural registers for restartable real-mode faults.
        let saved = self.clone();
        match self.execute(b) {
            Ok(()) => {
                if b.profile == crate::Profile::Pc9821ApDos
                    && saved.flags & TF != 0
                    && !self.trap_suppressed
                {
                    self.deliver_event(b, 1)
                        .map_err(|_| "invalid single-step interrupt stack".to_string())?;
                }
                Ok(())
            }
            Err(error) => {
                let vector = match error {
                    ExecError::Fault(6) if b.profile == crate::Profile::Pc9801Vm => {
                        return Err("invalid V30 instruction".into())
                    }
                    ExecError::Fault(n) => n,
                    ExecError::Unsupported(message) => return Err(message),
                };
                *self = saved;
                self.deliver_event(b, vector)
                    .map_err(|_| "fault while delivering real-mode exception".to_string())
            }
        }
    }
    fn extended(&mut self, b: &mut Bus, ov: Option<usize>) -> ExecResult<()> {
        let op = self.fetch(b)?;
        if self.lock_prefix && !matches!(op, 0xab | 0xb3 | 0xbb | 0xba | 0xb0 | 0xb1 | 0xc0 | 0xc1)
        {
            return Err(ExecError::Fault(6));
        }
        match op {
            0x01 => {
                let m = self.fetch(b)?;
                let kind = (m >> 3) & 7;
                if kind == 4 {
                    match self.decode(b, m, ov)? {
                        // Undefined upper bits in operand32 register form are
                        // preserved as a deterministic emulator policy.
                        Operand::Reg(r) => self.r[r] = self.machine_status,
                        Operand::Mem(s, off) => {
                            // Intel486 real-address SMSW table: #GP even for SS.
                            // Preflight both bytes before any bus write. The modern
                            // SDM instead specifies #SS for SS overruns.
                            if off > 0xfffe {
                                return Err(ExecError::Fault(13));
                            }
                            let address = self.segment(s) as usize * 16 + off as usize;
                            for (i, value) in
                                self.machine_status.to_le_bytes().into_iter().enumerate()
                            {
                                b.write(address + i, value);
                            }
                        }
                    }
                    return Ok(());
                }
                if !matches!(kind, 0..=3) {
                    return Err(ExecError::Unsupported(format!(
                        "unimplemented i486 opcode 0F 01 /{kind}"
                    )));
                }
                let Operand::Mem(s, off) = self.decode(b, m, ov)? else {
                    return Err(ExecError::Fault(6));
                };
                // i486 real-address contract for these six-byte operands is #GP,
                // including SS. Preflight before any table-register change or bus write.
                if off > 0xfffa {
                    return Err(ExecError::Fault(13));
                }
                let address = self.segment(s) as usize * 16 + off as usize;
                if kind >= 2 {
                    let limit = b.read16(address);
                    let mut base = 0;
                    for i in 0..if self.operand32 { 4 } else { 3 } {
                        base |= (b.read(address + 2 + i) as u32) << (8 * i);
                    }
                    if kind == 2 {
                        self.gdtr_limit = limit;
                        self.gdtr_base = base;
                    } else {
                        self.idtr_limit = limit;
                        self.idtr_base = base;
                    }
                } else {
                    // Full base with either operand size: explicit SDM SGDT/SIDT
                    // operation, despite older compatibility text (docs/hardware.md).
                    let (limit, base) = if kind == 0 {
                        (self.gdtr_limit, self.gdtr_base)
                    } else {
                        (self.idtr_limit, self.idtr_base)
                    };
                    let limit = limit.to_le_bytes();
                    let base = base.to_le_bytes();
                    for (i, value) in limit.into_iter().chain(base).enumerate() {
                        b.write(address + i, value);
                    }
                }
            }
            0x80..=0x8f => {
                let d = self.immediate(b, true)?;
                if self.cond(op & 15) {
                    self.branch(self.ip.wrapping_add(d))?;
                }
            }
            0x90..=0x9f => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                self.put(b, o, u32::from(self.cond(op & 15)), false)?;
            }
            0xa0 | 0xa8 => self.push_segment(b, if op == 0xa0 { 4 } else { 5 })?,
            0xa1 | 0xa9 => {
                let v = self.pop(b)? as u16;
                self.set_segment(if op == 0xa1 { 4 } else { 5 }, v);
            }
            0xb2 | 0xb4 | 0xb5 => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                self.load_pointer(
                    b,
                    o,
                    ((m >> 3) & 7) as usize,
                    match op {
                        0xb2 => SS,
                        0xb4 => 4,
                        _ => 5,
                    },
                )?;
            }
            0xb6 | 0xb7 | 0xbe | 0xbf => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let v = match o {
                    Operand::Reg(r) => {
                        if op & 1 == 0 {
                            self.byte(r) as u32
                        } else {
                            self.r[r] as u32
                        }
                    }
                    Operand::Mem(s, a) => {
                        self.read_mem(b, s, a, if op & 1 == 0 { 1 } else { 2 })?
                    }
                };
                let v = if op & 8 != 0 {
                    if op & 1 == 0 {
                        v as i8 as u32
                    } else {
                        v as i16 as u32
                    }
                } else {
                    v
                };
                self.put_register(((m >> 3) & 7) as usize, v);
            }
            0xaf => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let r = ((m >> 3) & 7) as usize;
                let v = self.signed(self.get(b, o, true)?, true)
                    * self.signed(self.get(b, Operand::Reg(r), true)?, true);
                self.put_register(r, v as u32);
                self.flag(CF, v != self.signed(v as u32, true));
                self.flag(OF, self.flags & CF != 0);
            }
            0xbc | 0xbd => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let v = self.get(b, o, true)?;
                self.flag(ZF, v == 0);
                if v != 0 {
                    self.put_register(
                        ((m >> 3) & 7) as usize,
                        if op == 0xbc {
                            v.trailing_zeros()
                        } else {
                            31 - v.leading_zeros()
                        },
                    );
                }
            }
            0xa3 | 0xab | 0xb3 | 0xbb | 0xba => {
                let m = self.fetch(b)?;
                let mut o = self.decode(b, m, ov)?;
                let bits = self.bytes(true) * 8;
                let (kind, index) = if op == 0xba {
                    let kind = (m >> 3) & 7;
                    if kind < 4 {
                        return Err(ExecError::Fault(6));
                    }
                    (kind - 4, self.fetch(b)? as u32)
                } else {
                    (
                        match op {
                            0xa3 => 0,
                            0xab => 1,
                            0xb3 => 2,
                            _ => 3,
                        },
                        self.get(b, Operand::Reg(((m >> 3) & 7) as usize), true)?,
                    )
                };
                if self.lock_prefix && kind == 0 {
                    return Err(ExecError::Fault(6));
                }
                if op != 0xba {
                    if let Operand::Mem(s, a) = o {
                        let delta = (self.signed(index, true)
                            >> if self.operand32 { 5 } else { 4 })
                            * self.bytes(true) as i64;
                        // The effective address-size addition wraps before limit checking.
                        let a = a.wrapping_add(delta as u32);
                        o = Operand::Mem(s, if self.address32 { a } else { a & 0xffff });
                    }
                }
                let bit = 1u32 << (index & (bits - 1));
                let v = self.get(b, o, true)?;
                self.flag(CF, v & bit != 0);
                let out = match kind {
                    1 => v | bit,
                    2 => v & !bit,
                    3 => v ^ bit,
                    _ => v,
                };
                if kind != 0 {
                    self.put(b, o, out, true)?;
                }
            }
            0xa4 | 0xa5 | 0xac | 0xad => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let n = (if op & 1 == 0 {
                    self.fetch(b)?
                } else {
                    self.byte(1)
                }) & 31;
                let dst = self.get(b, o, true)?;
                let src = self.get(b, Operand::Reg(((m >> 3) & 7) as usize), true)?;
                if n != 0 {
                    let bits = self.bytes(true) * 8;
                    // Intel leaves 16-bit results undefined for counts > 16; choose a
                    // deterministic rotation of the concatenated source/destination.
                    let mask = self.mask(true) as u64;
                    let pair = if op & 8 == 0 {
                        ((dst as u64) << bits) | src as u64
                    } else {
                        ((src as u64) << bits) | dst as u64
                    };
                    let out = if op & 8 == 0 {
                        self.flag(CF, pair & (1u64 << (bits * 2 - n as u32)) != 0);
                        ((pair << n) >> bits) as u32 & mask as u32
                    } else {
                        self.flag(CF, pair & (1u64 << (n - 1)) != 0);
                        (pair >> n) as u32 & mask as u32
                    };
                    self.szp(out, true);
                    if n == 1 {
                        self.flag(OF, (dst ^ out) & (1 << (bits - 1)) != 0);
                    }
                    self.put(b, o, out, true)?;
                }
            }
            0xb0 | 0xb1 | 0xc0 | 0xc1 => {
                let w = op & 1 != 0;
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let reg = Operand::Reg(((m >> 3) & 7) as usize);
                let dst = self.get(b, o, w)?;
                let src = self.get(b, reg, w)?;
                if op & 0x40 != 0 {
                    let result = self.alu(0, dst, src, w);
                    self.put(b, reg, dst, w)?;
                    self.put(b, o, result, w)?;
                } else {
                    self.alu(7, self.get(b, Operand::Reg(AX), w)?, dst, w);
                    if self.flags & ZF != 0 {
                        self.put(b, o, src, w)?;
                    } else {
                        self.put(b, Operand::Reg(AX), dst, w)?;
                    }
                }
            }
            0xc8..=0xcf => {
                if !self.operand32 {
                    return Err(ExecError::Fault(6));
                }
                let r = (op & 7) as usize;
                self.set_reg32(r, self.reg32(r).swap_bytes());
            }
            0x10..=0x1f | 0xa2 | 0x0b | 0xff => return Err(ExecError::Fault(6)),
            _ => {
                return Err(ExecError::Unsupported(format!(
                    "unimplemented i486 opcode 0F {op:02x}"
                )))
            }
        }
        Ok(())
    }
    fn execute(&mut self, b: &mut Bus) -> ExecResult<()> {
        if self.halted {
            return Ok(());
        }
        let start = self.ip;
        self.instruction_bytes = 0;
        self.operand32 = false;
        self.address32 = false;
        self.lock_prefix = false;
        self.trap_suppressed = false;
        let mut ov = None;
        let mut rep = 0;
        let mut prefixes = 0;
        let op = loop {
            let c = self.fetch(b)?;
            match c {
                0x26 => ov = Some(ES),
                0x2e => ov = Some(CS),
                0x36 => ov = Some(SS),
                0x3e => ov = Some(DS),
                0x64 | 0x65 if b.profile == crate::Profile::Pc9821ApDos => {
                    ov = Some((c - 0x60) as usize)
                }
                0x66 if b.profile == crate::Profile::Pc9821ApDos => self.operand32 = true,
                0x67 if b.profile == crate::Profile::Pc9821ApDos => self.address32 = true,
                0xf2 | 0xf3 => rep = c,
                0xf0 => self.lock_prefix = b.profile == crate::Profile::Pc9821ApDos,
                _ => break c,
            };
            prefixes += 1;
            if prefixes >= 15 {
                return Err(if b.profile == crate::Profile::Pc9821ApDos {
                    ExecError::Fault(13)
                } else {
                    "excessive instruction prefixes".into()
                });
            }
        };
        self.retired += 1;
        if self.operand32 || self.address32 {
            self.extended_retired += 1;
        }
        if self.lock_prefix
            && !(matches!(
                op,
                0x00 | 0x01
                    | 0x08
                    | 0x09
                    | 0x10
                    | 0x11
                    | 0x18
                    | 0x19
                    | 0x20
                    | 0x21
                    | 0x28
                    | 0x29
                    | 0x30
                    | 0x31
                    | 0x80
                    | 0x81
                    | 0x83
                    | 0x86
                    | 0x87
                    | 0xf6
                    | 0xf7
                    | 0xfe
                    | 0xff
                    | 0x0f
            ))
        {
            return Err(ExecError::Fault(6));
        }
        match op {
            0x00..=0x3d if op & 7 <= 5 => {
                let kind = op >> 3;
                let w = op & 1 != 0;
                if op & 7 < 4 {
                    let m = self.fetch(b)?;
                    let rm = self.decode(b, m, ov)?;
                    let reg = Operand::Reg(((m >> 3) & 7) as usize);
                    let (dst, src) = if op & 2 == 0 { (rm, reg) } else { (reg, rm) };
                    let v = self.alu(kind, self.get(b, dst, w)?, self.get(b, src, w)?, w);
                    if kind != 7 {
                        self.put(b, dst, v, w)?;
                    }
                } else {
                    let v = self.immediate(b, w)?;
                    let out = self.alu(kind, self.get(b, Operand::Reg(AX), w)?, v, w);
                    if kind != 7 {
                        self.put(b, Operand::Reg(0), out, w)?;
                    }
                }
            }
            0x06 | 0x0e | 0x16 | 0x1e => self.push_segment(b, (op >> 3) as usize)?,
            0x07 | 0x17 | 0x1f => {
                let r = (op >> 3) as usize;
                self.s[r] = self.pop(b)? as u16;
                if r == SS {
                    self.inhibit = 1;
                    self.trap_suppressed = true;
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
                self.szp(v as u32, false);
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
                let o = Operand::Reg((op & 7) as usize);
                let cf = self.flags & CF;
                let out = self.alu(
                    if op & 8 == 0 { 0 } else { 5 },
                    self.get(b, o, true)?,
                    1,
                    true,
                );
                self.put(b, o, out, true)?;
                self.flags = (self.flags & !CF) | cf;
            }
            0x50..=0x57 => self.push(b, self.get(b, Operand::Reg((op & 7) as usize), true)?)?,
            0x58..=0x5f => {
                let v = self.pop(b)?;
                self.put(b, Operand::Reg((op & 7) as usize), v, true)?;
            }
            0x60 => {
                let sp = self.get(b, Operand::Reg(SP), true)?;
                self.check_stack_sequence(b, 8, true)?;
                for i in [AX, CX, DX, BX, SP, BP, SI, DI] {
                    self.push(
                        b,
                        if i == SP {
                            sp
                        } else {
                            self.get(b, Operand::Reg(i), true)?
                        },
                    )?;
                }
            }
            0x61 => {
                self.check_stack_sequence(b, 8, false)?;
                for i in [DI, SI, BP, SP, BX, DX, CX, AX] {
                    let v = self.pop(b)?;
                    if i != SP {
                        self.put(b, Operand::Reg(i), v, true)?;
                    }
                }
            }
            0x62 => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                if let Operand::Mem(seg, a) = o {
                    let bytes = self.bytes(true);
                    self.check_mem(b, seg, a, bytes * 2)?;
                    let low = self.signed(self.read_mem(b, seg, a, bytes)?, true);
                    let high = self.signed(self.read_mem(b, seg, a + bytes, bytes)?, true);
                    let value = self.signed(
                        self.get(b, Operand::Reg(((m >> 3) & 7) as usize), true)?,
                        true,
                    );
                    if value < low || value > high {
                        if b.profile == crate::Profile::Pc9801Vm {
                            // NEC CHKIND conditionally performs BRK 5 after
                            // consuming the instruction. IRET resumes at next IP.
                            self.deliver_interrupt(b, 5)?;
                        } else {
                            return Err(ExecError::Fault(5));
                        }
                    }
                } else {
                    if b.profile == crate::Profile::Pc9801Vm {
                        return Err("invalid V30 BOUND: memory operand required".into());
                    }
                    return Err(ExecError::Fault(6));
                }
            }
            0x63 if b.profile == crate::Profile::Pc9821ApDos => return Err(ExecError::Fault(6)),
            0x68 | 0x6a => {
                let v = if op == 0x68 {
                    self.immediate(b, true)?
                } else {
                    self.fetch(b)? as i8 as u32
                };
                self.push(b, v)?;
            }
            0x69 | 0x6b => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let a = self.signed(self.get(b, o, true)?, true);
                let c = if op == 0x69 {
                    let v = self.immediate(b, true)?;
                    self.signed(v, true)
                } else {
                    self.fetch(b)? as i8 as i64
                };
                let v = a * c;
                self.put(b, Operand::Reg(((m >> 3) & 7) as usize), v as u32, true)?;
                self.flag(CF, v != self.signed(v as u32, true));
                self.flag(OF, self.flags & CF != 0);
            }
            0x70..=0x7f => {
                let d = self.fetch(b)? as i8 as u32;
                if self.cond(op & 15) {
                    self.branch(self.ip.wrapping_add(d))?;
                }
            }
            0x80..=0x83 => {
                if op == 0x82 && b.profile == crate::Profile::Pc9821ApDos {
                    return Err(ExecError::Fault(6));
                }
                let w = op & 1 != 0;
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let v = if op == 0x81 {
                    self.immediate(b, true)?
                } else if op == 0x83 {
                    self.fetch(b)? as i8 as u32
                } else {
                    self.fetch(b)? as u32
                };
                let kind = (m >> 3) & 7;
                if self.lock_prefix && kind == 7 {
                    return Err(ExecError::Fault(6));
                }
                let out = self.alu(kind, self.get(b, o, w)?, v, w);
                if kind != 7 {
                    self.put(b, o, out, w)?;
                }
            }
            0x84 | 0x85 => {
                let w = op & 1 != 0;
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                self.alu(
                    4,
                    self.get(b, o, w)?,
                    self.get(b, Operand::Reg(((m >> 3) & 7) as usize), w)?,
                    w,
                );
            }
            0x86 | 0x87 => {
                let w = op & 1 != 0;
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let reg = Operand::Reg(((m >> 3) & 7) as usize);
                let a = self.get(b, o, w)?;
                let c = self.get(b, reg, w)?;
                self.put(b, o, c, w)?;
                self.put(b, reg, a, w)?;
            }
            0x88..=0x8b => {
                let w = op & 1 != 0;
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let reg = Operand::Reg(((m >> 3) & 7) as usize);
                let (dst, src) = if op & 2 == 0 { (o, reg) } else { (reg, o) };
                let v = self.get(b, src, w)?;
                self.put(b, dst, v, w)?;
            }
            0x8c | 0x8e => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let s = ((m >> 3) & 7) as usize;
                let max = if b.profile == crate::Profile::Pc9801Vm {
                    3
                } else {
                    5
                };
                if s > max || (op == 0x8e && s == CS && b.profile == crate::Profile::Pc9821ApDos) {
                    return Err(ExecError::Fault(6));
                }
                if op == 0x8c {
                    // Operand override zero-extends register destinations; memory stays a word.
                    if let Operand::Mem(seg, a) = o {
                        self.write_mem(b, seg, a, self.segment(s) as u32, 2)?;
                    } else {
                        self.put(b, o, self.segment(s) as u32, true)?;
                    }
                } else {
                    let value = match o {
                        Operand::Mem(seg, a) => self.read_mem(b, seg, a, 2)?,
                        Operand::Reg(r) => self.r[r] as u32,
                    };
                    self.set_segment(s, value as u16);
                    if s == SS {
                        self.inhibit = 1;
                        self.trap_suppressed = true;
                    }
                }
            }
            0x8d => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                if let Operand::Mem(_, a) = o {
                    self.put(b, Operand::Reg(((m >> 3) & 7) as usize), a, true)?;
                } else {
                    return Err(ExecError::Fault(6));
                }
            }
            0x8f => {
                let m = self.fetch(b)?;
                if m & 0x38 != 0 {
                    return Err(ExecError::Fault(6));
                }
                // ESP-based addresses are calculated after incrementing the stack pointer.
                let next_sp = self.r[SP].wrapping_add(self.bytes(true) as u16);
                let old_sp = self.r[SP];
                self.r[SP] = next_sp;
                let o = self.decode(b, m, ov)?;
                self.r[SP] = old_sp;
                let v = self.pop(b)?;
                self.put(b, o, v, true)?;
            }
            0x90..=0x97 => {
                let o = Operand::Reg((op & 7) as usize);
                let a = self.get(b, Operand::Reg(AX), true)?;
                let v = self.get(b, o, true)?;
                self.put(b, Operand::Reg(AX), v, true)?;
                self.put(b, o, a, true)?;
            }
            0x98 => {
                let v = if self.operand32 {
                    self.r[AX] as i16 as u32
                } else {
                    self.byte(0) as i8 as u32
                };
                self.put(b, Operand::Reg(AX), v, true)?;
            }
            0x99 => {
                let a = self.get(b, Operand::Reg(AX), true)?;
                let v = if self.signed(a, true) < 0 {
                    u32::MAX
                } else {
                    0
                };
                self.put(b, Operand::Reg(DX), v, true)?;
            }
            0x9a => {
                let off = self.immediate(b, true)?;
                let seg = self.word(b)?;
                let ret = self.ip;
                self.branch(off)?;
                self.check_stack_sequence(b, 2, true)?;
                self.push(b, self.s[CS] as u32)?;
                self.push(b, ret)?;
                self.s[CS] = seg;
            }
            0x9b => {}
            0x9c => {
                let v = if b.profile == crate::Profile::Pc9801Vm {
                    (self.flags | 0xf002) as u32
                } else {
                    (self.flags | 2) as u32 | (((self.flags_upper & 4) as u32) << 16)
                };
                self.push(b, v)?;
            }
            0x9d => {
                let v = self.pop(b)?;
                self.load_flags(b, v);
            }
            0x9e => self.flags = (self.flags & !0xd5) | (self.byte(4) as u16 & 0xd5) | 2,
            0x9f => self.set_byte(4, self.flags as u8),
            0xa0..=0xa3 => {
                let w = op & 1 != 0;
                let a = if self.address32 {
                    self.dword(b)?
                } else {
                    self.word(b)? as u32
                };
                let o = Operand::Mem(ov.unwrap_or(DS), a);
                if op & 2 == 0 {
                    let v = self.get(b, o, w)?;
                    self.put(b, Operand::Reg(0), v, w)?
                } else {
                    self.put(b, o, self.get(b, Operand::Reg(0), w)?, w)?
                }
            }
            0xa4..=0xa7 | 0xaa..=0xaf | 0x6c..=0x6f => {
                let w = op & 1 != 0;
                if rep != 0 && self.index(CX) == 0 {
                    return Ok(());
                }
                let src = Operand::Mem(ov.unwrap_or(DS), self.index(SI));
                let dst = Operand::Mem(ES, self.index(DI));
                let a = self.get(b, Operand::Reg(0), w)?;
                match op & 0xfe {
                    0xa4 => {
                        let v = self.get(b, src, w)?;
                        self.put(b, dst, v, w)?;
                    }
                    0xa6 => {
                        self.alu(7, self.get(b, src, w)?, self.get(b, dst, w)?, w);
                    }
                    0xaa => self.put(b, dst, a, w)?,
                    0xac => {
                        let v = self.get(b, src, w)?;
                        self.put(b, Operand::Reg(0), v, w)?;
                    }
                    0xae => {
                        self.alu(7, a, self.get(b, dst, w)?, w);
                    }
                    0x6c => {
                        self.check_mem(b, ES, self.index(DI), self.bytes(w))?;
                        let v = self.input_width(b, self.r[DX], w);
                        self.put(b, dst, v, w)?;
                    }
                    0x6e => {
                        let v = self.get(b, src, w)?;
                        self.output_width(b, self.r[DX], v, w);
                    }
                    _ => unreachable!(),
                }
                let d = if self.flags & DF == 0 {
                    self.bytes(w)
                } else {
                    0u32.wrapping_sub(self.bytes(w))
                };
                if matches!(op & 0xfe, 0xa4 | 0xa6 | 0xac | 0x6e) {
                    self.set_index(SI, self.index(SI).wrapping_add(d));
                }
                if matches!(op & 0xfe, 0xa4 | 0xa6 | 0xaa | 0xae | 0x6c) {
                    self.set_index(DI, self.index(DI).wrapping_add(d));
                }
                if rep != 0 {
                    self.set_index(CX, self.index(CX).wrapping_sub(1));
                    let compare = matches!(op & 0xfe, 0xa6 | 0xae);
                    if self.index(CX) != 0 && (!compare || (self.flags & ZF != 0) == (rep == 0xf3))
                    {
                        self.ip = start;
                    }
                }
            }
            0xa8 | 0xa9 => {
                let w = op & 1 != 0;
                let v = self.immediate(b, w)?;
                self.alu(4, self.get(b, Operand::Reg(0), w)?, v, w);
            }
            0xb0..=0xb7 => {
                let v = self.fetch(b)?;
                self.set_byte((op & 7) as usize, v);
            }
            0xb8..=0xbf => {
                let v = self.immediate(b, true)?;
                self.put(b, Operand::Reg((op & 7) as usize), v, true)?;
            }
            0xc0 | 0xc1 | 0xd0..=0xd3 => {
                let w = op & 1 != 0;
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                let n = if op < 0xd0 {
                    self.fetch(b)?
                } else if op & 2 == 0 {
                    1
                } else {
                    self.byte(1)
                };
                let n = if b.profile == crate::Profile::Pc9821ApDos {
                    n & 31
                } else {
                    n
                };
                let v = self.shift((m >> 3) & 7, self.get(b, o, w)?, n, w);
                self.put(b, o, v, w)?;
            }
            0xc2 | 0xc3 | 0xca | 0xcb => {
                let n = if op & 1 == 0 { self.word(b)? } else { 0 };
                let off = self.pop(b)?;
                self.branch(off)?;
                if op & 8 != 0 {
                    self.s[CS] = self.pop(b)? as u16;
                }
                self.r[SP] = self.r[SP].wrapping_add(n);
            }
            0xc4 | 0xc5 => {
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                self.load_pointer(
                    b,
                    o,
                    ((m >> 3) & 7) as usize,
                    if op == 0xc4 { ES } else { DS },
                )?;
            }
            0xc6 | 0xc7 => {
                let w = op & 1 != 0;
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                if m & 0x38 != 0 {
                    return Err(ExecError::Fault(6));
                }
                let v = self.immediate(b, w)?;
                self.put(b, o, v, w)?;
            }
            0xc8 => {
                let size = self.word(b)?;
                let level = self.fetch(b)? & 31;
                self.check_stack_sequence(b, 1 + level as u32, true)?;
                let bytes = self.bytes(true);
                let original_bp = self.get(b, Operand::Reg(BP), true)?;
                // Stack address size stays 16-bit even with 66/67 overrides.
                let chain_bp = self.r[BP];
                for i in 1..level {
                    let bp = chain_bp.wrapping_sub((bytes * i as u32) as u16) as u32;
                    self.check_mem(b, SS, bp, bytes)?;
                }
                self.push(b, original_bp)?;
                let frame = self.get(b, Operand::Reg(SP), true)?;
                for i in 1..level {
                    let bp = chain_bp.wrapping_sub((bytes * i as u32) as u16) as u32;
                    let v = self.read_mem(b, SS, bp, bytes)?;
                    self.push(b, v)?;
                }
                if level > 0 {
                    self.push(b, frame)?;
                }
                self.put(b, Operand::Reg(BP), frame, true)?;
                self.r[SP] = self.r[SP].wrapping_sub(size);
            }
            0xc9 => {
                self.r[SP] = self.r[BP];
                let v = self.pop(b)?;
                self.put(b, Operand::Reg(BP), v, true)?;
            }
            0xcc => self.deliver_interrupt(b, 3)?,
            0xcd => {
                let n = self.fetch(b)?;
                self.deliver_interrupt(b, n)?;
            }
            0xce => {
                if self.flags & OF != 0 {
                    self.deliver_interrupt(b, 4)?
                }
            }
            0xcf => {
                self.check_stack_sequence(b, 3, false)?;
                let off = self.pop(b)?;
                self.branch(off)?;
                self.s[CS] = self.pop(b)? as u16;
                let v = self.pop(b)?;
                self.load_flags(b, v);
            }
            0xd4 | 0xd5 => {
                let encoded_radix = self.fetch(b)?;
                let radix = if b.profile == crate::Profile::Pc9801Vm {
                    10
                } else {
                    encoded_radix
                };
                if op == 0xd4 && radix == 0 {
                    self.ip = start;
                    self.deliver_interrupt(b, 0)?;
                    return Ok(());
                }
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
                self.szp(self.byte(0) as u32, false);
            }
            0xd7 => {
                let off = self.index(BX).wrapping_add(self.byte(0) as u32);
                let off = if self.address32 { off } else { off & 0xffff };
                let v = self.read_mem(b, ov.unwrap_or(DS), off, 1)?;
                self.set_byte(0, v as u8);
            }
            0xe0..=0xe3 => {
                let d = self.fetch(b)? as i8 as u32;
                let take = if op == 0xe3 {
                    self.index(CX) == 0
                } else {
                    self.set_index(CX, self.index(CX).wrapping_sub(1));
                    self.index(CX) != 0 && (op == 0xe2 || (self.flags & ZF != 0) == (op == 0xe1))
                };
                if take {
                    self.branch(self.ip.wrapping_add(d))?;
                }
            }
            0xe4..=0xe7 | 0xec..=0xef => {
                let w = op & 1 != 0;
                let p = if op < 0xe8 {
                    self.fetch(b)? as u16
                } else {
                    self.r[DX]
                };
                if op & 2 == 0 {
                    let v = self.input_width(b, p, w);
                    self.put(b, Operand::Reg(0), v, w)?;
                } else {
                    let v = self.get(b, Operand::Reg(0), w)?;
                    self.output_width(b, p, v, w);
                }
            }
            0xe8 | 0xe9 => {
                let d = self.immediate(b, true)?;
                let ret = self.ip;
                self.branch(ret.wrapping_add(d))?;
                if op == 0xe8 {
                    self.push(b, ret)?;
                }
            }
            0xea => {
                let off = self.immediate(b, true)?;
                let seg = self.word(b)?;
                self.branch(off)?;
                self.s[CS] = seg;
            }
            0xeb => {
                let d = self.fetch(b)? as i8 as u32;
                self.branch(self.ip.wrapping_add(d))?;
            }
            0xf4 => self.halted = true,
            0xf5 => self.flags ^= CF,
            0xf6 | 0xf7 => {
                let w = op & 1 != 0;
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                if self.lock_prefix && !matches!((m >> 3) & 7, 2 | 3) {
                    return Err(ExecError::Fault(6));
                }
                if m & 0x38 == 8 && b.profile == crate::Profile::Pc9821ApDos {
                    return Err(ExecError::Fault(6));
                }
                let v = self.get(b, o, w)?;
                match (m >> 3) & 7 {
                    0 | 1 => {
                        let c = self.immediate(b, w)?;
                        self.alu(4, v, c, w);
                    }
                    2 => self.put(b, o, !v, w)?,
                    3 => {
                        let out = self.alu(5, 0, v, w);
                        self.put(b, o, out, w)?;
                    }
                    4 | 5 => {
                        let signed = m & 8 != 0;
                        let a = self.get(b, Operand::Reg(AX), w)?;
                        let bits = self.bytes(w) * 8;
                        let out = if signed {
                            (self.signed(a, w) * self.signed(v, w)) as u64
                        } else {
                            a as u64 * v as u64
                        };
                        let low = out as u32 & self.mask(w);
                        let overflow = if signed {
                            out as i64 != self.signed(low, w)
                        } else {
                            out >> bits != 0
                        };
                        if w {
                            self.put(b, Operand::Reg(AX), low, true)?;
                            self.put(b, Operand::Reg(DX), (out >> bits) as u32, true)?;
                        } else {
                            self.r[AX] = out as u16;
                        }
                        self.flag(CF, overflow);
                        self.flag(OF, overflow);
                    }
                    6 | 7 => {
                        let bits = self.bytes(w) * 8;
                        let signed = m & 8 != 0;
                        let a = if w {
                            ((self.get(b, Operand::Reg(DX), true)? as u64) << bits)
                                | self.get(b, Operand::Reg(AX), true)? as u64
                        } else {
                            self.r[AX] as u64
                        };
                        let (q, r) = if v == 0 {
                            (i128::MAX, 0)
                        } else if signed {
                            let a = match bits {
                                8 => a as i16 as i128,
                                16 => a as i32 as i128,
                                _ => a as i64 as i128,
                            };
                            let c = self.signed(v, w) as i128;
                            (a / c, a % c)
                        } else {
                            (a as i128 / v as i128, a as i128 % v as i128)
                        };
                        let valid = if signed {
                            (-(1i128 << (bits - 1))..(1i128 << (bits - 1))).contains(&q)
                        } else {
                            (0..=self.mask(w) as i128).contains(&q)
                        };
                        if !valid {
                            return Err(ExecError::Fault(0));
                        }
                        if w {
                            self.put(b, Operand::Reg(AX), q as u32, true)?;
                            self.put(b, Operand::Reg(DX), r as u32, true)?;
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
                let m = self.fetch(b)?;
                let o = self.decode(b, m, ov)?;
                if self.lock_prefix && m & 0x38 > 8 {
                    return Err(ExecError::Fault(6));
                }
                if !w && m & 0x38 > 8 {
                    return Err(ExecError::Fault(6));
                }
                let v = self.get(b, o, w)?;
                match (m >> 3) & 7 {
                    0 | 1 => {
                        let cf = self.flags & CF;
                        let out = self.alu(if m & 8 == 0 { 0 } else { 5 }, v, 1, w);
                        self.flags = (self.flags & !CF) | cf;
                        self.put(b, o, out, w)?;
                    }
                    2 => {
                        let ret = self.ip;
                        self.branch(v)?;
                        self.push(b, ret)?;
                    }
                    3 | 5 => {
                        if let Operand::Mem(s, a) = o {
                            let seg = self.read_mem(b, s, a + self.bytes(true), 2)? as u16;
                            let ret = self.ip;
                            self.branch(v)?;
                            if m & 0x38 == 0x18 {
                                self.check_stack_sequence(b, 2, true)?;
                                self.push(b, self.s[CS] as u32)?;
                                self.push(b, ret)?;
                            }
                            self.s[CS] = seg;
                        } else {
                            return Err(ExecError::Fault(6));
                        }
                    }
                    4 => self.branch(v)?,
                    6 => self.push(b, v)?,
                    _ => return Err(ExecError::Fault(6)),
                }
            }
            // V30 TEST1/CLR1/SET1/NOT1, register bit index (10..17) or immediate (18..1f).
            0x0f => {
                if b.profile == crate::Profile::Pc9821ApDos {
                    return self.extended(b, ov);
                }
                let ext = self.fetch(b)?;
                if (0x10..=0x1f).contains(&ext) {
                    let w = ext & 1 != 0;
                    let m = self.fetch(b)?;
                    let o = self.decode(b, m, ov)?;
                    let bit = if ext & 8 != 0 {
                        self.fetch(b)?
                    } else {
                        self.byte(1)
                    } & if w { 15 } else { 7 };
                    let v = self.get(b, o, w)?;
                    let out = match (ext >> 1) & 3 {
                        0 => {
                            self.flag(ZF, v & (1 << bit) == 0);
                            self.flags &= !(CF | OF);
                            v
                        }
                        1 => v & !(1 << bit),
                        2 => v | (1 << bit),
                        _ => v ^ (1 << bit),
                    };
                    if ext & 6 != 0 {
                        self.put(b, o, out, w)?
                    }
                } else {
                    return Err(ExecError::Unsupported(format!(
                        "unsupported V30 opcode 0F {ext:02x}"
                    )));
                }
            }
            _ => {
                return Err(ExecError::Unsupported(format!(
                    "unsupported opcode {op:02x} at {:04x}:{start:04x}",
                    self.s[CS]
                )))
            }
        }
        Ok(())
    }
}

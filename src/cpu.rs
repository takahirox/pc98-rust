//! Original V30 and bounded i486 interpreter. Instructions never delegate to a host emulator.
use crate::bus::Bus;
mod paging;
mod protected;
mod vm86;
pub use paging::Paging;
use protected::GateSource;
pub use protected::SegmentCache;
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
/// Bounded x87 environment metadata. Stack data and numerical execution remain
/// unimplemented. Cold construction deterministically uses FNINIT metadata;
/// RESET without BIST retains it (Intel486 PRM 19.2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fpu {
    pub control: u16,
    pub status: u16,
    pub tag: u16,
    pub instruction_pointer: u32,
    pub instruction_selector: u16,
    pub data_pointer: u32,
    pub data_selector: u16,
    pub last_opcode: u16,
}
impl Default for Fpu {
    fn default() -> Self {
        Self {
            control: 0x037f,
            status: 0,
            tag: 0xffff,
            instruction_pointer: 0,
            instruction_selector: 0,
            data_pointer: 0,
            data_selector: 0,
            last_opcode: 0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Cpu {
    /// Low words stay public for the firmware and existing native callers.
    pub r: [u16; 8],
    pub s: [u16; 4],
    /// Upper words of EAX through EDI; byte/word writes preserve these.
    pub upper: [u16; 8],
    pub extra_segments: [u16; 2], // FS, GS
    /// i486 CR0: reset CD/NW/ET set; PE/PG clear. Shared by MOV, SMSW and x87.
    /// PE enables segmentation; PG enables two-level 4 KiB page translation.
    pub cr0: u32,
    /// Hidden state in ES/CS/SS/DS/FS/GS order. None derives conventional
    /// real-mode state from the public selector, preserving native callers.
    pub segment_caches: [Option<SegmentCache>; 6],
    pub cpl: u8,
    pub ldtr: u16,
    pub ldtr_cache: Option<SegmentCache>,
    /// Visible TR selector. Reset deterministically chooses zero; the target-era
    /// reset tables do not promise an initialized task register.
    pub tr: u16,
    /// Cached TSS descriptor; type 3 is 16-bit, type 11 is 32-bit (busy).
    /// None is the emulator's uninitialized reset state, not a fabricated TSS.
    pub tr_cache: Option<SegmentCache>,
    pub last_fault: Option<(u8, u32)>,
    /// Last successful VM86 gate: vector, gate type, target CS:EIP, SS0:ESP0.
    /// Bounded CPU metadata for opt-in tracing; no guest instruction bypass.
    pub last_vm86_gate: Option<(u8, u8, u16, u32, u16, u32)>,
    /// CR2/CR3 use deterministic zero on reset (emulator policy).
    /// CR2 records the last page-fault linear address; CR3 selects physical tables.
    pub cr2: u32,
    pub cr3: u32,
    #[doc(hidden)]
    pub paging: Paging,
    pub fpu: Fpu,
    /// Emulator reset policy: zero limit/base (hardware power-up GDTR is undefined).
    pub gdtr_limit: u16,
    /// Linear metadata; never masked by A20 or validated against installed RAM.
    pub gdtr_base: u32,
    /// Interrupt table (real vectors or protected gates); architectural reset.
    pub idtr_limit: u16,
    /// Unmasked linear metadata, independent of installed memory and A20.
    pub idtr_base: u32,
    /// Terminal IDT/triple-fault shutdown; only reset resumes execution.
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
            cr0: 0x60000010,
            segment_caches: [None; 6],
            cpl: 0,
            ldtr: 0,
            ldtr_cache: None,
            tr: 0,
            tr_cache: None,
            last_fault: None,
            last_vm86_gate: None,
            cr2: 0,
            cr3: 0,
            paging: Paging::default(),
            fpu: Fpu::default(),
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
    /// Control writes reject undefined/unsupported state before any mutation.
    ControlWriteRejected(String),
    Fault(u8),
    ProtectedFault(u8, u32),
    PageFault {
        linear: u32,
        code: u32,
    },
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
    fn fetch(&mut self, b: &mut Bus) -> ExecResult<u8> {
        if b.profile == crate::Profile::Pc9821ApDos && self.instruction_bytes >= 15 {
            return Err(ExecError::Fault(13));
        }
        self.check_access(b, CS, self.ip, 1, false, true)?;
        let physical = self.translate(
            b,
            self.linear(CS, self.ip) as u32,
            false,
            self.cpl == 3,
            false,
        )?;
        let v = b.read(physical as usize);
        self.ip = self.ip.wrapping_add(1);
        if b.profile == crate::Profile::Pc9801Vm {
            self.ip &= 0xffff;
        }
        self.instruction_bytes += 1;
        Ok(v)
    }
    fn word(&mut self, b: &mut Bus) -> ExecResult<u16> {
        let l = self.fetch(b)?;
        Ok(l as u16 | ((self.fetch(b)? as u16) << 8))
    }
    fn immediate(&mut self, b: &mut Bus, w: bool) -> ExecResult<u32> {
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
    fn check_mem(&self, b: &mut Bus, s: usize, off: u32, bytes: u32) -> ExecResult<()> {
        self.check_access(b, s, off, bytes, false, false)
    }
    fn read_mem(&self, b: &mut Bus, s: usize, off: u32, bytes: u32) -> ExecResult<u32> {
        self.check_mem(b, s, off, bytes)?;
        let mut v = 0;
        for i in 0..bytes {
            let a = if b.profile == crate::Profile::Pc9801Vm {
                (off + i) & 0xffff
            } else {
                off + i
            };
            let physical =
                self.translate(b, self.linear(s, a) as u32, false, self.cpl == 3, false)?;
            v |= (b.read(physical as usize) as u32) << (i * 8);
        }
        Ok(v)
    }
    fn write_mem(&self, b: &mut Bus, s: usize, off: u32, v: u32, bytes: u32) -> ExecResult<()> {
        self.check_access(b, s, off, bytes, true, false)?;
        for i in 0..bytes {
            let a = if b.profile == crate::Profile::Pc9801Vm {
                (off + i) & 0xffff
            } else {
                off + i
            };
            let physical =
                self.translate(b, self.linear(s, a) as u32, true, self.cpl == 3, true)?;
            b.write(physical as usize, (v >> (i * 8)) as u8);
        }
        Ok(())
    }
    fn push_width(&mut self, b: &mut Bus, v: u32, bytes: u32) -> ExecResult<()> {
        let sp = self.stack_offset(self.stack_pointer().wrapping_sub(bytes));
        self.write_mem(b, SS, sp, v, bytes)?;
        self.set_stack_pointer(sp);
        Ok(())
    }
    fn pop_width(&mut self, b: &mut Bus, bytes: u32) -> ExecResult<u32> {
        let v = self.read_mem(b, SS, self.stack_pointer(), bytes)?;
        self.set_stack_pointer(self.stack_pointer().wrapping_add(bytes));
        Ok(v)
    }
    fn push(&mut self, b: &mut Bus, v: u32) -> ExecResult<()> {
        self.push_width(b, v, self.bytes(true))
    }
    fn push_segment(&mut self, b: &mut Bus, s: usize) -> ExecResult<()> {
        // i486 selector pushes reserve the operand size but only access a word.
        let sp = self.stack_offset(self.stack_pointer().wrapping_sub(self.bytes(true)));
        self.write_mem(b, SS, sp, self.segment(s) as u32, 2)?;
        self.set_stack_pointer(sp);
        Ok(())
    }
    fn pop(&mut self, b: &mut Bus) -> ExecResult<u32> {
        self.pop_width(b, self.bytes(true))
    }
    fn deliver_interrupt(&mut self, b: &mut Bus, n: u8) -> ExecResult<()> {
        if self.shutdown {
            return Ok(());
        }
        if self.protected_mode() {
            return self.protected_interrupt(b, n, GateSource::Software, None);
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
                self.stack_offset(self.stack_pointer().wrapping_sub(i * bytes)),
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
        self.reload_real_code(u16::from_le_bytes([vector[2], vector[3]]));
        self.halted = false;
        Ok(())
    }
    // Events outside instruction execution keep their current resume IP. Software
    // INT uses deliver_interrupt directly so step restores the faulting prefix IP
    // before attempting #DF. This is a bounded IDT-limit path, not nested-fault emulation.
    fn deliver_event(&mut self, b: &mut Bus, n: u8) -> ExecResult<()> {
        if self.protected_mode() {
            return self.protected_event(b, n, None, GateSource::External);
        }
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
    /// Inject an external interrupt using the current real/protected table format.
    pub fn interrupt(&mut self, b: &mut Bus, n: u8) -> Result<(), String> {
        self.deliver_event(b, n)
            .map_err(|e| format!("interrupt delivery failed: {e:?}"))
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
    fn decode(&mut self, b: &mut Bus, m: u8, ov: Option<usize>) -> ExecResult<Operand> {
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
    fn dword(&mut self, b: &mut Bus) -> ExecResult<u32> {
        let l = self.word(b)? as u32;
        Ok(l | ((self.word(b)? as u32) << 16))
    }
    fn get(&self, b: &mut Bus, o: Operand, w: bool) -> ExecResult<u32> {
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
    fn get_modify(&self, b: &mut Bus, o: Operand, w: bool) -> ExecResult<u32> {
        // Preflight the full write span before any operand bus read: reads of
        // memory-mapped devices (for example EGC VRAM) can change device latches.
        if let Operand::Mem(s, a) = o {
            self.check_access(b, s, a, self.bytes(w), true, false)?;
        }
        self.get(b, o, w)
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
        if target > self.segment_cache(CS).limit {
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
    fn check_stack_sequence(&self, b: &mut Bus, count: u32, push: bool) -> ExecResult<()> {
        let bytes = self.bytes(true);
        for i in 0..count {
            let delta = if push { (i + 1) * bytes } else { i * bytes };
            let off = self.stack_offset(if push {
                self.stack_pointer().wrapping_sub(delta)
            } else {
                self.stack_pointer().wrapping_add(delta)
            });
            self.check_access(b, SS, off, bytes, push, false)?;
        }
        Ok(())
    }
    fn load_flags(&mut self, b: &Bus, value: u32) {
        let old = self.flags;
        let vm = self.flags_upper & 2;
        self.flags = if b.profile == crate::Profile::Pc9801Vm {
            (value as u16 & 0x0fd5) | 0xf002
        } else {
            (value as u16 & 0x7fd5) | 2
        };
        if self.protected_mode() && self.cpl != 0 {
            self.flags = (self.flags & !0x3000) | (old & 0x3000);
            if self.cpl > ((old >> 12) & 3) as u8 {
                self.flags = (self.flags & !IF) | (old & IF);
            }
        }
        if self.operand32 {
            self.flags_upper = (((value >> 16) as u16) & 4) | vm;
        }
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
    fn load_pointer(&mut self, b: &mut Bus, o: Operand, reg: usize, seg: usize) -> ExecResult<()> {
        if let Operand::Mem(s, a) = o {
            self.check_mem(b, s, a, self.bytes(true) + 2)?;
            let off = self.get(b, o, true)?;
            let segment = self.read_mem(b, s, a + self.bytes(true), 2)?;
            self.load_segment(b, seg, segment as u16)?;
            self.put_register(reg, off);
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
        self.step_with_observer(b, |_, _| {})
    }
    /// Observe the instruction about to execute, after pending IRQ delivery.
    /// Halted/shutdown slots and failed interrupt delivery do not fetch an
    /// instruction and therefore do not invoke the read-only observer.
    pub fn step_with_observer(
        &mut self,
        b: &mut Bus,
        mut before_instruction: impl FnMut(&Cpu, &Bus),
    ) -> Result<(), String> {
        if self.shutdown {
            return Ok(());
        }
        if self.flags & IF != 0 && self.inhibit == 0 {
            if let Some(n) = b.irq() {
                self.deliver_event(b, n)
                    .map_err(|e| format!("interrupt delivery failed: {e:?}"))?;
            }
        }
        if self.shutdown {
            return Ok(());
        }
        if self.inhibit > 0 {
            self.inhibit -= 1;
        }
        if !self.halted {
            before_instruction(self, b);
        }
        // Restore architectural registers for restartable instruction faults.
        let saved = self.clone();
        if self.protected_mode() || self.paging_enabled() {
            b.begin_cpu_memory();
        }
        let execution = self.execute(b);
        b.finish_cpu_memory(execution.is_ok());
        match execution {
            Ok(()) => {
                if b.profile == crate::Profile::Pc9821ApDos
                    && saved.flags & TF != 0
                    && !self.trap_suppressed
                {
                    if self.protected_mode() {
                        self.protected_event(b, 1, None, GateSource::Trap)
                            .map_err(|e| format!("protected single-step delivery failed: {e:?}"))?;
                    } else {
                        self.deliver_event(b, 1)
                            .map_err(|_| "invalid single-step interrupt stack".to_string())?;
                    }
                }
                Ok(())
            }
            Err(error) => {
                let vector = match error {
                    ExecError::Fault(6) if b.profile == crate::Profile::Pc9801Vm => {
                        return Err("invalid V30 instruction".into())
                    }
                    ExecError::Fault(n) => n,
                    ExecError::PageFault { linear, code } => {
                        *self = saved;
                        self.cr2 = linear;
                        return self.protected_event(b, 14, Some(code), GateSource::Fault)
                            .map_err(|e| format!("protected page fault delivery failed CR2={:08x} error={code:08x}: {e:?}", self.cr2));
                    }
                    ExecError::ProtectedFault(n, code) => {
                        *self = saved;
                        return self.protected_event(b, n, Some(code), GateSource::Fault)
                            .map_err(|e| format!("protected fault delivery failed vector={n} error={code:08x}: {e:?}"));
                    }
                    ExecError::ControlWriteRejected(message) => {
                        *self = saved;
                        return Err(message);
                    }
                    ExecError::Unsupported(message) => {
                        if saved.protected_mode() {
                            *self = saved;
                            return Err(format!("{message}; PE=1 PG={} CS:EIP={:04x}:{:08x} CPL={} CR0={:08x} GDT={:08x}/{:04x} IDT={:08x}/{:04x} TR={:04x} TSS={:?}", u8::from(self.paging_enabled()), self.s[CS], self.ip, self.cpl, self.cr0, self.gdtr_base, self.gdtr_limit, self.idtr_base, self.idtr_limit, self.tr, self.tr_cache));
                        }
                        return Err(message);
                    }
                };
                *self = saved;
                if self.protected_mode() {
                    let error = matches!(vector, 8 | 10..=14 | 17).then_some(0);
                    self.last_fault = Some((vector, error.unwrap_or(0)));
                    return self.protected_event(b, vector, error, GateSource::Fault)
                        .map_err(|e| format!("protected fault delivery failed vector={vector} error={error:?}: {e:?}"));
                }
                self.last_fault = Some((vector, 0));
                self.deliver_event(b, vector)
                    .map_err(|_| "fault while delivering real-mode exception".to_string())
            }
        }
    }
    fn extended(&mut self, b: &mut Bus, ov: Option<usize>) -> ExecResult<()> {
        let op = self.fetch(b)?;
        if self.protected_mode() && !self.vm86() && self.cpl != 0 && !matches!(op, 0x01 | 0x06) {
            return Err(ExecError::Unsupported(format!(
                "unsupported protected execution CPL={}; only ring 0 is implemented",
                self.cpl
            )));
        }
        if self.lock_prefix && !matches!(op, 0xab | 0xb3 | 0xbb | 0xba | 0xb0 | 0xb1 | 0xc0 | 0xc1)
        {
            return Err(ExecError::Fault(6));
        }
        match op {
            0x00 => self.system_group0(b, ov)?,
            0x02 | 0x03 if self.vm86() => return Err(ExecError::Fault(6)),
            0x08 | 0x09 if self.vm86() => return Err(ExecError::Fault(13)),
            0x21 | 0x23 | 0x24 | 0x26 if self.vm86() => {
                let m = self.fetch(b)?;
                if m >> 6 != 3 || (matches!(op, 0x24 | 0x26) && (m >> 3) & 7 < 3) {
                    return Err(ExecError::Fault(6));
                }
                return Err(ExecError::Fault(13));
            }
            0x06 => {
                // Intel486 PRM CLTS, p.26-55: no ModR/M, real mode or CPL0;
                // clear only TS. In particular, EM and the hidden caches survive.
                // The shared extended-opcode LOCK check above supplies #UD.
                if self.protected_mode() && self.cpl != 0 {
                    return Err(ExecError::Fault(13));
                }
                self.cr0 &= !8;
            }
            0x20 | 0x22 => {
                let m = self.fetch(b)?;
                if m >> 6 != 3 {
                    return Err(ExecError::Fault(6));
                }
                let value = match (m >> 3) & 7 {
                    0 => self.cr0,
                    2 => self.cr2,
                    3 => self.cr3,
                    _ => return Err(ExecError::Fault(6)),
                };
                if self.protected_mode() && self.cpl != 0 {
                    return Err(ExecError::Fault(13));
                }
                // Intel486 PRM (1992) pp. 26-212–213: always a full 32-bit GPR,
                // independent of size overrides. Arithmetic flags are undefined
                // there; preserve all flags as our deterministic policy.
                if op == 0x20 {
                    self.set_reg32((m & 7) as usize, value);
                } else {
                    let written = self.reg32((m & 7) as usize);
                    let selector = (m >> 3) & 7;
                    // Intel486 PRM §§4.1.3, 10.2, 12.2.1, 19.2 and MOV
                    // pp. 26-212–213. Validate before committing any CR state.
                    if selector == 0
                        && ((written & 0x80000001 == 0x80000000)
                            || (written & 0x60000000 == 0x20000000))
                    {
                        return Err(ExecError::Fault(13));
                    }
                    let reserved = match selector {
                        0 => written & !0xe005003f,
                        3 => written & !0xfffff018,
                        _ => 0,
                    };
                    let reason = if reserved != 0 {
                        Some("undefined reserved-bit write")
                    } else {
                        None
                    };
                    if let Some(reason) = reason {
                        let mode = if selector == 0 {
                            format!(" PE={} PG={}", written & 1, written >> 31)
                        } else {
                            String::new()
                        };
                        return Err(ExecError::ControlWriteRejected(format!(
                            "{reason}: CS:IP={:04x}:{:08x} opcode=0f 22 modrm={m:02x} CR{selector} GPR={} old={value:08x} written={written:08x}{mode} next-ip={:08x}; write not committed",
                            self.s[CS], self.ip.wrapping_sub(self.instruction_bytes as u32),
                            ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"][(m & 7) as usize],
                            self.ip
                        )));
                    }
                    match selector {
                        // ET is hardwired to 1 on the selected Intel486.
                        0 => self.commit_cr0(written),
                        2 => self.cr2 = written,
                        3 => {
                            self.cr3 = written;
                            self.paging.flush();
                        }
                        _ => unreachable!(),
                    }
                }
            }
            0x01 => {
                let m = self.fetch(b)?;
                let kind = (m >> 3) & 7;
                if kind == 7 {
                    if m >> 6 == 3 {
                        return Err(ExecError::Fault(6));
                    }
                    if self.protected_mode() && self.cpl != 0 {
                        return Err(ExecError::Fault(13));
                    }
                    let Operand::Mem(s, off) = self.decode(b, m, ov)? else {
                        unreachable!()
                    };
                    // Address calculation only: no operand read or page walk,
                    // including nonpresent pages and offsets beyond segment limit.
                    self.paging.invalidate(self.linear(s, off) as u32);
                    return Ok(());
                }
                if self.vm86() && matches!(kind, 2 | 3 | 6) {
                    return Err(ExecError::Fault(13));
                }
                if self.protected_mode() && !self.vm86() && self.cpl != 0 {
                    return Err(ExecError::Unsupported(
                        "unsupported protected system instruction outside CPL0".into(),
                    ));
                }
                if kind == 4 {
                    match self.decode(b, m, ov)? {
                        // Undefined upper bits in operand32 register form are
                        // preserved as a deterministic emulator policy.
                        Operand::Reg(r) => self.r[r] = self.cr0 as u16,
                        Operand::Mem(s, off) => {
                            if !self.protected_mode() && off > 0xfffe {
                                return Err(ExecError::Fault(13));
                            }
                            self.write_mem(b, s, off, self.cr0, 2)?;
                        }
                    }
                    return Ok(());
                }
                if kind == 6 {
                    let o = self.decode(b, m, ov)?;
                    let value = match o {
                        Operand::Reg(r) => self.r[r] as u32,
                        Operand::Mem(s, a) => self.read_mem(b, s, a, 2)?,
                    };
                    self.commit_cr0((self.cr0 & !15) | (value & 15) | (self.cr0 & 1));
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
                // Real-address 486 table operands fault #GP, including SS;
                // protected operands use ordinary descriptor access checks.
                if !self.protected_mode() && off > 0xfffa {
                    return Err(ExecError::Fault(13));
                }
                self.check_access(b, s, off, 6, kind < 2, false)?;
                if kind >= 2 {
                    let limit = self.read_mem(b, s, off, 2)? as u16;
                    let mut base = self.read_mem(b, s, off + 2, 4)?;
                    if !self.operand32 {
                        base &= 0xffffff;
                    }
                    if kind == 2 {
                        self.gdtr_limit = limit;
                        self.gdtr_base = base;
                    } else {
                        self.idtr_limit = limit;
                        self.idtr_base = base;
                    }
                } else {
                    let (limit, base) = if kind == 0 {
                        (self.gdtr_limit, self.gdtr_base)
                    } else {
                        (self.idtr_limit, self.idtr_base)
                    };
                    self.write_mem(b, s, off, limit as u32, 2)?;
                    self.write_mem(b, s, off + 2, base, 4)?;
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
                self.load_segment(b, if op == 0xa1 { 4 } else { 5 }, v)?;
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
                let v = if kind == 0 {
                    self.get(b, o, true)?
                } else {
                    self.get_modify(b, o, true)?
                };
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
                let dst = self.get_modify(b, o, true)?;
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
                let dst = self.get_modify(b, o, w)?;
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
                        // CMPXCHG performs a destination write cycle on mismatch too.
                        self.put(b, o, dst, w)?;
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
        self.flags_upper &= !1; // RF suppresses one instruction, then clears (IRET can reload it).
        self.instruction_bytes = 0;
        let default32 = b.profile == crate::Profile::Pc9821ApDos && self.segment_cache(CS).big();
        self.operand32 = default32;
        self.address32 = default32;
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
                0x66 if b.profile == crate::Profile::Pc9821ApDos => self.operand32 = !default32,
                0x67 if b.profile == crate::Profile::Pc9821ApDos => self.address32 = !default32,
                0x64 | 0x65 | 0xf2 | 0xf3 => {
                    if rep != 0 && rep != c && (matches!(rep, 0x64 | 0x65) || c < 0x66) {
                        return Err(
                            "unsupported V30 conflicting repeat prefixes (emulator limitation)"
                                .into(),
                        );
                    }
                    rep = c;
                }
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
        if matches!(rep, 0x64 | 0x65) && !matches!(op, 0xa6 | 0xa7 | 0xae | 0xaf) {
            return Err(
                "unsupported V30 carry-repeat with non-compare opcode (emulator limitation)".into(),
            );
        }
        // Decode CLTS at any CPL so privilege violations become guest #GP(0),
        // without enabling general execution outside the bounded ring-0 core.
        if self.protected_mode()
            && !self.vm86()
            && self.cpl != 0
            && (op != 0x0f || b.profile != crate::Profile::Pc9821ApDos)
        {
            return Err(ExecError::Unsupported(format!(
                "unsupported protected execution CPL={}; only ring 0 is implemented",
                self.cpl
            )));
        }
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
            0xd8..=0xdf if b.profile == crate::Profile::Pc9821ApDos => {
                let m = self.fetch(b)?;
                // All ESC instructions check EM/TS, including operations whose
                // numerical execution remains unsupported. Non-waiting forms do
                // not check pending numerical exceptions or update FPU pointers.
                if self.cr0 & 0x000c != 0 {
                    return Err(ExecError::Fault(7));
                }
                let memory_store = matches!(op, 0xd9 | 0xdd) && m < 0xc0 && (m >> 3) & 7 == 7;
                if !memory_store && !matches!((op, m), (0xdb, 0xe3) | (0xdf, 0xe0)) {
                    return Err(ExecError::Unsupported(format!(
                        "unimplemented x87 opcode {op:02x} {m:02x}"
                    )));
                }
                if memory_store {
                    let Operand::Mem(s, off) = self.decode(b, m, ov)? else {
                        unreachable!();
                    };
                    // Intel486 real-address contract: #GP even for SS, unlike
                    // the modern SDM. Preflight the entire fixed-width store.
                    if !self.protected_mode() && off > 0xfffe {
                        return Err(ExecError::Fault(13));
                    }
                    let value = if op == 0xd9 {
                        self.fpu.control
                    } else {
                        self.fpu.status
                    };
                    self.write_mem(b, s, off, value as u32, 2)?;
                } else if op == 0xdb {
                    self.fpu = Fpu::default();
                } else {
                    self.r[AX] = self.fpu.status;
                }
                // Preserve architecturally undefined store condition bits as
                // a deterministic policy; do not claim hardware guarantees.
            }
            0x00..=0x3d if op & 7 <= 5 => {
                let kind = op >> 3;
                let w = op & 1 != 0;
                if op & 7 < 4 {
                    let m = self.fetch(b)?;
                    let rm = self.decode(b, m, ov)?;
                    let reg = Operand::Reg(((m >> 3) & 7) as usize);
                    let (dst, src) = if op & 2 == 0 { (rm, reg) } else { (reg, rm) };
                    let a = if kind == 7 {
                        self.get(b, dst, w)?
                    } else {
                        self.get_modify(b, dst, w)?
                    };
                    let v = self.alu(kind, a, self.get(b, src, w)?, w);
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
                let value = self.pop(b)? as u16;
                self.load_segment(b, r, value)?;
                if b.profile == crate::Profile::Pc9801Vm || r == SS {
                    self.inhibit = 1;
                }
                if r == SS {
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
            0x50..=0x57 => {
                let value = self.get(b, Operand::Reg((op & 7) as usize), true)?;
                self.push(b, value)?;
            }
            0x58..=0x5f => {
                let v = self.pop(b)?;
                self.put(b, Operand::Reg((op & 7) as usize), v, true)?;
            }
            0x60 => {
                let sp = self.get(b, Operand::Reg(SP), true)?;
                self.check_stack_sequence(b, 8, true)?;
                for i in [AX, CX, DX, BX, SP, BP, SI, DI] {
                    let value = if i == SP {
                        sp
                    } else {
                        self.get(b, Operand::Reg(i), true)?
                    };
                    self.push(b, value)?;
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
            0x63 if b.profile == crate::Profile::Pc9821ApDos => {
                if self.vm86() {
                    return Err(ExecError::Fault(6));
                }
                if self.protected_mode() {
                    return Err(ExecError::Unsupported(
                        "unsupported protected ARPL privilege instruction".into(),
                    ));
                }
                return Err(ExecError::Fault(6));
            }
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
                let c = if op == 0x69 {
                    let v = self.immediate(b, true)?;
                    self.signed(v, true)
                } else {
                    self.fetch(b)? as i8 as i64
                };
                // Decode the whole instruction before a device-backed operand
                // read, so a truncated immediate cannot change device latches.
                let a = self.signed(self.get(b, o, true)?, true);
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
                let a = if kind == 7 {
                    self.get(b, o, w)?
                } else {
                    self.get_modify(b, o, w)?
                };
                let out = self.alu(kind, a, v, w);
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
                let a = self.get_modify(b, o, w)?;
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
                    self.load_segment(b, s, value as u16)?;
                    if s == SS {
                        self.trap_suppressed = true;
                    }
                }
                // NEC V20/V30 User's Manual 6-4: documented segment MOVs
                // in either direction protect the following instruction from INT.
                // Leave undocumented MOV-to-CS and Ap non-SS timing unchanged.
                if (b.profile == crate::Profile::Pc9801Vm && (op == 0x8c || s != CS))
                    || (op == 0x8e && s == SS)
                {
                    self.inhibit = 1;
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
                let next_sp = self.stack_pointer().wrapping_add(self.bytes(true));
                let old_sp = self.stack_pointer();
                self.set_stack_pointer(next_sp);
                let o = self.decode(b, m, ov)?;
                self.set_stack_pointer(old_sp);
                // Validate the complete destination before a stack device read.
                if let Operand::Mem(s, a) = o {
                    self.check_access(b, s, a, self.bytes(true), true, false)?;
                }
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
                self.far_transfer(b, seg, off, true, false)?;
            }
            0x9b => {
                if b.profile == crate::Profile::Pc9821ApDos {
                    // Intel486 PRM WAIT, p.26-280: EM alone does not trap WAIT.
                    if self.cr0 & 0x000a == 0x000a {
                        return Err(ExecError::Fault(7));
                    }
                    if self.fpu.status & 0x80 != 0 {
                        return Err(ExecError::Unsupported(
                            "unsupported x87 WAIT pending numeric exception".into(),
                        ));
                    }
                }
            }
            0x9c => {
                self.check_vm86_iopl()?;
                let v = if b.profile == crate::Profile::Pc9801Vm {
                    (self.flags | 0xf002) as u32
                } else {
                    (self.flags | 2) as u32 | (((self.flags_upper & 4) as u32) << 16)
                };
                self.push(b, v)?;
            }
            0x9d => {
                self.check_vm86_iopl()?;
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
                    let value = self.get(b, Operand::Reg(0), w)?;
                    self.put(b, o, value, w)?
                }
            }
            0xa4..=0xa7 | 0xaa..=0xaf | 0x6c..=0x6f => {
                let w = op & 1 != 0;
                if rep != 0 && self.index(CX) == 0 {
                    return Ok(());
                }
                if matches!(op, 0x6c..=0x6f) {
                    self.check_io(b, self.r[DX], self.bytes(w))?;
                }
                let src = Operand::Mem(ov.unwrap_or(DS), self.index(SI));
                let dst = Operand::Mem(ES, self.index(DI));
                let a = self.get(b, Operand::Reg(0), w)?;
                if matches!(op & 0xfe, 0xa4 | 0xa6) {
                    // Validate both full operands before either bus read: a
                    // source read can change device latches even if ES faults.
                    self.check_mem(b, ov.unwrap_or(DS), self.index(SI), self.bytes(w))?;
                    self.check_access(
                        b,
                        ES,
                        self.index(DI),
                        self.bytes(w),
                        op & 0xfe == 0xa4,
                        false,
                    )?;
                }
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
                        self.check_access(b, ES, self.index(DI), self.bytes(w), true, false)?;
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
                    let repeat = match rep {
                        // The first comparison is unconditional; only its resulting CF
                        // controls the next element. ZF is irrelevant to REPC/REPNC.
                        0x64 | 0x65 => (self.flags & CF != 0) == (rep == 0x65),
                        _ => !compare || (self.flags & ZF != 0) == (rep == 0xf3),
                    };
                    if self.index(CX) != 0 && repeat {
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
                let v = self.shift((m >> 3) & 7, self.get_modify(b, o, w)?, n, w);
                self.put(b, o, v, w)?;
            }
            0xc2 | 0xc3 | 0xca | 0xcb => {
                let n = if op & 1 == 0 { self.word(b)? } else { 0 };
                if op & 8 != 0 {
                    // A missing selector slot must fault before reading an offset
                    // from a device-backed stack, which can change device latches.
                    self.check_stack_sequence(b, 2, false)?;
                }
                let off = self.pop(b)?;
                if op & 8 != 0 {
                    let seg = self.pop(b)? as u16;
                    self.far_transfer(b, seg, off, false, true)?;
                } else {
                    self.branch(off)?;
                }
                self.set_stack_pointer(self.stack_pointer().wrapping_add(n as u32));
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
                let allocated = self.stack_offset(
                    self.stack_pointer()
                        .wrapping_sub(bytes * (1 + level as u32))
                        .wrapping_sub(size as u32),
                );
                self.check_access(b, SS, allocated, bytes, true, false)?;
                let original_bp = self.get(b, Operand::Reg(BP), true)?;
                // Operand size chooses BP/EBP and its wrapping; SS.B only
                // chooses SP/ESP for the pushes and allocation. 67 has no effect.
                let chain_mask = self.mask(true);
                for i in 1..level {
                    let bp = original_bp.wrapping_sub(bytes * i as u32) & chain_mask;
                    self.check_mem(b, SS, bp, bytes)?;
                }
                self.push(b, original_bp)?;
                let frame = self.get(b, Operand::Reg(SP), true)?;
                for i in 1..level {
                    let bp = original_bp.wrapping_sub(bytes * i as u32) & chain_mask;
                    let v = self.read_mem(b, SS, bp, bytes)?;
                    self.push(b, v)?;
                }
                if level > 0 {
                    self.push(b, frame)?;
                }
                self.put(b, Operand::Reg(BP), frame, true)?;
                self.set_stack_pointer(self.stack_pointer().wrapping_sub(size as u32));
            }
            0xc9 => {
                self.set_stack_pointer(if self.segment_cache(SS).big() {
                    self.reg32(BP)
                } else {
                    self.r[BP] as u32
                });
                let v = self.pop(b)?;
                self.put(b, Operand::Reg(BP), v, true)?;
            }
            0xcc => self.deliver_interrupt(b, 3)?,
            0xcd => {
                let n = self.fetch(b)?;
                self.check_vm86_iopl()?;
                self.deliver_interrupt(b, n)?;
            }
            0xce => {
                if self.flags & OF != 0 {
                    self.deliver_interrupt(b, 4)?
                }
            }
            0xcf => self.iret(b)?,
            0xd4 | 0xd5 => {
                let encoded_radix = self.fetch(b)?;
                let radix = if b.profile == crate::Profile::Pc9801Vm {
                    10
                } else {
                    encoded_radix
                };
                if op == 0xd4 && radix == 0 {
                    if self.protected_mode() {
                        return Err(ExecError::Fault(0));
                    }
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
                self.check_io(b, p, self.bytes(w))?;
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
                self.far_transfer(b, seg, off, false, false)?;
            }
            0xeb => {
                let d = self.fetch(b)? as i8 as u32;
                self.branch(self.ip.wrapping_add(d))?;
            }
            0xf4 => {
                if self.protected_mode() && self.cpl != 0 {
                    return Err(ExecError::Fault(13));
                }
                self.halted = true;
            }
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
                // Decode TEST completely before a device-backed operand read.
                let immediate = if matches!((m >> 3) & 7, 0 | 1) {
                    self.immediate(b, w)?
                } else {
                    0
                };
                let v = if matches!((m >> 3) & 7, 2 | 3) {
                    self.get_modify(b, o, w)?
                } else {
                    self.get(b, o, w)?
                };
                match (m >> 3) & 7 {
                    0 | 1 => {
                        self.alu(4, v, immediate, w);
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
            0xfa => {
                self.check_interrupt_privilege()?;
                self.flags &= !IF;
            }
            0xfb => {
                self.check_interrupt_privilege()?;
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
                if matches!((m >> 3) & 7, 3 | 5) {
                    if let Operand::Mem(s, a) = o {
                        self.check_mem(b, s, a, self.bytes(true) + 2)?;
                    }
                }
                match (m >> 3) & 7 {
                    // CALL/PUSH must validate every stack slot before reading
                    // an operand or far pointer that may change device latches.
                    2 | 6 => self.check_stack_sequence(b, 1, true)?,
                    3 if matches!(o, Operand::Mem(..)) => self.check_stack_sequence(b, 2, true)?,
                    _ => {}
                }
                let v = if m & 0x38 <= 8 {
                    self.get_modify(b, o, w)?
                } else {
                    self.get(b, o, w)?
                };
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
                            self.far_transfer(b, seg, v, m & 0x38 == 0x18, false)?;
                        } else {
                            return Err(ExecError::Fault(6));
                        }
                    }
                    4 => self.branch(v)?,
                    6 => self.push(b, v)?,
                    _ => return Err(ExecError::Fault(6)),
                }
            }
            // V30 bit operations, packed-BCD addition and memory nibble rotations.
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
                } else if ext == 0x20 {
                    // ADD4S has no ModR/M. NEC's byte-offset diagram is
                    // interpreted as increasing offsets, independent of DF,
                    // without changing SI/DI/CL. Odd counts include both nibbles
                    // of the last byte. Only CF/ZF are architecturally defined;
                    // preserving AF/OF/PF/SF is deterministic emulator policy.
                    let source = ov.unwrap_or(DS);
                    let count = (self.byte(CX) as u32).div_ceil(2);
                    let mut carry = 0;
                    let mut zero = true;
                    for i in 0..count {
                        let a = self.read_mem(b, source, self.r[SI] as u32 + i, 1)?;
                        let d = self.read_mem(b, ES, self.r[DI] as u32 + i, 1)?;
                        let low = (a & 15) + (d & 15) + carry;
                        let high = (a >> 4) + (d >> 4) + u32::from(low >= 10);
                        let result = ((high % 10) << 4) | (low % 10);
                        carry = u32::from(high >= 10);
                        zero &= result == 0;
                        self.write_mem(b, ES, self.r[DI] as u32 + i, result, 1)?;
                    }
                    self.flag(CF, carry != 0);
                    self.flag(ZF, zero);
                } else if matches!(ext, 0x28 | 0x2a) {
                    let m = self.fetch(b)?;
                    if m >> 6 == 3 || m & 0x38 != 0 {
                        return Err(ExecError::Unsupported(format!(
                            "unsupported V30 nibble rotation 0F {ext:02x} ModR/M {m:02x} (emulator limitation: memory /0 required)"
                        )));
                    }
                    let o = self.decode(b, m, ov)?;
                    // Snapshot both inputs before writing either output. AL's upper
                    // nibble is hardware-undefined; preserving it is emulator policy.
                    let d = self.get(b, o, false)? as u8;
                    let al = self.byte(AX);
                    let (memory, low) = if ext == 0x28 {
                        ((d << 4) | (al & 0x0f), d >> 4)
                    } else {
                        ((al << 4) | (d >> 4), d & 0x0f)
                    };
                    self.put(b, o, memory as u32, false)?;
                    self.set_byte(AX, (al & 0xf0) | low);
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

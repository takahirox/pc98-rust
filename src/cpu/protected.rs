//! Original paging-disabled i386/i486 segmentation and same-ring gate delivery.
//! Contracts: Intel 80386 PRM chapters 6, 9, 14, 16; Intel486 PRM MOV CR.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum GateSource {
    Software,
    External,
    Trap,
    Fault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentCache {
    pub base: u32,
    pub limit: u32,
    pub access: u8,
    /// Descriptor G/D(B)/reserved/AVL nibble, in architectural bit positions.
    pub attributes: u8,
}
impl SegmentCache {
    pub fn parse(raw: [u8; 8]) -> Self {
        let limit = u16::from_le_bytes([raw[0], raw[1]]) as u32 | ((raw[6] as u32 & 15) << 16);
        Self {
            base: u16::from_le_bytes([raw[2], raw[3]]) as u32
                | ((raw[4] as u32) << 16)
                | ((raw[7] as u32) << 24),
            limit: if raw[6] & 0x80 != 0 {
                (limit << 12) | 0xfff
            } else {
                limit
            },
            access: raw[5],
            attributes: raw[6] & 0xf0,
        }
    }
    pub fn big(self) -> bool {
        self.attributes & 0x40 != 0
    }
    fn code(self) -> bool {
        self.access & 8 != 0
    }
    fn dpl(self) -> u8 {
        (self.access >> 5) & 3
    }
    fn conforming(self) -> bool {
        self.code() && self.access & 4 != 0
    }
    fn contains(self, offset: u32, bytes: u32) -> bool {
        let Some(end) = offset.checked_add(bytes - 1) else {
            return false;
        };
        if !self.code() && self.access & 4 != 0 {
            offset > self.limit && end <= if self.big() { u32::MAX } else { 0xffff }
        } else {
            end <= self.limit
        }
    }
}
impl Cpu {
    pub fn protected_mode(&self) -> bool {
        self.cr0 & 1 != 0
    }
    pub fn segment_cache(&self, s: usize) -> SegmentCache {
        self.segment_caches[s].unwrap_or(SegmentCache {
            base: (self.segment(s) as u32) << 4,
            limit: 0xffff,
            access: if s == CS { 0x9b } else { 0x93 },
            attributes: 0,
        })
    }
    pub(super) fn commit_cr0(&mut self, value: u32) {
        if !self.protected_mode() && value & 1 != 0 {
            // PE changes checks, not hidden segment state. The first far transfer
            // reloads CS; data caches remain usable until explicitly reloaded.
            for s in 0..6 {
                self.segment_caches[s] = Some(self.segment_cache(s));
            }
            self.cpl = 0;
        }
        self.cr0 = value | 0x10; // Intel486 ET is hardwired.
    }
    pub(super) fn linear(&self, s: usize, off: u32) -> usize {
        self.segment_cache(s).base.wrapping_add(off) as usize
    }
    pub(super) fn check_access(
        &self,
        b: &Bus,
        s: usize,
        off: u32,
        bytes: u32,
        write: bool,
        execute: bool,
    ) -> ExecResult<()> {
        if b.profile == crate::Profile::Pc9801Vm {
            return Ok(());
        }
        let d = self.segment_cache(s);
        let fault = if s == SS { 12 } else { 13 };
        if !d.contains(off, bytes) {
            return Err(ExecError::Fault(fault));
        }
        if self.protected_mode()
            && (d.access & 0x80 == 0
                || (execute && !d.code())
                || (!execute && d.code() && (write || d.access & 2 == 0))
                || (write && d.access & 2 == 0))
        {
            return Err(ExecError::Fault(fault));
        }
        Ok(())
    }
    /// Resolve a complete descriptor span before fetching any bus bytes.
    fn descriptor_address(&self, selector: u16) -> ExecResult<u32> {
        let error = (selector & !3) as u32;
        let (base, limit) = if selector & 4 == 0 {
            (self.gdtr_base, self.gdtr_limit as u32)
        } else {
            let Some(ldt) = self.ldtr_cache else {
                return Err(ExecError::ProtectedFault(13, error));
            };
            (ldt.base, ldt.limit)
        };
        let index = (selector & !7) as u32;
        if selector & !3 == 0 || index + 7 > limit {
            return Err(ExecError::ProtectedFault(13, error));
        }
        Ok(base.wrapping_add(index))
    }
    /// Fetch descriptor bytes through the same PC-98 physical/A20 bus as operands.
    pub(super) fn descriptor(&self, b: &Bus, selector: u16) -> ExecResult<(SegmentCache, u32)> {
        let address = self.descriptor_address(selector)?;
        let raw = std::array::from_fn(|i| b.read(address.wrapping_add(i as u32) as usize));
        Ok((SegmentCache::parse(raw), address))
    }
    fn accessed(b: &mut Bus, d: SegmentCache, address: u32) -> SegmentCache {
        if d.access & 1 == 0 {
            b.write(address.wrapping_add(5) as usize, d.access | 1);
        }
        SegmentCache {
            access: d.access | 1,
            ..d
        }
    }
    pub(super) fn load_segment(&mut self, b: &mut Bus, s: usize, selector: u16) -> ExecResult<()> {
        if !self.protected_mode() {
            self.set_segment(s, selector);
            // Real-address loads replace base while retaining cached limits/B.
            if let Some(d) = &mut self.segment_caches[s] {
                d.base = (selector as u32) << 4;
            }
            return Ok(());
        }
        if s != SS && selector & !3 == 0 {
            self.set_segment(s, selector);
            self.segment_caches[s] = Some(SegmentCache {
                base: 0,
                limit: 0,
                access: 0,
                attributes: 0,
            });
            return Ok(());
        }
        let error = (selector & !3) as u32;
        let (d, address) = self.descriptor(b, selector)?;
        if d.access & 0x10 == 0 || d.attributes & 0x20 != 0 {
            return Err(ExecError::ProtectedFault(13, error));
        }
        let rpl = (selector & 3) as u8;
        if s == SS {
            if d.code() || d.access & 2 == 0 || d.dpl() != self.cpl || rpl != self.cpl {
                return Err(ExecError::ProtectedFault(13, error));
            }
        } else if (d.code() && d.access & 2 == 0)
            || (!d.conforming() && self.cpl.max(rpl) > d.dpl())
        {
            return Err(ExecError::ProtectedFault(13, error));
        }
        if d.access & 0x80 == 0 {
            return Err(ExecError::ProtectedFault(
                if s == SS { 12 } else { 11 },
                error,
            ));
        }
        let d = Self::accessed(b, d, address);
        self.set_segment(s, selector);
        self.segment_caches[s] = Some(d);
        Ok(())
    }
    pub(super) fn code_target(
        &self,
        b: &Bus,
        selector: u16,
        off: u32,
        returning: bool,
    ) -> ExecResult<(SegmentCache, u32)> {
        let error = (selector & !3) as u32;
        let (d, address) = self.descriptor(b, selector)?;
        if d.access & 0x10 == 0 {
            let operation = match d.access & 15 {
                1 | 3 | 9 | 11 => Some("hardware task switch"),
                5 => Some("task gate transfer"),
                4 | 12 => Some("call gate/privilege-stack transfer"),
                _ => None,
            };
            if let Some(operation) = operation {
                return Err(ExecError::Unsupported(format!(
                    "unsupported protected {operation} selector={selector:04x} descriptor={d:?}"
                )));
            }
            return Err(ExecError::ProtectedFault(13, error));
        }
        if !d.code() || d.attributes & 0x20 != 0 {
            return Err(ExecError::ProtectedFault(13, error));
        }
        if returning && selector & 3 > self.cpl as u16 {
            return Err(ExecError::Unsupported(format!(
                "unsupported protected privilege return/TSS selector={selector:04x}"
            )));
        }
        if (returning && selector & 3 != self.cpl as u16)
            || (!returning && !d.conforming() && selector & 3 > self.cpl as u16)
            || (d.conforming() && d.dpl() > self.cpl)
            || (!d.conforming() && d.dpl() != self.cpl)
        {
            return Err(ExecError::ProtectedFault(13, error));
        }
        if d.access & 0x80 == 0 {
            return Err(ExecError::ProtectedFault(11, error));
        }
        if off > d.limit {
            return Err(ExecError::Fault(13));
        }
        Ok((d, address))
    }
    pub(super) fn far_transfer(
        &mut self,
        b: &mut Bus,
        selector: u16,
        off: u32,
        call: bool,
        returning: bool,
    ) -> ExecResult<()> {
        let off = if self.operand32 { off } else { off & 0xffff };
        let target = if self.protected_mode() {
            Some(self.code_target(b, selector, off, returning)?)
        } else {
            if off > 0xffff {
                return Err(ExecError::Fault(13));
            }
            None
        };
        if call {
            self.check_stack_sequence(b, 2, true)?;
            self.push(b, self.s[CS] as u32)?;
            self.push(b, self.ip)?;
        }
        if let Some((d, address)) = target {
            self.segment_caches[CS] = Some(Self::accessed(b, d, address));
            self.s[CS] = (selector & !3) | self.cpl as u16;
        } else {
            self.reload_real_code(selector);
        }
        self.ip = off;
        Ok(())
    }
    /// Synthetic firmware tail transfers obey the same real CS reload contract.
    pub(crate) fn reload_real_code(&mut self, selector: u16) {
        self.s[CS] = selector;
        if let Some(old) = self.segment_caches[CS] {
            self.segment_caches[CS] = Some(SegmentCache {
                base: (selector as u32) << 4,
                limit: old.limit,
                access: 0x9b,
                attributes: 0,
            });
        }
        self.cpl = 0;
    }
    pub(super) fn stack_pointer(&self) -> u32 {
        if self.segment_cache(SS).big() {
            self.reg32(SP)
        } else {
            self.r[SP] as u32
        }
    }
    pub(super) fn stack_offset(&self, value: u32) -> u32 {
        if self.segment_cache(SS).big() {
            value
        } else {
            value & 0xffff
        }
    }
    pub(super) fn set_stack_pointer(&mut self, value: u32) {
        if self.segment_cache(SS).big() {
            self.set_reg32(SP, value);
        } else {
            self.r[SP] = value as u16;
        }
    }
    pub(super) fn protected_interrupt(
        &mut self,
        b: &mut Bus,
        n: u8,
        source: GateSource,
        error: Option<u32>,
    ) -> ExecResult<()> {
        let entry = n as u32 * 8;
        let external = u32::from(source == GateSource::External);
        let idt_error = entry | 2 | external;
        if entry + 7 > self.idtr_limit as u32 {
            return Err(ExecError::ProtectedFault(13, idt_error));
        }
        let raw: [u8; 8] = std::array::from_fn(|i| {
            b.read(self.idtr_base.wrapping_add(entry).wrapping_add(i as u32) as usize)
        });
        let access = raw[5];
        let kind = access & 31;
        if kind == 5 {
            return Err(ExecError::Unsupported(format!(
                "unsupported protected IDT task gate vector={n:02x}"
            )));
        }
        if !matches!(kind, 6 | 7 | 14 | 15) || raw[4] != 0 {
            return Err(ExecError::ProtectedFault(13, idt_error));
        }
        if source == GateSource::Software && self.cpl > (access >> 5) & 3 {
            return Err(ExecError::ProtectedFault(13, idt_error));
        }
        if access & 0x80 == 0 {
            return Err(ExecError::ProtectedFault(11, idt_error));
        }
        let selector = u16::from_le_bytes([raw[2], raw[3]]);
        let off = u16::from_le_bytes([raw[0], raw[1]]) as u32
            | if kind & 8 != 0 {
                (u16::from_le_bytes([raw[6], raw[7]]) as u32) << 16
            } else {
                0
            };
        if self.cpl != 0 {
            let (target, _) = self.descriptor(b, selector & !3).map_err(|e| match e {
                ExecError::ProtectedFault(n, code) => ExecError::ProtectedFault(n, code | external),
                other => other,
            })?;
            if target.access & 0x18 == 0x18 && !target.conforming() && target.dpl() < self.cpl {
                return Err(ExecError::Unsupported(format!(
                    "unsupported protected interrupt privilege/TSS stack switch vector={n:02x} selector={selector:04x}"
                )));
            }
        }
        let (d, address) = self
            .code_target(b, selector & !3, off, false)
            .map_err(|e| match e {
                ExecError::ProtectedFault(n, code) => ExecError::ProtectedFault(n, code | external),
                other => other,
            })?;
        let bytes = if kind & 8 != 0 { 4 } else { 2 };
        let count = if error.is_some() { 4 } else { 3 };
        // Validate the entire frame before the first stack write or accessed bit.
        for i in 1..=count {
            self.check_access(
                b,
                SS,
                self.stack_offset(self.stack_pointer().wrapping_sub(i * bytes)),
                bytes,
                true,
                false,
            )?;
        }
        self.push_width(
            b,
            self.flags as u32
                | ((self.flags_upper as u32) << 16)
                | if source == GateSource::Fault {
                    0x10000
                } else {
                    0
                },
            bytes,
        )?;
        self.push_width(b, self.s[CS] as u32, bytes)?;
        self.push_width(b, self.ip, bytes)?;
        if let Some(error) = error {
            self.push_width(b, error, bytes)?;
        }
        self.segment_caches[CS] = Some(Self::accessed(b, d, address));
        self.s[CS] = (selector & !3) | self.cpl as u16;
        self.ip = off;
        self.flags &= !(TF | 0x4000); // NT, TF cleared for both gate kinds.
        self.flags_upper &= !1; // RF
        if kind & 1 == 0 {
            self.flags &= !IF;
        }
        self.trap_suppressed = true;
        self.halted = false;
        if source == GateSource::Fault {
            self.last_fault = Some((n, error.unwrap_or(0)));
        }
        Ok(())
    }
    pub(super) fn protected_event(
        &mut self,
        b: &mut Bus,
        n: u8,
        error: Option<u32>,
        source: GateSource,
    ) -> ExecResult<()> {
        let contributory = |n| matches!(n, 0 | 10..=13);
        let mut vector = n;
        let mut code = error;
        let mut source = source;
        for _ in 0..3 {
            match self.protected_interrupt(b, vector, source, code) {
                Ok(()) => return Ok(()),
                Err(e @ (ExecError::Fault(_) | ExecError::ProtectedFault(_, _))) => {
                    if vector == 8 && source == GateSource::Fault {
                        self.shutdown = true;
                        return Err(ExecError::Unsupported(format!(
                            "protected triple fault/shutdown while delivering #DF: {e:?}"
                        )));
                    }
                    let (next, next_code) = match e {
                        ExecError::Fault(n) => (n, 0),
                        ExecError::ProtectedFault(n, code) => (n, code),
                        _ => unreachable!(),
                    };
                    if source == GateSource::Fault && contributory(vector) && contributory(next) {
                        vector = 8;
                        code = Some(0);
                    } else {
                        vector = next;
                        code = matches!(next, 8 | 10..=14 | 17).then_some(next_code);
                    }
                    source = GateSource::Fault;
                }
                Err(e) => return Err(e),
            }
        }
        self.shutdown = true;
        Err(ExecError::Unsupported(
            "protected nested-fault delivery limit/shutdown".into(),
        ))
    }
    /// Bounded, read-only CPU descriptor metadata for the offline trace. Never
    /// reads device windows or a decoded guest memory operand.
    pub fn task_descriptor_metadata(&self, b: &Bus, selector: u16) -> Option<SegmentCache> {
        if selector & 4 != 0 {
            return None;
        }
        let address = self.descriptor_address(selector).ok()?;
        if !(0..8).all(|i| b.is_plain_ram(address.wrapping_add(i))) {
            return None;
        }
        self.descriptor(b, selector).ok().map(|(d, _)| d)
    }
    fn load_task_register(&mut self, b: &mut Bus, selector: u16, m: u8) -> ExecResult<()> {
        let error = (selector & !3) as u32;
        // Null selectors and LDT selectors cannot name a TSS. RPL/DPL do not
        // participate in LTR's privilege check; CPL must already be zero.
        if selector & !3 == 0 || selector & 4 != 0 {
            return Err(ExecError::ProtectedFault(13, error));
        }
        let address = self.descriptor_address(selector)?;
        // The physical bus has no write faults: ROM ignores writes and device
        // reads may change latches. Stop before *any* such access rather than
        // pretend a descriptor became busy or perform a partial device RMW.
        if !(0..8).all(|i| b.is_plain_ram(address.wrapping_add(i))) {
            return Err(ExecError::Unsupported(format!(
                "unsupported protected LTR descriptor bus access opcode=0f 00 modrm={m:02x} selector={selector:04x} descriptor={address:08x}; requires writable ordinary RAM"
            )));
        }
        let (d, _) = self.descriptor(b, selector)?;
        if !matches!(d.access & 31, 1 | 9) {
            return Err(ExecError::ProtectedFault(13, error));
        }
        if d.access & 0x80 == 0 {
            return Err(ExecError::ProtectedFault(11, error));
        }
        // LTR never reads the TSS or checks its minimum size. The 44/104-byte
        // size checks belong to a task switch (Intel PRM 7.2/9.8.10), not LTR.
        // All fallible checks precede the busy write and the cache/selector
        // commit. One interpreter instruction is indivisible to our bus/IRQs.
        b.write(address.wrapping_add(5) as usize, d.access | 2);
        self.tr = selector;
        self.tr_cache = Some(SegmentCache {
            access: d.access | 2,
            ..d
        });
        Ok(())
    }
    pub(super) fn system_group0(&mut self, b: &mut Bus, ov: Option<usize>) -> ExecResult<()> {
        if !self.protected_mode() {
            return Err(ExecError::Fault(6));
        }
        let m = self.fetch(b)?;
        let kind = (m >> 3) & 7;
        if kind >= 6 {
            return Err(ExecError::Fault(6));
        }
        if matches!(kind, 2 | 3) && self.cpl != 0 {
            return Err(ExecError::Fault(13));
        }
        let o = self.decode(b, m, ov)?;
        match kind {
            0 => match o {
                Operand::Reg(r) => self.put_register(r, self.ldtr as u32),
                Operand::Mem(s, a) => self.write_mem(b, s, a, self.ldtr as u32, 2)?,
            },
            2 => {
                let selector = match o {
                    Operand::Reg(r) => self.r[r],
                    Operand::Mem(s, a) => self.read_mem(b, s, a, 2)? as u16,
                };
                if selector & !3 == 0 {
                    self.ldtr = selector;
                    self.ldtr_cache = None;
                    return Ok(());
                }
                if selector & 4 != 0 {
                    return Err(ExecError::ProtectedFault(13, (selector & !3) as u32));
                }
                let (d, _) = self.descriptor(b, selector)?;
                if d.access & 31 != 2 {
                    return Err(ExecError::ProtectedFault(13, (selector & !3) as u32));
                }
                if d.access & 0x80 == 0 {
                    return Err(ExecError::ProtectedFault(11, (selector & !3) as u32));
                }
                self.ldtr = selector;
                self.ldtr_cache = Some(d);
            }
            1 => match o {
                // Target-era STR is always a word, including with 66 or CS.D.
                Operand::Reg(r) => self.r[r] = self.tr,
                Operand::Mem(s, a) => self.write_mem(b, s, a, self.tr as u32, 2)?,
            },
            3 => {
                let selector = match o {
                    Operand::Reg(r) => self.r[r],
                    Operand::Mem(s, a) => {
                        self.check_mem(b, s, a, 2)?;
                        if (0..2).any(|i| b.read_has_side_effects(self.linear(s, a + i) as u32)) {
                            return Err(ExecError::Unsupported(format!(
                                "unsupported protected LTR source bus access opcode=0f 00 modrm={m:02x}; side-effecting device read"
                            )));
                        }
                        self.read_mem(b, s, a, 2)? as u16
                    }
                };
                self.load_task_register(b, selector, m)?;
            }
            _ => {
                return Err(ExecError::Unsupported(
                    "unsupported protected descriptor query instruction".into(),
                ))
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group0_privilege_check_precedes_source_read_and_str_is_unprivileged() {
        let mut b = Bus::new(crate::Profile::Pc9821ApDos);
        let mut c = Cpu {
            cr0: 0x11,
            cpl: 3,
            tr: 0x23,
            ..Cpu::default()
        };
        c.s[CS] = 0x1000;
        // Test the instruction handler directly: general nonzero-CPL execution
        // remains deliberately unavailable in execute(), independent of LTR.
        b.mem[0x10000] = 0x1d; // LTR with an incomplete disp32 source
        c.address32 = true;
        assert!(matches!(
            c.system_group0(&mut b, None),
            Err(ExecError::Fault(13))
        ));
        c.ip = 0;
        b.mem[0x10000] = 0xc8; // STR AX, no privilege requirement of its own
        c.set_reg32(AX, 0xabcdffff);
        c.system_group0(&mut b, None).unwrap();
        assert_eq!(c.reg32(AX), 0xabcd0023);
    }
}

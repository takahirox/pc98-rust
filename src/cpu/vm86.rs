//! Original i386/i486 virtual-8086 transitions and I/O protection.
//! Intel486 PRM (1992), chapters 8, 9, 23 and IRET/POPF instruction entries.
//! No CR4, VME/PVI, virtual interrupt flags or hardware task switching.
use super::*;

impl Cpu {
    pub fn vm86(&self) -> bool {
        self.protected_mode() && self.flags_upper & 2 != 0
    }
    pub fn eflags(&self) -> u32 {
        self.flags as u32 | ((self.flags_upper as u32) << 16)
    }
    /// Read-only, bounded CPU metadata. Never read TSS or frame bytes through
    /// MMIO, update A/D/TLB, or dump a guest stack. Bounds/mapping observations
    /// are diagnostic hints, not a replacement for architectural validation.
    fn diagnostic_system_word(&self, b: &Bus, linear: u32) -> Option<u32> {
        let mut bytes = [0; 4];
        for (i, byte) in bytes.iter_mut().enumerate() {
            let physical = self.peek_translation(b, linear.wrapping_add(i as u32))?;
            if !b.is_plain_ram(physical) {
                return None;
            }
            *byte = b.read(physical as usize);
        }
        Some(u32::from_le_bytes(bytes))
    }
    pub fn monitor_stack_metadata(&self, b: &Bus) -> Option<(u16, u32)> {
        let tss = self.tr_cache?;
        if tss.access & 0x9f != 0x8b || tss.limit < 9 {
            return None;
        }
        let esp = self.diagnostic_system_word(b, tss.base.wrapping_add(4))?;
        // The full word slot is bounded separately; do not require reserved
        // high SS0 bytes to be readable when the TSS ends at offset 9.
        let mut ss = [0; 2];
        for (i, byte) in ss.iter_mut().enumerate() {
            let physical = self.peek_translation(b, tss.base.wrapping_add(8 + i as u32))?;
            if !b.is_plain_ram(physical) {
                return None;
            }
            *byte = b.read(physical as usize);
        }
        Some((u16::from_le_bytes(ss), esp))
    }
    pub fn iret_frame_metadata(&self, b: &Bus, operand32: bool) -> (u16, u32, u32, bool) {
        let sp = self.stack_pointer();
        let mut bytes = if operand32 { 12 } else { 6 };
        if operand32
            && self.protected_mode()
            && !self.vm86()
            && self.cpl == 0
            && sp
                .checked_add(11)
                .is_some_and(|end| end <= self.segment_cache(SS).limit)
            && self
                .diagnostic_system_word(b, self.linear(SS, sp + 8) as u32)
                .is_some_and(|f| f & 0x20000 != 0)
        {
            bytes = 36;
        }
        let in_bounds = sp
            .checked_add(bytes - 1)
            .is_some_and(|end| end <= self.segment_cache(SS).limit);
        (self.s[SS], sp, bytes, in_bounds)
    }
    pub(super) fn load_vm86_segment(&mut self, s: usize, selector: u16) {
        self.set_segment(s, selector);
        self.segment_caches[s] = Some(SegmentCache {
            base: (selector as u32) << 4,
            limit: 0xffff,
            access: if s == CS { 0xfb } else { 0xf3 },
            attributes: 0,
        });
    }
    pub(super) fn check_vm86_iopl(&self) -> ExecResult<()> {
        if self.vm86() && self.flags & 0x3000 != 0x3000 {
            return Err(ExecError::Fault(13));
        }
        Ok(())
    }
    pub(super) fn check_interrupt_privilege(&self) -> ExecResult<()> {
        if self.protected_mode() && self.cpl > ((self.flags >> 12) & 3) as u8 {
            return Err(ExecError::Fault(13));
        }
        Ok(())
    }
    /// VM86 always consults the bitmap, including at IOPL=3. Intel486 reads
    /// TWO bytes even for byte I/O; the final all-ones sentinel prevents wide
    /// operations wrapping from FFFF to 0000 from bypassing protection.
    pub(super) fn check_io(&self, b: &mut Bus, port: u16, bytes: u32) -> ExecResult<()> {
        if !self.protected_mode() || (!self.vm86() && self.cpl <= ((self.flags >> 12) & 3) as u8) {
            return Ok(());
        }
        let denied = || ExecError::Fault(13);
        let tss = self.tr_cache.ok_or_else(denied)?;
        if tss.access & 0x9f != 0x8b || self.tr & !3 == 0 || self.tr & 4 != 0 || tss.limit < 103 {
            return Err(denied());
        }
        let base = u16::from_le_bytes(self.read_system::<2>(b, tss.base.wrapping_add(102))?) as u32;
        let offset = base + (port as u32 / 8);
        if offset + 1 > tss.limit {
            return Err(denied());
        }
        let bits = u16::from_le_bytes(self.read_system::<2>(b, tss.base.wrapping_add(offset))?);
        if (bits >> (port & 7)) & ((1 << bytes) - 1) != 0 {
            return Err(denied());
        }
        Ok(())
    }
    pub(super) fn iret(&mut self, b: &mut Bus) -> ExecResult<()> {
        let virtual_return = self.vm86();
        self.check_vm86_iopl()?; // before any stack bus access, even with NT set
        if self.protected_mode() && !virtual_return && self.flags & 0x4000 != 0 {
            return Err(ExecError::Unsupported(
                "unsupported protected IRET.NT hardware task return".into(),
            ));
        }
        // A frame is contiguous within the current stack segment. Do not wrap
        // individual pops across a segment limit while validating IRET.
        self.check_access(
            b,
            SS,
            self.stack_pointer(),
            self.bytes(true) * 3,
            false,
            false,
        )?;
        let sp = self.stack_pointer();
        let bytes = self.bytes(true);
        let off = self.read_mem(b, SS, sp, bytes)?;
        let seg = self.read_mem(b, SS, sp + bytes, bytes)? as u16;
        let flags = self.read_mem(b, SS, sp + 2 * bytes, bytes)?;
        if self.protected_mode()
            && !virtual_return
            && self.operand32
            && self.cpl == 0
            && flags & 0x20000 != 0
        {
            self.check_access(b, SS, sp, 36, false, false)?;
            if off > 0xffff {
                return Err(ExecError::Fault(13));
            }
            let mut frame = [0; 6];
            for (i, value) in frame.iter_mut().enumerate() {
                *value = self.read_mem(b, SS, sp + 12 + i as u32 * 4, 4)?;
            }
            // All nine dwords validated/read before replacing any architectural
            // register. SS:ESP are raw VM86 values, never GDT selectors.
            self.flags = (flags as u16 & 0x7fd5) | 2;
            self.flags_upper = ((flags >> 16) as u16) & 7;
            self.cpl = 3;
            self.ip = off;
            self.set_reg32(SP, frame[0]);
            for (s, selector) in [
                (CS, seg),
                (SS, frame[1] as u16),
                (ES, frame[2] as u16),
                (DS, frame[3] as u16),
                (4, frame[4] as u16),
                (5, frame[5] as u16),
            ] {
                self.load_vm86_segment(s, selector);
            }
            return Ok(());
        }
        if virtual_return && off > 0xffff {
            return Err(ExecError::Fault(13));
        }
        self.far_transfer(b, seg, off, false, true)?;
        self.set_stack_pointer(sp.wrapping_add(3 * bytes));
        self.load_flags(b, flags);
        if self.protected_mode() && self.operand32 {
            // IRET can restore RF, unlike POPFD. VM and IOPL cannot be changed
            // by a VM86 return. A nonzero protected CPL cannot enter VM86.
            self.flags_upper = ((flags >> 16) as u16 & 5) | if virtual_return { 2 } else { 0 };
        }
        Ok(())
    }
    /// Switch only the VM86 -> nonconforming CPL0 386 gate path. General
    /// protected outer-ring tasks/call gates remain outside this CPU contract.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn vm86_interrupt(
        &mut self,
        b: &mut Bus,
        n: u8,
        source: GateSource,
        error: Option<u32>,
        selector: u16,
        off: u32,
        kind: u8,
    ) -> ExecResult<()> {
        let ext = u32::from(source == GateSource::External);
        let target_error = (selector & !3) as u32 | ext;
        if !matches!(kind, 14 | 15) {
            return Err(ExecError::ProtectedFault(13, (n as u32 * 8) | 2 | ext));
        }
        // Validate at the destination CPL; neither the old CS RPL nor the
        // VM86 segment caches are protected descriptor selectors.
        let mut next = self.clone();
        next.cpl = 0;
        next.flags_upper &= !2;
        let (code, code_address) = next
            .code_target(b, selector & !3, off, false)
            .map_err(|e| match e {
                ExecError::ProtectedFault(n, code) => ExecError::ProtectedFault(n, code | ext),
                other => other,
            })?;
        if code.access & 4 != 0 || (code.access >> 5) & 3 != 0 {
            return Err(ExecError::ProtectedFault(13, target_error));
        }
        let tr_error = (self.tr & !3) as u32 | ext;
        let invalid_tss = || ExecError::ProtectedFault(10, tr_error);
        let tss = self.tr_cache.ok_or_else(invalid_tss)?;
        if self.tr & !3 == 0 || self.tr & 4 != 0 || tss.access & 0x9f != 0x8b || tss.limit < 9 {
            return Err(invalid_tss());
        }
        let raw = self.read_system::<6>(b, tss.base.wrapping_add(4))?;
        let esp0 = u32::from_le_bytes(raw[..4].try_into().unwrap());
        let ss0 = u16::from_le_bytes(raw[4..].try_into().unwrap());
        let ss_error = (ss0 & !3) as u32 | ext;
        let (stack, stack_address) = self.descriptor(b, ss0).map_err(|e| match e {
            ExecError::ProtectedFault(_, _) => ExecError::ProtectedFault(10, ss_error),
            other => other,
        })?;
        if ss0 & 3 != 0
            || stack.access & 0x1a != 0x12
            || stack.access & 0x60 != 0
            || stack.attributes & 0x20 != 0
        {
            return Err(ExecError::ProtectedFault(10, ss_error));
        }
        if stack.access & 0x80 == 0 {
            return Err(ExecError::ProtectedFault(12, ss_error));
        }
        next.s[SS] = ss0;
        next.segment_caches[SS] = Some(stack);
        next.set_reg32(SP, esp0);
        let values = [
            self.segment(5) as u32,
            self.segment(4) as u32,
            self.s[DS] as u32,
            self.s[ES] as u32,
            self.s[SS] as u32,
            self.reg32(SP),
            self.eflags()
                | if source == GateSource::Fault {
                    0x10000
                } else {
                    0
                },
            self.s[CS] as u32,
            self.ip,
        ];
        let count = 9 + u32::from(error.is_some());
        for i in 1..=count {
            next.check_access(
                b,
                SS,
                next.stack_offset(next.stack_pointer().wrapping_sub(4 * i)),
                4,
                true,
                false,
            )?;
        }
        // Preflight descriptor writes before any stack/device access. The bus
        // journal also restores RAM/VRAM/latches if a later access faults.
        for (d, address) in [(code, code_address), (stack, stack_address)] {
            if d.access & 1 == 0 {
                next.check_pages(b, address.wrapping_add(5), 1, true, false)?;
            }
        }
        for value in values {
            next.push_width(b, value, 4)?;
        }
        if let Some(value) = error {
            next.push_width(b, value, 4)?;
        }
        next.segment_caches[CS] = Some(next.accessed(b, code, code_address)?);
        next.segment_caches[SS] = Some(next.accessed(b, stack, stack_address)?);
        for s in [ES, DS, 4, 5] {
            next.set_segment(s, 0);
            next.segment_caches[s] = Some(SegmentCache {
                base: 0,
                limit: 0,
                access: 0,
                attributes: 0,
            });
        }
        next.s[CS] = selector & !3;
        next.ip = off;
        next.flags &= !(TF | 0x4000);
        next.flags_upper &= !3; // VM and RF; AC survives on i486 gate delivery
        if kind == 14 {
            next.flags &= !IF;
        }
        next.trap_suppressed = true;
        next.halted = false;
        if source == GateSource::Fault {
            next.last_fault = Some((n, error.unwrap_or(0)));
        }
        next.last_vm86_gate = Some((n, kind, selector & !3, off, ss0, esp0));
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(b: &mut Bus, a: usize, value: u32) {
        b.mem[a..a + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn entry() -> (Cpu, Bus) {
        let mut b = Bus::new(crate::Profile::Pc9821ApDos);
        b.a20 = true;
        let mut c = Cpu {
            cr0: 0x60000011,
            s: [16, 8, 16, 16],
            ip: 0x1000,
            operand32: true,
            flags: 2,
            ..Cpu::default()
        };
        for s in 0..6 {
            c.segment_caches[s] = Some(SegmentCache {
                base: 0,
                limit: 0xffff,
                access: if s == CS { 0x9b } else { 0x93 },
                attributes: 0,
            });
        }
        c.set_reg32(SP, 0xffdc);
        for (i, v) in [0x1234, 0, 0x2ffff, 0xabcdfefe, 0xffff, 0, 0xabcd, 0xfedc, 1]
            .into_iter()
            .enumerate()
        {
            put(&mut b, 0xffdc + i * 4, v);
        }
        (c, b)
    }
    #[test]
    fn full_frame_preflight_limit_and_invalid_eip_are_atomic() {
        for missing in 0..36 {
            let (mut c, mut b) = entry();
            c.segment_caches[SS].as_mut().unwrap().limit = 0xffff - missing;
            let old = format!("{c:?}");
            let mem = b.mem.clone();
            let result = c.iret(&mut b);
            if missing == 0 {
                result.unwrap();
                assert!(c.vm86());
                assert_eq!(c.s, [0, 0, 0xffff, 0xabcd]);
                assert_eq!(c.extra_segments, [0xfedc, 1]);
                assert_eq!(c.ip, 0x1234);
                assert_eq!(c.reg32(SP), 0xabcdfefe);
            } else {
                assert!(
                    matches!(result, Err(ExecError::Fault(12))),
                    "{missing}: {result:?}"
                );
                assert_eq!(format!("{c:?}"), old);
            }
            assert_eq!(b.mem, mem);
        }
        let (mut c, mut b) = entry();
        put(&mut b, 0xffdc, 0x10000);
        let old = format!("{c:?}");
        assert!(matches!(c.iret(&mut b), Err(ExecError::Fault(13))));
        assert_eq!(format!("{c:?}"), old);
    }
    #[test]
    fn incomplete_paged_frame_preflight_preserves_registers_memory_and_latches() {
        let (mut c, mut b) = entry();
        c.cr0 |= 0x80000000;
        c.cr3 = 0x1000;
        c.set_reg32(SP, 0xffe0);
        c.segment_caches[SS].as_mut().unwrap().limit = 0x1ffff;
        put(&mut b, 0x1000, 0x2003);
        // Full 36-byte preflight reaches an absent page after reading the
        // valid header; the incomplete transition must not commit registers.
        b.egc.enabled = true;
        b.grcg_mode = 0x80;
        let old = c.clone();
        let egc = b.egc.clone();
        let graphics = b.graphics.clone();
        let mem = b.mem.clone();
        put(&mut b, 0x2000 + 15 * 4, 0x30003);
        put(&mut b, 0x30fe0, 0x1234);
        put(&mut b, 0x30fe4, 0x4000);
        put(&mut b, 0x30fe8, 0x23002);
        let frame_mem = b.mem[0x30000..0x31000].to_vec();
        let result = c.iret(&mut b);
        assert!(
            matches!(
                result,
                Err(ExecError::PageFault {
                    linear: 0x10000,
                    code: 0
                })
            ),
            "{result:?}"
        );
        assert_eq!(c.s, old.s);
        assert_eq!(c.reg32(SP), old.reg32(SP));
        assert_eq!(c.eflags(), old.eflags());
        assert_eq!(c.ip, old.ip);
        assert_eq!(b.egc, egc);
        assert_eq!(b.graphics, graphics);
        assert_eq!(&b.mem[0x30000..0x31000], frame_mem);
        assert_eq!(&b.mem[0x8000..0x9000], &mem[0x8000..0x9000]);
    }
    #[test]
    fn vm86_iret_iopl_and_nt_priority_precedes_unreadable_stack() {
        for iopl in 0..3 {
            let (mut c, mut b) = entry();
            c.flags_upper = 2;
            c.cpl = 3;
            c.flags = 0x4002 | (iopl << 12);
            c.segment_caches[SS].as_mut().unwrap().limit = 0;
            let old = format!("{c:?}");
            assert!(matches!(c.iret(&mut b), Err(ExecError::Fault(13))));
            assert_eq!(format!("{c:?}"), old);
        }
    }
    #[test]
    fn stacked_vm_is_ignored_by_nonzero_protected_cpl_and_word_iret() {
        for word in [false, true] {
            let (mut c, mut b) = entry();
            c.gdtr_base = 0x5000;
            c.gdtr_limit = 31;
            let selector = if word { 8 } else { 11 };
            let access = if word { 0x9b } else { 0xfb };
            b.mem[0x5008..0x5010].copy_from_slice(&[255, 255, 0, 0, 0, access, 0, 0]);
            c.cpl = if word { 0 } else { 3 };
            c.operand32 = !word;
            c.set_reg32(SP, 0x8000);
            if word {
                for (i, v) in [0x1234u16, selector, 0x3002].into_iter().enumerate() {
                    b.mem[0x8000 + i * 2..0x8002 + i * 2].copy_from_slice(&v.to_le_bytes());
                }
                c.segment_caches[SS].as_mut().unwrap().limit = 0x8005;
            } else {
                for (i, v) in [0x1234, selector as u32, 0x23002].into_iter().enumerate() {
                    put(&mut b, 0x8000 + i * 4, v);
                }
                c.segment_caches[SS].as_mut().unwrap().limit = 0x800b;
            }
            c.iret(&mut b).unwrap();
            assert!(!c.vm86());
            assert_eq!(c.ip, 0x1234);
            assert_eq!(c.r[SP], 0x8000 + if word { 6 } else { 12 });
            assert_eq!(c.cpl, if word { 0 } else { 3 });
        }
    }
    #[test]
    fn bitmap_missing_tss_and_16bit_tss_deny_without_any_io() {
        let (mut c, mut b) = entry();
        c.flags_upper = 2;
        c.cpl = 3;
        c.flags = 0x3002;
        c.tr = 24;
        for cache in [
            None,
            Some(SegmentCache {
                base: 0xa8000,
                limit: 0xffff,
                access: 0x83,
                attributes: 0,
            }),
            Some(SegmentCache {
                base: 0xa8000,
                limit: 0x66,
                access: 0x8b,
                attributes: 0,
            }),
        ] {
            c.tr_cache = cache;
            let egc = b.egc.clone();
            assert!(matches!(
                c.check_io(&mut b, 0xf2, 1),
                Err(ExecError::Fault(13))
            ));
            assert_eq!(b.egc, egc);
            assert!(b.a20);
        }
    }
    #[test]
    fn low_iopl_popf_and_iret_privilege_masks_are_independent() {
        let (mut c, b) = entry();
        for cpl in 1..=3 {
            c.cpl = cpl;
            c.flags = 0x1002;
            c.flags_upper = 0;
            c.load_flags(&b, 0x7fd5 | IF as u32 | 0x60000);
            assert_eq!(c.flags & 0x3000, 0x1000);
            assert_eq!(c.flags & IF, if cpl == 1 { IF } else { 0 });
            assert_eq!(c.flags_upper, 4);
        }
    }
    #[test]
    fn diagnostic_metadata_is_bounded_and_never_reads_egc_or_updates_pages() {
        let (c, mut b) = entry();
        let old = format!("{c:?}");
        let mem = b.mem.clone();
        let egc = b.egc.clone();
        assert_eq!(c.iret_frame_metadata(&b, true), (16, 0xffdc, 36, true));
        let mut c = c;
        c.tr_cache = Some(SegmentCache {
            base: 0xa8000,
            limit: 103,
            access: 0x8b,
            attributes: 0,
        });
        assert_eq!(c.monitor_stack_metadata(&b), None);
        assert_eq!(b.egc, egc);
        assert_eq!(b.mem, mem);
        c.tr_cache = None;
        assert_eq!(format!("{c:?}"), old);
        // Ordinary TSS metadata is readable without exposing any TSS bytes.
        c.tr_cache = Some(SegmentCache {
            base: 0x8000,
            limit: 9,
            access: 0x8b,
            attributes: 0,
        });
        put(&mut b, 0x8004, 0x18000);
        put(&mut b, 0x8008, 16);
        assert_eq!(c.monitor_stack_metadata(&b), Some((16, 0x18000)));
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;
    fn put(b: &mut Bus, a: usize, v: u32) {
        b.mem[a..a + 4].copy_from_slice(&v.to_le_bytes());
    }
    #[test]
    fn invalid_gate_tss_and_ss0_errors_include_correct_selector_and_external_bit() {
        for external in [false, true] {
            for case in 0..14 {
                let mut b = Bus::new(crate::Profile::Pc9821ApDos);
                let mut c = Cpu {
                    cr0: 0x11,
                    cpl: 3,
                    flags_upper: 2,
                    flags: 0x3002,
                    ip: 0x1234,
                    tr: 24,
                    tr_cache: Some(SegmentCache {
                        base: 0x9000,
                        limit: 103,
                        access: 0x8b,
                        attributes: 0,
                    }),
                    gdtr_base: 0x5000,
                    gdtr_limit: 31,
                    idtr_base: 0x6000,
                    idtr_limit: 0x7ff,
                    ..Cpu::default()
                };
                for (selector, access) in [(8, 0x9b), (16, 0x93)] {
                    b.mem[0x5000 + selector..0x5008 + selector]
                        .copy_from_slice(&[255, 255, 0, 0, 0, access, 0xcf, 0]);
                }
                b.mem[0x6180..0x6188].copy_from_slice(&[0, 0x10, 8, 0, 0, 0xee, 1, 0]);
                put(&mut b, 0x9004, 0x18000);
                put(&mut b, 0x9008, 16);
                for s in 0..6 {
                    c.load_vm86_segment(s, 0x2000 + s as u16 * 0x1000);
                }
                c.set_reg32(SP, 0xabcd7000);
                let (vector, code) = match case {
                    0 => {
                        c.tr_cache = None;
                        (10, 24)
                    }
                    1 => {
                        c.tr_cache.as_mut().unwrap().access = 0x83;
                        (10, 24)
                    }
                    2 => {
                        c.tr_cache.as_mut().unwrap().access = 0x0b;
                        (10, 24)
                    }
                    3 => {
                        c.tr_cache.as_mut().unwrap().limit = 8;
                        (10, 24)
                    }
                    4 => {
                        put(&mut b, 0x9008, 0);
                        (10, 0)
                    }
                    5 => {
                        put(&mut b, 0x9008, 19);
                        (10, 16)
                    }
                    6 => {
                        b.mem[0x5015] = 0xf3;
                        (10, 16)
                    }
                    7 => {
                        b.mem[0x5015] = 0x91;
                        (10, 16)
                    }
                    8 => {
                        b.mem[0x5015] = 0x13;
                        (12, 16)
                    }
                    9 => {
                        put(&mut b, 0x9004, 2);
                        (12, 0)
                    }
                    10 => {
                        b.mem[0x6185] = 0xe6;
                        (13, 0x182)
                    }
                    11 => {
                        b.mem[0x500d] = 0x9f;
                        (13, 8)
                    }
                    12 => {
                        b.mem[0x500d] = 0x1b;
                        (11, 8)
                    }
                    _ => {
                        c.tr |= 4;
                        (10, 28)
                    }
                };
                let cpu = format!("{c:?}");
                let mem = b.mem.clone();
                let egc = b.egc.clone();
                let result = c.protected_interrupt(
                    &mut b,
                    0x30,
                    if external {
                        GateSource::External
                    } else {
                        GateSource::Software
                    },
                    None,
                );
                if case == 9 {
                    assert!(matches!(result, Err(ExecError::Fault(12))), "{result:?}");
                } else {
                    assert!(
                        matches!(result,Err(ExecError::ProtectedFault(n,e)) if n==vector && e==code|u32::from(external)),
                        "case={case} ext={external}: {result:?}"
                    );
                }
                assert_eq!(format!("{c:?}"), cpu);
                assert_eq!(b.mem, mem);
                assert_eq!(b.egc, egc);
            }
        }
    }
}

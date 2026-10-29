//! Original Intel486 two-level 4 KiB paging. Architectural contract: Intel486
//! PRM (1992), 5.3, 6.8, 9.9.8/14, 26 INVLPG. No PSE/PAE/NX or data cache.
use super::*;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug)]
struct Entry {
    page: u32,
    frame: u32,
    pte: u32,
    writable: bool,
    user: bool,
    dirty: bool,
}

/// Original 32-slot direct-mapped TLB; replacement/cycles are emulator policy.
/// Kept public for Cpu's existing struct-update construction API.
#[derive(Clone, Debug, Default)]
pub struct Paging {
    entries: RefCell<[Option<Entry>; 32]>,
}
impl Paging {
    pub(super) fn flush(&self) {
        self.entries.replace([None; 32]);
    }
    pub(super) fn invalidate(&self, linear: u32) {
        let page = linear >> 12;
        let mut entries = self.entries.borrow_mut();
        let slot = &mut entries[page as usize & 31];
        if slot.is_some_and(|e| e.page == page) {
            *slot = None;
        }
    }
}
impl Cpu {
    pub fn paging_enabled(&self) -> bool {
        self.cr0 & 0x80000000 != 0
    }

    /// Trace-only instruction byte. Never walks MMIO or updates architectural
    /// state; a missing mapping is reported as unavailable to the observer.
    pub fn diagnostic_code_byte(&self, b: &Bus, offset: u32) -> Option<u8> {
        if offset > self.segment_cache(CS).limit {
            return None;
        }
        let physical = self.peek_translation(b, self.linear(CS, offset) as u32)?;
        if b.read_has_side_effects(physical) {
            return None;
        }
        Some(b.read(physical as usize))
    }

    fn page_fault(linear: u32, write: bool, user: bool, present: bool) -> ExecError {
        ExecError::PageFault {
            linear,
            code: u32::from(present) | (u32::from(write) << 1) | (u32::from(user) << 2),
        }
    }
    fn page_permission(&self, entry: Entry, write: bool, user: bool) -> bool {
        (!user || entry.user) && (!write || entry.writable || (!user && self.cr0 & 0x10000 == 0))
    }
    // Page tables must be writable RAM on this PC-98 bus. There is no physical
    // bus-error/#PF signal for absent RAM or ignored ROM writes. Diagnose such a
    // walk before touching device latches rather than fabricate a successful A/D
    // update. This is an emulator boundary, not a modern reserved-bit #PF.
    fn page_word(b: &Bus, address: u32) -> ExecResult<u32> {
        if !(0..4).all(|i| b.is_plain_ram(address.wrapping_add(i))) {
            return Err(ExecError::Unsupported(format!("unsupported paging table physical bus access at {address:08x}; requires writable ordinary RAM")));
        }
        Ok(u32::from_le_bytes(std::array::from_fn(|i| {
            b.read(address.wrapping_add(i as u32) as usize)
        })))
    }
    fn page_flags(value: u32, address: u32, directory: bool) -> ExecResult<()> {
        // 486 bits 7/8 are reserved, not PS/PAT/G. PDE bit 6 is undefined and
        // preserved; software bits 9..11 are retained. Nonpresent entries are
        // entirely software-owned. Do not invent a later RSVD error-code bit.
        if value & 1 != 0 && value & 0x180 != 0 {
            return Err(ExecError::Unsupported(format!("unsupported i486 reserved paging flags {}={value:08x} physical={address:08x}; no PSE/PAT/global pages", if directory { "PDE" } else { "PTE" })));
        }
        Ok(())
    }
    /// Segmentation has already checked the complete operand. Walk physical
    /// tables only; apply PC-98 A20/bus decoding after translation at every level.
    /// A preflight may set A, but D is set only for the actual write operation.
    pub(super) fn translate(
        &self,
        b: &mut Bus,
        linear: u32,
        write: bool,
        user: bool,
        dirty: bool,
    ) -> ExecResult<u32> {
        if !self.paging_enabled() || b.profile == crate::Profile::Pc9801Vm {
            return Ok(linear);
        }
        let page = linear >> 12;
        let index = page as usize & 31;
        let cached = self.paging.entries.borrow()[index].filter(|e| e.page == page);
        let mut entry = if let Some(entry) = cached {
            entry
        } else {
            let pde_address = (self.cr3 & 0xfffff000).wrapping_add((linear >> 22) * 4);
            let pde = Self::page_word(b, pde_address)?;
            if pde & 1 == 0 {
                return Err(Self::page_fault(linear, write, user, false));
            }
            Self::page_flags(pde, pde_address, true)?;
            b.page_accessed(pde_address);
            let pte_address = (pde & 0xfffff000).wrapping_add(((linear >> 12) & 1023) * 4);
            let pte = Self::page_word(b, pte_address)?;
            if pte & 1 == 0 {
                return Err(Self::page_fault(linear, write, user, false));
            }
            Self::page_flags(pte, pte_address, false)?;
            let entry = Entry {
                page,
                frame: pte & 0xfffff000,
                pte: pte_address,
                writable: pde & pte & 2 != 0,
                user: pde & pte & 4 != 0,
                dirty: pte & 0x40 != 0,
            };
            if !self.page_permission(entry, write, user) {
                return Err(Self::page_fault(linear, write, user, true));
            }
            b.page_accessed(pte_address);
            self.paging.entries.borrow_mut()[index] = Some(entry);
            entry
        };
        if !self.page_permission(entry, write, user) {
            return Err(Self::page_fault(linear, write, user, true));
        }
        if dirty && write && !entry.dirty {
            // Preserve every other flag, including software bits. A cached page
            // remains stale until explicit invalidation even if software edits
            // the backing PTE. D stays cached until invalidation as on hardware.
            // A20 can change after this clean entry was cached. Revalidate the
            // current physical decode before a dirty-bit RMW, without rewalking
            // or replacing the cached frame/permissions.
            let value = Self::page_word(b, entry.pte)? as u8 | 0x40;
            b.write(entry.pte as usize, value);
            entry.dirty = true;
            self.paging.entries.borrow_mut()[index] = Some(entry);
        }
        Ok(entry.frame | (linear & 4095))
    }
    pub(super) fn check_pages(
        &self,
        b: &mut Bus,
        linear: u32,
        bytes: u32,
        write: bool,
        user: bool,
    ) -> ExecResult<()> {
        if !self.paging_enabled() {
            return Ok(());
        }
        // Operands/system tables are small; check each byte to report the exact
        // first inaccessible address, including wrap at the 32-bit linear limit.
        for i in 0..bytes {
            self.translate(b, linear.wrapping_add(i), write, user, false)?;
        }
        Ok(())
    }
    pub(super) fn read_system<const N: usize>(
        &self,
        b: &mut Bus,
        address: u32,
    ) -> ExecResult<[u8; N]> {
        self.check_pages(b, address, N as u32, false, false)?;
        let mut raw = [0; N];
        for (i, byte) in raw.iter_mut().enumerate() {
            let physical =
                self.translate(b, address.wrapping_add(i as u32), false, false, false)?;
            *byte = b.read(physical as usize);
        }
        Ok(raw)
    }
    pub(super) fn write_system_byte(&self, b: &mut Bus, address: u32, value: u8) -> ExecResult<()> {
        let physical = self.translate(b, address, true, false, true)?;
        b.write(physical as usize, value);
        Ok(())
    }
    /// Bounded observer-only translation; never changes A/D, TLB, CR2 or devices.
    pub(super) fn peek_translation(&self, b: &Bus, linear: u32) -> Option<u32> {
        if !self.paging_enabled() {
            return Some(linear);
        }
        let page = linear >> 12;
        if let Some(e) = self.paging.entries.borrow()[page as usize & 31].filter(|e| e.page == page)
        {
            return Some(e.frame | (linear & 4095));
        }
        let pde_address = (self.cr3 & 0xfffff000).wrapping_add((linear >> 22) * 4);
        let pde = Self::page_word(b, pde_address).ok()?;
        if pde & 1 == 0 || Self::page_flags(pde, pde_address, true).is_err() {
            return None;
        }
        let pte_address = (pde & 0xfffff000).wrapping_add(((linear >> 12) & 1023) * 4);
        let pte = Self::page_word(b, pte_address).ok()?;
        if pte & 1 == 0 || Self::page_flags(pte, pte_address, false).is_err() {
            return None;
        }
        Some((pte & 0xfffff000) | (linear & 4095))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put32(b: &mut Bus, a: usize, v: u32) {
        b.mem[a..a + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn get32(b: &Bus, a: usize) -> u32 {
        u32::from_le_bytes(b.mem[a..a + 4].try_into().unwrap())
    }
    fn setup(linear: u32, directory: u32, table: u32) -> (Cpu, Bus, usize) {
        let mut b = Bus::new(crate::Profile::Pc9821ApDos);
        b.a20 = true;
        let c = Cpu {
            cr0: 0x80010011,
            cr3: 0x1018,
            ..Cpu::default()
        };
        let pte = 0x2000 + ((linear >> 12) as usize & 1023) * 4;
        put32(
            &mut b,
            0x1000 + (linear >> 22) as usize * 4,
            0x2000 | directory,
        );
        put32(&mut b, pte, 0x30000 | table);
        (c, b, pte)
    }
    #[test]
    fn both_levels_permissions_present_cpl_and_wp_matrix() {
        for directory in 0..8 {
            for table in 0..8 {
                for user in [false, true] {
                    for write in [false, true] {
                        for wp in [false, true] {
                            let (mut c, mut b, _) = setup(0x12345678, directory, table);
                            if !wp {
                                c.cr0 &= !0x10000;
                            }
                            let result = c.translate(&mut b, 0x12345678, write, user, write);
                            let present = directory & table & 1 != 0;
                            let allowed = present
                                && (!user || directory & table & 4 != 0)
                                && (!write || directory & table & 2 != 0 || (!user && !wp));
                            if allowed {
                                assert_eq!(result.unwrap(), 0x30678);
                            } else {
                                let ExecError::PageFault { linear, code } = result.unwrap_err()
                                else {
                                    panic!("expected #PF");
                                };
                                assert_eq!(linear, 0x12345678);
                                assert_eq!(
                                    code,
                                    u32::from(present)
                                        | (u32::from(write) << 1)
                                        | (u32::from(user) << 2)
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn accessed_dirty_hits_misses_software_bits_and_explicit_invalidation() {
        let (c, mut b, pte) = setup(0x400000, 0xe5b, 0xe1b);
        assert_eq!(
            c.translate(&mut b, 0x400007, false, false, false).unwrap(),
            0x30007
        );
        assert_eq!(get32(&b, 0x1004), 0x2000 | 0xe7b); // PDE D untouched
        assert_eq!(get32(&b, pte), 0x30000 | 0xe3b);
        put32(&mut b, pte, 0x40000 | 0xe1b); // remove A, change frame without invalidation
        assert_eq!(
            c.translate(&mut b, 0x400009, false, false, false).unwrap(),
            0x30009
        );
        assert_eq!(get32(&b, pte), 0x40000 | 0xe1b); // hit does not restore cleared A
        assert_eq!(
            c.translate(&mut b, 0x400009, true, false, true).unwrap(),
            0x30009
        );
        assert_eq!(get32(&b, pte), 0x40000 | 0xe5b); // first write sets D
        put32(&mut b, pte, 0x40000 | 0xe1b);
        c.translate(&mut b, 0x400009, true, false, true).unwrap();
        assert_eq!(get32(&b, pte), 0x40000 | 0xe1b); // cached dirty stays set
        c.paging.invalidate(0x400fff);
        assert_eq!(
            c.translate(&mut b, 0x400009, true, false, true).unwrap(),
            0x40009
        );
        assert_eq!(get32(&b, pte), 0x40000 | 0xe7b);
        put32(&mut b, pte, 0);
        c.paging.flush();
        assert!(matches!(
            c.translate(&mut b, 0x400000, false, false, false),
            Err(ExecError::PageFault { code: 0, .. })
        ));
        put32(&mut b, pte, 0x50003); // not-present translations were not cached
        assert_eq!(
            c.translate(&mut b, 0x400000, false, false, false).unwrap(),
            0x50000
        );
    }
    #[test]
    fn physical_walk_a20_pc98_ram_rom_mmio_and_no_modern_reserved_fault_bits() {
        for frame in [
            0x30000, 0x100000, 0xa0000, 0xa8000, 0xe0000, 0xf0000, 0xfff00000,
        ] {
            let (c, mut b, pte) = setup(0x400000, 3, 3);
            put32(&mut b, pte, frame | 3);
            for a20 in [false, true] {
                b.a20 = a20;
                let physical = c.translate(&mut b, 0x400005, false, false, false).unwrap();
                assert_eq!(physical, frame | 5); // no blanket physical address mask
                let value = c.read_system::<1>(&mut b, 0x400005).unwrap()[0];
                assert_eq!(value, b.read((frame | 5) as usize));
                c.write_system_byte(&mut b, 0x400005, 0x5a).unwrap();
                assert_eq!(
                    b.read((frame | 5) as usize),
                    if frame == 0xfff00000 {
                        255
                    } else if frame == 0xf0000 {
                        value
                    } else {
                        0x5a
                    }
                );
            }
        }
        let (mut c, mut b, _) = setup(0x400000, 3, 3);
        // A20 applies to page-directory bus cycles too, independently of linear addresses.
        c.cr3 = 0x101000;
        b.a20 = false;
        assert_eq!(
            c.translate(&mut b, 0x400000, false, false, false).unwrap(),
            0x30000
        );
        c.paging.flush();
        b.a20 = true;
        assert!(matches!(
            c.translate(&mut b, 0x400000, false, false, false),
            Err(ExecError::PageFault { code: 0, .. })
        ));
        c.cr3 = 0xa8000;
        let egc = b.egc.clone();
        assert!(matches!(
            c.translate(&mut b, 0, false, false, false),
            Err(ExecError::Unsupported(_))
        ));
        assert_eq!(b.egc, egc);
        for reserved in [0x80, 0x100] {
            for directory in [false, true] {
                let (c, mut b, pte) = setup(0x400000, 3, 3);
                let address = if directory { 0x1004 } else { pte };
                let value = get32(&b, address) | reserved;
                put32(&mut b, address, value);
                assert!(matches!(
                    c.translate(&mut b, 0x400000, false, false, false),
                    Err(ExecError::Unsupported(_))
                ));
                assert_eq!(get32(&b, address), value);
            }
        }
        let (c, mut b, pte) = setup(0x400000, 3, 0xffe);
        assert!(matches!(
            c.translate(&mut b, 0x400000, true, false, false),
            Err(ExecError::PageFault { code: 2, .. })
        ));
        assert_eq!(get32(&b, pte), 0x30ffe); // every nonpresent bit is software-owned
    }

    #[test]
    fn cached_clean_entry_revalidates_a20_before_dirty_update_without_mmio_reads() {
        let (c, mut b, _) = setup(0x400000, 3, 3);
        put32(&mut b, 0x1004, 0x1a8003);
        put32(&mut b, 0x1a8000, 0x30003);
        assert_eq!(
            c.translate(&mut b, 0x400000, false, false, false).unwrap(),
            0x30000
        );
        b.a20 = false;
        b.egc.enabled = true;
        b.grcg_mode = 0x80;
        let egc = b.egc.clone();
        let graphics = b.graphics.clone();
        assert!(matches!(
            c.translate(&mut b, 0x400000, true, false, true),
            Err(ExecError::Unsupported(_))
        ));
        assert_eq!(b.egc, egc);
        assert_eq!(b.graphics, graphics);
    }
    #[test]
    fn cross_page_system_reads_and_writes_wrap_linear_but_not_physical_frames() {
        let (c, mut b, pte) = setup(0xffff_fffe, 3, 3);
        put32(&mut b, 0x1000, 0x4003);
        put32(&mut b, 0x4000, 0x50003);
        b.mem[0x30ffe..0x31000].copy_from_slice(&[0x12, 0x34]);
        b.mem[0x50000..0x50002].copy_from_slice(&[0x56, 0x78]);
        assert_eq!(
            c.read_system::<4>(&mut b, 0xffff_fffe).unwrap(),
            [0x12, 0x34, 0x56, 0x78]
        );
        assert_eq!(get32(&b, pte) & 0x60, 0x20);
        put32(&mut b, 0x4000, 0);
        c.paging.flush();
        assert!(matches!(
            c.read_system::<4>(&mut b, 0xffff_fffe),
            Err(ExecError::PageFault { linear: 0, code: 0 })
        ));
    }
}

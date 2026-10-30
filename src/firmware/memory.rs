//! Request-wide VM86 access preflight. No device/FIFO access occurs until every
//! memory operand is permitted. A failed request restarts at the ROM OUT opcode.
use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Memory {
    pages: Option<BTreeMap<u32, (u32, bool, u32)>>,
}
impl Memory {
    pub fn for_cpu(c: &Cpu) -> Self {
        Self {
            pages: c.vm86().then(BTreeMap::new),
        }
    }
    pub fn check(
        &mut self,
        c: &Cpu,
        b: &mut Bus,
        address: usize,
        len: usize,
        write: bool,
    ) -> ExecResult<()> {
        let Some(pages) = &mut self.pages else {
            return Ok(());
        };
        let mut linear = address as u32;
        let end = linear + len as u32;
        while linear < end {
            let page = linear >> 12;
            if !pages.get(&page).is_some_and(|p| !write || p.1) {
                let physical = c.translate(b, linear, write, true, false)?;
                pages.insert(page, (physical & !4095, write, linear));
            }
            linear = ((linear | 4095) + 1).min(end);
        }
        Ok(())
    }
    pub fn segment(
        &mut self,
        c: &Cpu,
        b: &mut Bus,
        seg: u16,
        off: u16,
        len: usize,
        write: bool,
    ) -> ExecResult<()> {
        let first = len.min(65536 - off as usize);
        self.check(c, b, Cpu::phys(seg, off), first, write)?;
        if len > first {
            self.check(c, b, Cpu::phys(seg, 0), len - first, write)?;
        }
        Ok(())
    }
    pub fn ready(&self, c: &Cpu, b: &mut Bus) -> ExecResult<()> {
        if let Some(pages) = &self.pages {
            // Permission checks all succeeded. Set D only for planned writes;
            // never read guest/MMIO data while checking permissions.
            for &(_, write, linear) in pages.values() {
                if write {
                    c.translate(b, linear, true, true, true)?;
                }
            }
        }
        Ok(())
    }
    pub fn physical(&self, address: usize) -> usize {
        if let Some(pages) = &self.pages {
            pages[&(address as u32 >> 12)].0 as usize | (address & 4095)
        } else {
            address
        }
    }
    pub fn read(&self, b: &Bus, address: usize) -> u8 {
        b.read(self.physical(address))
    }
    pub fn write(&self, b: &mut Bus, address: usize, value: u8) {
        b.write(self.physical(address), value);
    }
    pub fn read_seg(&self, b: &Bus, seg: u16, off: u16, word: bool) -> u16 {
        self.read(b, Cpu::phys(seg, off)) as u16
            | if word {
                (self.read(b, Cpu::phys(seg, off.wrapping_add(1))) as u16) << 8
            } else {
                0
            }
    }
    pub fn write_seg(&self, b: &mut Bus, seg: u16, off: u16, value: u16, word: bool) {
        self.write(b, Cpu::phys(seg, off), value as u8);
        if word {
            self.write(b, Cpu::phys(seg, off.wrapping_add(1)), (value >> 8) as u8);
        }
    }
}

pub(super) fn prepare(c: &Cpu, b: &mut Bus, int: u8) -> ExecResult<Memory> {
    let mut m = Memory::for_cpu(c);
    if !c.vm86() {
        return Ok(m);
    }
    let ah = c.byte(4);
    // Flags and diagnostics use a 16-bit BIOS interrupt frame, independent of
    // the monitor's 32-bit VM86 gate frame. Each BIOS word wraps its offset.
    if int == 0x1b {
        let offset = c.r[SP].wrapping_add(4);
        c.check_access(b, SS, offset as u32, 2, true, false)?;
        m.segment(c, b, c.s[SS], offset, 2, true)?;
    }
    match int {
        8 => m.check(c, b, 0x58a, 2, true)?,
        9 => {
            // A pending scan can change any key state and enqueue a BIOS word.
            // Preflight before consuming the FIFO, even for command replies.
            m.check(c, b, 0x502, 0x3a, true)?;
        }
        0x18 => match ah {
            0 | 3 | 5 => m.check(c, b, 0x502, 0x3a, true)?,
            4 if c.byte(0) < 16 => m.check(c, b, 0x52a + c.byte(0) as usize, 1, false)?,
            0x0a | 0x0b => m.check(c, b, 0x53c, 1, ah == 0x0a)?,
            0x14 if b.profile == crate::Profile::Pc9821ApDos
                && (0x21..=0x7e).contains(&c.byte(6)) =>
            {
                m.segment(c, b, c.r[BX], c.r[CX].wrapping_add(2), 32, true)?
            }
            0x14 => m.segment(c, b, c.s[ES], c.r[BX], 16, true)?,
            0x16 => {
                m.check(c, b, 0xa0000, 4000, true)?;
                m.check(c, b, 0xa2000, 4000, true)?;
            }
            0x1a => m.segment(c, b, c.r[BX], c.r[CX].wrapping_add(2), 32, false)?,
            0x40 | 0x41 if b.profile == crate::Profile::Pc9821ApDos => {
                m.check(c, b, 0x54c, 1, true)?
            }
            0x42 | 0x4a => m.check(c, b, 0x54d, 1, true)?,
            _ => (),
        },
        0x1b => {
            let op = ah & 15;
            let bytes = c.r[BX] as usize;
            let hdd = c.byte(0) & 0x7f == 0;
            let transfers = if hdd {
                b.ide.disk.as_ref().is_some_and(|disk| {
                    let lba = if c.byte(0) & 128 != 0 {
                        disk.chs(c.r[CX], c.byte(6), c.byte(2))
                    } else {
                        Some(c.r[CX] as usize | ((c.r[DX] as usize) << 16))
                    };
                    matches!(op, 5 | 6)
                        && bytes != 0
                        && bytes.is_multiple_of(512)
                        && lba.is_some_and(|l| l + bytes / 512 <= disk.sector_count())
                })
            } else {
                let device = c.byte(0) & 0xfc;
                let drive = (c.byte(0) & 3) as usize;
                matches!(op, 1 | 5 | 6)
                    && [0x90, 0x30, 0xb0, 0x70].contains(&device)
                    && drive < 2
                    && b.disks[drive].is_some()
                    && c.byte(5) <= 3
                    && bytes != 0
                    && bytes.is_multiple_of(128 << c.byte(5))
            };
            if transfers {
                // Disk DMA/PIO advances a linear buffer, unlike the BIOS
                // character/clock interfaces which wrap a 16-bit offset.
                m.check(c, b, Cpu::phys(c.s[ES], c.r[BP]), bytes, op != 5)?;
            }
        }
        0x1c => match ah {
            0 => m.segment(c, b, c.s[ES], c.r[BX], 6, true)?,
            2 => m.check(c, b, 0x58a, 2, true)?,
            _ => (),
        },
        // This service explicitly operates on physical extended RAM. It is
        // not a guest-linear copy and has no VM86 monitor contract yet.
        0x1f => return Err("unsupported VM86 BIOS INT 1F physical extended copy".into()),
        0x0a..=0x17 | 0x1e => (),
        _ => m.segment(c, b, c.s[SS], c.r[SP], 4, false)?,
    }
    m.ready(c, b)?;
    Ok(m)
}

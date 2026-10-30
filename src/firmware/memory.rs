//! Firmware operands use the caller's linear address space, never a physical
//! alias of its VM86 RAM. Preflight service outputs before consuming device I/O.
use crate::{bus::Bus, cpu::*};

pub(crate) fn check(
    c: &Cpu,
    b: &mut Bus,
    address: usize,
    bytes: usize,
    write: bool,
) -> ExecResult<()> {
    c.check_pages(b, address as u32, bytes as u32, write, c.vm86())
}
pub(crate) fn resolve(c: &Cpu, b: &mut Bus, address: usize, write: bool) -> ExecResult<usize> {
    Ok(c.translate(b, address as u32, write, c.vm86(), false)? as usize)
}
pub(crate) fn physical(c: &Cpu, b: &mut Bus, address: usize, write: bool) -> ExecResult<usize> {
    Ok(c.translate(b, address as u32, write, c.vm86(), write)? as usize)
}
pub(crate) fn read(c: &Cpu, b: &mut Bus, address: usize) -> ExecResult<u8> {
    let p = physical(c, b, address, false)?;
    Ok(b.read(p))
}
pub(crate) fn write(c: &Cpu, b: &mut Bus, address: usize, v: u8) -> ExecResult<()> {
    let p = physical(c, b, address, true)?;
    b.write(p, v);
    Ok(())
}
pub(crate) fn read_seg(c: &Cpu, b: &mut Bus, seg: u16, off: u16, word: bool) -> ExecResult<u16> {
    check_seg(c, b, seg, off, word, false)?;
    let lo = read(c, b, Cpu::phys(seg, off))? as u16;
    Ok(lo
        | if word {
            (read(c, b, Cpu::phys(seg, off.wrapping_add(1)))? as u16) << 8
        } else {
            0
        })
}
pub(crate) fn write_seg(
    c: &Cpu,
    b: &mut Bus,
    seg: u16,
    off: u16,
    v: u16,
    word: bool,
) -> ExecResult<()> {
    check_seg(c, b, seg, off, word, true)?;
    write(c, b, Cpu::phys(seg, off), v as u8)?;
    if word {
        write(c, b, Cpu::phys(seg, off.wrapping_add(1)), (v >> 8) as u8)?;
    }
    Ok(())
}
pub(crate) fn check_seg(
    c: &Cpu,
    b: &mut Bus,
    seg: u16,
    off: u16,
    word: bool,
    write: bool,
) -> ExecResult<()> {
    if c.vm86() && word && off == 0xffff {
        return Err(ExecError::Fault(if seg == c.s[SS] { 12 } else { 13 }));
    }
    check(c, b, Cpu::phys(seg, off), 1, write)?;
    if word {
        check(c, b, Cpu::phys(seg, off.wrapping_add(1)), 1, write)?;
    }
    Ok(())
}

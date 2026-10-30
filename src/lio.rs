//! Original Graph LIO subset, from the NEC BIOS databook's interface description.
//! No NEC implementation bytes or private work-area layout are reproduced.
use crate::{bus::Bus, cpu::*, firmware::memory};

const ROM: usize = 0xf9900;
const THUNKS: usize = 0x100;

pub(crate) fn initialize(b: &mut Bus) {
    // Printed p.135: byte count, three reserved bytes, then ID:u16/offset:u16.
    // The count excludes the final ID=0 GRAPH BIO entry.
    b.mem[ROM..ROM + 4].copy_from_slice(&[0x11, 0, 0, 0]);
    for i in 0..18 {
        let id = match i {
            0..=15 => 0xa0 + i as u8,
            16 => 0xce,
            _ => 0,
        };
        let off = (THUNKS + i * 4) as u16;
        b.mem[ROM + 4 + i * 4..ROM + 8 + i * 4].copy_from_slice(&[
            id,
            0,
            off as u8,
            (off >> 8) as u8,
        ]);
        b.mem[ROM + off as usize..ROM + off as usize + 4]
            .copy_from_slice(&[0xe6, 0xf0, 0xcf, 0x90]); // OUT F0 / IRET / NOP
    }
}

pub(crate) fn service(c: &mut Cpu, b: &mut Bus) -> ExecResult<()> {
    let ip = c.ip as usize;
    if !(THUNKS + 2..THUNKS + 18 * 4 + 2).contains(&ip) || (ip - THUNKS - 2) & 3 != 0 {
        return Err("firmware trap outside synthetic LIO ROM".into());
    }
    let entry = (ip - THUNKS - 2) / 4;
    if entry != 0 {
        return Err(format!("unsupported Graph LIO ROM entry {entry} (GINIT only)").into());
    }
    let work = usize::from(c.s[DS]) * 16;
    // Do not wrap into the IVT, video memory or ROM. Validate before mutations.
    if work < 0x600 || work + 0x1200 > 0xa0000 {
        return Err("unsupported GINIT work area (requires disjoint conventional RAM)".into());
    }
    // Check all written pages and physical aliases of the return frame before
    // changing device state. The gaps remain available to the caller.
    let mut frame = Vec::new();
    for i in 0..6 {
        let physical = memory::physical(c, b, Cpu::phys(c.s[SS], c.r[SP].wrapping_add(i)), false)?;
        frame.push(b.address(physical));
    }
    if !c.vm86() && frame.iter().any(|a| (work..work + 0x1200).contains(a)) {
        return Err("unsupported GINIT work area (requires disjoint conventional RAM)".into());
    }
    for range in [0x620..0x6a0, 0x1000..0x1200] {
        memory::check(c, b, work + range.start, range.len(), true)?;
        for i in range {
            let physical = memory::physical(c, b, work + i, false)?;
            if !b.is_plain_ram(physical as u32) || frame.contains(&b.address(physical)) {
                return Err(
                    "unsupported GINIT work area (requires disjoint conventional RAM)".into(),
                );
            }
        }
    }
    memory::check(c, b, 0x54c, 2, false)?;
    memory::check(c, b, 0x54c, 2, true)?;
    for range in [0x620..0x6a0, 0x1000..0x1200] {
        for i in range {
            memory::write(c, b, work + i, 0)?;
        }
    }
    for (i, byte) in [0, 0, 0, 1, 0, 7, 0, 0, 0, 0, 0, 0, 0x7f, 2, 199, 0]
        .into_iter()
        .enumerate()
    {
        memory::write(c, b, work + 0x620 + i, byte)?;
    }

    b.output(0x7c, 0); // ordinary drawing, no charger
    b.egc.enabled = false;
    b.pegc = false;
    b.output(0x6a, 0); // eight digital colors
    b.output(0xa4, 0);
    b.output(0xa6, 0);
    // LIO display=1 selects the first logical 200-line screen in bank 0;
    // it is not hardware bank 1 (printed pp.145-146).
    b.output(0x68, 9);
    b.gdcs[1].start = 0;
    b.gdcs[1].pitch = 40;
    b.gdcs[1].enabled = true;
    let mode = memory::read(c, b, 0x54c)?;
    memory::write(c, b, 0x54c, mode | 0x80)?;
    // Color, LOWER region; preserve the independently selected drawing timing.
    let mode = memory::read(c, b, 0x54d)?;
    memory::write(c, b, 0x54d, (mode & 8) | 0x40)?;
    for (i, color) in b.palette.iter_mut().enumerate() {
        let level = match i {
            0..=7 => 255,
            8 => 119,
            _ => 170,
        };
        *color = [
            if i & 2 != 0 || i == 8 { level } else { 0 },
            if i & 4 != 0 || i == 8 { level } else { 0 },
            if i & 1 != 0 || i == 8 { level } else { 0 },
        ];
    }
    c.set_byte(4, 0);
    Ok(())
}

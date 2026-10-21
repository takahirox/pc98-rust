//! Newly written synthetic firmware services behind executable ROM thunks.
//! INT 21h is deliberately not implemented: DOS must execute its own kernel.
use crate::{bus::Bus, cpu::*};
/// INT 1C/02 has one interval and one ES:BX target, not an event queue.
pub(crate) struct IntervalTimer {
    pub deadline: u64,
    pub segment: u16,
    pub offset: u16,
}
pub fn initialize(b: &mut Bus) {
    let rom = include_bytes!("../guest/firmware.bin");
    b.mem[0xf0000..0xf0000 + rom.len()].copy_from_slice(rom);
    b.mem[0xffff0..0xffff5].copy_from_slice(&[0xea, 0, 0, 0, 0xf0]);
    for n in 0..256usize {
        let off = 0x1000 + n * 4;
        b.mem[0xf0000 + off..0xf0000 + off + 4].copy_from_slice(&[0xe6, 0xf0, 0xcf, 0x90]);
        b.write_seg(0, (n * 4) as u16, off as u16, true);
        b.write_seg(0, (n * 4 + 2) as u16, 0xf000, true);
    }
    crate::lio::initialize(b);
    b.mem[0x501] = 4; // 640 KiB, normal resolution, 8 MHz PIT.
    b.mem[0x500] = 0x20;
    b.mem[0x481] = if b.profile == crate::Profile::Pc9821ApDos {
        0x40
    } else {
        0
    }; // desktop keyboard without NUM key; no EGC advertised
    b.mem[0x401] = if b.profile == crate::Profile::Pc9821ApDos {
        24
    } else {
        0
    }; // 3 MiB in 128 KiB units
    b.mem[0x54c] = if b.profile == crate::Profile::Pc9821ApDos {
        0x66 // 640x400, 31 kHz CRT, fourth plane and GRCG
    } else {
        0x20
    };
    b.mem[0x53c] = 1; // 80 columns, 25 rows
    b.mem[0x55c] = u8::from(b.disks[0].is_some()) | if b.disks[1].is_some() { 2 } else { 0 };
    if b.profile == crate::Profile::Pc9821ApDos && b.ide.disk.is_some() {
        b.mem[0x55d] = 1;
    }
    b.mem[0x584] = if (b.boot_hdd || b.disks[0].is_none()) && b.mem[0x55d] != 0 {
        0x80
    } else {
        0x90
    };
    b.write_seg(0, 0x524, 0x502, true);
    b.write_seg(0, 0x526, 0x502, true);
    for cell in 0..2048 {
        b.mem[0xa2000 + cell * 2] = 0xe1;
    }
    b.gdcs[0].pitch = 80;
    b.gdcs[1].pitch = 40;
    b.gdcs[0].cursor_bottom = 15;
}
fn carry(c: &Cpu, b: &mut Bus, on: bool) {
    let p = c.r[SP].wrapping_add(4);
    let mut f = b.read_seg(c.s[SS], p, true);
    if on {
        f |= CF
    } else {
        f &= !CF
    }
    b.write_seg(c.s[SS], p, f, true);
}
fn sync_keys(b: &mut Bus) {
    let count = b.keyboard.bios.len().min(16);
    for i in 0..count {
        b.write_seg(0, 0x502 + i as u16 * 2, b.keyboard.bios[i], true);
    }
    b.write_seg(0, 0x524, 0x502, true);
    b.write_seg(0, 0x526, 0x502 + (count as u16 * 2) % 32, true);
    b.mem[0x528] = count as u8;
    b.mem[0x53a] = b.keyboard.modifiers;
}
/// One BIOS request goes through the same command/result FIFO, channel 2 DMA,
/// scheduler and IRQ as guest-issued I/O. No sector data bypasses the controller.
pub fn disk_transfer(
    b: &mut Bus,
    drive: u8,
    chrn: [u8; 4],
    bytes: u16,
    address: usize,
    write: bool,
) -> bool {
    let [c, h, r, n] = chrn;
    if bytes == 0 || drive > 1 || h > 1 || n > 3 || r == 0 {
        return false;
    }
    b.output(0x94, 0x18); // spindle motor + DMA handshake enable
    if b.fdc.cylinder[drive as usize] != c {
        for v in [15, drive, c] {
            b.output(0x92, v)
        }
        b.tick(160);
        b.output(0x92, 8);
        b.tick(160);
        while !b.fdc.result.is_empty() {
            b.input(0x92);
        }
    }
    b.output(0x19, 0);
    b.output(0x09, address as u8);
    b.output(0x09, (address >> 8) as u8);
    b.output(0x23, (address >> 16) as u8);
    b.output(0x0b, (bytes - 1) as u8);
    b.output(0x0b, ((bytes - 1) >> 8) as u8);
    b.output(0x17, if write { 0x4a } else { 0x46 });
    b.output(0x15, 2);
    while !b.fdc.result.is_empty() {
        b.input(0x92);
    }
    for v in [
        if write { 0x45 } else { 0x46 },
        drive | (h << 2),
        c,
        h,
        r,
        n,
        255,
        0x1b,
        255,
    ] {
        b.output(0x92, v)
    }
    b.tick(160);
    let st0 = b.input(0x92);
    let st1 = b.input(0x92);
    for _ in 0..5 {
        b.input(0x92);
    } // drain result phase
    st0 & 0xc0 == 0 && st1 == 0
}
pub fn service(c: &mut Cpu, b: &mut Bus) -> Result<(), String> {
    if c.s[CS] == 0xf990 {
        return crate::lio::service(c, b);
    }
    if c.s[CS] != 0xf000 || !(0x1002..0x1402).contains(&c.ip) || (c.ip - 0x1002) & 3 != 0 {
        return Err("firmware trap outside synthetic ROM".into());
    }
    let int = ((c.ip - 0x1002) / 4) as u8;
    let ah = c.byte(4);
    match int {
        8 => {
            let ticks = b.read16(0x58a).wrapping_add(1);
            b.write_seg(0, 0x58a, ticks, true);
            b.output(0, 0x20);
            if b.interval.as_ref().is_some_and(|t| b.cycles >= t.deadline) {
                let timer = b.interval.take().unwrap();
                // IRQ0 already pushed FLAGS, CS, IP and cleared IF/TF. Hand
                // that frame to the guest; its IRET resumes interrupted code.
                // Disarm before entry so the handler may rearm the service.
                c.s[CS] = timer.segment;
                c.ip = timer.offset as u32;
            }
        }
        9 => {
            b.keyboard.service();
            sync_keys(b);
            b.output(0, 0x20);
        }
        0x0a..=0x17 => {
            if int >= 0x10 {
                b.output(8, 0x20);
            }
            b.output(0, 0x20);
        }
        0x18 => match ah {
            0 => {
                if let Some(key) = b.keyboard.bios.pop_front() {
                    c.r[AX] = key;
                    sync_keys(b);
                } else {
                    c.ip = c.ip.wrapping_sub(2);
                    c.halted = true;
                    c.flags |= IF;
                }
            }
            1 | 5 => {
                if let Some(&key) = b.keyboard.bios.front() {
                    c.r[AX] = key;
                    c.set_byte(7, 1);
                    if ah == 5 {
                        b.keyboard.bios.pop_front();
                        sync_keys(b);
                    }
                } else {
                    c.set_byte(7, 0)
                }
            }
            2 => c.set_byte(0, b.keyboard.modifiers),
            3 => {
                b.keyboard.bios.clear();
                sync_keys(b);
            }
            4 => {
                let group = c.byte(0) as usize;
                c.set_byte(4, b.keyboard.pressed.get(group).copied().unwrap_or(0))
            }
            0x0a => b.mem[0x53c] = c.byte(0),
            0x0b => c.set_byte(0, b.mem[0x53c]),
            0x0c => b.gdcs[0].enabled = true,
            0x0d => b.gdcs[0].enabled = false,
            0x0e => {
                let start = c.r[DX] as usize / 2;
                b.gdcs[0].start = start as u32
            }
            0x10 => {
                b.gdcs[0].cursor_top = c.byte(1) & 31;
                b.gdcs[0].cursor_bottom = c.byte(5) & 31
            }
            0x11 => b.gdcs[0].cursor_visible = true,
            0x12 => b.gdcs[0].cursor_visible = false,
            0x13 => b.gdcs[0].cursor = (c.r[DX] / 2) as u32,
            0x14 if b.profile == crate::Profile::Pc9821ApDos
                && (0x21..=0x7e).contains(&c.byte(6)) =>
            {
                // Full-width JIS read: scratch word, then 16 left/right rows.
                for i in 0..32u16 {
                    let byte = b.user_characters.byte(c.byte(6), c.byte(2), i as usize);
                    b.write_seg(c.r[BX], c.r[CX].wrapping_add(2 + i), byte as u16, false);
                }
            }
            0x14 => {
                for y in 0..16 {
                    b.write_seg(
                        c.s[ES],
                        c.r[BX].wrapping_add(y),
                        crate::font::row(c.byte(6), y as u8) as u16,
                        false,
                    )
                }
            }
            0x16 => {
                for i in 0..2000 {
                    b.write_seg(0xa000, i * 2, c.r[DX] & 255, true);
                    b.write_seg(0xa200, i * 2, (c.r[DX] >> 8) & 255, true);
                }
            }
            0x17 => b.set_beep(true),
            0x18 => b.set_beep(false),
            0x1a if b.profile == crate::Profile::Pc9821ApDos => {
                if crate::font::UserCharacters::index(c.r[DX]).is_none() {
                    // Hardware does not validate codes; out-of-contract writes
                    // have no promised result. Do not invent a BIOS status code.
                    return Err(format!(
                        "unsupported user-character registration code {:04x}",
                        c.r[DX]
                    ));
                }
                let mut pattern = [0; 32];
                for (i, byte) in pattern.iter_mut().enumerate() {
                    *byte = b.read_seg(c.r[BX], c.r[CX].wrapping_add(2 + i as u16), false) as u8;
                }
                b.user_characters.define(c.r[DX], pattern);
            }
            0x40 => {
                b.gdcs[1].enabled = true;
                if b.profile == crate::Profile::Pc9821ApDos {
                    b.mem[0x54c] |= 128;
                }
            }
            0x41 => {
                b.gdcs[1].enabled = false;
                if b.profile == crate::Profile::Pc9821ApDos {
                    b.mem[0x54c] &= !128;
                }
            }
            0x42 => {
                if c.byte(5) & 0xc0 != 0xc0 {
                    return Err("unsupported graphics BIOS mode".into());
                }
                b.mem[0x54d] = c.byte(5);
                b.graphics_200 = false;
            }
            _ => return Err(format!("unsupported BIOS INT 18 AH={ah:02x}")),
        },
        0x1b => {
            if c.byte(0) & 0x7f == 0 && b.profile == crate::Profile::Pc9821ApDos {
                hdd_service(c, b);
                return Ok(());
            }
            let drive = c.byte(0) & 3;
            let device = c.byte(0) & 0xfc;
            if ![0x90, 0x30, 0xb0, 0x70].contains(&device)
                || drive > 1
                || b.disks[drive as usize].is_none()
            {
                c.set_byte(4, 0x80);
                carry(c, b, true);
                return Ok(());
            }
            let op = ah & 15;
            let success = match op {
                0 | 3 | 7 => {
                    b.fdc.cylinder[drive as usize] = 0;
                    true
                }
                4 => true,
                1 | 5 | 6 => {
                    let n = c.byte(5);
                    let mut cylinder = c.byte(1);
                    let mut h = c.byte(6);
                    let mut r = c.byte(2);
                    let mut left = c.r[BX] as usize;
                    let mut addr = Cpu::phys(c.s[ES], c.r[BP]);
                    let mut ok = true;
                    let sector_size = 128usize.checked_shl(n as u32).unwrap_or(0);
                    if n > 3 || left == 0 || !left.is_multiple_of(sector_size) {
                        return Err("invalid BIOS disk transfer size".into());
                    }
                    while left > 0 {
                        if !disk_transfer(
                            b,
                            drive,
                            [cylinder, h, r, n],
                            sector_size as u16,
                            addr,
                            op == 5,
                        ) {
                            ok = false;
                            break;
                        }
                        left -= sector_size;
                        if left == 0 {
                            break;
                        }
                        addr = (addr + sector_size) & 0xfffff;
                        if let Some(next) = r.checked_add(1).filter(|&next| {
                            b.disks[drive as usize]
                                .as_ref()
                                .unwrap()
                                .sector(cylinder, h, next, n)
                                .is_some()
                        }) {
                            r = next;
                        } else {
                            r = 1;
                            h ^= 1;
                            if h == 0 {
                                cylinder += 1;
                            }
                        }
                    }
                    ok
                }
                0x0a => {
                    if let Some(s) = b.disks[drive as usize]
                        .as_ref()
                        .unwrap()
                        .sectors
                        .iter()
                        .find(|s| s.c == c.byte(1) && s.h == c.byte(6))
                    {
                        c.r[CX] = s.c as u16 | ((s.n as u16) << 8);
                        c.r[DX] = ((s.h as u16) << 8) | s.r as u16;
                        true
                    } else {
                        false
                    }
                }
                _ => {
                    c.set_byte(4, 0x40);
                    carry(c, b, true);
                    return Ok(());
                }
            };
            c.set_byte(4, if success { 0 } else { 0x40 });
            carry(c, b, !success);
        }
        0x1c => {
            match ah {
                0 => {
                    // Deterministic RTC: 2026-01-01 Thu, 00:00:00. PC-98 packed BCD.
                    for (i, v) in [0x26, 0x14, 0x01, 0, 0, 0].into_iter().enumerate() {
                        b.write_seg(c.s[ES], c.r[BX].wrapping_add(i as u16), v, false)
                    }
                }
                2 => {
                    b.write_seg(0, 0x58a, 0, true);
                    let count = if c.r[CX] == 0 { 65_536 } else { c.r[CX] as u64 };
                    b.interval = Some(IntervalTimer {
                        deadline: b.cycles + count * 80_000, // 10 ms at 8 MHz
                        segment: c.s[ES],
                        offset: c.r[BX],
                    });
                }
                _ => return Err(format!("unsupported BIOS INT 1C AH={ah:02x}")),
            }
        }
        0x1e => return Err("Boot failed: no readable PC-98 floppy or IDE IPL".into()),
        0x1f => {
            let ok = ah == 0x90 && b.profile == crate::Profile::Pc9821ApDos && extended_copy(c, b);
            carry(c, b, !ok);
        }
        _ => {
            return Err(format!(
                "unhandled BIOS INT {int:02x} AH={ah:02x} at guest return {:04x}:{:04x}",
                b.read_seg(c.s[SS], c.r[SP].wrapping_add(2), true),
                b.read_seg(c.s[SS], c.r[SP], true)
            ))
        }
    }
    Ok(())
}

/// NEC INT 1F/90: CX bytes, ES:BX GDT, SI=DI=0 (RBIL release 61).
/// Only RAM data descriptors are accepted; invalid requests have no side effects.
fn extended_copy(c: &Cpu, b: &mut Bus) -> bool {
    if c.r[SI] != 0 || c.r[DI] != 0 {
        return false;
    }
    let table = Cpu::phys(c.s[ES], c.r[BX]);
    let descriptor = |offset: usize, write: bool| -> Option<usize> {
        let p = table.checked_add(offset)?;
        if p + 8 > 0xa0000 {
            return None;
        }
        let access = b.read(p + 5);
        // Present, ordinary data, expand-up. Destination must be writable.
        if access & 0xfc != 0x90 || (write && access & 2 == 0) || b.read(p + 6) != 0 {
            return None;
        }
        let base = b.read16(p + 2) as usize
            | ((b.read(p + 4) as usize) << 16)
            | ((b.read(p + 7) as usize) << 24);
        let len = c.r[CX] as usize;
        if len > b.read16(p) as usize + 1 {
            return None;
        }
        let end = base.checked_add(len)?;
        if (base < 0xa0000 && end <= 0xa0000) || (base >= 0x100000 && end <= 0x400000) {
            Some(base)
        } else {
            None
        }
    };
    let (Some(source), Some(dest)) = (descriptor(0x10, false), descriptor(0x18, true)) else {
        return false;
    };
    // Service accesses physical extended RAM without changing the caller's A20
    // latch. The CPU still cannot address beyond its real-mode segment limits.
    b.mem.copy_within(source..source + c.r[CX] as usize, dest);
    true
}

/// PC-98 IDE uses the SASI BIOS DA/UA 00h (LBA) or 80h (zero-based CHS).
/// Transfers use the same PIO task file and data buffer as guest port accesses.
fn hdd_service(c: &mut Cpu, b: &mut Bus) {
    let Some(disk) = b.ide.disk.as_ref() else {
        c.set_byte(4, 0x80);
        carry(c, b, true);
        return;
    };
    let ah = c.byte(4);
    let op = ah & 15;
    if op == 4 {
        c.r[BX] = 512;
        c.r[CX] = disk.cylinders;
        c.r[DX] = ((disk.heads as u16) << 8) | disk.sectors as u16;
        c.set_byte(4, 0);
        carry(c, b, false);
        return;
    }
    if matches!(op, 0 | 3 | 7 | 0xe) {
        b.output(0x74c, 4);
        b.output(0x74c, 0);
        c.set_byte(4, 0);
        carry(c, b, false);
        return;
    }
    let lba = if c.byte(0) & 128 != 0 {
        disk.chs(c.r[CX], c.byte(6), c.byte(2))
    } else {
        Some(c.r[CX] as usize | ((c.r[DX] as usize) << 16))
    };
    let count = c.r[BX] as usize / 512;
    let Some(mut lba) = lba.filter(|&l| {
        count != 0
            && c.r[BX].is_multiple_of(512)
            && l + count <= disk.sector_count()
            && matches!(op, 1 | 5 | 6)
    }) else {
        c.set_byte(4, 0x40);
        carry(c, b, true);
        return;
    };
    let mut addr = Cpu::phys(c.s[ES], c.r[BP]);
    b.output(0x432, 0);
    // Firmware polls, so mask controller IRQ while preserving guest PIC state.
    b.output(0x74c, 2);
    let mut ok = true;
    for _ in 0..count {
        b.output(0x644, 1);
        b.output(0x646, lba as u8);
        b.output(0x648, (lba >> 8) as u8);
        b.output(0x64a, (lba >> 16) as u8);
        b.output(0x64c, 0xe0 | ((lba >> 24) as u8 & 15));
        b.output(
            0x64e,
            if op == 5 {
                0x30
            } else if op == 1 {
                0x40
            } else {
                0x20
            },
        );
        if b.input(0x64e) & 1 != 0 {
            ok = false;
            break;
        }
        if op != 1 {
            for i in 0..512 {
                if op == 5 {
                    let v = b.read(addr + i);
                    b.output(0x640 + (i & 1) as u16, v);
                } else {
                    let v = b.input(0x640 + (i & 1) as u16);
                    b.write(addr + i, v);
                }
            }
            if b.input(0x64e) & 1 != 0 {
                ok = false;
                break;
            }
        }
        lba += 1;
        addr += 512;
    }
    b.output(0x74c, 0);
    c.set_byte(4, if ok { 0 } else { 0x40 });
    carry(c, b, !ok);
}

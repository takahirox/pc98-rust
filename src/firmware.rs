//! Newly written synthetic firmware services behind executable ROM thunks.
//! INT 21h is deliberately not implemented: DOS must execute its own kernel.
use crate::{bus::Bus, cpu::*};
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
    b.mem[0x55c] = 1 | if b.disks[1].is_some() { 2 } else { 0 };
    b.mem[0x584] = 0x90;
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
            }
            _ => return Err(format!("unsupported BIOS INT 18 AH={ah:02x}")),
        },
        0x1b => {
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
                }
                _ => return Err(format!("unsupported BIOS INT 1C AH={ah:02x}")),
            }
        }
        0x1e => {
            return Err(
                "Boot failed: drive 1 needs supported PC-98 1024-byte-sector boot media".into(),
            )
        }
        0x1f => {
            carry(c, b, true);
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

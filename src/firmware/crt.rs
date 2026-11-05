//! Original CRT BIOS adapter. NEC BIOS volume (1992), printed pp. 41–55.
//! See docs/crt-bios.md for the normal/high-resolution profile distinction.
use super::*;

fn command(b: &mut Bus, op: u8, params: &[u8]) {
    b.output(0x62, op);
    for &byte in params {
        b.output(0x60, byte);
    }
}
fn area(b: &mut Bus, index: u8, start: u16, lines: u16) {
    command(
        b,
        0x70 + index * 4,
        &[
            start as u8,
            (start >> 8) as u8,
            (lines << 4) as u8,
            (lines >> 4) as u8,
        ],
    );
}
fn single_area(b: &mut Bus, start: u16) {
    area(b, 0, start, 400);
    for index in 1..4 {
        area(b, index, 0, 0);
    }
}
fn cursor(b: &mut Bus, visible: bool, blink: bool, rate: u8, top: u8, bottom: u8) {
    let rate = if rate == 0 { 12 } else { rate };
    command(
        b,
        0x4b,
        &[
            (b.gdcs[0].character_lines - 1) | if visible { 128 } else { 0 },
            top | if blink { 0 } else { 32 } | ((rate & 3) << 6),
            (bottom << 3) | (rate >> 2),
        ],
    );
}
fn mode(b: &mut Bus, al: u8) {
    let height = if al & 1 != 0 { 20 } else { 16 };
    b.output(0x68, (al >> 2) & 1); // Attribute selection, then column selection.
    b.output(0x68, 2 | ((al >> 1) & 1));
    if b.profile == crate::Profile::Pc9821ApDos {
        b.output(0x68, 0x0a | ((al >> 3) & 1));
    }
    b.gdcs[0].character_lines = height;
    command(b, 0x47, &[80]);
    single_area(b, 0);
    cursor(b, b.gdcs[0].cursor_visible, true, 12, 0, height - 1);
}
pub(super) fn initialize(b: &mut Bus) {
    b.gdcs[0] = crate::devices::Gdc::default();
    b.output(0x68, 0x0f);
    mode(b, 0);
    b.mem[0x53c] = 0; // Normal mode: 80 columns, 25 rows, code access.
}
fn validate_mode(al: u8) -> ExecResult<()> {
    if al & 0xf0 != 0 {
        return Err("unsupported CRT mode (normal-resolution modes 00h..0fh only)".into());
    }
    Ok(())
}
fn validate_shape(b: &Bus, dh: u8, dl: u8) -> ExecResult<()> {
    if dh > dl || dl >= b.gdcs[0].character_lines {
        return Err("unsupported CRT cursor lines".into());
    }
    Ok(())
}
pub(super) fn service(c: &mut Cpu, b: &mut Bus, ah: u8) -> ExecResult<()> {
    let al = c.byte(0);
    match ah {
        0x0a => {
            validate_mode(al)?;
            let sense = al
                & if b.profile == crate::Profile::Pc9821ApDos {
                    15
                } else {
                    7
                };
            // Write through translated guest memory before changing devices.
            write(c, b, 0x53c, sense)?;
            mode(b, al);
        }
        0x0b => c.set_byte(0, read(c, b, 0x53c)?),
        0x0c => command(b, 0x0d, &[]),
        0x0d => command(b, 0x0c, &[]),
        0x0e => single_area(b, (c.r[DX] / 2) & 4095),
        0x0f => {
            let first = c.byte(6);
            let count = c.byte(2);
            if count == 0 || first >= 4 || count > 4 - first {
                return Err("unsupported CRT display-area range".into());
            }
            let mut entries = Vec::with_capacity(count as usize);
            let height = u16::from(b.gdcs[0].character_lines);
            for i in 0..u16::from(count) {
                let off = c.r[CX].wrapping_add(i * 4);
                let start = read_seg(c, b, c.r[BX], off, true)?;
                let rows = read_seg(c, b, c.r[BX], off.wrapping_add(2), true)?;
                if start >= 4096 || rows == 0 || rows > 400 / height {
                    return Err("unsupported CRT display-area entry".into());
                }
                entries.push((start, rows * height));
            }
            // Validate the entire guest list before the first device side effect.
            // Unselected partitions retain their parameter RAM contents.
            for (i, (start, lines)) in entries.into_iter().enumerate() {
                area(b, first + i as u8, start, lines);
            }
        }
        0x10 => {
            if al > 1 {
                return Err("unsupported CRT cursor blink selector".into());
            }
            cursor(
                b,
                b.gdcs[0].cursor_visible,
                al == 0,
                12,
                0,
                b.gdcs[0].character_lines - 1,
            );
        }
        0x11 | 0x12 => cursor(
            b,
            ah == 0x11,
            b.gdcs[0].cursor_blink,
            b.gdcs[0].blink_rate,
            b.gdcs[0].cursor_top,
            b.gdcs[0].cursor_bottom,
        ),
        0x13 => command(
            b,
            0x49,
            &[(c.r[DX] / 2) as u8, ((c.r[DX] / 2) >> 8) as u8, 0],
        ),
        0x1b => {
            if al > 1 || (al == 1 && b.profile != crate::Profile::Pc9821ApDos) {
                return Err("unsupported KCG access mode for profile".into());
            }
            let sense = read(c, b, 0x53c)?;
            write(c, b, 0x53c, (sense & !8) | (al << 3))?;
            b.output(0x68, 0x0a | al);
        }
        // These three are explicitly an Ap compatibility extension. NEC's
        // original contracts are high-resolution-only; no high-res CRT exists
        // in either profile. Do not advertise them as common VM ROM services.
        0x1c..=0x1e if b.profile == crate::Profile::Pc9821ApDos => match ah {
            0x1c => {
                if c.byte(6) > c.byte(2) || c.byte(2) >= 16 {
                    return Err("unsupported CRT initialization cursor lines".into());
                }
                write(c, b, 0x53c, (al & 32) >> 2)?;
                mode(b, (al & 32) >> 2);
                cursor(
                    b,
                    al & 64 != 0,
                    al & 128 != 0,
                    al & 31,
                    c.byte(6),
                    c.byte(2),
                );
            }
            0x1d => {
                if !(80..=162).contains(&al) || al & 1 != 0 {
                    return Err("unsupported CRT width (even columns 80..162 only)".into());
                }
                command(b, 0x47, &[al]);
            }
            _ => {
                validate_shape(b, c.byte(6), c.byte(2))?;
                cursor(
                    b,
                    b.gdcs[0].cursor_visible,
                    al & 128 != 0,
                    al & 31,
                    c.byte(6),
                    c.byte(2),
                );
            }
        },
        _ => return Err(format!("unsupported BIOS INT 18 AH={ah:02x} for profile").into()),
    }
    Ok(())
}

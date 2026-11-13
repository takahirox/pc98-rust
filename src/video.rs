//! Immutable scanout boundary shared by native, inline-Wasm and render Worker paths.
use crate::{devices::Gdc, font::UserCharacters, profile::Profile};

pub const HEADER: usize = 128;
pub const GLYPHS: usize = HEADER + 16 * 3 + 256 * 3;
pub const TEXT: usize = GLYPHS + 188 * 32;
pub const GRAPHICS: usize = TEXT + 0x4000;
pub const MAX_SNAPSHOT: usize = GRAPHICS + 0x80000;
const MAGIC: &[u8; 4] = b"V98\x01";

// One byte expands to eight palette-index bytes, in left-to-right pixel order.
// Plane shifts combine these lanes without carrying between pixels.
const PLANE_DOTS: [u64; 256] = {
    let mut table = [0; 256];
    let mut byte = 0;
    while byte < 256 {
        let mut pixel = 0;
        while pixel < 8 {
            table[byte] |= (((byte >> (7 - pixel)) & 1) as u64) << (pixel * 8);
            pixel += 1;
        }
        byte += 1;
    }
    table
};

pub struct VideoView<'a> {
    pub profile: Profile,
    pub cg_dot: bool,
    pub text_40: bool,
    pub text_graph: bool,
    pub cycles: u64,
    pub display_page: u8,
    pub graphics_200: bool,
    pub analog: bool,
    pub pegc: bool,
    pub text_enabled: bool,
    pub mem: &'a [u8],
    pub graphics: &'a [u8],
    pub gdcs: &'a [Gdc; 2],
    pub palette: &'a [[u8; 3]; 16],
    pub pegc_palette: &'a [[u8; 3]; 256],
    pub user_characters: &'a UserCharacters,
}
impl VideoView<'_> {
    /// Full, versioned snapshot, copied synchronously between guest slices.
    /// GRCG/EGC are write engines: their observable scanout is already in VRAM.
    pub fn snapshot(&self, out: &mut Vec<u8>) {
        out.resize(GRAPHICS + self.graphics.len(), 0);
        out[..HEADER].fill(0);
        out[..4].copy_from_slice(MAGIC);
        out[4] = u8::from(self.profile == Profile::Pc9821ApDos);
        for (i, value) in [
            self.cg_dot,
            self.text_40,
            self.text_graph,
            self.graphics_200,
            self.analog,
            self.pegc,
            self.text_enabled,
        ]
        .iter()
        .enumerate()
        {
            out[5] |= u8::from(*value) << i;
        }
        out[6] = self.display_page;
        out[8..16].copy_from_slice(&self.cycles.to_le_bytes());
        for (gdc, bytes) in self.gdcs.iter().zip(out[16..80].as_chunks_mut::<32>().0) {
            bytes[..8].copy_from_slice(&[
                u8::from(gdc.enabled),
                gdc.pitch,
                gdc.character_lines,
                u8::from(gdc.cursor_visible),
                gdc.cursor_top,
                gdc.cursor_bottom,
                u8::from(gdc.cursor_blink),
                gdc.blink_rate,
            ]);
            bytes[8..12].copy_from_slice(&gdc.start.to_le_bytes());
            bytes[12..16].copy_from_slice(&gdc.cursor.to_le_bytes());
            bytes[16..32].copy_from_slice(&gdc.scroll);
        }
        for (rgb, bytes) in self
            .palette
            .iter()
            .chain(self.pegc_palette.iter())
            .zip(out[HEADER..GLYPHS].as_chunks_mut::<3>().0)
        {
            bytes.copy_from_slice(rgb);
        }
        self.user_characters.snapshot(&mut out[GLYPHS..TEXT]);
        out[TEXT..GRAPHICS].copy_from_slice(self.mem);
        out[GRAPHICS..].copy_from_slice(self.graphics);
    }
    pub fn frame(&self, out: &mut [u8]) {
        let graphics = self.gdcs[1].enabled;
        let text = self.text_enabled && self.gdcs[0].enabled;
        let page = self.display_page as usize * self.profile.planes() * 0x8000;
        let gdc = &self.gdcs[0];
        let height = gdc.character_lines.max(1) as usize;
        let frame = self.cycles / 133_333;
        let blink_off =
            frame % (8 * u64::from(gdc.blink_rate.max(1))) >= 6 * u64::from(gdc.blink_rate.max(1));
        let cursor_on = gdc.cursor_on(frame);
        let cell_pixels = if self.text_40 { 16 } else { 8 };
        let mut colors = [[0; 4]; 256];
        let palette = if self.pegc {
            &self.pegc_palette[..]
        } else {
            &self.palette[..]
        };
        for (color, rgb) in colors.iter_mut().zip(palette) {
            *color = [rgb[0], rgb[1], rgb[2], 255];
        }
        for y in 0..400 {
            for left in (0..640).step_by(cell_pixels) {
                // Address, glyph and attributes are shared by a character row.
                // Decode once, then composite its dots over the current bitmap.
                // In 40-column mode each span doubles its dots and skips the
                // odd VRAM word, so its physical column is still left / 8.
                let mut ink = 0;
                let mut text_color = 0;
                if let Some((cell, glyph_y)) = text.then(|| gdc.text_address(y, left / 8)).flatten()
                {
                    let ch = self.mem[cell * 2];
                    let attr = self.mem[0x2000 + cell * 2];
                    let high = self.mem[cell * 2 + 1];
                    ink = if self.profile == Profile::Pc9821ApDos && high != 0 {
                        // Dot access blanks text Kanji while preserving glyph RAM.
                        if self.cg_dot || glyph_y >= 16 {
                            0
                        } else {
                            self.user_characters.byte(
                                (ch & 127).wrapping_add(0x20),
                                high,
                                glyph_y * 2 + usize::from(ch & 128 != 0),
                            )
                        }
                    } else if self.text_graph && attr & 16 != 0 {
                        if glyph_y < 16 {
                            let blocks = ch >> (glyph_y / 4 * 2);
                            (if blocks & 1 != 0 { 0xf0 } else { 0 })
                                | (if blocks & 2 != 0 { 0x0f } else { 0 })
                        } else {
                            0
                        }
                    } else if glyph_y < 16 {
                        crate::font::row(ch, glyph_y as u8)
                    } else {
                        0
                    };
                    if attr & 4 != 0 {
                        ink = !ink;
                    }
                    if attr & 8 != 0 && glyph_y == height - 1 {
                        ink = 255;
                    }
                    if !self.text_graph && attr & 16 != 0 {
                        ink |= 1;
                    }
                    if attr & 2 != 0 && blink_off {
                        ink = 0;
                    }
                    if cursor_on
                        && cell == gdc.cursor as usize
                        && (gdc.cursor_top..=gdc.cursor_bottom).contains(&(glyph_y as u8))
                    {
                        ink = !ink;
                    }
                    if attr & 1 == 0 {
                        ink = 0;
                    }
                    text_color = (attr >> 5) & 7;
                }
                let text_rgb = [
                    if text_color & 2 != 0 { 255 } else { 0 },
                    if text_color & 4 != 0 { 255 } else { 0 },
                    if text_color & 1 != 0 { 255 } else { 0 },
                    255,
                ];
                let mut planar = 0;
                for x in left..left + cell_pixels {
                    let mut color = 0;
                    if graphics && self.pegc {
                        let start =
                            self.display_page as usize * 0x40000 + self.gdcs[1].start as usize * 8;
                        let a = (start + y * self.gdcs[1].pitch as usize * 8 + x) & 0x7ffff;
                        color = self.graphics[a];
                    } else if graphics {
                        if x & 7 == 0 {
                            let line = if self.graphics_200 { y / 2 } else { y };
                            let a = ((self.gdcs[1].start as usize * 2 + line * 80 + x / 8)
                                & 0x7fff)
                                + page;
                            planar = 0;
                            for plane in 0..if self.analog {
                                self.profile.planes()
                            } else {
                                3
                            } {
                                planar |=
                                    PLANE_DOTS[self.graphics[a + plane * 0x8000] as usize] << plane;
                            }
                        }
                        color = planar as u8;
                        planar >>= 8;
                    }
                    let dot = if self.text_40 { x / 2 } else { x };
                    let text_ink = ink & (128 >> (dot % 8)) != 0;
                    let rgb = if text_ink && self.profile == Profile::Pc9821ApDos {
                        &text_rgb
                    } else {
                        if text_ink {
                            color = text_color;
                        }
                        &colors[color as usize]
                    };
                    let idx = (y * 640 + x) * 4;
                    out[idx..idx + 4].copy_from_slice(rgb);
                }
            }
        }
    }
}

/// Validate before interpreting any offsets. The renderer has no Machine/Bus.
/// Reuse scratch glyph/text storage; immutable ROM glyphs live in the module.
#[derive(Default)]
pub struct SnapshotRenderer {
    glyphs: UserCharacters,
    previous: Vec<u8>,
    pixels: Vec<u8>,
    blink_phase: (bool, bool),
}
impl SnapshotRenderer {
    pub fn render(&mut self, bytes: &[u8], out: &mut [u8]) -> bool {
        if out.len() != 640 * 400 * 4 {
            return false;
        }
        if let Some(pixels) = self.frame(bytes) {
            out.copy_from_slice(pixels);
            true
        } else {
            false
        }
    }
    /// Retain pixels inside the renderer so the Worker can transfer them without
    /// an extra full-frame cache copy, including when every frame changes.
    pub fn frame(&mut self, bytes: &[u8]) -> Option<&[u8]> {
        if self.update(bytes)? {
            self.previous.clear();
            self.previous.extend_from_slice(bytes);
        }
        Some(&self.pixels)
    }
    /// Recycle the previous input allocation on a changed frame. The Worker
    /// obtains a fresh transfer pointer for each incoming snapshot.
    pub fn frame_owned(&mut self, bytes: &mut Vec<u8>) -> Option<&[u8]> {
        if self.update(bytes)? {
            std::mem::swap(&mut self.previous, bytes);
        }
        Some(&self.pixels)
    }
    fn update(&mut self, bytes: &[u8]) -> Option<bool> {
        if bytes.len() < GRAPHICS
            || &bytes[..4] != MAGIC
            || bytes[4] > 1
            || bytes[6] > 1
            || bytes[5] & 128 != 0
        {
            return None;
        }
        let profile = if bytes[4] == 0 {
            Profile::Pc9801Vm
        } else {
            Profile::Pc9821ApDos
        };
        let graphics_len = if profile == Profile::Pc9801Vm {
            0x30000
        } else {
            0x80000
        };
        if bytes.len() != GRAPHICS + graphics_len
            || profile == Profile::Pc9801Vm && bytes[5] & 32 != 0
        {
            return None;
        }
        let gdcs = std::array::from_fn(|i| {
            let b = &bytes[16 + i * 32..48 + i * 32];
            let mut g = Gdc {
                enabled: b[0] != 0,
                pitch: b[1],
                character_lines: b[2],
                cursor_visible: b[3] != 0,
                cursor_top: b[4],
                cursor_bottom: b[5],
                cursor_blink: b[6] != 0,
                blink_rate: b[7],
                start: u32::from_le_bytes(b[8..12].try_into().unwrap()),
                cursor: u32::from_le_bytes(b[12..16].try_into().unwrap()),
                ..Gdc::default()
            };
            g.scroll.copy_from_slice(&b[16..32]);
            g
        });
        let cycles = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let frame = cycles / 133_333;
        let gdc = &gdcs[0];
        let text = bytes[5] & 64 != 0 && gdc.enabled;
        let rate = u64::from(gdc.blink_rate.max(1));
        let blink_phase = (
            text && frame % (8 * rate) >= 6 * rate,
            text && gdc.cursor_on(frame),
        );
        // Time affects scanout only through text/cursor blink. Compare the
        // complete remaining snapshot, including palettes and writable glyphs,
        // rather than guessing which guest writes might affect the screen.
        if self.previous.len() == bytes.len()
            && self.blink_phase == blink_phase
            && self.previous[..8] == bytes[..8]
            && self.previous[16..] == bytes[16..]
        {
            return Some(false);
        }
        let palette = std::array::from_fn(|i| {
            bytes[HEADER + i * 3..HEADER + i * 3 + 3]
                .try_into()
                .unwrap()
        });
        let pegc_palette = std::array::from_fn(|i| {
            bytes[HEADER + 48 + i * 3..HEADER + 48 + i * 3 + 3]
                .try_into()
                .unwrap()
        });
        self.glyphs.restore_snapshot(&bytes[GLYPHS..TEXT]);
        self.pixels.resize(640 * 400 * 4, 0);
        VideoView {
            profile,
            cg_dot: bytes[5] & 1 != 0,
            text_40: bytes[5] & 2 != 0,
            text_graph: bytes[5] & 4 != 0,
            graphics_200: bytes[5] & 8 != 0,
            analog: bytes[5] & 16 != 0,
            pegc: bytes[5] & 32 != 0,
            text_enabled: bytes[5] & 64 != 0,
            display_page: bytes[6],
            cycles,
            mem: &bytes[TEXT..GRAPHICS],
            graphics: &bytes[GRAPHICS..],
            gdcs: &gdcs,
            palette: &palette,
            pegc_palette: &pegc_palette,
            user_characters: &self.glyphs,
        }
        .frame(&mut self.pixels);
        self.blink_phase = blink_phase;
        Some(true)
    }
}

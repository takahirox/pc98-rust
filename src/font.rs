//! Original compact Latin glyphs (MIT) and a licensed Shinonome JIS CG.
//! See fixtures/fonts/README.md for the unchanged upstream source/license.
//! Five columns, seven rows expanded to the PC-98 8x16 cell. No ROM data.
pub fn row(ch: u8, y: u8) -> u8 {
    if !(1..15).contains(&y) {
        return 0;
    }
    let pattern = ank_pattern(ch).unwrap_or([31, 17, 5, 4, 4, 0, 4]);
    pattern[((y - 1) / 2) as usize] << 2
}

/// Whether the original ANK font has this code, rather than its fallback glyph.
pub fn ank_supported(ch: u8) -> bool {
    ank_pattern(ch).is_some()
}

fn ank_pattern(ch: u8) -> Option<[u8; 7]> {
    Some(match ch.to_ascii_uppercase() {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        b'.' => [0, 0, 0, 0, 0, 0, 4],
        b',' => [0, 0, 0, 0, 0, 4, 8],
        b':' => [0, 4, 0, 0, 4, 0, 0],
        b';' => [0, 4, 0, 0, 4, 4, 8],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'_' => [0, 0, 0, 0, 0, 0, 31],
        b'=' => [0, 0, 31, 0, 31, 0, 0],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'\\' => [16, 8, 8, 4, 2, 2, 1],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b'[' => [14, 8, 8, 8, 8, 8, 14],
        b']' => [14, 2, 2, 2, 2, 2, 14],
        b'<' => [1, 2, 4, 8, 4, 2, 1],
        b'>' => [16, 8, 4, 2, 4, 8, 16],
        b'*' => [0, 21, 14, 31, 14, 21, 0],
        b'#' => [10, 31, 10, 10, 31, 10, 0],
        b'@' => [14, 17, 23, 21, 23, 16, 14],
        b'"' => [10, 10, 0, 0, 0, 0, 0],
        b'\'' => [4, 4, 0, 0, 0, 0, 0],
        b' ' => [0; 7],
        _ => return None,
    })
}

/// JIS row/cell lookup. Rows are 16-bit MSB-first, left byte then right byte.
/// The source and conversion are reproduced by tools/build_font.py.
pub fn jis_byte(row: u8, cell: u8, byte: usize) -> u8 {
    if !(0x21..=0x7e).contains(&row) || !(0x21..=0x7e).contains(&cell) || byte >= 32 {
        return 0;
    }
    static JIS: &[u8; 94 * 94 * 32] = include_bytes!("../fixtures/fonts/jis0208.bin");
    JIS[((row as usize - 0x21) * 94 + cell as usize - 0x21) * 32 + byte]
}

/// Ap user-character RAM: two JIS rows, 94 full-width characters per row.
/// Kept separately from the immutable replacement font; machine reset clears it.
pub struct UserCharacters {
    patterns: Vec<[u8; 32]>,
}
impl Default for UserCharacters {
    fn default() -> Self {
        Self {
            patterns: vec![[0; 32]; 188],
        }
    }
}
impl UserCharacters {
    pub fn index(code: u16) -> Option<usize> {
        let row = (code >> 8) as u8;
        let cell = code as u8;
        if (0x76..=0x77).contains(&row) && (0x21..=0x7e).contains(&cell) {
            Some((row as usize - 0x76) * 94 + cell as usize - 0x21)
        } else {
            None
        }
    }
    pub fn define(&mut self, code: u16, pattern: [u8; 32]) {
        self.patterns[Self::index(code).expect("validated user-character code")] = pattern;
    }
    pub fn byte(&self, row: u8, cell: u8, byte: usize) -> u8 {
        if let Some(index) = Self::index(u16::from_be_bytes([row, cell])) {
            self.patterns[index].get(byte).copied().unwrap_or(0)
        } else {
            jis_byte(row, cell, byte)
        }
    }
}

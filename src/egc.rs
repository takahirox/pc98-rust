//! Original EGC aligned raster subset, from UNDOCUMENTED 9801/9821 io_egc.txt.
use std::cell::Cell;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Egc {
    pub enabled: bool,
    pub registers: [u16; 8],
    source: Cell<[[u8; 4]; 2]>,
    pattern: Cell<[[u8; 4]; 2]>,
}
impl Default for Egc {
    fn default() -> Self {
        Self {
            enabled: false,
            registers: [0xfff0, 0x00ff, 0x2000, 0, 0, 0, 0, 15],
            source: Cell::new([[0; 4]; 2]),
            pattern: Cell::new([[0; 4]; 2]),
        }
    }
}
impl Egc {
    pub fn read(&self, data: [u8; 4], lane: usize) -> u8 {
        let mut source = self.source.get();
        source[lane] = data;
        self.source.set(source);
        if self.registers[2] & 0x300 == 0x100 {
            let mut pattern = self.pattern.get();
            pattern[lane] = data;
            self.pattern.set(pattern);
        }
        if self.registers[2] & 0x2000 != 0 {
            data[(self.registers[1] >> 8) as usize & 3]
        } else {
            data.iter().enumerate().fold(255, |v, (plane, byte)| {
                v & if self.registers[3] & (1 << plane) != 0 {
                    *byte
                } else {
                    !byte
                }
            })
        }
    }
    pub fn write(&self, data: [u8; 4], value: u8, lane: usize) -> [u8; 4] {
        let mut result = data;
        let source = self.source.get()[lane];
        let pattern = self.pattern.get()[lane];
        for plane in 0..4 {
            if self.registers[0] & (1 << plane) != 0 {
                continue;
            }
            let p = match self.registers[1] & 0x6000 {
                0x4000 => {
                    if self.registers[3] & (1 << plane) != 0 {
                        255
                    } else {
                        0
                    }
                }
                0x2000 => {
                    if self.registers[5] & (1 << plane) != 0 {
                        255
                    } else {
                        0
                    }
                }
                _ => pattern[plane],
            };
            let s = if self.registers[2] & 0x400 != 0 {
                value
            } else {
                source[plane]
            };
            let d = data[plane];
            let mut raster = 0;
            for index in 0..8 {
                if self.registers[2] & (1 << index) != 0 {
                    raster |= (if index & 4 != 0 { s } else { !s })
                        & (if index & 2 != 0 { d } else { !d })
                        & (if index & 1 != 0 { p } else { !p });
                }
            }
            let v = match self.registers[2] & 0x1800 {
                0x800 => raster,
                0x1000 => p,
                _ => value,
            };
            let mask = (self.registers[4] >> (lane * 8)) as u8;
            result[plane] = (d & mask) | (v & !mask);
        }
        if self.registers[2] & 0x300 == 0x200 {
            let mut pattern = self.pattern.get();
            pattern[lane] = data;
            self.pattern.set(pattern);
        }
        result
    }
}

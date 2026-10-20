//! Flat PC-98 media and single-disk D88. Parsing never trusts file offsets.
#[derive(Clone, Debug)]
pub struct Sector {
    pub c: u8,
    pub h: u8,
    pub r: u8,
    pub n: u8,
    pub offset: usize,
    pub len: usize,
}
#[derive(Clone)]
pub struct Disk {
    pub bytes: Vec<u8>,
    pub sectors: Vec<Sector>,
    pub protected: bool,
    pub writes: u64,
}
impl Disk {
    pub fn parse(bytes: Vec<u8>, name: &str) -> Result<Self, String> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("floppy image exceeds 4 MiB".into());
        }
        let mut sectors = Vec::new();
        let mut protected = false;
        if name.to_ascii_lowercase().ends_with(".d88")
            || name.to_ascii_lowercase().ends_with(".d98")
        {
            if bytes.len() < 0x2a0 {
                return Err("truncated D88 header".into());
            }
            let u32at = |p: usize| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
            if u32at(0x1c) != bytes.len() {
                return Err("D88 size mismatch or multiple disks (split before loading)".into());
            }
            if bytes[0x1b] != 0x20 && bytes[0x1b] != 0x10 {
                return Err("D88 media must be 2HD or 2DD".into());
            }
            protected = bytes[0x1a] != 0;
            let header = (0..160)
                .map(|i| u32at(0x20 + i * 4))
                .find(|&x| x != 0)
                .ok_or("D88 has no tracks")?;
            if header != 0x2a0 && header != 0x2b0 {
                return Err("invalid D88 track table".into());
            }
            let tracks = (header - 0x20) / 4;
            let mut previous = header - 1;
            for t in 0..tracks {
                let mut p = u32at(0x20 + t * 4);
                if p == 0 || p == bytes.len() {
                    continue;
                }
                if p < header || p <= previous || p + 16 > bytes.len() {
                    return Err("invalid D88 track offset".into());
                }
                previous = p;
                let end = ((t + 1)..tracks)
                    .map(|i| u32at(0x20 + i * 4))
                    .find(|&x| x != 0)
                    .unwrap_or(bytes.len());
                if end > bytes.len() || end <= p {
                    return Err("overlapping D88 tracks".into());
                }
                let count = u16::from_le_bytes([bytes[p + 4], bytes[p + 5]]) as usize;
                if count == 0 || count > 64 {
                    return Err("invalid D88 sector count".into());
                }
                for _ in 0..count {
                    if p + 16 > end {
                        return Err("truncated D88 sector header".into());
                    }
                    let c = bytes[p];
                    let h = bytes[p + 1];
                    let r = bytes[p + 2];
                    let n = bytes[p + 3];
                    let len = u16::from_le_bytes([bytes[p + 14], bytes[p + 15]]) as usize;
                    if c > 81
                        || h > 1
                        || r == 0
                        || n > 3
                        || len != 128usize << n
                        || u16::from_le_bytes([bytes[p + 4], bytes[p + 5]]) as usize != count
                        || bytes[p + 6] != 0
                        || bytes[p + 7] != 0
                        || bytes[p + 8] != 0
                    {
                        return Err(
                            "unsupported or corrupt D88 sector (requires normal MFM CHRN)".into(),
                        );
                    }
                    if p + 16 + len > end
                        || sectors
                            .iter()
                            .any(|s: &Sector| s.c == c && s.h == h && s.r == r)
                    {
                        return Err("truncated or duplicate D88 sector".into());
                    }
                    sectors.push(Sector {
                        c,
                        h,
                        r,
                        n,
                        offset: p + 16,
                        len,
                    });
                    p += 16 + len;
                }
                if p != end {
                    return Err("D88 track length mismatch".into());
                }
            }
        } else {
            let (cyl, spt, size, n) =
                match bytes.len() {
                    1261568 => (77, 8, 1024, 3),
                    1228800 => (80, 15, 512, 2),
                    655360 => (80, 8, 512, 2),
                    _ => return Err(
                        "unsupported raw geometry; expected 77x2x8x1024, 80x2x15x512 or 80x2x8x512"
                            .into(),
                    ),
                };
            for c in 0..cyl {
                for h in 0..2 {
                    for r in 1..=spt {
                        sectors.push(Sector {
                            c: c as u8,
                            h: h as u8,
                            r: r as u8,
                            n,
                            offset: ((c * 2 + h) * spt + r - 1) * size,
                            len: size,
                        });
                    }
                }
            }
        }
        Ok(Self {
            bytes,
            sectors,
            protected,
            writes: 0,
        })
    }
    pub fn sector(&self, c: u8, h: u8, r: u8, n: u8) -> Option<&Sector> {
        self.sectors
            .iter()
            .find(|s| s.c == c && s.h == h && s.r == r && s.n == n)
    }
}

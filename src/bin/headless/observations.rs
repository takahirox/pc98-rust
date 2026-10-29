//! Bounded local observations. No code bytes, disk payload or decoded guest text.
use pc98_rust::{bus::Bus, cpu::Cpu, font, Profile};
use std::collections::{BTreeMap, VecDeque};

#[derive(Default)]
pub struct Progress {
    // Exact uniqueness only in this rolling window, never an unbounded address set.
    window: VecDeque<(u16, u32, u8)>,
    pub attempts: u64,
    bios: BTreeMap<u8, u64>,
    disk_calls: VecDeque<String>,
    disk_pending: bool,
}
impl Progress {
    pub fn observe(&mut self, c: &Cpu, _b: &Bus) {
        self.attempts += 1;
        if self.window.len() == 4096 {
            self.window.pop_front();
        }
        let mode = u8::from(c.protected_mode())
            | (u8::from(c.vm86()) << 1)
            | (u8::from(c.paging_enabled()) << 2);
        self.window.push_back((c.s[1], c.ip, mode));
        if !c.protected_mode() && c.s[1] == 0xf000 && c.ip == 0x106e && self.disk_pending {
            if let Some(last) = self.disk_calls.back_mut() {
                last.push_str(&format!(" returned-ah={:02x}", c.byte(4)));
            }
            self.disk_pending = false;
        }
        if !c.protected_mode() && c.s[1] == 0xf000 && c.ip == 0x106c {
            if self.disk_calls.len() == 16 {
                self.disk_calls.pop_front();
            }
            self.disk_calls.push_back(format!(
                "bios-disk ax={:04x} bytes={} cx={:04x} dx={:04x} buffer={:04x}:{:04x}",
                c.r[0], c.r[3], c.r[1], c.r[2], c.s[0], c.r[5]
            ));
            self.disk_pending = true;
        }
        // Count actual entries to synthetic firmware thunks, including hardware
        // vectors. Software INT instructions with guest-installed handlers are
        // deliberately not labelled BIOS calls.
        if !c.protected_mode()
            && c.s[1] == 0xf000
            && (0x1000..0x1400).contains(&c.ip)
            && c.ip & 3 == 0
        {
            *self.bios.entry(((c.ip - 0x1000) / 4) as u8).or_default() += 1;
        }
    }
    pub fn disk_history(&self) -> impl Iterator<Item = &str> {
        self.disk_calls.iter().map(String::as_str)
    }
    pub fn report(&self, slots: u64, c: &Cpu, b: &Bus) -> String {
        let unique = self
            .window
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let repeat = self.window.len() - unique;
        let bios: Vec<_> = self
            .bios
            .iter()
            .map(|(n, count)| format!("{n:02x}:{count}"))
            .collect();
        format!(
            "progress slots={slots} attempts={} retired={} window={} unique={unique} repeats={repeat} pc={:04x}:{:08x} pe={} vm={} pg={} flags={:04x}{:04x} halted={} shutdown={} faults={} last-fault={:?} irq-acks={} pit0={}/{}/{} pic={:02x}/{:02x}/{:02x},{:02x}/{:02x}/{:02x} bios=[{}]\nide {}",
            self.attempts, c.retired, self.window.len(), c.s[1], c.ip,
            u8::from(c.protected_mode()), u8::from(c.vm86()), u8::from(c.paging_enabled()),
            c.flags_upper, c.flags, c.halted, c.shutdown, c.fault_count, c.last_fault,
            c.irq_acks, b.pit.c[0].reload, b.pit.c[0].remaining, u8::from(b.pit.c[0].output),
            b.pics[0].mask, b.pics[0].irr, b.pics[0].isr,
            b.pics[1].mask, b.pics[1].irr, b.pics[1].isr, bios.join(","), b.ide.diagnostic()
        )
    }
}

pub fn text_summary(b: &Bus) -> String {
    let mut cells = BTreeMap::new();
    let (mut literal, mut fallback, mut jis, mut invalid_jis) = (0, 0, 0, 0);
    for visible in 0..2000 {
        let cell = (visible + b.gdcs[0].start as usize) % 2048;
        let low = b.mem[0xa0000 + cell * 2];
        let high = b.mem[0xa0001 + cell * 2];
        let attr = b.mem[0xa2000 + cell * 2];
        let attr_high = b.mem[0xa2001 + cell * 2];
        *cells.entry((low, high, attr, attr_high)).or_insert(0usize) += 1;
        if b.profile == Profile::Pc9821ApDos && high != 0 {
            jis += 1;
            if !(0x21..=0x7e).contains(&(low & 127).wrapping_add(0x20))
                || !(0x21..=0x7e).contains(&high)
            {
                invalid_jis += 1;
            }
        } else {
            literal += usize::from(low == b'?');
            fallback += usize::from(!font::ank_supported(low));
        }
    }
    let mut frequent: Vec<_> = cells.into_iter().collect();
    frequent.sort_by_key(|&(bytes, count)| (std::cmp::Reverse(count), bytes));
    let distinct = frequent.len();
    frequent.truncate(16);
    let raw: Vec<_> = frequent
        .iter()
        .map(|((lo, hi, a, ah), n)| format!("{lo:02x}/{hi:02x}/{a:02x}/{ah:02x}:{n}"))
        .collect();
    format!(
        "text-vram cells=2000 literal-3f={literal} ank-fallback={fallback} jis={jis} invalid-jis={invalid_jis} distinct={distinct} top16-low/high/attr/attr-high=[{}] text-enable={} text-gdc={}/{} graphics-gdc={}/{} display-page={} pegc={}",
        raw.join(","), b.text_enabled, b.gdcs[0].enabled, b.gdcs[0].start,
        b.gdcs[1].enabled, b.gdcs[1].start, b.display_page, b.pegc
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_uses_visible_ring_and_both_planes_without_changing_glyphs() {
        let mut b = Bus::new(Profile::Pc9821ApDos);
        // Scroll across the text ring boundary; a literal outside the viewport
        // must not be counted. Attributes are word-spaced independently.
        for cell in 0..2048 {
            b.mem[0xa0000 + cell * 2] = b' ';
        }
        b.gdcs[0].start = 2047;
        b.text_enabled = false;
        b.gdcs[1].enabled = true;
        b.gdcs[1].start = 123;
        for (cell, low, high, attr_high) in [
            (2047, b'?', 0, 0x55),
            (0, 1, 0, 0),
            (1, 4, 0x4f, 0),
            (2046, b'?', 0, 0),
        ] {
            b.mem[0xa0000 + cell * 2] = low;
            b.mem[0xa0001 + cell * 2] = high;
            b.mem[0xa2000 + cell * 2] = 0xe1;
            b.mem[0xa2001 + cell * 2] = attr_high;
        }
        let summary = text_summary(&b);
        assert!(
            summary.contains("literal-3f=1 ank-fallback=1 jis=1 invalid-jis=0"),
            "{summary}"
        );
        assert!(summary.contains("3f/00/e1/55:1"));
        assert!(summary.contains("text-enable=false text-gdc=false/2047 graphics-gdc=true/123"));
        // Preserve the two different question-like bitmap patterns. Diagnostic
        // classification never replaces unsupported bytes with spaces.
        assert!(font::ank_supported(b'?'));
        assert!(!font::ank_supported(1));
        assert_eq!(font::row(b'?', 1), 14 << 2);
        assert_eq!(font::row(1, 1), 31 << 2);
        assert_eq!(font::row(b' ', 1), 0);
    }
}

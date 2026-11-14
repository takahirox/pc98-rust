//! Instruction clocks and explicit bus-policy penalties. See docs/cpu-timing.md.
use crate::{cpu::Cpu, Profile};

pub const SCHEDULER_HZ: u64 = 8_000_000;

/// Elapsed core clocks for one bounded execution unit, even when it faults.
/// Firmware waits are already applied to devices and must not be ticked again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepTiming {
    pub execution: u32,
    pub bus: u32,
    pub idle: u32,
    pub firmware_scheduler: u64,
    pub instructions: u32,
    pub rep_elements: u32,
    /// A table range, fault path, firmware service or board policy was used.
    pub approximated: bool,
}
impl StepTiming {
    pub fn cpu_cycles(self) -> u32 {
        self.execution + self.bus + self.idle
    }
}

/// Rational clock conversion. The remainder belongs to the machine, not a slice.
#[derive(Default)]
pub struct ClockConverter {
    remainder: u64,
}
impl ClockConverter {
    #[cfg(feature = "block-experiment")]
    pub(crate) fn before_deadline(&self, clocks: u64, profile: Profile) -> u32 {
        // Largest CPU budget whose converted scheduler time stays strictly
        // before the event. Preserve the fraction accumulated by prior units.
        let scaled = clocks
            .saturating_mul(profile.cpu_hz())
            .saturating_sub(1 + self.remainder);
        (scaled / SCHEDULER_HZ).min(u64::from(u32::MAX)) as u32
    }
    pub fn advance(&mut self, profile: Profile, cycles: u32) -> u32 {
        let scaled = self.remainder + u64::from(cycles) * SCHEDULER_HZ;
        let hz = profile.cpu_hz();
        self.remainder = scaled % hz;
        (scaled / hz) as u32
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Instruction {
    pub op: u8,
    pub ext: u8,
    pub modrm: u8,
    pub prefixes: u32,
    pub rep: bool,
    pub continuation: bool,
    pub count: u32,
    pub width: u32,
    pub cl: u8,
    pub flags: u16,
    pub protected: bool,
    pub vm86: bool,
    pub io_checked: bool,
}
impl Instruction {
    pub fn new(c: &Cpu, op: u8, prefixes: u32, rep: u8) -> Self {
        Self {
            op,
            prefixes: prefixes
                - u32::from(rep != 0 && matches!(op, 0x6c..=0x6f | 0xa4..=0xa7 | 0xaa..=0xaf)),
            rep: rep != 0,
            count: if c.address32 {
                c.reg32(1)
            } else {
                c.r[1] as u32
            },
            width: if c.operand32 { 4 } else { 2 },
            cl: c.byte(1),
            flags: c.flags,
            protected: c.protected_mode(),
            vm86: c.vm86(),
            io_checked: c.vm86() || c.cpl > ((c.flags >> 12) & 3) as u8,
            ..Self::default()
        }
    }
    pub fn string(self) -> bool {
        matches!(self.op, 0x6c..=0x6f | 0xa4..=0xa7 | 0xaa..=0xaf)
    }
}

/// Timing-only unified i486 cache: 128 sets, four 16-byte lines, round-robin
/// replacement. Values always come from the physical Bus; no cached guest data.
/// Replacement, serialized fills and board waits are explicit approximations.
pub(crate) struct TimingState {
    pub active: bool,
    pub untimed: Option<(u8, u8)>,
    pub instruction: Option<Instruction>,
    /// Original instruction fault, before any fault raised by event delivery.
    pub fault_vector: Option<u8>,
    pub taken: bool,
    pub invlpg_hit: bool,
    pub element: bool,
    pub extra_execution: u32,
    /// Successful address-calculation latency, replaced by DX2 #PF timing.
    pub address_execution: u32,
    pub penalty: u32,
    pub approximated: bool,
    pub last_byte: u8,
    pub value: u32,
    pub previous_writes: u8,
    pub rep_fetch: bool,
    // A physical address divided by 16 can never equal usize::MAX. A sentinel
    // keeps each tag to one word and each hot cache probe to one comparison.
    cache: [[usize; 4]; 128],
    cache_populated: bool,
    victim: [usize; 128],
    fetch_word: Option<usize>,
    cache_enabled: bool,
}
impl Default for TimingState {
    fn default() -> Self {
        Self {
            active: false,
            untimed: None,
            instruction: None,
            fault_vector: None,
            taken: false,
            invlpg_hit: false,
            element: false,
            extra_execution: 0,
            address_execution: 0,
            penalty: 0,
            approximated: false,
            last_byte: 0,
            value: 0,
            previous_writes: 0,
            rep_fetch: false,
            cache: [[usize::MAX; 4]; 128],
            cache_populated: false,
            victim: [0; 128],
            fetch_word: None,
            cache_enabled: false,
        }
    }
}
impl TimingState {
    #[cfg(feature = "block-experiment")]
    pub(crate) fn block_fetch_end(&mut self, last: usize, byte: u8) {
        self.fetch_word = Some(last / 4);
        self.last_byte = byte;
        self.approximated = true;
    }
    pub fn begin(&mut self, cr0: u32) {
        self.active = true;
        self.untimed = None;
        self.instruction = None;
        self.fault_vector = None;
        self.taken = false;
        self.invlpg_hit = false;
        self.element = false;
        self.extra_execution = 0;
        self.address_execution = 0;
        self.penalty = 0;
        self.approximated = false;
        self.last_byte = 0;
        self.value = 0;
        self.rep_fetch = false;
        self.fetch_word = None;
        self.cache_enabled = cr0 & 0x40000000 == 0;
    }
    /// Continue a register-only Ap JIT run after its first normal begin.
    /// Mode, cache enable, REP/fault state and extra execution stay invariant.
    #[inline(always)]
    pub(crate) fn begin_jit_register(&mut self, previous_writes: u8) {
        self.active = true;
        self.taken = false;
        self.address_execution = 0;
        self.penalty = 0;
        self.fetch_word = None;
        self.previous_writes = previous_writes;
    }
    /// A guarded JIT instruction occupies at most two conventional-RAM cache
    /// lines. Repeated four-byte fetch beats in one line cannot evict it.
    #[inline(always)]
    pub(crate) fn jit_fetch(&mut self, address: usize, length: usize, last_byte: u8) {
        debug_assert!(self.active && !self.rep_fetch && (1..=7).contains(&length));
        let last = address + length - 1;
        self.approximated = true;
        self.fetch_word = Some(last / 4);
        self.last_byte = last_byte;
        if self.cache_enabled {
            self.read_cache_line(address / 16);
            if address / 16 != last / 16 {
                self.read_cache_line(last / 16);
            }
        } else {
            self.penalty += ((last / 4 - address / 4 + 1) * 4) as u32;
        }
    }
    #[inline(always)]
    fn read_cache_line(&mut self, line: usize) {
        let set = line % 128;
        if !self.cache[set].contains(&line) {
            let way = self.victim[set];
            self.cache[set][way] = line;
            self.cache_populated = true;
            self.victim[set] = (way + 1) % 4;
            // DX2 external 2-1-1-1 fill, two core clocks per bus clock.
            self.penalty += 10;
        }
    }
    pub fn invalidate(&mut self, address: usize) {
        if !self.cache_populated {
            return;
        }
        let line = address / 16;
        for tag in &mut self.cache[line % 128] {
            if *tag == line {
                *tag = usize::MAX;
            }
        }
    }
    pub fn flush(&mut self) {
        // Firmware polling flushes frequently even with CR0.CD set. Avoid
        // clearing 512 already-empty tags; retain the same guest cache misses.
        if self.cache_populated {
            self.cache = [[usize::MAX; 4]; 128];
            self.cache_populated = false;
        }
    }
    /// Account once per physical beat, not once per Rust byte read. Nominal
    /// V30 aligned bus clocks are included in the NEC instruction tables.
    #[allow(clippy::too_many_arguments)] // One actual physical access, with its decoding facts.
    #[inline(always)] // Specialize fetch/read/write facts at the CPU access site.
    pub fn memory(
        &mut self,
        profile: Profile,
        address: usize,
        byte: u32,
        bytes: u32,
        write: bool,
        fetch: bool,
        ram: bool,
        video: bool,
    ) {
        if !self.active || (fetch && self.rep_fetch) {
            return;
        }
        if profile == Profile::Pc9801Vm {
            if fetch {
                return;
            } // NEC execution tables assume available prefetch.
            if byte == 0 && bytes == 2 && address & 1 != 0 {
                self.penalty += 4;
            }
            if video && (byte == 0 || address & 1 == 0) {
                self.penalty += 2; // Board policy: two waits per 16-bit VRAM beat.
                self.approximated = true;
            }
            return;
        }
        self.approximated = true; // Serialized bus/cache policy, not pipeline simulation.
        if byte == 0 && bytes > 1 && !address.is_multiple_of(bytes.min(4) as usize) {
            self.penalty += 3;
        }
        let beat = if video { 2 } else { 4 };
        let word = address / beat;
        if fetch {
            if self.fetch_word == Some(word) {
                return;
            }
            self.fetch_word = Some(word);
        } else if byte != 0 && !address.is_multiple_of(beat) {
            return;
        }
        if ram && self.cache_enabled {
            if !write {
                self.read_cache_line(address / 16);
            }
            // Write-through, no write allocate; serialize a 2-bus-clock write.
            if write {
                self.penalty += 4;
            }
        } else {
            self.penalty += if video { 8 } else { 4 };
        }
    }
    pub fn io(&mut self, profile: Profile, port: u16, bytes: u32) {
        if !self.active {
            return;
        }
        if profile == Profile::Pc9801Vm {
            if bytes == 2 && port & 1 != 0 {
                self.penalty += 4;
            }
        } else {
            // PC-98 ports are 8-bit devices: extra byte handshakes cost two
            // external clocks each, in addition to the Intel IN/OUT table cost.
            self.penalty += (bytes - 1) * 4;
            self.approximated = true;
        }
    }
    pub fn address(&mut self, profile: Profile, address32: bool, m: u8, sib: Option<u8>) {
        if !self.active || profile != Profile::Pc9821ApDos || m >= 0xc0 {
            return;
        }
        let mode = m >> 6;
        let rm = m & 7;
        let (base, index) = if address32 {
            if let Some(sib) = sib {
                let base = sib & 7;
                (
                    if mode == 0 && base == 5 {
                        None
                    } else {
                        Some(base)
                    },
                    sib & 0x38 != 0x20,
                )
            } else {
                (if mode == 0 && rm == 5 { None } else { Some(rm) }, false)
            }
        } else {
            (
                match rm {
                    0 | 1 | 7 => Some(3),
                    2 | 3 => Some(5),
                    4 => Some(6),
                    5 => Some(7),
                    6 if mode != 0 => Some(5),
                    _ => None,
                },
                rm < 4,
            )
        };
        if index {
            self.address_execution += 1;
            self.approximated = true;
        }
        if base.is_some_and(|reg| self.previous_writes & (1 << reg) != 0) {
            self.address_execution += 1;
        }
        if let Some(i) = self.instruction {
            let displacement = mode != 0 || base.is_none();
            let immediate = matches!(i.op, 0x69 | 0x6b | 0x80..=0x83 | 0xc0 | 0xc1 | 0xc6 | 0xc7)
                || matches!(i.op, 0xf6 | 0xf7) && m & 0x38 == 0
                || i.op == 0x0f && matches!(i.ext, 0xa4 | 0xac | 0xba);
            if displacement && immediate {
                self.address_execution += 1;
                self.approximated = true;
            }
        }
    }
    pub fn finish(&mut self, profile: Profile, fault: bool, firmware: u64) -> StepTiming {
        let mut result = StepTiming {
            bus: self.penalty,
            firmware_scheduler: firmware,
            approximated: self.approximated || fault || firmware != 0,
            ..StepTiming::default()
        };
        if profile == Profile::Pc9821ApDos && self.fault_vector == Some(14) {
            // DX2 pp.10-12..13: #PF = INT + 24, including code-fetch
            // faults before decoding. Replace successful opcode/prefix latency;
            // event delivery and descriptor loads remain in extra_execution,
            // and actual physical transfers remain in the separate bus field.
            result.execution = 24 + self.extra_execution;
        } else if let Some(i) = self.instruction {
            let (clocks, approximate) = if profile == Profile::Pc9801Vm {
                self.v30(i, fault)
            } else {
                self.i486(i, self.taken)
            };
            if !fault && clocks == 0 {
                self.untimed = Some((i.op, i.ext));
            }
            result.execution = clocks + self.address_execution + self.extra_execution;
            result.approximated |= approximate || (i.rep && !i.string());
            result.rep_elements = u32::from(!fault && i.string() && self.element);
            result.instructions = u32::from(!(fault || i.rep && i.string() && self.taken));
        } else if self.extra_execution != 0 {
            result.execution = self.extra_execution;
        } else {
            // Idle time is a scheduler quantum, never an instruction fallback.
            result.idle = (profile.cpu_hz() / 1_000_000) as u32;
        }
        if self.untimed.is_some() {
            result.instructions = 0;
            result.approximated = true;
        }
        self.active = false;
        result
    }
    fn string_clocks(&self, i: Instruction, vm: bool) -> u32 {
        let (setup, iteration, single) = if vm {
            match i.op & 0xfe {
                0xa4 => (11, 8, 11),
                0xa6 => (7, 14, 13),
                0xaa => (7, 4, 7),
                0xac => (7, 9, 7),
                0xae => (7, 10, 7),
                0x6c | 0x6e => (9, 8, 10),
                _ => unreachable!(),
            }
        } else {
            match i.op & 0xfe {
                // Intel PRM REP MOVS has a separate C=1 path.
                0xa4 if i.count == 1 && !i.continuation => (10, 3, 7),
                0xa4 => (12, 3, 7),
                0xa6 => (7, 7, 8),
                0xaa => (7, 4, 5),
                0xac => (7, 4, 5),
                0xae => (7, 5, 6),
                0x6c | 0x6e => {
                    let setup = if i.op & 2 == 0 { 19 } else { 20 };
                    let iteration = if i.op & 2 == 0 { 11 } else { 8 };
                    let adjustment = if i.vm86 {
                        13
                    } else if i.protected {
                        if i.io_checked {
                            14
                        } else {
                            -6
                        }
                    } else {
                        0
                    };
                    (
                        (setup + adjustment) as u32,
                        iteration,
                        if i.vm86 {
                            33
                        } else if i.protected {
                            if i.io_checked {
                                35
                            } else {
                                13
                            }
                        } else {
                            20
                        },
                    )
                }
                _ => unreachable!(),
            }
        };
        if !i.rep {
            return single;
        }
        if i.count == 0 {
            return if vm || matches!(i.op, 0x6c..=0x6f) {
                setup
            } else {
                5
            };
        }
        iteration + if i.continuation { 0 } else { setup }
    }
    fn v30(&self, i: Instruction, fault: bool) -> (u32, bool) {
        let mem = i.modrm < 0xc0;
        let group = (i.modrm >> 3) & 7;
        let rm = |reg, memory| if mem { memory } else { reg };
        let mut approximate = false;
        let clocks = match i.op {
            // NEC table 6-1: DIVU error = 45; signed DIV error = 45..55.
            // These replace successful division clocks, including interrupt
            // entry. Keep the upper bound for the signed data-dependent range.
            0xf6 | 0xf7 if fault && group >= 6 => {
                approximate = true;
                if group == 6 {
                    45
                } else {
                    55
                }
            }
            op if i.string() => {
                let _ = op;
                self.string_clocks(i, true)
            }
            op @ 0x00..=0x3d if op & 7 <= 5 => {
                if op & 7 >= 4 {
                    4
                } else if !mem {
                    2
                } else if op & 2 != 0 || op >> 3 == 7 {
                    11
                } else {
                    16
                }
            }
            0x06 | 0x0e | 0x16 | 0x1e | 0x07 | 0x17 | 0x1f | 0x50..=0x5f | 0x68 | 0x9c | 0x9d => 8,
            0x27 | 0x37 => 3,
            0x2f | 0x3f => 7,
            0x40..=0x4f => 2,
            0x60 => 35,
            0x61 => 43,
            0x62 => {
                approximate = self.taken;
                if self.taken {
                    53
                } else {
                    18
                }
            }
            0x69 | 0x6b => {
                approximate = true;
                rm(
                    if i.op == 0x69 { 42 } else { 34 },
                    if i.op == 0x69 { 48 } else { 40 },
                )
            }
            0x6a => 7,
            0x70..=0x7f => {
                if self.taken {
                    14
                } else {
                    4
                }
            }
            0x80..=0x83 => rm(4, if group == 7 { 13 } else { 18 }),
            0x84 | 0x85 => rm(2, 10),
            0x86 | 0x87 => rm(3, 16),
            0x88 | 0x89 => rm(2, 9),
            0x8a | 0x8b => rm(2, 11),
            0x8c => rm(2, 10),
            0x8e => rm(2, 11),
            0x8d => 4,
            0x8f => rm(8, 17),
            0x90..=0x97 => 3,
            0x98 => 2,
            0x99 => {
                approximate = true;
                5
            }
            0x9a => 21,
            0x9b => {
                // NEC p.12-171: 2 + 5n. Assume POLL is ready on the first
                // sample (n = 1); external pin changes are not modeled.
                approximate = true;
                2 + 5
            }
            0x9e => 3,
            0x9f => 2,
            0xa0 | 0xa1 => 10,
            0xa2 | 0xa3 => 9,
            0xa8 | 0xa9 | 0xb0..=0xbf => 4,
            0xc0 | 0xc1 | 0xd0..=0xd3 => {
                if matches!(i.op, 0xd0 | 0xd1) {
                    rm(2, 16)
                } else {
                    rm(7, 19)
                        + if i.op < 0xd0 {
                            self.last_byte as u32
                        } else {
                            i.cl as u32
                        }
                }
            }
            0xc2 => 20,
            0xc3 => 15,
            0xc4 | 0xc5 => 18,
            0xc6 | 0xc7 => rm(4, 11),
            0xc8 => {
                let n = self.last_byte as u32 & 31;
                approximate = n == 1;
                if n == 0 {
                    12
                } else {
                    19 + 8 * (n - 1)
                }
            }
            0xc9 => 6,
            0xca => 24,
            0xcb => 21,
            0xcc | 0xcd => 38,
            0xce => {
                if i.flags & crate::cpu::OF != 0 {
                    40
                } else {
                    4
                }
            }
            0xcf => 27,
            0xd4 => 15,
            0xd5 => 7,
            0xd7 => 9,
            0xe0 | 0xe1 => {
                if self.taken {
                    14
                } else {
                    5
                }
            }
            0xe2 | 0xe3 => {
                if self.taken {
                    13
                } else {
                    5
                }
            }
            0xe4 | 0xe5 => 9,
            0xe6 | 0xe7 | 0xec..=0xef => 8,
            0xe8 => 16,
            0xe9 | 0xeb => 12,
            0xea => 15,
            0xf4 | 0xf5 | 0xf8..=0xfd => 2,
            0xf6 | 0xf7 => match group {
                0 | 1 => rm(4, 11),
                2 | 3 => rm(2, 16),
                4 => {
                    approximate = true;
                    rm(
                        if i.op == 0xf6 { 22 } else { 30 },
                        if i.op == 0xf6 { 28 } else { 36 },
                    )
                }
                5 => {
                    approximate = true;
                    rm(
                        if i.op == 0xf6 { 39 } else { 47 },
                        if i.op == 0xf6 { 45 } else { 53 },
                    )
                }
                6 => rm(
                    if i.op == 0xf6 { 19 } else { 25 },
                    if i.op == 0xf6 { 25 } else { 31 },
                ),
                7 => {
                    approximate = true;
                    rm(
                        if i.op == 0xf6 { 34 } else { 43 },
                        if i.op == 0xf6 { 40 } else { 49 },
                    )
                }
                _ => unreachable!(),
            },
            0xfe | 0xff => match group {
                0 | 1 => rm(2, 16),
                2 => rm(14, 23),
                3 => 31,
                4 => rm(11, 20),
                5 => 27,
                6 => rm(8, 18),
                _ => 0,
            },
            0x0f => match i.ext {
                0x10..=0x1f => {
                    let immediate = u32::from(i.ext & 8 != 0);
                    match (i.ext >> 1) & 3 {
                        0 => rm(3, 12) + immediate,
                        1 => rm(5, 14) + immediate,
                        2 => rm(4, 13) + immediate,
                        3 => rm(4, 18) + immediate,
                        _ => unreachable!(),
                    }
                }
                0x20 => 7 + 19 * (i.cl as u32).div_ceil(2),
                0x28 => 28,
                0x2a => 33,
                _ => {
                    approximate = true;
                    0
                }
            },
            // Unsupported opcodes are diagnosed by execution. No supported
            // instruction silently receives a generic instruction cost.
            _ => {
                approximate = true;
                0
            }
        };
        (
            clocks + if i.continuation { 0 } else { i.prefixes * 2 },
            approximate,
        )
    }
    /// The guarded real-mode JIT subset has static opcode latency apart from
    /// branch direction. Resolve both paths from the shared table at decode.
    pub(crate) fn jit_execution(&self, op: u8, modrm: u8, prefixes: u32) -> [u32; 2] {
        let instruction = Instruction {
            op,
            modrm,
            prefixes,
            width: 2,
            ..Instruction::default()
        };
        [false, true].map(|taken| {
            let (clocks, approximate) = self.i486(instruction, taken);
            debug_assert!(clocks > 0 && !approximate);
            clocks
        })
    }
    /// JIT guards exclude faults, REP, firmware and variable-latency opcodes.
    /// Fetch, operand and address penalties still come from the actual bus.
    pub(crate) fn finish_jit(&mut self, execution: u32) -> StepTiming {
        debug_assert!(self.fault_vector.is_none() && self.untimed.is_none());
        debug_assert_eq!(self.extra_execution, 0);
        self.active = false;
        StepTiming {
            execution: execution + self.address_execution,
            bus: self.penalty,
            instructions: 1,
            approximated: self.approximated,
            ..StepTiming::default()
        }
    }
    fn i486(&self, i: Instruction, taken: bool) -> (u32, bool) {
        // RV/P table entries group virtual-8086 with real mode. Interrupt
        // entry and I/O retain their separate virtual-mode cases below.
        let protected = i.protected && !i.vm86;
        let mem = i.modrm < 0xc0;
        let group = (i.modrm >> 3) & 7;
        let rm = |reg, memory| if mem { memory } else { reg };
        let mut approximate = false;
        let clocks = match i.op {
            // DX2 p.10-12: fault-specific overhead replaces successful
            // instruction latency. INT entry and descriptor costs are already
            // accumulated separately by event delivery.
            0x62 if self.fault_vector == Some(5) => 24,
            0xfa | 0xfb | 0x9d if i.vm86 && self.fault_vector == Some(13) => 8,
            0x9c | 0xcd | 0xcf if i.vm86 && self.fault_vector == Some(13) => 9,
            0xe4..=0xe7 if i.vm86 && self.fault_vector == Some(13) => 50,
            0xec..=0xef if i.vm86 && self.fault_vector == Some(13) => 51,
            0x6c..=0x6f if i.vm86 && self.fault_vector == Some(13) => {
                if i.rep {
                    51
                } else {
                    50
                }
            }
            op if i.string() => {
                let _ = op;
                self.string_clocks(i, false)
            }
            op @ 0x00..=0x3d if op & 7 <= 5 => {
                if op & 7 >= 4 || !mem {
                    1
                } else if op & 2 != 0 || op >> 3 == 7 {
                    2
                } else {
                    3
                }
            }
            0x06 | 0x0e | 0x16 | 0x1e => 3,
            0x07 | 0x17 | 0x1f => {
                if protected {
                    9
                } else {
                    3
                }
            }
            0x27 | 0x2f => 2,
            0x37 | 0x3f => 3,
            0x40..=0x57 => 1,
            0x58..=0x5f => 1,
            0x60 => 11,
            0x61 => 9,
            0x62 => 7,
            0x68 | 0x6a => 1,
            0x69 | 0x6b => {
                approximate = true;
                if i.width == 4 {
                    42
                } else {
                    26
                }
            }
            0x70..=0x7f => {
                if taken {
                    3
                } else {
                    1
                }
            }
            0x80..=0x83 => rm(1, if group == 7 { 2 } else { 3 }),
            0x84 | 0x85 => rm(1, 2),
            0x86 | 0x87 => rm(3, 5),
            0x88..=0x8b | 0xa0..=0xa3 | 0xb0..=0xbf | 0xc6 | 0xc7 => 1,
            0x8c => 3,
            0x8e => {
                if protected {
                    rm(9, 9)
                } else {
                    3
                }
            }
            0x8d => 1,
            0x8f => rm(4, 5),
            0x90 => 1,
            0x91..=0x97 => 3,
            0x98 | 0x99 => 3,
            0x9a => {
                if protected {
                    20
                } else {
                    18
                }
            }
            0x9b => 1,
            0x9c => {
                if protected {
                    3
                } else {
                    4
                }
            }
            0x9d => {
                if protected {
                    6
                } else {
                    9
                }
            }
            0x9e => 2,
            0x9f => 3,
            0xa8 | 0xa9 => 1,
            0xc0 | 0xc1 | 0xd0..=0xd3 => {
                if matches!(group, 2 | 3) && !matches!(i.op, 0xd0 | 0xd1) {
                    let count = if i.op < 0xd0 { self.last_byte } else { i.cl } & 31;
                    let bits = if i.op & 1 == 0 { 8 } else { i.width * 8 };
                    if u32::from(count) <= bits {
                        rm(8, 9)
                    } else {
                        9 + u32::from(count) / bits * 7
                    }
                } else {
                    rm(if i.op < 0xd0 { 2 } else { 3 }, 4)
                }
            }
            0xc2 | 0xc3 => 5,
            0xc4 | 0xc5 => {
                if protected {
                    12
                } else {
                    6
                }
            }
            0xc8 => {
                let n = self.last_byte as u32 & 31;
                match n {
                    0 => 14,
                    1 => 17,
                    _ => 17 + 3 * n,
                }
            }
            0xc9 => 5,
            0xca => {
                if protected {
                    18
                } else {
                    14
                }
            }
            0xcb => {
                if protected {
                    17
                } else {
                    13
                }
            }
            0xcc | 0xcd => {
                if i.vm86 {
                    82 + if i.op == 0xcd { 4 } else { 0 }
                } else if protected {
                    44
                } else {
                    26 + if i.op == 0xcd { 4 } else { 0 }
                }
            }
            0xce => {
                if i.flags & crate::cpu::OF != 0 {
                    if i.vm86 {
                        84
                    } else if protected {
                        46
                    } else {
                        28
                    }
                } else {
                    3
                }
            }
            0xcf => {
                if protected {
                    20
                } else {
                    15
                }
            }
            0xd4 => 15,
            0xd5 => 14,
            0xd7 => 4,
            0xd9 | 0xdd => 3,
            0xdb => 17,
            0xdf => 3,
            0xe0 => {
                if taken {
                    9
                } else {
                    8
                }
            }
            0xe1 => {
                if taken {
                    9
                } else {
                    6
                }
            }
            0xe2 => {
                if taken {
                    7
                } else {
                    6
                }
            }
            0xe3 => {
                if taken {
                    8
                } else {
                    5
                }
            }
            0xe4..=0xe7 | 0xec..=0xef => {
                let output = i.op & 2 != 0;
                if i.vm86 {
                    if output {
                        32
                    } else {
                        30
                    }
                } else if protected {
                    11 + u32::from(output) * 2
                        + u32::from(i.op < 0xe8)
                        + if i.io_checked { 20 } else { 0 }
                } else if output {
                    19
                } else {
                    17
                }
            }
            0xe8 | 0xe9 | 0xeb => 3,
            0xea => {
                if protected {
                    19
                } else {
                    17
                }
            }
            0xf4 => 4,
            0xf5 | 0xf8 | 0xf9 | 0xfc | 0xfd => 2,
            0xfa | 0xfb => 5,
            0xf6 | 0xf7 => match group {
                0 | 1 => rm(1, 2),
                2 | 3 => rm(1, 3),
                4 | 5 => {
                    approximate = true;
                    if i.op == 0xf6 {
                        18
                    } else if i.width == 4 {
                        42
                    } else {
                        26
                    }
                }
                6 => {
                    if i.op == 0xf6 {
                        16
                    } else if i.width == 4 {
                        40
                    } else {
                        24
                    }
                }
                7 => rm(
                    if i.op == 0xf6 {
                        19
                    } else if i.width == 4 {
                        43
                    } else {
                        27
                    },
                    if i.op == 0xf6 {
                        20
                    } else if i.width == 4 {
                        44
                    } else {
                        28
                    },
                ),
                _ => unreachable!(),
            },
            0xfe | 0xff => match group {
                0 | 1 => rm(1, 3),
                2 => 5,
                3 => {
                    if protected {
                        20
                    } else {
                        17
                    }
                }
                4 => 5,
                5 => {
                    if protected {
                        18
                    } else {
                        13
                    }
                }
                6 => 4,
                _ => 0,
            },
            0x0f => match i.ext {
                0x00 => match group {
                    0 | 1 => rm(2, 3),
                    2 => 11,
                    3 => 20,
                    _ => 0,
                },
                0x01 => match group {
                    0 | 1 => 10,
                    2 | 3 => 12,
                    4 => rm(2, 3),
                    6 => 13,
                    // Intel486 DX2 table 10.1: TLB hit / no hit = 12 / 11.
                    7 => 11 + u32::from(self.invlpg_hit),
                    _ => 0,
                },
                0x06 => 7,
                0x20 => 4,
                0x22 => {
                    if group == 0 {
                        17
                    } else {
                        4
                    }
                }
                0x80..=0x8f => {
                    if taken {
                        3
                    } else {
                        1
                    }
                }
                0x90..=0x9f => {
                    if taken {
                        rm(4, 3)
                    } else {
                        rm(3, 4)
                    }
                }
                0xa0 | 0xa8 => 3,
                0xa1 | 0xa9 => {
                    if protected {
                        9
                    } else {
                        3
                    }
                }
                0xa3 => rm(3, 8),
                0xab | 0xb3 | 0xbb => rm(6, 13),
                0xba => {
                    if group == 4 {
                        3
                    } else {
                        rm(6, 8)
                    }
                }
                0xa4 | 0xac => rm(2, 3),
                0xa5 | 0xad => rm(3, 4),
                0xaf => {
                    approximate = true;
                    if i.width == 4 {
                        42
                    } else {
                        26
                    }
                }
                0xb0 | 0xb1 => rm(6, if taken { 7 } else { 10 }),
                0xb2 | 0xb4 | 0xb5 => {
                    if protected {
                        12
                    } else {
                        6
                    }
                }
                0xb6 | 0xb7 | 0xbe | 0xbf => 3,
                0xbc | 0xbd => {
                    if self.value == 0 {
                        rm(6, 7)
                    } else if i.ext == 0xbc {
                        let bit = self.value.trailing_zeros();
                        rm(8, 9) + 4 * (bit / 8 + 1) + 3 * ((bit % 8) / 4 + 1) + 3 * (bit % 4 + 1)
                    } else {
                        rm(7, 8) + 3 * (32 - (31 - self.value.leading_zeros()))
                    }
                }
                0xc0 | 0xc1 => rm(3, 4),
                0xc8..=0xcf => 1,
                _ => {
                    approximate = true;
                    0
                }
            },
            _ => {
                approximate = true;
                0
            }
        };
        (
            clocks + if i.continuation { 0 } else { i.prefixes },
            approximate,
        )
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[cfg(feature = "block-experiment")]
    #[test]
    fn block_budget_stays_before_events_at_every_fraction() {
        for profile in [Profile::Pc9801Vm, Profile::Pc9821ApDos] {
            // Obtain fractions through real conversions, not a copy of the
            // inverse formula. Verify both safety and maximality independently.
            for prefix in 0..128 {
                let mut converter = ClockConverter::default();
                converter.advance(profile, prefix);
                for clocks in 1..500 {
                    let budget = converter.before_deadline(clocks, profile);
                    let mut at = ClockConverter {
                        remainder: converter.remainder,
                    };
                    let mut next = ClockConverter {
                        remainder: converter.remainder,
                    };
                    assert!(u64::from(at.advance(profile, budget)) < clocks);
                    assert!(u64::from(next.advance(profile, budget + 1)) >= clocks);
                }
                assert_eq!(converter.before_deadline(0, profile), 0);
            }
        }
    }

    #[test]
    fn guarded_fetch_matches_beats_through_alignment_cache_conflicts_and_invalidation() {
        for cr0 in [0, 0x4000_0000] {
            for address in [0, 1, 3, 12, 13, 14, 15, 16, 0x9fff9] {
                for length in 1..=7 {
                    for warm in [0, 1, 5] {
                        let mut actual = TimingState::default();
                        let mut reference = TimingState::default();
                        for timing in [&mut actual, &mut reference] {
                            timing.begin(cr0);
                            for conflict in 0..warm {
                                timing.memory(
                                    Profile::Pc9821ApDos,
                                    address + conflict * 2048,
                                    0,
                                    1,
                                    false,
                                    false,
                                    true,
                                    false,
                                );
                            }
                            timing.begin(cr0);
                        }
                        actual.jit_fetch(address, length, 0xa5);
                        let last = address + length - 1;
                        for beat in address / 4..=last / 4 {
                            reference.memory(
                                Profile::Pc9821ApDos,
                                beat * 4,
                                0,
                                1,
                                false,
                                true,
                                true,
                                false,
                            );
                        }
                        reference.last_byte = 0xa5;
                        assert_eq!(actual.penalty, reference.penalty);
                        assert_eq!(actual.fetch_word, reference.fetch_word);
                        assert_eq!(actual.last_byte, reference.last_byte);
                        assert_eq!(actual.cache, reference.cache);
                        assert_eq!(actual.victim, reference.victim);
                        assert_eq!(actual.cache_populated, reference.cache_populated);
                        assert_eq!(actual.finish_jit(1), reference.finish_jit(1));
                        for timing in [&mut actual, &mut reference] {
                            timing.invalidate(address);
                            timing.begin(cr0);
                            timing.memory(
                                Profile::Pc9821ApDos,
                                last,
                                0,
                                1,
                                false,
                                false,
                                true,
                                false,
                            );
                        }
                        assert_eq!(actual.penalty, reference.penalty);
                        assert_eq!(actual.cache, reference.cache);
                        assert_eq!(actual.victim, reference.victim);
                    }
                }
            }
        }
    }

    #[test]
    fn compact_cache_tags_preserve_cold_hits_replacement_and_invalidation() {
        let mut state = TimingState::default();
        let read = |state: &mut TimingState, address| {
            state.begin(0);
            state.memory(
                Profile::Pc9821ApDos,
                address,
                0,
                1,
                false,
                false,
                true,
                false,
            );
            state.penalty
        };
        // Zero and the highest possible line must both start invalid.
        for address in [0, usize::MAX] {
            assert_eq!(read(&mut state, address), 10);
            assert_eq!(read(&mut state, address), 0);
            state.invalidate(address);
            assert_eq!(read(&mut state, address), 10);
            state.flush();
            assert_eq!(read(&mut state, address), 10);
        }
        state.flush();
        // Five lines in the same four-way set evict the oldest line.
        for line in 0..5 {
            assert_eq!(read(&mut state, line * 2048), 10);
        }
        assert_eq!(read(&mut state, 2048), 0);
        assert_eq!(read(&mut state, 0), 10);
        state.flush();
        state.begin(0);
        state.memory(Profile::Pc9821ApDos, 0, 0, 1, true, false, true, false);
        assert_eq!(state.penalty, 4); // Write-through, no allocation.
        assert_eq!(read(&mut state, 0), 10);
    }
}

//! Guarded browser JIT bridge. Generated Wasm supplies register semantics;
//! Rust retains instruction fetches, timing, interrupts and device ownership.
use super::*;
use crate::{Machine, Profile};

const HEAT: u8 = 32;
const SLOTS: usize = 4096;
const PROBES: usize = 16384;
const BLOCK: usize = 16;

#[derive(Clone, Copy, Debug)]
struct Op {
    ip: u32,
    bytes: [u8; 6],
    len: u8,
    modrm: u8,
    memory: Option<Memory>,
    register_writes: u8,
}
#[derive(Clone, Copy, Debug)]
struct Memory {
    segment: u8,
    base: u8,
    index: u8,
    width: u8,
    displacement: u16,
    // Read, write, word stack, return, ModR/M effective-address timing.
    flags: u8,
}
impl Memory {
    const READ: u8 = 1;
    const WRITE: u8 = 2;
    const STACK: u8 = 4;
    const RETURN: u8 = 8;
    const ADDRESS: u8 = 16;
    fn offset(self, c: &Cpu) -> u32 {
        let base = if self.base < 8 {
            c.r[self.base as usize]
        } else {
            0
        };
        let index = if self.index < 8 {
            c.r[self.index as usize]
        } else {
            0
        };
        u32::from(base.wrapping_add(index).wrapping_add(self.displacement))
    }
}
impl Op {
    fn branch(self) -> bool {
        matches!(self.bytes[0], 0x70..=0x7f | 0xe0..=0xe3 | 0xe8 | 0xe9 | 0xeb | 0xc3)
    }
    fn memory_write(self) -> bool {
        let op = self.bytes[0];
        match op {
            0x50..=0x57 | 0xe8 | 0xa2 | 0xa3 | 0x86..=0x89 | 0xc6 | 0xc7 => true,
            0x80 | 0x81 | 0x83 => self.modrm & 0x38 != 0x38,
            0xf6 | 0xf7 => self.modrm & 0x38 != 0,
            _ => op <= 0x3d && op & 3 < 2 && op >> 3 != 7,
        }
    }
    fn memory_read(self) -> bool {
        !matches!(
            self.bytes[0],
            0x50..=0x57 | 0xe8 | 0xa2 | 0xa3 | 0x88 | 0x89 | 0xc6 | 0xc7
        )
    }
    fn register_policy(self) -> u8 {
        let op = self.bytes[0];
        match op {
            // These word operations always change the named low register.
            0x40..=0x4f => 1 << (op & 7),
            0x50..=0x57 | 0xc3 | 0xe8 => 1 << SP,
            0xe0..=0xe2 => 1 << CX,
            0x00..=0x3d if op & 7 <= 5 => {
                if op >> 3 == 7 || op & 7 < 2 && self.modrm < 0xc0 {
                    0
                } else {
                    0xff
                }
            }
            0x70..=0x7f
            | 0x84
            | 0x85
            | 0x90
            | 0x9e
            | 0xa2
            | 0xa3
            | 0xa8
            | 0xa9
            | 0xe3
            | 0xe9
            | 0xeb
            | 0xf5
            | 0xf8
            | 0xf9
            | 0xfc
            | 0xfd => 0,
            0x80 | 0x81 | 0x83 if self.modrm < 0xc0 || self.modrm & 0x38 == 0x38 => 0,
            0x88 | 0x89 | 0xc6 | 0xc7 if self.modrm < 0xc0 => 0,
            0xf6 | 0xf7 if self.modrm < 0xc0 || self.modrm & 0x38 == 0 => 0,
            _ => 0xff, // Preserve actual-change comparison for aliases/identity operations.
        }
    }
    fn target(self) -> u32 {
        let displacement = if matches!(self.bytes[0], 0xe8 | 0xe9) {
            u16::from_le_bytes([self.bytes[1], self.bytes[2]]) as i16 as i32
        } else {
            self.bytes[1] as i8 as i32
        };
        self.ip
            .wrapping_add(u32::from(self.len))
            .wrapping_add(displacement as u32)
            & 0xffff
    }
}
struct Plan {
    id: u32,
    key: u64,
    ready: bool,
    ops: Vec<Op>,
}
#[derive(Clone, Copy, Default)]
struct Probe {
    key: u64,
    hits: u8,
    id: u32,
}

/// No generated code or guest bytes leave the owned emulator Worker.
pub struct Engine {
    probes: Box<[Probe]>,
    plans: Vec<Option<Plan>>,
    serial: u32,
    packet: Vec<u32>,
    active: Option<(u32, usize, [u16; 8], u8, bool)>,
    operand: Option<(u32, u32)>,
    scratch: [u32; 2],
    deadline: u64,
    remaining: u32,
    done: u32,
    sample_phase: u32,
    /// Aggregate observations only: offers, committed JIT units, invalidations.
    pub counters: [u64; 3],
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            probes: vec![Probe::default(); PROBES].into_boxed_slice(),
            plans: (0..SLOTS).map(|_| None).collect(),
            serial: 0,
            packet: Vec::new(),
            active: None,
            operand: None,
            scratch: [0; 2],
            deadline: 0,
            remaining: 0,
            done: 0,
            sample_phase: 0,
            counters: [0; 3],
        }
    }
}
fn key(cpu: &Cpu) -> u64 {
    (u64::from(cpu.segment_cache(CS).base) << 32) | u64::from(cpu.ip)
}
fn probe(key: u64) -> usize {
    ((key ^ (key >> 29) ^ (key >> 7)) as usize) & (PROBES - 1)
}
fn eligible(m: &Machine) -> bool {
    let c = &m.cpu;
    m.bus.profile == Profile::Pc9821ApDos
        && c.cr0 & 0x8000_0001 == 0
        && !c.halted
        && !c.shutdown
        && c.flags & TF == 0
        && c.rep_resume.is_none()
        && !c.segment_cache(CS).big()
}
fn code(m: &Machine, ip: u32, length: u32) -> Option<usize> {
    let cs = m.cpu.segment_cache(CS);
    let address = cs.base.checked_add(ip)?;
    // Exclude mapped video/ROM and wrapping fetches. The interpreter retains
    // fault delivery, unusual hidden caches and all other address spaces.
    (ip.checked_add(length)? <= 0x10000
        && cs.contains(ip, length)
        && address.checked_add(length)? <= 0xa0000)
        .then_some(address as usize)
}
fn displacement_len(m: u8) -> u32 {
    match m >> 6 {
        0 if m & 7 == 6 => 2,
        1 => 1,
        2 => 2,
        _ => 0,
    }
}
fn memory_modrm(op: u8) -> bool {
    matches!(op, 0x80 | 0x81 | 0x83..=0x8b | 0xc6 | 0xc7 | 0xf6 | 0xf7) || op <= 0x3d && op & 7 < 4
}
fn memory_spec(op: Op) -> Option<Memory> {
    let opcode = op.bytes[0];
    let flags = (if op.memory_read() { Memory::READ } else { 0 })
        | (if op.memory_write() { Memory::WRITE } else { 0 });
    let (segment, base, index, displacement, width, extra) =
        if matches!(opcode, 0x50..=0x5f | 0xe8 | 0xc3) {
            (
                SS,
                SP,
                8,
                if flags & Memory::WRITE != 0 {
                    0xfffe
                } else {
                    0
                },
                2,
                Memory::STACK | if opcode == 0xc3 { Memory::RETURN } else { 0 },
            )
        } else if matches!(opcode, 0xa0..=0xa3) {
            (
                DS,
                8,
                8,
                u16::from_le_bytes([op.bytes[1], op.bytes[2]]),
                1 + (opcode & 1),
                0,
            )
        } else if memory_modrm(opcode) && op.modrm < 0xc0 {
            let mode = op.modrm >> 6;
            let (base, index, stack) = match op.modrm & 7 {
                0 => (BX, SI, false),
                1 => (BX, DI, false),
                2 => (BP, SI, true),
                3 => (BP, DI, true),
                4 => (SI, 8, false),
                5 => (DI, 8, false),
                6 if mode == 0 => (8, 8, false),
                6 => (BP, 8, true),
                _ => (BX, 8, false),
            };
            let displacement = match mode {
                0 if op.modrm & 7 == 6 => u16::from_le_bytes([op.bytes[2], op.bytes[3]]),
                1 => op.bytes[2] as i8 as u16,
                2 => u16::from_le_bytes([op.bytes[2], op.bytes[3]]),
                _ => 0,
            };
            (
                if stack { SS } else { DS },
                base,
                index,
                displacement,
                1 + (opcode & 1),
                Memory::ADDRESS,
            )
        } else {
            return None;
        };
    Some(Memory {
        segment: segment as u8,
        base: base as u8,
        index: index as u8,
        displacement,
        width,
        flags: flags | extra,
    })
}
fn decode(m: &Machine, ip: u32) -> Option<Op> {
    let at = code(m, ip, 1)?;
    let op = m.bus.mem[at];
    let mut modrm = 0;
    let length = match op {
        0x40..=0x5f | 0xc3 | 0x90..=0x97 | 0x9e | 0x9f | 0xf5 | 0xf8 | 0xf9 | 0xfc | 0xfd => 1,
        0xb0..=0xb7 | 0x70..=0x7f | 0xe0..=0xe3 | 0xeb => 2,
        0xb8..=0xbf | 0xe8 | 0xe9 => 3,
        0x00..=0x3d if op & 7 <= 5 => {
            if op & 7 >= 4 {
                2 + u32::from(op & 1)
            } else {
                code(m, ip, 2)?;
                modrm = m.bus.mem[at + 1];
                2 + displacement_len(modrm)
            }
        }
        0x80 | 0x81 | 0x83 | 0x84..=0x8b | 0xc6 | 0xc7 | 0xf6 | 0xf7 => {
            code(m, ip, 2)?;
            modrm = m.bus.mem[at + 1];
            if matches!(op, 0xc6 | 0xc7) && modrm & 0x38 != 0
                || matches!(op, 0xf6 | 0xf7) && !matches!(modrm & 0x38, 0 | 16 | 24)
            {
                return None;
            }
            displacement_len(modrm)
                + match op {
                    0xf6 | 0xf7 if modrm & 0x38 != 0 => 2,
                    0x81 | 0xc7 | 0xf7 => 4,
                    0x80 | 0x83 | 0xc6 | 0xf6 => 3,
                    _ => 2,
                }
        }
        0xa0..=0xa3 => 3,
        0xa8 => 2,
        0xa9 => 3,
        _ => return None,
    };
    code(m, ip, length)?;
    let mut bytes = [0; 6];
    bytes[..length as usize].copy_from_slice(&m.bus.mem[at..at + length as usize]);
    let mut result = Op {
        ip,
        bytes,
        len: length as u8,
        modrm,
        memory: None,
        register_writes: 0,
    };
    result.register_writes = result.register_policy();
    result.memory = memory_spec(result);
    if result.branch() && op != 0xc3 {
        code(m, result.target(), 1)?;
    }
    Some(result)
}
impl Engine {
    /// Reset cache ownership while never reusing an asynchronously compiled ID.
    pub fn clear(&mut self) {
        self.probes.fill(Probe::default());
        self.plans.iter_mut().for_each(|p| *p = None);
        self.active = None;
        self.operand = None;
        self.remaining = 0;
        self.done = 0;
        self.counters = [0; 3];
        self.sample_phase = 0;
        self.packet.clear();
    }
    pub fn begin_slice(&mut self, m: &Machine, clocks: u32, cap: u32) {
        self.deadline = m.bus.cycles + u64::from(clocks.min(80_000));
        self.remaining = cap.min(100_000);
        self.done = 0;
    }
    fn runnable(&self, m: &Machine) -> bool {
        self.remaining != 0 && m.bus.cycles < self.deadline && m.error.is_none()
    }
    fn plan(&self, id: u32) -> Option<&Plan> {
        self.plans[id as usize % SLOTS]
            .as_ref()
            .filter(|p| p.id == id)
    }
    /// Distinguish a new offer from a guarded entry that invalidates its plan.
    pub fn enter(&mut self, m: &mut Machine, id: u32) -> u32 {
        if !self.runnable(m) {
            return 0;
        }
        let Some(plan) = self.plan(id) else {
            return 0;
        };
        if !plan.ready {
            return 2;
        }
        u32::from(self.prepare(m, id, 0))
    }
    pub fn register(&mut self, id: u32, ready: bool) -> bool {
        if let Some(p) = self.plans[id as usize % SLOTS]
            .as_mut()
            .filter(|p| p.id == id)
        {
            if ready {
                p.ready = true;
            } else {
                self.plans[id as usize % SLOTS] = None;
            }
            true
        } else {
            false
        }
    }
    /// Interpreter execution stops only at a ready block or one compilation offer.
    pub fn dispatch(&mut self, m: &mut Machine, compile: bool) -> u32 {
        while self.runnable(m) {
            // Cold/unsupported addresses pay only a mode/key probe. Full IRQ,
            // descriptor and trap guards belong at a translation/entry boundary.
            if m.bus.profile == Profile::Pc9821ApDos
                && m.cpu.cr0 & 0x8000_0001 == 0
                && !m.cpu.halted
                && !m.cpu.shutdown
                && m.cpu.rep_resume.is_none()
            {
                let key = key(&m.cpu);
                let index = probe(key);
                if self.probes[index].hits == 0 || self.probes[index].key != key {
                    self.probes[index] = Probe {
                        key,
                        hits: 1,
                        id: 0,
                    };
                } else {
                    let p = &mut self.probes[index];
                    p.hits = p.hits.saturating_add(1);
                    let id = p.id;
                    if id != 0 && self.plan(id).is_none() {
                        self.probes[index].id = 0;
                        self.probes[index].hits = HEAT - 1;
                    }
                    let id = self.probes[index].id;
                    if let Some(plan) = self.plan(id).filter(|p| p.key == key) {
                        if plan.ready {
                            return id;
                        }
                    } else if id == 0 && self.probes[index].hits == HEAT && compile && eligible(m) {
                        let mut ops = Vec::new();
                        let mut ip = m.cpu.ip;
                        while ops.len() < BLOCK {
                            let Some(op) = decode(m, ip) else {
                                break;
                            };
                            ip += u32::from(op.len);
                            ops.push(op);
                            if op.branch() {
                                break;
                            }
                        }
                        if !ops.is_empty() {
                            let Some(serial) = self.serial.checked_add(1) else {
                                self.probes[index].hits = u8::MAX;
                                self.fallback(m);
                                continue;
                            };
                            self.serial = serial;
                            let id = serial;
                            self.packet.clear();
                            self.packet.extend([id, ops.len() as u32]);
                            for op in &ops {
                                self.packet.extend([
                                    op.ip,
                                    u32::from(op.len),
                                    u32::from_le_bytes(op.bytes[..4].try_into().unwrap()),
                                    u32::from(u16::from_le_bytes([op.bytes[4], op.bytes[5]])),
                                ]);
                            }
                            self.plans[id as usize % SLOTS] = Some(Plan {
                                id,
                                key,
                                ready: false,
                                ops,
                            });
                            self.probes[index].id = id;
                            self.counters[0] += 1;
                            return id;
                        }
                    } else if id == 0 && self.probes[index].hits == HEAT && !compile {
                        self.probes[index].hits -= 1;
                    }
                }
            }
            // Bound discovery work in cold code. Vary the short
            // stride so regular loops do not always sample the same position.
            // Ready blocks still chain directly without an interpreter burst.
            let cap = self.remaining.min(3 + (self.sample_phase & 3));
            self.sample_phase = self.sample_phase.wrapping_add(1);
            let done = m.step_scheduler((self.deadline - m.bus.cycles) as u32, cap);
            self.remaining -= done;
            self.done += done;
        }
        0
    }
    pub fn packet(&self) -> &[u32] {
        &self.packet
    }
    pub fn done(&self) -> u32 {
        self.done
    }
    pub fn start(&mut self, m: &mut Machine, id: u32) -> bool {
        if !self.runnable(m) || !self.plan(id).is_some_and(|p| p.ready) {
            return false;
        }
        self.prepare(m, id, 0)
    }
    fn prepare(&mut self, m: &mut Machine, id: u32, index: usize) -> bool {
        let Some(plan) = self.plan(id) else {
            return false;
        };
        let Some(op) = plan.ops.get(index).copied() else {
            return false;
        };
        let plan_key = plan.key;
        if index == 0 {
            if !eligible(m) || key(&m.cpu) != plan_key {
                return false;
            }
        } else if m.cpu.ip != op.ip {
            return false;
        }
        // Generated blocks run synchronously and these operations cannot change
        // CPU mode, segments, TF, halt or REP state. Unsupported control changes
        // leave the block and must pass the complete entry guard again. IRQs,
        // deadlines, code bytes and memory spans remain instruction boundaries.
        if m.cpu.flags & IF != 0 && m.cpu.inhibit == 0 {
            // The interpreter refreshes the cascade wire even when INTA finds
            // no vector. Preserve its deassertion/next-edge history too.
            m.bus.sync_cascade();
            if m.bus.pics[0].pending().is_some() {
                return false;
            }
        }
        let Some(at) = code(m, op.ip, u32::from(op.len)) else {
            return false;
        };
        if op.branch() && op.bytes[0] != 0xc3 && code(m, op.target(), 1).is_none() {
            return false;
        }
        if m.bus.mem[at..at + op.len as usize] != op.bytes[..op.len as usize] {
            self.counters[2] += 1;
            self.register(id, false);
            // Retry translation only after another warm-up, never stale code.
            let p = &mut self.probes[probe(plan_key)];
            if p.key == plan_key {
                p.hits = 1;
                p.id = 0;
            }
            return false;
        }
        // Static addressing metadata is decoded once, while registers, hidden
        // segment limits and device-backed accesses are checked at execution.
        self.operand = None;
        if let Some(memory) = op.memory {
            let segment = memory.segment as usize;
            let offset = memory.offset(&m.cpu);
            let width = u32::from(memory.width);
            let descriptor = m.cpu.segment_cache(segment);
            if memory.flags & Memory::STACK != 0 && descriptor.big() {
                return false;
            }
            if !descriptor.contains(offset, width) {
                return false;
            }
            let physical = descriptor.base.wrapping_add(offset);
            if memory.flags & Memory::RETURN != 0 {
                // Return target peeks are limited to ordinary RAM. All actual
                // reads still use the bus after the complete preflight.
                let at = physical as usize;
                if at.checked_add(2).is_none_or(|end| end > 0xa0000) {
                    return false;
                }
                let target = u16::from_le_bytes([m.bus.mem[at], m.bus.mem[at + 1]]) as u32;
                if code(m, target, 1).is_none() {
                    return false;
                }
            }
            self.operand = Some((physical, width));
        }
        let before = if op.register_writes == 0xff {
            m.cpu.r
        } else {
            [0; 8]
        };
        m.cpu.jit_prepare(&mut m.bus, op, at as u32);
        if op
            .memory
            .is_some_and(|memory| memory.flags & Memory::READ != 0)
        {
            if let Some((physical, width)) = self.operand {
                let mut value = 0;
                for byte in 0..width {
                    value |=
                        u32::from(
                            m.bus
                                .cpu_read(physical.wrapping_add(byte), byte, width, false),
                        ) << (byte * 8);
                }
                self.scratch[0] = value;
            }
        }
        self.active = Some((
            id,
            index,
            before,
            op.register_writes,
            op.memory
                .is_some_and(|memory| memory.flags & Memory::WRITE != 0),
        ));
        true
    }
    pub fn scratch_ptr(&mut self) -> *mut u32 {
        self.scratch.as_mut_ptr()
    }
    /// Commit one translated instruction, tick devices, then guard the next one.
    pub fn next(&mut self, m: &mut Machine, index: usize, taken: bool) -> bool {
        let Some((id, previous, before, register_writes, write)) = self.active.take() else {
            return false;
        };
        if write {
            if let Some((physical, width)) = self.operand {
                for byte in 0..width {
                    m.bus.cpu_write(
                        physical.wrapping_add(byte),
                        byte,
                        width,
                        (self.scratch[1] >> (byte * 8)) as u8,
                    );
                }
            }
        }
        m.bus.timing.taken = taken;
        let timing = m.cpu.jit_finish(&mut m.bus, before, register_writes);
        let clocks = m
            .clock_converter
            .advance(m.bus.profile, timing.cpu_cycles());
        m.bus.tick(clocks);
        self.remaining -= 1;
        self.done += 1;
        self.counters[1] += 1;
        index == previous + 1 && self.runnable(m) && self.prepare(m, id, index)
    }
    pub fn fallback(&mut self, m: &mut Machine) {
        if self.runnable(m) {
            m.step_unit(&mut |_, _| {});
            self.remaining -= 1;
            self.done += 1;
        }
    }
}
impl Cpu {
    fn jit_prepare(&mut self, b: &mut Bus, op: Op, physical: u32) {
        b.timing.begin(self.cr0);
        b.timing.previous_writes = self.timing_writes;
        if self.inhibit > 0 {
            self.inhibit -= 1;
        }
        self.flags_upper &= !1;
        self.instruction_bytes = 0;
        self.operand32 = false;
        self.address32 = false;
        self.lock_prefix = false;
        self.trap_suppressed = false;
        // Entry already validated the complete CS span, mode and raw bytes.
        // Retain every original bus fetch without repeating translation/limits.
        for byte in 0..u32::from(op.len) {
            b.timing.last_byte = b.cpu_read(physical + byte, 0, 1, true);
        }
        self.ip += u32::from(op.len);
        self.instruction_bytes = op.len;
        let mut instruction = crate::timing::Instruction::new(self, op.bytes[0], 0, 0);
        instruction.modrm = op.modrm;
        b.timing.instruction = Some(instruction);
        if op
            .memory
            .is_some_and(|memory| memory.flags & Memory::ADDRESS != 0)
        {
            b.timing.address(b.profile, false, op.modrm, None);
        }
        self.retired += 1;
    }
    fn jit_finish(
        &mut self,
        b: &mut Bus,
        before: [u16; 8],
        register_writes: u8,
    ) -> crate::timing::StepTiming {
        let timing = b.timing.finish(b.profile, false, 0);
        self.timing_writes = if register_writes == 0xff {
            changed_registers(&before, &self.upper, &self.r, &self.upper)
        } else {
            register_writes
        };
        self.last_timing = timing;
        self.cycles += u64::from(timing.cpu_cycles());
        self.completed_instructions += u64::from(timing.instructions);
        self.rep_elements += u64::from(timing.rep_elements);
        self.approximated_units += u64::from(timing.approximated);
        timing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn machine() -> Machine {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.cpu.s = [0; 4];
        m.cpu.ip = 0x100;
        m.cpu.flags = 2;
        m.bus.mem[0x100..0x104].copy_from_slice(&[0x40, 0x48, 0xeb, 0xfc]);
        m
    }
    fn offered(m: &mut Machine, e: &mut Engine) -> u32 {
        let ip = m.cpu.ip;
        let mut id = 0;
        for _ in 0..HEAT {
            m.cpu.ip = ip;
            e.begin_slice(m, 80_000, 1);
            id = e.dispatch(m, true);
        }
        assert_ne!(id, 0);
        assert!(e.register(id, true));
        id
    }
    #[test]
    fn jit_declines_faulting_fetches_and_prefixes() {
        let mut m = machine();
        for bytes in [[0x66, 0x40, 0x90], [0xf0, 0x40, 0x90]] {
            m.bus.mem[0x100..0x103].copy_from_slice(&bytes);
            assert!(decode(&m, 0x100).is_none());
        }
        m.bus.mem[0x100..0x103].copy_from_slice(&[0xb8, 0x34, 0x12]);
        m.cpu.segment_caches[CS] = Some(SegmentCache {
            base: 0,
            limit: 0x101,
            access: 0x9b,
            attributes: 0,
        });
        assert!(decode(&m, 0x100).is_none());
        m.cpu.segment_caches[CS] = None;
        m.bus.mem[0xffff] = 0xb8;
        assert!(decode(&m, 0xffff).is_none());
        m.cpu.segment_caches[CS] = Some(SegmentCache {
            base: 0x9ff00,
            limit: 0xffff,
            access: 0x9b,
            attributes: 0,
        });
        assert!(code(&m, 0x100, 1).is_none());
    }
    #[test]
    fn stack_guards_exit_before_fetch_or_bus_activity() {
        let mut m = machine();
        m.bus.mem[0x100..0x104].copy_from_slice(&[0x90, 0x58, 0xeb, 0xfc]);
        let mut e = Engine::default();
        let id = offered(&mut m, &mut e);
        m.cpu.ip = 0x100;
        m.cpu.r[SP] = 0xffff;
        e.begin_slice(&m, 80_000, 2);
        assert!(e.start(&mut m, id));
        assert!(!e.next(&mut m, 1, false));
        assert_eq!(m.cpu.ip, 0x101);
        assert_eq!(e.done(), 1);
        let retired = m.cpu.retired;
        e.fallback(&mut m);
        assert_eq!(m.cpu.retired, retired); // Fault rollback retains the reference retirement count.
        assert_eq!(m.cpu.fault_count, 1);
    }
    #[test]
    fn return_target_guard_does_not_pop_or_fetch_an_invalid_target() {
        let mut m = machine();
        m.bus.mem[0x100..0x104].copy_from_slice(&[0x90, 0xc3, 0xeb, 0xfc]);
        // Avoid executing RET while warming the plan.
        let mut e = Engine::default();
        let mut id = 0;
        for _ in 0..HEAT {
            m.cpu.ip = 0x100;
            e.begin_slice(&m, 80_000, 1);
            id = e.dispatch(&mut m, true);
        }
        assert_ne!(id, 0);
        assert!(e.register(id, true));
        m.cpu.ip = 0x100;
        m.cpu.r[SP] = 0x8000;
        m.bus.mem[0x8000..0x8002].copy_from_slice(&0x200u16.to_le_bytes());
        m.cpu.segment_caches[CS] = Some(SegmentCache {
            base: 0,
            limit: 0x101,
            access: 0x9b,
            attributes: 0,
        });
        e.begin_slice(&m, 80_000, 2);
        assert!(e.start(&mut m, id));
        assert!(!e.next(&mut m, 1, false));
        assert_eq!((m.cpu.ip, m.cpu.r[SP]), (0x101, 0x8000));
    }
    #[test]
    fn memory_span_guard_preserves_mapped_read_latches_and_hidden_bounds() {
        let mut m = machine();
        m.bus.mem[0x100..0x104].copy_from_slice(&[0x90, 0x8b, 0x07, 0x90]);
        let mut e = Engine::default();
        let id = offered(&mut m, &mut e);
        m.cpu.segment_caches[DS] = Some(SegmentCache {
            base: 0xa8000,
            limit: 0,
            access: 0x93,
            attributes: 0,
        });
        m.cpu.r[BX] = 0;
        m.bus.egc.enabled = true;
        m.bus.egc.registers[2] = 0x2100;
        m.bus.grcg_mode = 0x80;
        m.bus.graphics[0] = 0x55;
        let before = m.bus.egc.clone();
        m.cpu.ip = 0x100;
        e.begin_slice(&m, 80_000, 2);
        assert!(e.start(&mut m, id));
        assert!(!e.next(&mut m, 1, false));
        assert_eq!(m.cpu.ip, 0x101);
        assert_eq!(m.bus.egc, before);
        // The same complete-span rule applies to unreal-mode limits above 64K.
        m.cpu.segment_caches[DS].as_mut().unwrap().limit = 0xffff;
        m.cpu.r[BX] = 0xffff;
        m.cpu.ip = 0x100;
        e.begin_slice(&m, 80_000, 2);
        assert!(e.start(&mut m, id));
        assert!(!e.next(&mut m, 1, false));
        m.cpu.segment_caches[DS].as_mut().unwrap().limit = 0x10000;
        m.cpu.ip = 0x100;
        e.begin_slice(&m, 80_000, 2);
        assert!(e.start(&mut m, id));
        assert!(e.next(&mut m, 1, false));
    }
    #[test]
    fn big_or_mapped_return_stacks_decline_before_reading_or_popping() {
        let mut m = machine();
        m.bus.mem[0x100..0x104].copy_from_slice(&[0x90, 0xc3, 0x90, 0x90]);
        let mut e = Engine::default();
        let id = offered(&mut m, &mut e);
        for (base, attributes) in [(0, 0x40), (0xa8000, 0)] {
            m.cpu.segment_caches[SS] = Some(SegmentCache {
                base,
                limit: 0xffff,
                access: 0x93,
                attributes,
            });
            m.cpu.r[SP] = 0;
            m.cpu.ip = 0x100;
            e.begin_slice(&m, 80_000, 2);
            assert!(e.start(&mut m, id));
            assert!(!e.next(&mut m, 1, false));
            assert_eq!((m.cpu.ip, m.cpu.r[SP]), (0x101, 0));
        }
    }
    #[test]
    fn jit_entry_guards_leave_fetches_and_architecture_untouched() {
        let mut m = machine();
        let mut e = Engine::default();
        let id = offered(&mut m, &mut e);
        for (cr0, flags, halted) in [
            (1, 2, false),
            (0x80000001, 2, false),
            (0, TF | 2, false),
            (0, 2, true),
        ] {
            m.cpu.cr0 = cr0;
            m.cpu.flags = flags;
            m.cpu.halted = halted;
            let before = (m.cpu.ip, m.cpu.retired, m.cpu.cycles, m.bus.cycles, m.cpu.r);
            assert!(!e.start(&mut m, id));
            assert_eq!(
                (m.cpu.ip, m.cpu.retired, m.cpu.cycles, m.bus.cycles, m.cpu.r),
                before
            );
        }
        m.cpu.cr0 = 0;
        m.cpu.flags = IF | 2;
        m.cpu.halted = false;
        m.bus.pics[0].mask &= !2;
        m.bus.key(0x1d, true);
        assert!(!e.start(&mut m, id));
        m.cpu.inhibit = 1;
        let done = e.done();
        assert!(e.start(&mut m, id)); // STI/MOV SS shadow allows exactly this instruction.
        assert_eq!(m.cpu.inhibit, 0);
        m.cpu.r[AX] = 1;
        assert!(!e.next(&mut m, 1, false)); // Pending IRQ prevents the next translated operation.
        assert_eq!(e.done(), done + 1);
    }
    #[test]
    fn jit_reset_does_not_reuse_a_compiling_identity() {
        let mut m = machine();
        let mut e = Engine::default();
        let old = offered(&mut m, &mut e);
        e.clear();
        assert!(!e.register(old, true));
        let new = offered(&mut m, &mut e);
        assert_ne!(old, new);
        assert!(!e.register(old, true));
    }
    #[test]
    fn jit_refreshes_a_deasserted_slave_wire_before_the_next_edge() {
        let mut m = machine();
        let mut e = Engine::default();
        let id = offered(&mut m, &mut e);
        m.bus.pics[1].mask = 0xef;
        m.bus.pics[1].request(4);
        m.bus.input(8); // Latch slave INT into the master and retain the wire history.
        m.bus.pics[0].irr = 0;
        m.bus.pics[1].irr = 0; // A cleared source must be observed before another edge.
        m.cpu.flags = IF | 2;
        e.begin_slice(&m, 80_000, 2);
        assert!(e.start(&mut m, id));
        m.cpu.r[AX] = 1;
        assert!(e.next(&mut m, 1, false));
        m.cpu.r[AX] = 0;
        e.next(&mut m, 2, false);
        m.bus.pics[1].request(4);
        m.bus.input(8);
        assert_ne!(m.bus.pics[0].irr & 0x80, 0);
    }
    #[test]
    fn cached_branch_rechecks_current_segment_bounds_without_inventing_a_fault() {
        let mut m = machine();
        m.bus.mem[0x100..0x105].copy_from_slice(&[0x90, 0x74, 0x20, 0xeb, 0xfb]);
        let mut e = Engine::default();
        let id = offered(&mut m, &mut e);
        m.cpu.segment_caches[CS] = Some(SegmentCache {
            base: 0,
            limit: 0x102,
            access: 0x9b,
            attributes: 0,
        });
        e.begin_slice(&m, 80_000, 2);
        assert!(e.start(&mut m, id));
        assert!(!e.next(&mut m, 1, false));
        assert_eq!(m.cpu.ip, 0x101); // JZ has not fetched or executed.
        e.fallback(&mut m);
        assert_eq!(m.cpu.ip, 0x103); // Untaken JZ is valid despite its out-of-limit target.
        assert!(m.error.is_none());
        assert_eq!(e.done(), 2);
    }
}

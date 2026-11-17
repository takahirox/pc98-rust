//! Experimental event-bounded conventional-RAM execution. Generated Wasm owns
//! each instruction's semantics and uncached timing; Rust commits once per run.
use super::*;

#[derive(Default)]
#[repr(C)]
pub(super) struct Context {
    ram: u32,
    budget: u32,
    limit: u32,
    committed: u32,
    cycles: u32,
    writes: u32,
    last_execution: u32,
    last_bus: u32,
    last_address: u32,
    last_previous_writes: u32,
    code_start: u32,
    code_end: u32,
    segments: [u32; 6],
    last_index: u32,
    last_taken: u32,
    idtr_base: u32,
    idtr_limit: u32,
    cs: u32,
    bios_valid: u32,
    upper_clear: u32,
    last_bios: u32,
    last_plan: u32,
    pub(super) mode: u32, // Bit 0: direct entry; bit 1: all segment bases may be linked.
    blocks: u32,
}
const _: () = assert!(std::mem::size_of::<Context>() == 116);

// Full keys prevent hash collisions from entering an unrelated table slot.
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub(super) struct Link {
    ip: u32,
    cs: u32,
    id: u32,
}

// Shared with the conventional-memory lookup in web/block-linker.js.
const _: () = assert!(std::mem::size_of::<Link>() == 12 && PROBES == 16384 && SLOTS == 4096);

pub(super) struct Plan {
    spans: Vec<CodeSpan>,
    segments: u8,
    bios: bool,
    runs: u16,
    exits: [u16; BLOCK],
}
struct CodeSpan {
    ip: u32,
    bytes: Vec<u8>,
}
impl Plan {
    pub(super) fn supports(m: &Machine, op: Op) -> bool {
        ordinary(m.cpu.segment_cache(CS))
            && op.memory.is_none_or(|memory| {
                memory.flags & (Memory::READ | Memory::WRITE) == 0
                    || ordinary(m.cpu.segment_cache(memory.segment as usize))
            })
    }
    pub(super) fn new(m: &Machine, ops: &[Op]) -> Option<Self> {
        let mut spans: Vec<CodeSpan> = Vec::new();
        let mut segments = 1 << CS;
        for op in ops {
            if let Some(memory) = op
                .memory
                .filter(|mem| mem.flags & (Memory::READ | Memory::WRITE) != 0)
            {
                segments |= 1 << memory.segment;
            }
            if spans
                .last()
                .is_none_or(|span| span.ip + span.bytes.len() as u32 != op.ip)
            {
                spans.push(CodeSpan {
                    ip: op.ip,
                    bytes: Vec::new(),
                });
            }
            spans
                .last_mut()?
                .bytes
                .extend_from_slice(&op.bytes[..usize::from(op.len)]);
        }
        for i in 0..6 {
            if segments & (1 << i) != 0 && !ordinary(m.cpu.segment_cache(i)) {
                return None;
            }
        }
        Some(Self {
            spans,
            segments,
            bios: ops.iter().any(|op| op.opcode == 0xcd),
            runs: 0,
            exits: [0; BLOCK],
        })
    }
}
// A compound operation still retires INT, the ROM OUT and IRET separately.
// The generated code admits all three only before the next observable event.
fn bios_context(m: &Machine) -> bool {
    let cs = m.cpu.segment_cache(CS);
    m.cpu.cpl == 0
        && cs.base == u32::from(m.cpu.s[CS]) << 4
        && cs.access == 0x9b
        && cs.attributes == 0
        && !m.bus.firmware_request
}
pub(super) fn decode_trace(m: &Machine, ip: u32, ops: &[Op]) -> Option<Op> {
    let at = code(m, ip, 1)?;
    if m.bus.mem[at] != 0xcd {
        return decode(m, ip);
    }
    code(m, ip, 2)?;
    if ip + 2 >= 0x10000
        || !bios_context(m)
        || !ordinary(m.cpu.segment_cache(CS))
        || !ordinary(m.cpu.segment_cache(SS))
    {
        return None;
    }
    // This is only a selection hint; the emitted code checks AH and the current
    // vector/ROM on every execution, including after a preceding RAM store.
    let mut ah = Some(m.cpu.byte(4));
    for op in ops {
        match op.opcode {
            0xb4 => ah = Some(op.bytes[usize::from(op.prefix_len) + 1]),
            0xb8 => ah = Some(op.bytes[usize::from(op.prefix_len) + 2]),
            0xb0 => {}
            _ if op.register_writes == 0xff || op.register_writes & 1 != 0 || op.opcode == 0xcd => {
                ah = None
            }
            _ => {}
        }
    }
    if ah != Some(4) {
        return None;
    }
    let vector = m.bus.mem[at + 1];
    let entry = u32::from(vector) * 4;
    if entry + 3 > u32::from(m.cpu.idtr_limit) {
        return None;
    }
    let address = m.cpu.idtr_base.checked_add(entry)? as usize;
    if address > 0x9fffc
        || m.bus.mem[address..address + 4] != [0x60, 0x10, 0, 0xf0]
        || m.bus.mem[0xf1060..0xf1063] != [0xe6, 0xf0, 0xcf]
    {
        return None;
    }
    let mut bytes = [0; 7];
    bytes[..2].copy_from_slice(&m.bus.mem[at..at + 2]);
    let latency = |op| m.bus.timing.jit_execution(op, 0, 0)[0];
    Some(Op {
        ip,
        bytes,
        opcode: 0xcd,
        len: 2,
        prefix_len: 0,
        modrm: 0,
        memory: Some(Memory {
            segment: SS as u8,
            base: SP as u8,
            index: 8,
            width: NonZeroU8::new(6)?,
            displacement: 0xfffa,
            flags: Memory::READ | Memory::WRITE | Memory::STACK | Memory::BIOS,
        }),
        register_writes: 1 << SP,
        execution: [latency(0xcd) + latency(0xe6) + latency(0xcf), latency(0xcf)],
    })
}
// LEA is offered only by this direct tier. The guarded production compiler
// keeps its existing opcode contract, including on feature-enabled test cores.
pub(super) fn decode(m: &Machine, ip: u32) -> Option<Op> {
    let at = code(m, ip, 1)?;
    let prefix = matches!(m.bus.mem[at], 0x26 | 0x2e | 0x36 | 0x3e | 0x64 | 0x65);
    let skip = u32::from(prefix);
    let at = code(m, ip + skip, 1)?;
    if m.bus.mem[at] != 0x8d {
        return super::decode(m, ip);
    }
    if !ordinary(m.cpu.segment_cache(CS)) {
        return None;
    }
    code(m, ip + skip, 2)?;
    let modrm = m.bus.mem[at + 1];
    if modrm >= 0xc0 {
        return None;
    }
    let len = skip + 2 + displacement_len(modrm);
    let at = code(m, ip, len)?;
    let mut bytes = [0; 7];
    bytes[..len as usize].copy_from_slice(&m.bus.mem[at..at + len as usize]);
    let mut op = Op {
        ip,
        bytes,
        opcode: 0x8d,
        len: len as u8,
        prefix_len: skip as u8,
        modrm,
        memory: None,
        register_writes: 0xff,
        execution: m.bus.timing.jit_execution(0x8d, modrm, skip),
    };
    let mut memory = memory_spec(op)?;
    memory.flags = Memory::ADDRESS; // No read, write, segment or physical-span effect.
    op.memory = Some(memory);
    Some(op)
}
fn ordinary(d: SegmentCache) -> bool {
    d.limit == 0xffff && !d.big() && matches!(d.access & 0x1f, 0x13 | 0x1b)
}
impl Engine {
    pub(super) fn conditional_successor(&self, cpu: &Cpu, op: Op, fallthrough: u32) -> u32 {
        let key = (u64::from(cpu.segment_cache(CS).base) << 32) | u64::from(op.ip);
        let choice = self.branch_choices[probe(key)];
        if choice.key == key && choice.target == op.target() {
            choice.target
        } else {
            fallthrough
        }
    }
    fn observe_branch(&mut self, id: u32, op: Op, index: usize, taken: bool) {
        let Some(plan) = self.plans[id as usize % SLOTS]
            .as_mut()
            .filter(|p| p.id == id)
        else {
            return;
        };
        let Some(block) = &mut plan.block else {
            return;
        };
        block.runs = block.runs.saturating_add(1);
        if !taken
            || !matches!(op.opcode, 0x70..=0x7f | 0xe0..=0xe3)
            || op.target() <= op.ip + u32::from(op.len)
            || plan
                .ops
                .get(index + 1)
                .is_none_or(|next| next.ip == op.target())
        {
            return;
        }
        block.exits[index] = block.exits[index].saturating_add(1);
        if block.exits[index] < 32 || u32::from(block.exits[index]) * 4 < u32::from(block.runs) * 3
        {
            return;
        }
        let key = (plan.key & !u64::from(u32::MAX)) | u64::from(op.ip);
        self.branch_choices[probe(key)] = BranchChoice {
            key,
            target: op.target(),
        };
        self.invalidate_block(id);
    }
    fn invalidate_block(&mut self, id: u32) {
        let Some(key) = self.plan(id).map(|p| p.key) else {
            return;
        };
        self.counters[2] += 1;
        self.register(id, false);
        let p = &mut self.probes[probe(key)];
        if p.key == key {
            p.hits = 1;
            p.id = 0;
        }
    }
    pub fn begin_block_slice(&mut self, m: &Machine, clocks: u32, cap: u32) {
        self.begin_slice(m, clocks, cap);
        self.block_mode = true;
    }
    pub fn block_packet(&self) -> &[u32] {
        &self.block_packet
    }
    pub fn block_links_ptr(&self) -> *const u8 {
        self.links.as_ptr().cast()
    }
    pub fn block_context_offset(&self) -> usize {
        std::mem::offset_of!(Scratch, block)
    }
    pub(super) fn register_link(&mut self, id: u32, ready: bool) {
        let Some(plan) = self.plan(id) else {
            return;
        };
        let key = plan.key;
        let direct = plan.block.is_some();
        let link = &mut self.links[probe(key)];
        if ready && direct {
            *link = Link {
                ip: key as u32,
                cs: (key >> 32) as u32,
                id,
            };
        } else if link.id == id {
            *link = Link::default();
        }
    }
    pub(super) fn make_block_packet(&mut self, id: u32) {
        let plan = self.plan(id).unwrap();
        let mut packet = vec![
            plan.block
                .as_ref()
                .map_or(0, |b| 5 | (u32::from(b.bios) << 1)),
            std::mem::offset_of!(Scratch, block) as u32,
        ];
        if let Some(block) = &plan.block {
            let cs = (plan.key >> 32) as u32;
            packet.extend([
                cs,
                cs + block.spans.iter().map(|s| s.ip).min().unwrap(),
                cs + block
                    .spans
                    .iter()
                    .map(|s| s.ip + s.bytes.len() as u32)
                    .max()
                    .unwrap(),
            ]);
            for op in &plan.ops {
                let mut address = 0;
                let mut base_mask = 0;
                if let Some(mem) = op.memory.filter(|m| m.flags & Memory::ADDRESS != 0) {
                    let mode = op.modrm >> 6;
                    address = u32::from(op.modrm & 7 < 4);
                    base_mask = if mem.base < 8 { 1 << mem.base } else { 0 };
                    let immediate = matches!(op.opcode, 0x80..=0x83 | 0xc6 | 0xc7)
                        || matches!(op.opcode, 0xf6 | 0xf7) && op.modrm & 0x38 == 0;
                    address += u32::from((mode != 0 || mem.base == 8) && immediate);
                }
                let mem = op.memory;
                packet.extend([
                    op.execution[0],
                    op.execution[1],
                    mem.map_or(0, |m| u32::from(m.segment)),
                    mem.map_or(8, |m| u32::from(m.base)),
                    mem.map_or(8, |m| u32::from(m.index)),
                    mem.map_or(0, |m| u32::from(m.displacement)),
                    mem.map_or(0, |m| u32::from(m.width.get())),
                    mem.map_or(0, |m| u32::from(m.flags)),
                    address,
                    base_mask,
                    u32::from(op.register_writes),
                    (((plan.key >> 32) as u32 + op.ip + u32::from(op.len) - 1) / 4
                        - ((plan.key >> 32) as u32 + op.ip) / 4
                        + 1)
                        * 4,
                    u32::from_le_bytes(op.bytes[..4].try_into().unwrap()),
                    u32::from_le_bytes([op.bytes[4], op.bytes[5], op.bytes[6], 0]),
                ]);
            }
        }
        self.block_packet = packet;
    }
    pub(super) fn prepare_block(&mut self, m: &mut Machine, id: u32) -> bool {
        if !eligible(m) || m.cpu.inhibit != 0 {
            return false;
        }
        if m.cpu.cr0 & 0x4000_0000 == 0 {
            self.invalidate_block(id);
            return false;
        }
        let Some(plan) = self.plan(id) else {
            return false;
        };
        let Some(block) = &plan.block else {
            return false;
        };
        if key(&m.cpu) != plan.key {
            return false;
        }
        let mut bases = [0; 6];
        let mut linkable = true;
        for (i, base) in bases.iter_mut().enumerate() {
            let descriptor = m.cpu.segment_cache(i);
            if !ordinary(descriptor) {
                if block.segments & (1 << i) != 0 {
                    self.invalidate_block(id);
                    return false;
                }
                linkable = false;
            }
            *base = descriptor.base;
        }
        // Materialize any deferred device work before validating code bytes
        // or pending interrupts. The resulting event budget then owns the run.
        let Some(event) = m.bus.block_deadline() else {
            return false;
        };
        m.bus.sync_cascade();
        if m.cpu.flags & IF != 0 && m.bus.pics[0].pending().is_some() {
            return false;
        }
        let mut at = usize::MAX;
        let mut end = 0;
        for span in &block.spans {
            let Some(start) = code(m, span.ip, span.bytes.len() as u32) else {
                return false;
            };
            if m.bus.mem[start..start + span.bytes.len()] != span.bytes {
                self.invalidate_block(id);
                return false;
            }
            at = at.min(start);
            end = end.max(start + span.bytes.len());
        }
        let clocks = event.min(self.deadline).saturating_sub(m.bus.cycles);
        let budget = m.clock_converter.before_deadline(clocks, m.bus.profile);
        if budget == 0 {
            return false;
        }
        self.scratch.block = Context {
            ram: m.bus.mem.as_mut_ptr() as usize as u32,
            budget,
            limit: self.remaining,
            writes: u32::from(m.cpu.timing_writes),
            code_start: at as u32,
            code_end: end as u32,
            segments: bases,
            idtr_base: m.cpu.idtr_base,
            idtr_limit: u32::from(m.cpu.idtr_limit),
            cs: u32::from(m.cpu.s[CS]),
            bios_valid: u32::from(bios_context(m)),
            last_plan: id,
            mode: 1 | (u32::from(linkable) << 1),
            upper_clear: 1,
            ..Context::default()
        };
        self.block_active = Some(id);
        true
    }
    pub fn start_block(&mut self, m: &mut Machine, id: u32) -> bool {
        self.runnable(m)
            && self.plan(id).is_some_and(|p| p.ready && p.block.is_some())
            && self.prepare_block(m, id)
    }
    pub fn finish_block(&mut self, m: &mut Machine) {
        let Some(_) = self.block_active.take() else {
            return;
        };
        let ctx = &self.scratch.block;
        let id = ctx.last_plan;
        let count = ctx.committed;
        self.block_counters[0] += 1;
        self.block_counters[3] += u64::from(ctx.blocks);
        if count == 0 {
            self.block_counters[2] += 1;
            self.fallback(m);
            return;
        }
        let Some(op) = self
            .plan(id)
            .and_then(|p| p.ops.get(ctx.last_index as usize))
            .copied()
        else {
            m.error = Some("Invalid experimental block completion".into());
            return;
        };
        if count > ctx.limit || count > self.remaining || ctx.cycles > ctx.budget {
            m.error = Some("Experimental block exceeded its work/event budget".into());
            return;
        }
        let clocks = m.clock_converter.advance(m.bus.profile, ctx.cycles);
        let b = &mut m.bus;
        if ctx.upper_clear & 4 != 0 {
            b.timing.flush();
        }
        b.timing.begin(m.cpu.cr0);
        b.timing.previous_writes = ctx.last_previous_writes as u8;
        b.timing.taken = ctx.last_taken != 0;
        if op
            .memory
            .is_some_and(|mem| mem.flags & Memory::ADDRESS != 0)
        {
            b.timing.instruction = Some(crate::timing::Instruction {
                op: op.opcode,
                ..Default::default()
            });
        }
        b.timing.penalty = ctx.last_bus;
        b.timing.address_execution = ctx.last_address;
        let (physical, last_byte, len) = if ctx.last_bios != 0 {
            (0xf1062, 0xcf, 1)
        } else {
            (
                m.cpu.segment_cache(CS).base.wrapping_add(op.ip),
                op.bytes[usize::from(op.len) - 1],
                op.len,
            )
        };
        b.timing
            .block_fetch_end(physical as usize + usize::from(len) - 1, last_byte);
        let timing = b.timing.finish_jit(ctx.last_execution - ctx.last_address);
        m.cpu.flags_upper &= !(ctx.upper_clear as u16);
        m.cpu.operand32 = false;
        m.cpu.address32 = false;
        m.cpu.lock_prefix = false;
        m.cpu.trap_suppressed = false;
        m.cpu.instruction_bytes = len;
        m.cpu.retired += u64::from(count);
        m.cpu.completed_instructions += u64::from(count);
        m.cpu.approximated_units += u64::from(count);
        m.cpu.cycles += u64::from(ctx.cycles);
        m.cpu.timing_writes = ctx.writes as u8;
        m.cpu.last_timing = timing;
        b.tick(clocks);
        self.remaining -= count;
        self.done += count;
        self.counters[1] += u64::from(count);
        self.block_counters[1] += u64::from(count);
        let index = ctx.last_index as usize;
        let taken = ctx.last_taken != 0;
        self.observe_branch(id, op, index, taken);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> (Machine, Engine, u32) {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.cpu.s = [0; 4];
        m.cpu.flags = 2;
        m.bus.mem[0x100..0x104].copy_from_slice(&[0x40, 0x48, 0xeb, 0xfc]);
        let mut e = Engine::default();
        let mut id = 0;
        for _ in 0..HEAT {
            m.cpu.ip = 0x100;
            e.begin_block_slice(&m, 80_000, 1);
            id = e.dispatch(&mut m, true);
        }
        assert_ne!(id, 0);
        assert_eq!(e.block_packet()[0], 5);
        assert!(e.register(id, true));
        m.cpu.ip = 0x100;
        (m, e, id)
    }
    #[test]
    fn links_clear_on_slot_reuse_unregistration_and_reset() {
        let (mut m, mut e, id) = plan();
        let old_key = e.plan(id).unwrap().key;
        let pointer = e.block_links_ptr();
        assert_eq!(e.links[probe(old_key)].id, id);
        e.serial = id + SLOTS as u32 - 1;
        m.bus.mem[0x180..0x184].copy_from_slice(&[0x43, 0x4b, 0xeb, 0xfc]);
        let mut next = 0;
        for _ in 0..HEAT {
            m.cpu.ip = 0x180;
            e.begin_block_slice(&m, 80_000, 1);
            next = e.dispatch(&mut m, true);
        }
        assert_eq!(next, id + SLOTS as u32);
        assert_eq!(e.links[probe(old_key)].id, 0);
        let new_key = e.plan(next).unwrap().key;
        assert!(e.register(next, true));
        assert_eq!(e.links[probe(new_key)].id, next);
        assert!(!e.register(id, false)); // A stale completion cannot remove the replacement.
        assert_eq!(e.links[probe(new_key)].id, next);
        e.clear();
        assert_eq!(pointer, e.block_links_ptr());
        assert!(e.links.iter().all(|link| link.id == 0));
        let (_, mut unregistered, id) = plan();
        assert!(unregistered.register(id, false));
        assert!(unregistered.links.iter().all(|link| link.id == 0));
    }

    #[test]
    fn an_unused_unusual_segment_disables_linking_without_rejecting_the_trace() {
        let (mut m, mut e, id) = plan();
        m.cpu.segment_caches[4] = Some(SegmentCache {
            base: 0,
            limit: 0xff,
            access: 0x93,
            attributes: 0,
        });
        m.bus.begin_device_run();
        e.begin_block_slice(&m, 80_000, 100);
        assert!(e.start_block(&mut m, id));
        assert_eq!(e.scratch.block.mode, 1);
        m.bus.end_device_run();
    }

    #[test]
    fn learned_forward_successors_require_hot_taken_exits_and_match_current_targets() {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.cpu.s = [0; 4];
        m.cpu.flags = 2;
        m.bus.mem[0x100..0x10b]
            .copy_from_slice(&[0x3d, 0, 0, 0x75, 3, 0x43, 0xeb, 0xf8, 0x42, 0xeb, 0xf5]);
        let mut e = Engine::default();
        let mut id = 0;
        for _ in 0..HEAT {
            m.cpu.ip = 0x100;
            e.begin_block_slice(&m, 80_000, 1);
            id = e.dispatch(&mut m, true);
        }
        let op = e.plan(id).unwrap().ops[1];
        assert_eq!(e.conditional_successor(&m.cpu, op, 0x105), 0x105);
        for _ in 0..31 {
            e.observe_branch(id, op, 1, false);
        }
        for _ in 0..92 {
            e.observe_branch(id, op, 1, true);
        }
        assert!(e.plan(id).is_some()); // Hot taken exits are still below 75%.
        e.observe_branch(id, op, 1, true);
        assert!(e.plan(id).is_none());
        assert_eq!(e.conditional_successor(&m.cpu, op, 0x105), 0x108);
        let mut changed = op;
        changed.bytes[1] = 4;
        assert_eq!(e.conditional_successor(&m.cpu, changed, 0x105), 0x105);
        e.clear();
        assert_eq!(e.conditional_successor(&m.cpu, op, 0x105), 0x105);
    }

    #[test]
    fn compound_decode_is_conservative_about_context_and_ah_selection() {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.cpu.s = [0; 4];
        m.cpu.r[AX] = 0x0403;
        m.bus.mem[0x100..0x102].copy_from_slice(&[0xcd, 0x18]);
        assert!(decode_trace(&m, 0x100, &[]).is_some());
        for scenario in 0..11 {
            let mut copy = Machine::new(Profile::Pc9821ApDos);
            copy.cpu.s = [0; 4];
            copy.cpu.r[AX] = 0x0403;
            copy.bus.mem[0x100..0x102].copy_from_slice(&[0xcd, 0x18]);
            copy.bus.mem[0x200..0x202].copy_from_slice(&[0xcd, 0x18]);
            match scenario {
                0 => copy.cpu.cpl = 1,
                1 => copy.cpu.r[AX] = 0x0103,
                2 => copy.cpu.idtr_limit = 0x60,
                3 => copy.cpu.idtr_base = u32::MAX,
                4 => copy.bus.mem[0x60] ^= 1,
                5 => copy.bus.mem[0xf1060] ^= 1,
                6 => copy.bus.firmware_request = true,
                7 => {
                    copy.cpu.segment_caches[CS] = Some(SegmentCache {
                        base: 0x100,
                        limit: 0xffff,
                        access: 0x9b,
                        attributes: 0,
                    })
                }
                8..=10 => {
                    let seg = if scenario == 8 { CS } else { SS };
                    let mut d = copy.cpu.segment_cache(seg);
                    if scenario == 10 {
                        d.attributes = 0x40;
                    } else {
                        d.limit = 0x7fff;
                    }
                    copy.cpu.segment_caches[seg] = Some(d);
                }
                _ => unreachable!(),
            }
            assert!(
                decode_trace(&copy, 0x100, &[]).is_none(),
                "scenario {scenario}"
            );
        }
        m.bus.mem[0x200..0x202].copy_from_slice(&[0x8b, 0xc3]);
        let unknown_ax = decode(&m, 0x200).unwrap();
        assert!(decode_trace(&m, 0x100, &[unknown_ax]).is_none());
        m.bus.mem[0x200..0x202].copy_from_slice(&[0xb4, 4]);
        let restore_ah = decode(&m, 0x200).unwrap();
        assert!(decode_trace(&m, 0x100, &[unknown_ax, restore_ah]).is_some());
    }

    #[test]
    fn block_admission_keeps_observable_cpu_state_untouched() {
        for scenario in 0..8 {
            let (mut m, mut e, id) = plan();
            match scenario {
                0 => m.cpu.inhibit = 1,
                1 => m.cpu.flags |= TF,
                2 => {
                    m.cpu.flags |= IF;
                    m.bus.pics[0].mask &= !2;
                    m.bus.key(0x1d, true);
                }
                3 => m.cpu.cr0 &= !0x4000_0000,
                4 => {
                    m.cpu.segment_caches[CS] = Some(SegmentCache {
                        base: 0,
                        limit: 0xff,
                        access: 0x9b,
                        attributes: 0,
                    })
                }
                5 => m.bus.mem[0x102] = 0x40,
                6 => m.cpu.halted = true,
                _ => {} // No deferred device scope.
            }
            if scenario != 7 {
                m.bus.begin_device_run();
            }
            e.begin_block_slice(&m, 80_000, 10);
            let before = format!("{:?}", m.cpu);
            let cycles = m.bus.cycles;
            assert!(!e.start_block(&mut m, id), "scenario {scenario}");
            assert_eq!(format!("{:?}", m.cpu), before);
            assert_eq!(m.bus.cycles, cycles);
            assert_eq!(e.done(), 0);
            if matches!(scenario, 3..=5) {
                assert!(e.plan(id).is_none());
            }
            if scenario != 7 {
                m.bus.end_device_run();
            }
        }
    }
    #[test]
    fn direct_stack_admission_defers_unusual_hidden_stack_descriptors() {
        for (limit, attributes) in [(0x7fff, 0), (0xffff, 0x40), (0xffff_ffff, 0)] {
            let mut m = Machine::new(Profile::Pc9821ApDos);
            m.cpu.s = [0; 4];
            m.cpu.flags = 2;
            m.cpu.r[SP] = 0x8000;
            m.bus.mem[0x100..0x105].copy_from_slice(&[0x50, 0x5b, 0xd1, 0xe0, 0xc3]);
            let mut e = Engine::default();
            let mut id = 0;
            for _ in 0..HEAT {
                m.cpu.ip = 0x100;
                e.begin_block_slice(&m, 80_000, 1);
                id = e.dispatch(&mut m, true);
            }
            assert_ne!(id, 0);
            assert_eq!(e.block_packet()[0], 5);
            assert!(e.register(id, true));
            m.cpu.ip = 0x100;
            m.cpu.r[SP] = 0x8000;
            m.cpu.segment_caches[SS] = Some(SegmentCache {
                base: 0,
                limit,
                access: 0x93,
                attributes,
            });
            m.bus.begin_device_run();
            e.begin_block_slice(&m, 80_000, 10);
            let before = format!("{:?}", m.cpu);
            let cycles = m.bus.cycles;
            assert!(!e.start_block(&mut m, id));
            assert!(e.plan(id).is_none());
            assert_eq!(format!("{:?}", m.cpu), before);
            assert_eq!(m.bus.cycles, cycles);
            m.bus.end_device_run();
        }
    }

    #[test]
    fn conditional_fallthrough_is_exclusive_to_direct_trace_collection() {
        for direct in [true, false] {
            let mut m = Machine::new(Profile::Pc9821ApDos);
            m.cpu.s = [0; 4];
            m.cpu.flags = 2;
            m.bus.mem[0x100..0x10b].copy_from_slice(&[
                0x8b, 0x07, 0x75, 3, 0x43, 0xeb, 0xf9, 0x42, 0xeb, 0xf6, 0x90,
            ]);
            if !direct {
                m.cpu.segment_caches[DS] = Some(SegmentCache {
                    base: 0,
                    limit: 0xffff_ffff,
                    access: 0x93,
                    attributes: 0,
                });
            }
            let mut e = Engine::default();
            let mut id = 0;
            for _ in 0..HEAT {
                m.cpu.ip = 0x100;
                e.begin_block_slice(&m, 80_000, 1);
                id = e.dispatch(&mut m, true);
            }
            assert_ne!(id, 0);
            let plan = e.plan(id).unwrap();
            assert_eq!(plan.block.is_some(), direct);
            assert_eq!(
                plan.ops.iter().map(|op| op.ip).collect::<Vec<_>>(),
                if direct {
                    vec![0x100, 0x102, 0x104, 0x105]
                } else {
                    vec![0x100, 0x102]
                }
            );
        }
    }

    #[test]
    fn lea_is_exclusive_to_direct_offers_and_ignores_data_segment_access() {
        let mut m = Machine::new(Profile::Pc9821ApDos);
        m.cpu.s = [0; 4];
        m.cpu.flags = 2;
        m.bus.mem[0x100..0x106].copy_from_slice(&[0x8d, 0x1e, 0xff, 0xff, 0xeb, 0xfa]);
        assert!(super::super::decode(&m, 0x100).is_none());
        m.cpu.segment_caches[DS] = Some(SegmentCache {
            base: 0xa8000,
            limit: 0,
            access: 0,
            attributes: 0x40,
        });
        let op = decode(&m, 0x100).unwrap();
        assert!(Plan::supports(&m, op));
        assert!(Plan::new(&m, &[op]).is_some());
        assert_eq!(op.memory.unwrap().flags, Memory::ADDRESS);
        m.cpu.segment_caches[CS] = Some(SegmentCache {
            base: 0,
            limit: 0xffff,
            access: 0x9b,
            attributes: 0x40,
        });
        assert!(decode(&m, 0x100).is_none());
    }

    #[test]
    fn trace_collection_keeps_unusual_data_segments_on_the_guarded_contract() {
        for unsupported_first in [false, true] {
            let mut m = Machine::new(Profile::Pc9821ApDos);
            m.cpu.s = [0; 4];
            m.cpu.flags = 2;
            m.cpu.r[BX] = 0x8000;
            m.cpu.segment_caches[DS] = Some(SegmentCache {
                base: 0,
                limit: 0xffff_ffff,
                access: 0x93,
                attributes: 0,
            });
            if unsupported_first {
                m.bus.mem[0x100..0x105].copy_from_slice(&[0x8b, 0x07, 0xe9, 0xfb, 0]);
                m.bus.mem[0x200..0x204].copy_from_slice(&[0x8d, 0x1e, 0, 0]);
            } else {
                m.bus.mem[0x100..0x103].copy_from_slice(&[0xe9, 0xfd, 0]);
                m.bus.mem[0x200..0x202].copy_from_slice(&[0x8b, 0x07]);
            }
            let mut e = Engine::default();
            let mut id = 0;
            for _ in 0..HEAT {
                m.cpu.ip = 0x100;
                e.begin_block_slice(&m, 80_000, 1);
                id = e.dispatch(&mut m, true);
            }
            assert_ne!(id, 0);
            let plan = e.plan(id).unwrap();
            assert_eq!(plan.block.is_some(), !unsupported_first);
            assert_eq!(
                plan.ops.iter().map(|op| op.ip).collect::<Vec<_>>(),
                if unsupported_first {
                    vec![0x100, 0x102]
                } else {
                    vec![0x100]
                }
            );
        }
    }

    #[test]
    fn unsupported_timing_mode_rewarms_into_the_guarded_tier() {
        let (mut m, mut e, id) = plan();
        m.cpu.cr0 &= !0x4000_0000;
        e.begin_block_slice(&m, 80_000, 10);
        assert!(!e.start_block(&mut m, id));
        let mut next = 0;
        for _ in 0..HEAT {
            m.cpu.ip = 0x100;
            e.begin_block_slice(&m, 80_000, 1);
            next = e.dispatch(&mut m, true);
            if next != 0 {
                break;
            }
        }
        assert_ne!(next, 0);
        assert_ne!(next, id);
        assert_eq!(e.block_packet()[0], 0);
        assert!(e.register(next, true));
        m.cpu.ip = 0x100;
        e.begin_block_slice(&m, 80_000, 1);
        assert_eq!(e.dispatch_entry(&mut m, false), u64::from(next)); // Original guarded prepare.
        assert_eq!(m.cpu.ip, 0x101);
    }
}

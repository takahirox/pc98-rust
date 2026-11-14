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
}
const _: () = assert!(std::mem::size_of::<Context>() == 80);

pub(super) struct Plan {
    bytes: Vec<u8>,
    segments: u8,
}
impl Plan {
    pub(super) fn new(m: &Machine, ops: &[Op]) -> Option<Self> {
        let mut bytes = Vec::new();
        let mut segments = 1 << CS;
        for op in ops {
            // Shifts and stack transactions retain the guarded tier.
            if matches!(
                op.opcode,
                0x50..=0x5f | 0xc0 | 0xc1 | 0xc3 | 0xd0 | 0xd1 | 0xe8
            ) {
                return None;
            }
            if let Some(memory) = op.memory {
                segments |= 1 << memory.segment;
            }
            bytes.extend_from_slice(&op.bytes[..usize::from(op.len)]);
        }
        for i in 0..6 {
            if segments & (1 << i) != 0 && !ordinary(m.cpu.segment_cache(i)) {
                return None;
            }
        }
        Some(Self { bytes, segments })
    }
}
fn ordinary(d: SegmentCache) -> bool {
    d.limit == 0xffff && !d.big() && matches!(d.access & 0x1f, 0x13 | 0x1b)
}
impl Engine {
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
    pub(super) fn make_block_packet(&mut self, id: u32) {
        let plan = self.plan(id).unwrap();
        let mut packet = vec![
            u32::from(plan.block.is_some()),
            std::mem::offset_of!(Scratch, block) as u32,
        ];
        if plan.block.is_some() {
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
        for (i, base) in bases.iter_mut().enumerate() {
            if block.segments & (1 << i) != 0 {
                let descriptor = m.cpu.segment_cache(i);
                if !ordinary(descriptor) {
                    self.invalidate_block(id);
                    return false;
                }
                *base = descriptor.base;
            }
        }
        let Some(at) = code(m, m.cpu.ip, block.bytes.len() as u32) else {
            return false;
        };
        if m.bus.mem[at..at + block.bytes.len()] != block.bytes {
            self.invalidate_block(id);
            return false;
        }
        let end = at + block.bytes.len();
        m.bus.sync_cascade();
        if m.cpu.flags & IF != 0 && m.bus.pics[0].pending().is_some() {
            return false;
        }
        let Some(event) = m.bus.block_deadline() else {
            return false;
        };
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
        let Some(id) = self.block_active.take() else {
            return;
        };
        let ctx = &self.scratch.block;
        let count = ctx.committed;
        self.block_counters[0] += 1;
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
        let physical = m.cpu.segment_cache(CS).base.wrapping_add(op.ip);
        b.timing.block_fetch_end(
            physical as usize + usize::from(op.len) - 1,
            op.bytes[usize::from(op.len) - 1],
        );
        let timing = b.timing.finish_jit(ctx.last_execution - ctx.last_address);
        m.cpu.flags_upper &= !1;
        m.cpu.operand32 = false;
        m.cpu.address32 = false;
        m.cpu.lock_prefix = false;
        m.cpu.trap_suppressed = false;
        m.cpu.instruction_bytes = op.len;
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
        assert_eq!(e.block_packet()[0], 1);
        assert!(e.register(id, true));
        m.cpu.ip = 0x100;
        (m, e, id)
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

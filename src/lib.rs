//! Original PC-98 experiment. See docs/hardware.md for implemented boundaries.
pub mod bus;
pub mod cpu;
pub mod devices;
pub mod disk;
pub mod egc;
pub mod firmware;
pub mod font;
pub mod hdd;
mod lio;
pub mod mouse;
pub mod pcm;
pub mod profile;
pub mod rtc;
pub mod sound;
pub mod timing;
pub mod video;
use bus::Bus;
use cpu::Cpu;
use disk::Disk;
pub use profile::Profile;
pub struct Machine {
    pub cpu: Cpu,
    pub bus: Bus,
    pub error: Option<String>,
    pub framebuffer: Vec<u8>,
    pub clock_converter: timing::ClockConverter,
}
impl Default for Machine {
    fn default() -> Self {
        Self::new(Profile::Pc9801Vm)
    }
}
impl Machine {
    pub fn new(profile: Profile) -> Self {
        let mut m = Self {
            cpu: Cpu::default(),
            bus: Bus::new(profile),
            error: None,
            framebuffer: vec![0; 640 * 400 * 4],
            clock_converter: timing::ClockConverter::default(),
        };
        if profile == Profile::Pc9821ApDos {
            m.cpu.flags = 2;
        }
        firmware::initialize(&mut m.bus);
        m
    }
    pub fn insert(&mut self, drive: usize, bytes: Vec<u8>, name: &str) -> Result<(), String> {
        if drive == 2 {
            if self.bus.profile != Profile::Pc9821ApDos {
                return Err("IDE requires the Ap profile".into());
            }
            let disk = hdd::HardDisk::parse(bytes)?;
            self.bus.ide.mount(Some(disk));
            return Ok(());
        }
        if drive > 1 {
            return Err("only two floppy drives".into());
        }
        let disk = Disk::parse(bytes, name)?;
        self.bus.mount_floppy(drive, Some(disk));
        Ok(())
    }
    /// Select the next reset device; mounting/ejecting media never changes it.
    pub fn set_boot_hdd(&mut self, hdd: bool) {
        self.bus.boot_hdd = hdd;
    }
    pub fn reset(&mut self) {
        let disks = std::mem::take(&mut self.bus.disks);
        let hdd = self.bus.ide.disk.take();
        let boot_hdd = self.bus.boot_hdd;
        let rtc = std::mem::take(&mut self.bus.rtc);
        // RESET without BIST retains the x87 environment (Intel486 PRM 19.2.3).
        self.cpu = Cpu {
            fpu: self.cpu.fpu,
            ..Cpu::default()
        };
        if self.bus.profile == Profile::Pc9821ApDos {
            self.cpu.flags = 2;
        }
        self.bus = Bus::new(self.bus.profile);
        self.bus.rtc = rtc;
        self.bus.disks = disks;
        self.bus.ide.disk = hdd;
        self.bus.boot_hdd = boot_hdd;
        self.error = None;
        self.clock_converter = timing::ClockConverter::default();
        firmware::initialize(&mut self.bus);
    }
    pub fn step(&mut self, budget: u32) -> u32 {
        self.step_with_observer(budget, |_, _| {})
    }
    /// Step with a read-only callback before each instruction, after IRQ delivery.
    pub fn step_with_observer(
        &mut self,
        budget: u32,
        mut before_instruction: impl FnMut(&Cpu, &Bus),
    ) -> u32 {
        let mut done = 0;
        for _ in 0..budget.min(100_000) {
            if self.error.is_some() {
                break;
            }
            self.step_unit(&mut before_instruction);
            done += 1;
        }
        done
    }
    // Both budget APIs share one timed unit. Inline it into their loops so the
    // scheduler does not enter a second capped loop for every instruction.
    #[inline(always)]
    fn step_unit(&mut self, observer: &mut impl FnMut(&Cpu, &Bus)) {
        let (elapsed, result) = self.cpu.step_timed_with_observer(&mut self.bus, observer);
        let clocks = self
            .clock_converter
            .advance(self.bus.profile, elapsed.cpu_cycles());
        self.bus.tick(clocks);
        if let Err(e) = result {
            self.error = Some(e);
        }
    }
    /// Advance toward a device-clock deadline, with an independent work cap.
    /// Returns bounded units executed; callers retain any unfinished time debt.
    /// One instruction/firmware transaction can overshoot the requested time.
    pub fn step_scheduler(&mut self, clocks: u32, max_units: u32) -> u32 {
        self.step_scheduler_with_observer(clocks, max_units, |_, _| {})
    }
    pub fn step_scheduler_with_observer(
        &mut self,
        clocks: u32,
        max_units: u32,
        mut observer: impl FnMut(&Cpu, &Bus),
    ) -> u32 {
        let deadline = self.bus.cycles + u64::from(clocks.min(80_000));
        let mut done = 0;
        while self.bus.cycles < deadline && done < max_units.min(100_000) && self.error.is_none() {
            self.step_unit(&mut observer);
            done += 1;
        }
        done
    }
    pub fn render(&mut self) -> &[u8] {
        self.bus.frame(&mut self.framebuffer);
        &self.framebuffer
    }
}
#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use std::cell::RefCell;
    thread_local! {
        static JIT: RefCell<cpu::jit::Engine> = RefCell::new(cpu::jit::Engine::default());
        static JIT_LAYOUT: RefCell<[usize; 4]> = const { RefCell::new([0; 4]) };
    }
    #[no_mangle]
    pub extern "C" fn jit_clear() {
        JIT.with(|j| j.borrow_mut().clear());
    }
    #[no_mangle]
    pub extern "C" fn jit_begin_slice(clocks: u32, cap: u32) {
        MACHINE.with(|m| JIT.with(|j| j.borrow_mut().begin_slice(&m.borrow(), clocks, cap)));
    }
    #[no_mangle]
    pub extern "C" fn jit_dispatch(compile: u32) -> u32 {
        MACHINE.with(|m| JIT.with(|j| j.borrow_mut().dispatch(&mut m.borrow_mut(), compile != 0)))
    }
    #[no_mangle]
    pub extern "C" fn jit_packet() -> *const u32 {
        JIT.with(|j| j.borrow().packet().as_ptr())
    }
    #[no_mangle]
    pub extern "C" fn jit_packet_len() -> usize {
        JIT.with(|j| j.borrow().packet().len())
    }
    #[no_mangle]
    pub extern "C" fn jit_done() -> u32 {
        JIT.with(|j| j.borrow().done())
    }
    #[no_mangle]
    pub extern "C" fn jit_metric(index: usize) -> f64 {
        JIT.with(|j| j.borrow().counters.get(index).copied().unwrap_or(0) as f64)
    }
    #[no_mangle]
    pub extern "C" fn jit_enter(id: u32) -> u32 {
        MACHINE.with(|m| JIT.with(|j| j.borrow_mut().enter(&mut m.borrow_mut(), id)))
    }
    #[no_mangle]
    pub extern "C" fn jit_register(id: u32, ready: u32) -> u32 {
        JIT.with(|j| u32::from(j.borrow_mut().register(id, ready != 0)))
    }
    #[no_mangle]
    pub extern "C" fn jit_start(id: u32) -> u32 {
        MACHINE.with(|m| JIT.with(|j| u32::from(j.borrow_mut().start(&mut m.borrow_mut(), id))))
    }
    #[no_mangle]
    pub extern "C" fn jit_next(index: usize, taken: u32) -> u32 {
        MACHINE.with(|m| {
            JIT.with(|j| u32::from(j.borrow_mut().next(&mut m.borrow_mut(), index, taken != 0)))
        })
    }
    #[no_mangle]
    pub extern "C" fn jit_batch_next(end: usize, taken: u32) -> u32 {
        MACHINE.with(|m| {
            JIT.with(|j| {
                u32::from(
                    j.borrow_mut()
                        .next_batch(&mut m.borrow_mut(), end, taken != 0),
                )
            })
        })
    }
    #[no_mangle]
    pub extern "C" fn jit_fallback() {
        MACHINE.with(|m| JIT.with(|j| j.borrow_mut().fallback(&mut m.borrow_mut())));
    }
    #[no_mangle]
    pub extern "C" fn jit_layout() -> *const usize {
        MACHINE.with(|m| {
            JIT_LAYOUT.with(|layout| {
                let mut m = m.borrow_mut();
                *layout.borrow_mut() = [
                    m.cpu.r.as_mut_ptr() as usize,
                    &mut m.cpu.flags as *mut u16 as usize,
                    &mut m.cpu.ip as *mut u32 as usize,
                    JIT.with(|j| j.borrow_mut().scratch_ptr() as usize),
                ];
                layout.borrow().as_ptr()
            })
        })
    }
    thread_local! {static MACHINE:RefCell<Machine>=RefCell::new(Machine::default());static TRANSFER:RefCell<Vec<u8>>=const{RefCell::new(Vec::new())};static MESSAGE:RefCell<Vec<u8>>=const{RefCell::new(Vec::new())};static PENDING:RefCell<Option<hdd::HdiImport>>=const{RefCell::new(None)};static EXPORTED:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
    #[no_mangle]
    pub extern "C" fn input_buffer(len: usize) -> *mut u8 {
        if len > 4 * 1024 * 1024 {
            return std::ptr::null_mut();
        }
        TRANSFER.with(|b| {
            let mut b = b.borrow_mut();
            let additional = len.saturating_sub(b.len());
            if b.try_reserve_exact(additional).is_err() {
                return std::ptr::null_mut();
            }
            b.resize(len, 0);
            b.as_mut_ptr()
        })
    }
    #[no_mangle]
    pub extern "C" fn insert(drive: usize, format: u32) -> u32 {
        let bytes = TRANSFER.with(|b| std::mem::take(&mut *b.borrow_mut()));
        MACHINE.with(|m| {
            let mut m = m.borrow_mut();
            match m.insert(
                drive,
                bytes,
                if format == 1 { "disk.d88" } else { "disk.hdm" },
            ) {
                Ok(()) => 1,
                Err(e) => {
                    MESSAGE.with(|b| *b.borrow_mut() = e.into_bytes());
                    0
                }
            }
        })
    }
    #[no_mangle]
    pub extern "C" fn set_profile(id: u32) -> u32 {
        let profile = match id {
            0 => Profile::Pc9801Vm,
            1 => Profile::Pc9821ApDos,
            _ => return 0,
        };
        MACHINE.with(|m| {
            let mut m = m.borrow_mut();
            m.bus.profile = profile;
            m.reset();
        });
        jit_clear();
        1
    }
    #[no_mangle]
    pub extern "C" fn set_boot_source(hdd: u32) -> u32 {
        if hdd > 1 {
            return 0;
        }
        MACHINE.with(|m| m.borrow_mut().set_boot_hdd(hdd == 1));
        1
    }
    #[no_mangle]
    pub extern "C" fn reset() {
        MACHINE.with(|m| m.borrow_mut().reset());
        jit_clear();
    }
    #[no_mangle]
    pub extern "C" fn eject(drive: usize) {
        if drive < 2 {
            MACHINE.with(|m| {
                m.borrow_mut().bus.mount_floppy(drive, None);
            })
        } else if drive == 2 {
            MACHINE.with(|m| m.borrow_mut().bus.ide.mount(None))
        }
    }
    #[no_mangle]
    pub extern "C" fn step(budget: u32) -> u32 {
        MACHINE.with(|m| m.borrow_mut().step(budget))
    }
    #[no_mangle]
    pub extern "C" fn step_scheduler(clocks: u32, max_units: u32) -> u32 {
        MACHINE.with(|m| m.borrow_mut().step_scheduler(clocks, max_units))
    }
    #[no_mangle]
    pub extern "C" fn frame() -> *const u8 {
        MACHINE.with(|m| m.borrow_mut().render().as_ptr())
    }
    thread_local! {
        static VIDEO: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        static RENDERER: RefCell<video::SnapshotRenderer> = RefCell::new(video::SnapshotRenderer::default());
        static PIXELS: RefCell<Vec<u8>> = RefCell::new(vec![0; 640*400*4]);
    }
    #[no_mangle]
    pub extern "C" fn video_snapshot() -> *const u8 {
        VIDEO.with(|v| {
            let mut v = v.borrow_mut();
            MACHINE.with(|m| m.borrow().bus.video().snapshot(&mut v));
            v.as_ptr()
        })
    }
    #[no_mangle]
    pub extern "C" fn video_snapshot_len() -> usize {
        VIDEO.with(|v| v.borrow().len())
    }
    // A render-only instance initializes neither Machine nor devices.
    #[no_mangle]
    pub extern "C" fn video_render(len: usize) -> *const u8 {
        TRANSFER.with(|v| {
            PIXELS.with(|pixels| {
                RENDERER.with(|renderer| {
                    let bytes = v.borrow();
                    let mut pixels = pixels.borrow_mut();
                    if len != bytes.len() || !renderer.borrow_mut().render(&bytes, &mut pixels) {
                        return std::ptr::null();
                    }
                    pixels.as_ptr()
                })
            })
        })
    }
    #[no_mangle]
    pub extern "C" fn key(scan: u8, down: u32) {
        MACHINE.with(|m| m.borrow_mut().bus.key(scan, down != 0))
    }
    #[no_mangle]
    pub extern "C" fn mouse(dx: i32, dy: i32, buttons: u8) {
        MACHINE.with(|m| m.borrow_mut().bus.mouse.motion(dx, dy, buttons))
    }
    #[no_mangle]
    pub extern "C" fn peek(addr: usize) -> u32 {
        MACHINE.with(|m| m.borrow().bus.read(addr) as u32)
    }
    #[no_mangle]
    pub extern "C" fn beep() -> u32 {
        MACHINE.with(|m| {
            let m = m.borrow();
            if m.bus.beep {
                m.bus.pit.beep_hz()
            } else {
                0
            }
        })
    }
    #[no_mangle]
    pub extern "C" fn audio_ptr() -> *const f32 {
        MACHINE.with(|m| m.borrow().bus.audio.as_ptr())
    }
    #[no_mangle]
    pub extern "C" fn audio_len() -> usize {
        MACHINE.with(|m| m.borrow().bus.audio.len())
    }
    #[no_mangle]
    pub extern "C" fn audio_clear() {
        MACHINE.with(|m| m.borrow_mut().bus.audio.clear())
    }
    #[no_mangle]
    pub extern "C" fn metric(id: u32) -> f64 {
        MACHINE.with(|m| {
            let m = m.borrow();
            match id {
                0 => m.cpu.retired as f64,
                1 => m.bus.fdc.reads as f64,
                2 => m.bus.fdc.writes as f64,
                3 => m.bus.beep_edges as f64,
                4 => u8::from(m.error.is_some()) as f64,
                6 => m.bus.opn.writes as f64,
                7 => m.bus.opn.key_edges as f64,
                8 => m.cpu.extended_retired as f64,
                9 => m.bus.ide.reads as f64,
                10 => m.bus.ide.writes as f64,
                11 => m.bus.mouse.events as f64,
                12 => m.bus.mouse.latches as f64,
                13 => u8::from(m.bus.pegc) as f64,
                14 => m.bus.pcm.writes as f64,
                15 => m.bus.pcm.samples as f64,
                16 => u8::from(m.cpu.shutdown) as f64,
                17 => m.cpu.s[1] as f64,
                18 => m.cpu.ip as f64,
                19 => m.cpu.fault_count as f64,
                20 => m.cpu.irq_acks as f64,
                // Actual profile and retained boot preference for browser controls.
                21 => u8::from(m.bus.profile == Profile::Pc9821ApDos) as f64,
                22 => u8::from(m.bus.boot_hdd) as f64,
                23 => m.bus.sound_irq_edges as f64,
                24 => u16::from_le_bytes([m.bus.opn.registers[0xa0], m.bus.opn.registers[0xa4]])
                    as f64,
                25 => u16::from_le_bytes([m.bus.opn.registers[0], m.bus.opn.registers[1]]) as f64,
                26 => m.cpu.cycles as f64,
                27 => m.cpu.completed_instructions as f64,
                28 => m.cpu.rep_elements as f64,
                29 => m.bus.cycles as f64 / timing::SCHEDULER_HZ as f64,
                30 => m.cpu.approximated_units as f64,
                31 => m.bus.profile.cpu_hz() as f64,
                _ => m.bus.cycles as f64,
            }
        })
    }
    #[no_mangle]
    pub extern "C" fn message(kind: u32) -> *const u8 {
        let s = MACHINE.with(|m| {
            let m = m.borrow();
            if kind == 1 {
                m.bus.text()
            } else {
                m.error.clone().unwrap_or_default()
            }
        });
        MESSAGE.with(|b| {
            if kind != 2 {
                *b.borrow_mut() = s.into_bytes()
            }
            b.borrow().as_ptr()
        })
    }
    #[no_mangle]
    pub extern "C" fn message_len() -> usize {
        MESSAGE.with(|b| b.borrow().len())
    }
    fn import_result(result: Result<(), String>) -> u32 {
        match result {
            Ok(()) => 1,
            Err(e) => {
                MESSAGE.with(|b| *b.borrow_mut() = e.into_bytes());
                0
            }
        }
    }
    #[no_mangle]
    pub extern "C" fn hdi_begin(file_len: u32) -> u32 {
        PENDING.with(|p| *p.borrow_mut() = None);
        let result = TRANSFER.with(|b| hdd::HdiImport::new(&b.borrow(), file_len as u64));
        import_result(result.map(|disk| PENDING.with(|p| *p.borrow_mut() = Some(disk))))
    }
    #[no_mangle]
    pub extern "C" fn hdi_append() -> u32 {
        let result = PENDING.with(|p| {
            let mut pending = p.borrow_mut();
            let result = pending
                .as_mut()
                .ok_or_else(|| "No HDI import in progress".to_string())
                .and_then(|disk| TRANSFER.with(|b| disk.append(&b.borrow())));
            if result.is_err() {
                *pending = None;
            }
            result
        });
        import_result(result)
    }
    #[no_mangle]
    pub extern "C" fn hdi_cancel() {
        PENDING.with(|p| *p.borrow_mut() = None);
    }
    #[no_mangle]
    pub extern "C" fn hdi_commit() -> u32 {
        let result = PENDING.with(|p| {
            p.borrow_mut()
                .take()
                .ok_or_else(|| "No HDI import in progress".to_string())
                .and_then(hdd::HdiImport::finish)
        });
        import_result(result.and_then(|disk| {
            MACHINE.with(|m| {
                let mut m = m.borrow_mut();
                if m.bus.profile != Profile::Pc9821ApDos {
                    return Err("IDE requires the Ap profile".into());
                }
                m.bus.ide.mount(Some(disk));
                Ok(())
            })
        }))
    }
    #[no_mangle]
    pub extern "C" fn hdi_dirty_page(from: usize) -> u32 {
        MACHINE.with(|m| {
            m.borrow()
                .bus
                .ide
                .disk
                .as_ref()
                .and_then(|d| d.dirty_page(from))
                .map_or(u32::MAX, |page| page as u32)
        })
    }
    #[no_mangle]
    pub extern "C" fn hdi_len() -> usize {
        MACHINE.with(|m| m.borrow().bus.ide.disk.as_ref().map_or(0, |d| d.len()))
    }
    #[no_mangle]
    pub extern "C" fn hdi_resident_bytes() -> usize {
        MACHINE.with(|m| {
            m.borrow()
                .bus
                .ide
                .disk
                .as_ref()
                .map_or(0, |d| d.resident_bytes())
        })
    }
    #[no_mangle]
    pub extern "C" fn hdi_read_chunk(at: usize, len: usize) -> *const u8 {
        if len > hdd::TRANSFER_BYTES {
            return std::ptr::null();
        }
        let ptr = input_buffer(len);
        if ptr.is_null() {
            return ptr;
        }
        let result = MACHINE.with(|m| {
            let m = m.borrow();
            m.bus
                .ide
                .disk
                .as_ref()
                .ok_or_else(|| "No HDI mounted".to_string())
                .and_then(|d| TRANSFER.with(|b| d.read_bytes(at, &mut b.borrow_mut())))
        });
        if import_result(result) == 0 {
            std::ptr::null()
        } else {
            ptr
        }
    }
    #[no_mangle]
    pub extern "C" fn export_disk(drive: usize) -> *const u8 {
        // Floppies are small and directly borrowed; HDD export is chunked.
        MACHINE.with(|m| {
            let m = m.borrow();
            let bytes = m
                .bus
                .disks
                .get(drive)
                .and_then(|d| d.as_ref().map(|d| &d.bytes));
            EXPORTED.with(|len| len.set(bytes.map_or(0, Vec::len)));
            bytes.map_or(std::ptr::null(), |bytes| bytes.as_ptr())
        })
    }
    #[no_mangle]
    pub extern "C" fn export_len() -> usize {
        EXPORTED.with(|len| len.get())
    }
}

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
use bus::Bus;
use cpu::Cpu;
use disk::Disk;
pub use profile::Profile;
pub struct Machine {
    pub cpu: Cpu,
    pub bus: Bus,
    pub error: Option<String>,
    pub framebuffer: Vec<u8>,
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
            self.bus.boot_hdd = true;
            return Ok(());
        }
        if drive > 1 {
            return Err("only two floppy drives".into());
        }
        let disk = Disk::parse(bytes, name)?;
        self.bus.mount_floppy(drive, Some(disk));
        if drive == 0 {
            self.bus.boot_hdd = false;
        }
        Ok(())
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
            let mut result = self
                .cpu
                .step_with_observer(&mut self.bus, &mut before_instruction);
            if self.bus.firmware_request {
                self.bus.firmware_request = false;
                if result.is_ok() {
                    result = firmware::service(&mut self.cpu, &mut self.bus)
                }
            }
            self.bus.tick(8);
            done += 1;
            if let Err(e) = result {
                self.error = Some(e);
                break;
            }
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
    thread_local! {static MACHINE:RefCell<Machine>=RefCell::new(Machine::default());static TRANSFER:RefCell<Vec<u8>>=const{RefCell::new(Vec::new())};static MESSAGE:RefCell<Vec<u8>>=const{RefCell::new(Vec::new())};}
    #[no_mangle]
    pub extern "C" fn input_buffer(len: usize) -> *mut u8 {
        if len > hdd::MAX_HDI_BYTES {
            return std::ptr::null_mut();
        }
        TRANSFER.with(|b| {
            let mut b = b.borrow_mut();
            b.resize(len, 0);
            b.as_mut_ptr()
        })
    }
    #[no_mangle]
    pub extern "C" fn insert(drive: usize, format: u32) -> u32 {
        let bytes = TRANSFER.with(|b| b.borrow().clone());
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
        1
    }
    #[no_mangle]
    pub extern "C" fn reset() {
        MACHINE.with(|m| m.borrow_mut().reset())
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
    pub extern "C" fn frame() -> *const u8 {
        MACHINE.with(|m| m.borrow_mut().render().as_ptr())
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
    #[no_mangle]
    pub extern "C" fn export_disk(drive: usize) -> *const u8 {
        MACHINE.with(|m| {
            let m = m.borrow();
            let bytes = if drive == 2 {
                m.bus.ide.disk.as_ref().map(|d| &d.bytes)
            } else {
                m.bus
                    .disks
                    .get(drive)
                    .and_then(|d| d.as_ref().map(|d| &d.bytes))
            };
            if let Some(bytes) = bytes {
                TRANSFER.with(|b| {
                    *b.borrow_mut() = bytes.clone();
                    b.borrow().as_ptr()
                })
            } else {
                TRANSFER.with(|b| b.borrow_mut().clear());
                std::ptr::null()
            }
        })
    }
    #[no_mangle]
    pub extern "C" fn export_len() -> usize {
        TRANSFER.with(|b| b.borrow().len())
    }
}

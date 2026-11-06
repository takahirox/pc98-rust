//! Deliberately bounded DOS hardware configurations; see docs/hardware.md.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    #[default]
    Pc9801Vm,
    Pc9821ApDos,
}

impl Profile {
    /// Selected clocks: VM V30-10; Ap Intel486DX2-66 (33 MHz external bus).
    pub fn cpu_hz(self) -> u64 {
        match self {
            Self::Pc9801Vm => 10_000_000,
            Self::Pc9821ApDos => 66_000_000,
        }
    }
    pub fn planes(self) -> usize {
        match self {
            Self::Pc9801Vm => 3,
            Self::Pc9821ApDos => 4,
        }
    }
}

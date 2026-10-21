//! Deliberately bounded DOS hardware configurations; see docs/hardware.md.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    #[default]
    Pc9801Vm,
    Pc9821ApDos,
}

impl Profile {
    pub fn planes(self) -> usize {
        match self {
            Self::Pc9801Vm => 3,
            Self::Pc9821ApDos => 4,
        }
    }
}

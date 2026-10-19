//! Evidence requirements shared by capture-backed scenarios.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Mismatch,
    MissingCapture,
    Overflow,
    Infrastructure,
    Unqualified,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "oracle {self:?}")
    }
}

pub fn fail(kind: Failure, reason: impl fmt::Display) -> ! {
    panic!("{kind}: {reason}")
}

/// A quiet log alone does not prove a particular protocol outcome. These are
/// prerequisites for judging it; the scenario must still assert its milestone.
pub fn check_log(overflowed: bool, coherent: Option<bool>, settled: bool) -> Result<(), Failure> {
    if overflowed {
        Err(Failure::Overflow)
    } else if coherent != Some(true) || !settled {
        Err(Failure::Infrastructure)
    } else {
        Ok(())
    }
}

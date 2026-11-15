//! Internal X25519 and XEdDSA arithmetic, limited to the Signal callers.
//!
//! Arithmetic and tables derive from curve25519-dalek 5.0.0. The u64
//! field and projective Montgomery conversion incorporate changes from
//! <https://github.com/jlucaso1/curve25519-dalek/tree/7c7e3b35f72551177b2d0e4db09d7c6c603c4326>.
//! See LICENSE for the retained BSD terms. Secret scalar multiplication uses
//! fixed loops and subtle selections. Variable-time NAF is only for verification.
//! Both limb backends share table data, converted during constant evaluation.
//! On x86_64, verification selects AVX2 after runtime CPU detection and retains
//! a serial fallback. Vector operations are confined to the private AVX2 module.
//! XEdDSA parsing and equations are shared by the provider and prepared keys.
#![deny(dead_code)]
#![allow(clippy::op_ref, clippy::needless_range_loop)]

#[cfg(not(target_pointer_width = "64"))]
#[path = "constants32.rs"]
mod constants;
#[cfg(target_pointer_width = "64")]
#[path = "constants64.rs"]
mod constants;
pub(crate) mod edwards;
mod field;
#[cfg(not(target_pointer_width = "64"))]
mod field2625;
#[cfg(target_pointer_width = "64")]
mod field51;
mod models;
pub(crate) mod montgomery;
pub(crate) mod scalar;
#[cfg(not(target_pointer_width = "64"))]
mod scalar29;
#[cfg(target_pointer_width = "64")]
mod scalar52;
mod tables;
mod xeddsa;
pub(crate) use xeddsa::{ParsedSignature, sign, verify};
#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
#[allow(unsafe_code)]
mod avx2;
mod vartime;
mod window;
mod traits {
    pub(super) trait Identity {
        fn identity() -> Self;
    }
}
#[cfg(test)]
pub(crate) use tables::EIGHT_TORSION;

#[cfg(test)]
mod tests;

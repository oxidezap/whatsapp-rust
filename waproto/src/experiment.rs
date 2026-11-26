#![forbid(unsafe_code)]

#[path = "../experiment/wire.rs"]
pub mod wire;
include!(concat!(env!("OUT_DIR"), "/experiment.rs"));

#![allow(clippy::module_inception)]

pub mod alu;
pub mod cpu;
pub mod psr;

pub mod exception;
mod modes;

#[cfg(test)]
mod tests;

pub use crate::bit_r;
pub use modes::ShiftType;


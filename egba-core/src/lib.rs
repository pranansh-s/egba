pub mod cartridge;
pub mod bios;
pub mod gba;
pub mod rom;
pub mod keypad;
pub mod cpu;

mod bus;
mod interrupt;
mod memory;

mod constants;
use constants::*;
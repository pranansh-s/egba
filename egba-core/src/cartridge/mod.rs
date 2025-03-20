pub mod backup;

use crate::rom::{InvalidROM, Rom};

pub struct Cartridge {
    rom: Rom,
    // backup: Option<Backup>
}

impl Cartridge {
    pub fn new(rom: Rom) -> Result<Cartridge, InvalidROM> {
        if rom.len() > 0x2000000 {
            return Err(InvalidROM);
        }

        Ok(
            Self {
                rom
            }
        )
    }
}
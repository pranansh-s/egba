pub mod backup;

use backup::BackupMedia;

use crate::{bus::Bus, rom::{InvalidROM, Rom}};

pub struct Cartridge {
    rom: Rom,
    // backup: Option<BackupMedia>
}

impl Cartridge {
    #[must_use]
    pub fn new(rom: Rom) -> Result<Cartridge, InvalidROM> {
        if rom.len() > 0x2000000 {
            return Err(InvalidROM);
        }

        Ok(
            Self {
                rom,
                // backup
            }
        )
    }

    pub fn read(&self, addr: u32) -> u8 {
        self.rom.read_byte(addr)
    }

    pub fn write(&mut self, addr: u32, value: u8) {
        self.rom.write_byte(addr, value)
    }
}
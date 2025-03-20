use crate::{bus::Bus, rom::{InvalidROM, Rom}};

pub struct Bios {
    rom: Rom,
    skip_bios: bool,
}

impl Bios {
    pub fn new(rom: Rom) -> Result<Bios, InvalidROM> {
        if rom.len() != 0x4000 {
            return Err(InvalidROM);
        }
        
        Ok(
            Self {
                rom,
                skip_bios: false
            }
        )
    }

    pub fn read(&self, addr: u32) -> u8 {
        self.rom.read_byte(addr)
    }
}
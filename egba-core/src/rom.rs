use std::{error::Error, fmt};

use crate::{bus::Bus, cartridge::backup::BackupType};

pub struct Rom(Box<[u8]>);

#[derive(Debug)]
pub struct InvalidROM;

impl fmt::Display for InvalidROM {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid Rom size detected")
    }
}

impl Error for InvalidROM {}

impl Rom {
    pub fn new(data: &Vec<u8>) -> Self {
        Self(data.clone().into_boxed_slice())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn data(&self) -> &Box<[u8]> {
        &self.0
    }

    pub fn get_backup_type(&self) -> BackupType {
        const IDS: [(&[u8], BackupType); 5] = [
            (b"EEPROM_V", BackupType::Eeprom8KB),
            (b"SRAM_V", BackupType::Sram32KB),
            (b"FLASH_V", BackupType::Flash64KB),
            (b"FLASH512_V", BackupType::Flash64KB),
            (b"FLASH1M_V", BackupType::Flash128KB),
        ];

        for i in (0..self.len()).step_by(4) {
            let data = self.data();
            for (id, backup_type) in IDS {
                if data[i..].starts_with(id) {
                    return backup_type.clone()
                }
            }
        }
        BackupType::NoBackup
    }
}

impl Bus for Rom {
    fn read_byte(&self, addr: u32) -> u8 {
        self.0[addr as usize]
    }
}
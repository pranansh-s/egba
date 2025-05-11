pub mod backup;

use std::{fs, path::PathBuf};

use backup::{BackupMedia, BackupType};
use crate::{bus::Bus, rom::{InvalidROM, Rom}};

fn read_type(rom: &Rom) -> BackupType {
    let IDS: [(&[u8], BackupType); 5] = [
        (b"EEPROM_V", BackupType::Eeprom8KB),
        (b"SRAM_V", BackupType::Sram32KB),
        (b"FLASH_V", BackupType::Flash64KB),
        (b"FLASH512_V", BackupType::Flash64KB),
        (b"FLASH1M_V", BackupType::Flash128KB),
    ];

    for i in (0..rom.len()).step_by(4) {
        let data = rom.data();
        for (id, backup_type) in &IDS {
            if data[i..].starts_with(id) {
                return backup_type.clone()
            }
        }
    }
    BackupType::NoBackup
}

pub struct Cartridge {
    rom: Rom,
    backup: Option<BackupMedia>
}

impl Cartridge {
    fn load_backup(path: &PathBuf) -> Option<BackupMedia> {
        if let Ok(buf) = fs::read(path) {
            // derive type and use backup media
        }
        else {
            // use a fallback type and create backup
        }
        None
    }

    #[must_use]
    pub fn new(rom: Rom, backup_path: &PathBuf) -> Result<Cartridge, InvalidROM> {
        if rom.len() > 0x2000000 {
            return Err(InvalidROM);
        }

        let backup_type = read_type(&rom);

        // let backup = match backup_type {
        //     BackupType::None => None,
        //     BackupType::Eeprom512B => Some(BackupMedia::Eeprom(Eeprom::new())),
        //     BackupType::Eeprom8KB => Some(BackupMedia::Eeprom(Eeprom::new())),
        //     BackupType::Flash64KB => Some(BackupMedia::Flash(Flash::new())),
        //     BackupType::Flash128KB => Some(BackupMedia::Flash(Flash::new())),
        //     BackupType::Sram32KB => Some(BackupMedia::Sram(Sram::new(vec![0x0, 32 * 1024].into()))),
        // };

        Ok(
            Self {
                rom,
                backup
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
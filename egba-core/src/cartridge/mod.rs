pub mod backup;

use std::{fs, path::PathBuf};

use backup::{eeprom::EEPROM, flash::Flash, sram::SRAM, BackupBuffer, BackupMedia, BackupType};
use crate::{bus::Bus, rom::{InvalidROM, Rom}};

pub struct Cartridge {
    rom: Rom,
    backup: Option<BackupMedia>,
    eeprom_mask: Option<usize>
}

impl Cartridge {
    #[must_use]
    pub fn new(rom: Rom, backup_path: &PathBuf) -> Result<Cartridge, InvalidROM> {
        if rom.len() > 0x2000000 {
            return Err(InvalidROM);
        }

        let backup = if let Ok(buf) = fs::read(backup_path) {
            match buf.len() {
                0x8000 => Some(BackupMedia::Sram(SRAM::from(buf))),
                0x200 | 0x2000 => Some(BackupMedia::Eeprom(EEPROM::from(buf))),
                0x10000 | 0x20000 => Some(BackupMedia::Flash(Flash::from(buf))),
                _ => None
            }
        }
        else {
            match rom.get_backup_type() {
                BackupType::Eeprom512B => Some(BackupMedia::Eeprom(EEPROM::new(1))),
                BackupType::Eeprom8KB => Some(BackupMedia::Eeprom(EEPROM::new(8))),
                BackupType::Flash64KB => Some(BackupMedia::Flash(Flash::new(64))),
                BackupType::Flash128KB => Some(BackupMedia::Flash(Flash::new(128))),
                BackupType::Sram32KB => Some(BackupMedia::Sram(SRAM::new())),
                BackupType::NoBackup => None
            }
        };

        let eeprom_mask = match backup {
            Some(BackupMedia::Eeprom(_)) => {
                Some(if rom.len() > 0x0100_0000 {
                    0x01ff_ff00
                } else {
                    0x0100_0000
                })
            }
            _ => None,
        };

        Ok(
            Self {
                rom,
                backup,
                eeprom_mask
            }
        )
    }

    fn eeprom_read(&self, addr: usize) -> bool {
        self.eeprom_mask.map_or(false, |mask| (addr & mask) == mask)
    }

    //TODO: ws behaviour
    pub fn read(&self, addr: u32) -> u8 {
        match addr {
            0x0800_0000..=0x09ff_ffff | 0x0a00_0000..=0x0bff_ffff | 0x0c00_0000..=0x0dff_ffff => {
                if self.eeprom_read(addr as usize) {
                    //EEPROM
                    0
                }
                else {
                    self.rom.data()[(addr & 0x01ff_ffff) as usize]
                }
            },
            0x0e00_0000..=0x0e00_ffff => {
                match &self.backup {
                    Some(BackupMedia::Sram(media)) => media.read_byte(addr & 0x7fff),
                    Some(BackupMedia::Flash(media)) => media.read_byte(addr & 0xffff),
                    None | _ => panic!("SRAM/FLASH Backup Media not found"),
                }
            },
            _ => panic!("Unreachable catridge memory write")
        }
    }

    pub fn write(&mut self, addr: u32, value: u8) {
        match addr {
            0x0800_0000..=0x09ff_ffff | 0x0a00_0000..=0x0bff_ffff | 0x0c00_0000..=0x0dff_ffff => {
                //EPROM
            },
            0x0e00_0000..=0x0e00_ffff => {
                match self.backup.as_mut() {
                    Some(BackupMedia::Sram(media)) => media.write_byte(addr & 0x7fff, value),
                    Some(BackupMedia::Flash(media)) => media.write_byte(addr & 0xffff, value),
                    None | _ => panic!("SRAM/FLASH Backup Media not found"),
                }
            },
            _ => panic!("Unreachable catridge memory write")
        }
    }
}
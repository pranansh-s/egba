pub mod eeprom;
pub mod flash;
pub mod sram;

use std::path::PathBuf;

use self::{eeprom::EEPROM, flash::Flash, sram::SRAM};

pub enum BackupMedia {
    Eeprom(EEPROM),
    Flash(Flash),
    Sram(SRAM),
}

#[derive(Clone, PartialEq)]
pub enum BackupType {
    NoBackup,
    Eeprom512B,
    Eeprom8KB,
    Flash64KB,
    Flash128KB,
    Sram32KB
}

pub trait BackupBuffer {
    fn init(size: usize) -> Box<[u8]> {
        vec![0; size * 1024].into_boxed_slice()
    }

    fn load(&mut self, path: &PathBuf) {}
    fn save(&self, path: &PathBuf) {}
}
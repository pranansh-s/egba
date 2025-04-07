pub mod eeprom;
pub mod flash;
pub mod sram;

use self::{eeprom::Eeprom, flash::Flash, sram::Sram};

pub enum BackupMedia {
    Eeprom(Eeprom),
    Flash(Flash),
    Sram(Sram),
}

pub enum BackupType {
    Eeprom512B,
    Eeprom8KB,
    Flash64KB,
    Flash128KB,
    Sram32KB
}
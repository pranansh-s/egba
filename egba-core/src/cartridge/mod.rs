pub mod backup;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    bus::Bus,
    rom::{InvalidROM, Rom},
};
use backup::{eeprom::EEPROM, flash::Flash, sram::SRAM, BackupBuffer, BackupMedia, BackupType};

#[derive(Clone, Copy)]
enum EepromRange {
    Full,
    Last256,
}

pub struct Cartridge {
    rom: Rom,
    backup: Option<BackupMedia>,
    eeprom_range: Option<EepromRange>,
    sav_path: PathBuf,
}

impl Cartridge {
    pub fn new(rom: Rom, backup_path: &Path) -> Result<Cartridge, InvalidROM> {
        if rom.len() > 0x2000000 {
            return Err(InvalidROM);
        }

        let backup = if let Ok(buf) = fs::read(backup_path) {
            match buf.len() {
                0x8000 => Some(BackupMedia::Sram(SRAM::from(buf))),
                0x200 | 0x2000 => Some(BackupMedia::Eeprom(EEPROM::from(buf))),
                0x10000 | 0x20000 => Some(BackupMedia::Flash(Flash::from(buf))),
                _ => None,
            }
        } else {
            match rom.get_backup_type() {
                BackupType::Eeprom512B => Some(BackupMedia::Eeprom(EEPROM::new(1))),
                BackupType::Eeprom8KB => Some(BackupMedia::Eeprom(EEPROM::new(8))),
                BackupType::Flash64KB => Some(BackupMedia::Flash(Flash::new(64))),
                BackupType::Flash128KB => Some(BackupMedia::Flash(Flash::new(128))),
                BackupType::Sram32KB => Some(BackupMedia::Sram(SRAM::new())),
                BackupType::NoBackup => None,
            }
        };

        let eeprom_range = match &backup {
            Some(BackupMedia::Eeprom(_)) => Some(if rom.len() > 0x0100_0000 {
                EepromRange::Last256
            } else {
                EepromRange::Full
            }),
            _ => None,
        };

        Ok(Self {
            rom,
            backup,
            eeprom_range,
            sav_path: backup_path.to_path_buf(),
        })
    }

    fn eeprom_read(&self, addr: usize) -> bool {
        match self.eeprom_range {
            Some(EepromRange::Full) => (0x0D00_0000..=0x0DFF_FFFF).contains(&addr),
            Some(EepromRange::Last256) => (0x0DFF_FF00..=0x0DFF_FFFF).contains(&addr),
            None => false,
        }
    }

    pub fn save(&self) {
        match &self.backup {
            Some(BackupMedia::Sram(m)) => m.save(&self.sav_path),
            Some(BackupMedia::Flash(m)) => m.save(&self.sav_path),
            Some(BackupMedia::Eeprom(m)) => m.save(&self.sav_path),
            None => {}
        }
    }
}

impl Bus for Cartridge {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x0800_0000..=0x0DFF_FFFF => {
                if self.eeprom_read(addr as usize) {
                    match &self.backup {
                        Some(BackupMedia::Eeprom(eeprom)) => eeprom.read_byte(addr),
                        _ => 0,
                    }
                } else {
                    let rom_addr = (addr & 0x01FF_FFFF) as usize;
                    if rom_addr < self.rom.len() {
                        self.rom.data()[rom_addr]
                    } else {
                        let halfword = (addr >> 1) as u16;
                        if addr & 1 == 0 {
                            halfword as u8
                        } else {
                            (halfword >> 8) as u8
                        }
                    }
                }
            }
            0x0E00_0000..=0x0E00_FFFF => match &self.backup {
                Some(BackupMedia::Sram(media)) => media.read_byte(addr & 0x7FFF),
                Some(BackupMedia::Flash(media)) => media.read_byte(addr & 0xFFFF),
                _ => 0xFF,
            },
            _ => 0xFF,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x0800_0000..=0x0DFF_FFFF if self.eeprom_read(addr as usize) => {
                if let Some(BackupMedia::Eeprom(eeprom)) = self.backup.as_mut() {
                    eeprom.write_byte(addr, value);
                }
            }
            0x0800_0000..=0x0DFF_FFFF => {}
            0x0E00_0000..=0x0E00_FFFF => match self.backup.as_mut() {
                Some(BackupMedia::Sram(media)) => media.write_byte(addr & 0x7FFF, value),
                Some(BackupMedia::Flash(media)) => media.write_byte(addr & 0xFFFF, value),
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_with_id(id: &[u8], offset: usize, total_size: usize) -> Rom {
        let mut buf = vec![0u8; total_size];
        buf[offset..offset + id.len()].copy_from_slice(id);
        Rom::new(&buf)
    }

    #[test]
    fn eeprom_range_full_for_small_carts() {
        let rom = rom_with_id(b"EEPROM_V100", 0xAC, 0x0080_0000);
        let cart = Cartridge::new(rom, Path::new("/nonexistent/no.sav")).expect("cart");
        assert!(cart.eeprom_read(0x0D00_0000), "EEPROM gate at 0x0D000000 start");
        assert!(cart.eeprom_read(0x0DAB_CDEF), "EEPROM gate mid-range");
        assert!(cart.eeprom_read(0x0DFF_FFFF), "EEPROM gate at top");
        assert!(
            !cart.eeprom_read(0x0900_0000),
            "0x09000000 must NOT be intercepted (regression vs old bit-24 mask)"
        );
        assert!(
            !cart.eeprom_read(0x0B00_0000),
            "0x0B000000 must NOT be intercepted"
        );
        assert!(
            !cart.eeprom_read(0x0CFF_FFFF),
            "addresses below 0x0D000000 must fall through to ROM"
        );
    }

    #[test]
    fn eeprom_range_last_256_for_large_carts() {
        let rom = rom_with_id(b"EEPROM_V100", 0xAC, 0x0180_0000);
        let cart = Cartridge::new(rom, Path::new("/nonexistent/no.sav")).expect("cart");
        assert!(!cart.eeprom_read(0x0D00_0000), ">16MB cart must NOT gate low EEPROM range");
        assert!(!cart.eeprom_read(0x0DFF_FEFF), "just below last-256 must not gate");
        assert!(cart.eeprom_read(0x0DFF_FF00), "last 256 bytes must gate (start)");
        assert!(cart.eeprom_read(0x0DFF_FFFF), "last 256 bytes must gate (end)");
    }

    #[test]
    fn no_eeprom_range_when_rom_lacks_id() {
        let rom = Rom::new(&vec![0u8; 0x1000]);
        let cart = Cartridge::new(rom, Path::new("/nonexistent/no.sav")).expect("cart");
        assert!(!cart.eeprom_read(0x0D00_0000));
        assert!(!cart.eeprom_read(0x0DFF_FFFF));
    }

    #[test]
    fn cart_oob_open_bus_address_halfword() {
        let rom = Rom::new(&vec![0u8; 0x1000]);
        let cart = Cartridge::new(rom, Path::new("/nonexistent/no.sav")).expect("cart");
        let cases: [(u32, u8, &str); 4] = [
            (0x0800_2000, 0x00, "even byte: low byte of (addr>>1)&0xFFFF = 0x1000 low = 0"),
            (0x0800_2001, 0x10, "odd byte: high byte of (addr>>1)&0xFFFF = 0x1000 high = 0x10"),
            (0x0800_2002, 0x01, "next even: (0x800_2002>>1)&0xFFFF = 0x1001 low"),
            (0x0800_2003, 0x10, "next odd: 0x1001 high"),
        ];
        for (addr, want, label) in cases {
            assert_eq!(cart.read_byte(addr), want, "{label}");
        }
    }
}

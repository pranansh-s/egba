use std::{fs, path::PathBuf};

use crate::bus::Bus;

use super::BackupBuffer;

pub struct SRAM(Box<[u8]>);

impl From<Vec<u8>> for SRAM {
    fn from(value: Vec<u8>) -> Self {
        Self(value.clone().into_boxed_slice())
    }
}

impl BackupBuffer for SRAM {
    fn load(&mut self, path: &PathBuf) {
        match fs::read(path) {
            Ok(buf) => self.0 = buf.into_boxed_slice(),
            Err(_) => panic!("Failed to load save data from: {:?}", path.file_name()),
        }
    }
    
    fn save(&self, path: &PathBuf) {
        if fs::write(path, self.0.clone()).is_err() {
            panic!("Failed to save data to: {:?}", path.file_name());
        }
    }
}

impl Bus for SRAM {
    fn read_byte(&self, addr: u32) -> u8 {
        self.0[addr as usize]
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        self.0[addr as usize] = val;
    }
}

impl SRAM {
    pub fn new() -> Self {
        Self(<Self as BackupBuffer>::init(32))
    }
}

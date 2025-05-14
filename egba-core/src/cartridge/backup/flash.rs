use crate::bus::Bus;

use super::BackupBuffer;

pub struct Flash {
    data: Box<[u8]>,
}

impl From<Vec<u8>> for Flash {
    fn from(value: Vec<u8>) -> Self {
        Self {
            data: value.clone().into_boxed_slice()
        }
    }
}

impl BackupBuffer for Flash {}


impl Bus for Flash {
    fn read_byte(&self, addr: u32) -> u8 {
        self.data[addr as usize]
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        self.data[addr as usize] = val;
    }
}

impl Flash {
    pub fn new(size: usize) -> Self {
        Self {
            data: <Self as BackupBuffer>::init(size)
        }
    }
}
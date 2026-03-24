use std::{cell::RefCell, fs, path::PathBuf};

use crate::bus::Bus;

use super::BackupBuffer;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EepromState {
    Ready,
    Command,
    AddressRead,
    AddressWrite,
    WriteData,
    WriteStop,
    ReadDummy,
    ReadData,
}

pub struct EEPROM {
    data: RefCell<Box<[u8]>>,
    size: usize,
    address_bits: usize,
    state: RefCell<EepromState>,
    buffer: RefCell<u64>,
    address: RefCell<usize>,
    bits_read: RefCell<usize>,
}

impl From<Vec<u8>> for EEPROM {
    fn from(value: Vec<u8>) -> Self {
        let size = value.len();
        let address_bits = if size <= 512 { 6 } else { 14 };
        Self {
            data: RefCell::new(value.clone().into_boxed_slice()),
            size,
            address_bits,
            state: RefCell::new(EepromState::Ready),
            buffer: RefCell::new(0),
            address: RefCell::new(0),
            bits_read: RefCell::new(0),
        }
    }
}

impl BackupBuffer for EEPROM {
    fn save(&self, path: &PathBuf) {
        if fs::write(path, self.data.borrow().clone()).is_err() {
            panic!("Failed to save data to: {:?}", path.file_name());
        }
    }
}

impl EEPROM {
    pub fn new(size: usize) -> Self {
        let mut data = <Self as BackupBuffer>::init(size).into_vec();
        if size == 1 {
            data.truncate(512);
        }
        let real_size = data.len();
        let address_bits = if real_size <= 512 { 6 } else { 14 };

        Self {
            data: RefCell::new(data.into_boxed_slice()),
            size: real_size,
            address_bits,
            state: RefCell::new(EepromState::Ready),
            buffer: RefCell::new(0),
            address: RefCell::new(0),
            bits_read: RefCell::new(0),
        }
    }
}

impl Bus for EEPROM {
    fn read_byte(&self, _addr: u32) -> u8 {
        let mut state = self.state.borrow_mut();
        let mut bits_read = self.bits_read.borrow_mut();
        let buffer = self.buffer.borrow();

        match *state {
            EepromState::ReadDummy => {
                *bits_read += 1;
                if *bits_read >= 4 {
                    *bits_read = 0;
                    *state = EepromState::ReadData;
                }
                0
            }
            EepromState::ReadData => {
                let bit = (*buffer >> (63 - *bits_read)) & 1;
                *bits_read += 1;
                if *bits_read >= 64 {
                    *bits_read = 0;
                    *state = EepromState::Ready;
                }
                bit as u8
            }
            _ => 1,
        }
    }

    fn write_byte(&mut self, _addr: u32, val: u8) {
        let bit = (val & 1) as u64;

        let mut state = self.state.borrow_mut();
        let mut bits_read = self.bits_read.borrow_mut();
        let mut buffer = self.buffer.borrow_mut();
        let mut address = self.address.borrow_mut();

        match *state {
            EepromState::Ready => {
                if bit == 1 {
                    *state = EepromState::Command;
                }
            }
            EepromState::Command => {
                if bit == 1 {
                    *state = EepromState::AddressRead;
                } else {
                    *state = EepromState::AddressWrite;
                }
                *bits_read = 0;
                *buffer = 0;
                *address = 0;
            }
            EepromState::AddressRead => {
                *address = (*address << 1) | (bit as usize);
                *bits_read += 1;
                if *bits_read == self.address_bits + 1 {
                    *address >>= 1;

                    let mut data_buf = 0u64;
                    let mut offset = (*address * 8);
                    if self.size > 0 {
                        offset %= self.size;
                    }

                    let data = self.data.borrow();
                    for i in 0..8 {
                        let b = if offset + i < self.size {
                            data[offset + i]
                        } else {
                            0xFF
                        };
                        data_buf = (data_buf << 8) | (b as u64);
                    }
                    *buffer = data_buf;
                    *bits_read = 0;
                    *state = EepromState::ReadDummy;
                }
            }
            EepromState::AddressWrite => {
                *address = (*address << 1) | (bit as usize);
                *bits_read += 1;
                if *bits_read == self.address_bits {
                    *buffer = 0;
                    *bits_read = 0;
                    *state = EepromState::WriteData;
                }
            }
            EepromState::WriteData => {
                *buffer = (*buffer << 1) | bit;
                *bits_read += 1;
                if *bits_read == 64 {
                    *bits_read = 0;
                    *state = EepromState::WriteStop;
                }
            }
            EepromState::WriteStop => {
                let mut offset = (*address * 8);
                if self.size > 0 {
                    offset %= self.size;
                }

                let mut data = self.data.borrow_mut();
                for i in 0..8 {
                    if offset + 7 - i < self.size {
                        data[offset + 7 - i] = ((*buffer >> (i * 8)) & 0xFF) as u8;
                    }
                }
                *state = EepromState::Ready;
            }
            _ => {
                *state = EepromState::Ready;
            }
        }
    }
}

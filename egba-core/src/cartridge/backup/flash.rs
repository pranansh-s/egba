use std::{fs, path::PathBuf};

use super::BackupBuffer;
use crate::bus::Bus;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlashState {
    Ready,
    Cmd1,
    Cmd2,
    EraseSetup,
    EraseCmd1,
    EraseCmd2,
    WriteSetup,
    BankSwitchSetup,
    IdMode,
}

pub struct Flash {
    data: Box<[u8]>,
    size: usize,
    state: FlashState,
    bank: usize,
    manufacturer_id: u8,
    device_id: u8,
}

impl Flash {
    pub fn new(size_kb: usize) -> Self {
        let size = size_kb * 1024;
        let (manuf, dev) = if size_kb == 64 {
            (0x32, 0x1B)
        } else {
            (0xC2, 0x09)
        };

        let mut data = <Self as BackupBuffer>::init(size);
        data.fill(0xFF);

        Self {
            data,
            size,
            state: FlashState::Ready,
            bank: 0,
            manufacturer_id: manuf,
            device_id: dev,
        }
    }
}

impl From<Vec<u8>> for Flash {
    fn from(value: Vec<u8>) -> Self {
        let size = value.len();
        let (manuf, dev) = if size == 65536 {
            (0x32, 0x1B)
        } else {
            (0xC2, 0x09)
        };
        Self {
            data: value.clone().into_boxed_slice(),
            size,
            state: FlashState::Ready,
            bank: 0,
            manufacturer_id: manuf,
            device_id: dev,
        }
    }
}

impl BackupBuffer for Flash {
    fn save(&self, path: &PathBuf) {
        if fs::write(path, self.data.clone()).is_err() {
            panic!("Failed to save data to: {:?}", path.file_name());
        }
    }
}

impl Bus for Flash {
    fn read_byte(&self, addr: u32) -> u8 {
        let offset = addr as usize & 0xFFFF;
        if self.state == FlashState::IdMode {
            if offset == 0 {
                return self.manufacturer_id;
            } else if offset == 1 {
                return self.device_id;
            }
        }

        let physical_addr = (self.bank * 0x10000) + offset;
        if physical_addr < self.size {
            self.data[physical_addr]
        } else {
            0xFF
        }
    }

    fn write_byte(&mut self, addr: u32, val: u8) {
        let offset = (addr as usize) & 0xFFFF;

        if val == 0xF0 {
            self.state = FlashState::Ready;
            return;
        }

        match self.state {
            FlashState::Ready => {
                if offset == 0x5555 && val == 0xAA {
                    self.state = FlashState::Cmd1;
                }
            }
            FlashState::Cmd1 => {
                if offset == 0x2AAA && val == 0x55 {
                    self.state = FlashState::Cmd2;
                } else {
                    self.state = FlashState::Ready;
                }
            }
            FlashState::Cmd2 => {
                if offset == 0x5555 {
                    match val {
                        0x90 => self.state = FlashState::IdMode,
                        0x80 => self.state = FlashState::EraseSetup,
                        0xA0 => self.state = FlashState::WriteSetup,
                        0xB0 => self.state = FlashState::BankSwitchSetup,
                        _ => self.state = FlashState::Ready,
                    }
                } else {
                    self.state = FlashState::Ready;
                }
            }
            FlashState::EraseSetup => {
                if offset == 0x5555 && val == 0xAA {
                    self.state = FlashState::EraseCmd1;
                } else {
                    self.state = FlashState::Ready;
                }
            }
            FlashState::EraseCmd1 => {
                if offset == 0x2AAA && val == 0x55 {
                    self.state = FlashState::EraseCmd2;
                } else {
                    self.state = FlashState::Ready;
                }
            }
            FlashState::EraseCmd2 => {
                if offset == 0x5555 && val == 0x10 {
                    self.data.fill(0xFF);
                } else if val == 0x30 {
                    let sector_base = (self.bank * 0x10000) + (offset & 0xF000);
                    if sector_base + 0x1000 <= self.size {
                        self.data[sector_base..sector_base + 0x1000].fill(0xFF);
                    }
                }
                self.state = FlashState::Ready;
            }
            FlashState::WriteSetup => {
                let physical_addr = (self.bank * 0x10000) + offset;
                if physical_addr < self.size {
                    self.data[physical_addr] = val;
                }
                self.state = FlashState::Ready;
            }
            FlashState::BankSwitchSetup => {
                if offset == 0x0000 {
                    self.bank = (val & 1) as usize;
                }
                self.state = FlashState::Ready;
            }
            FlashState::IdMode => {
                if offset == 0x5555 && val == 0xAA {
                    self.state = FlashState::Cmd1;
                }
            }
        }
    }
}

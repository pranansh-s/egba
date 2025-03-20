use std::{error::Error, fmt};

use crate::bus::Bus;

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
}

impl Bus for Rom {
    fn read_byte(&self, addr: u32) -> u8 {
        self.0[addr as usize]
    }
}
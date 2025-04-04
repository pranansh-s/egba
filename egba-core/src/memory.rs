use std::thread::sleep;

use crate::{bios::Bios, bus::Bus, cartridge::Cartridge};

pub struct Memory {
    pub(crate) bios: Bios,
    pub(crate) ewram: Box<[u8]>,
    pub(crate) iwram: Box<[u8]>,

    pub(crate) io: Box<[u8]>,

    pub(crate) pram: Box<[u8]>,
    pub(crate) vram: Box<[u8]>,
    pub(crate) oam: Box<[u8]>,

    pub(crate) cartridge: Cartridge,
}

impl Memory {
    pub fn new(bios: Bios, cartridge: Cartridge) -> Self {
        Self {
            bios,
            ewram: vec![0; 0x40000].into_boxed_slice(),
            iwram: vec![0; 0x8000].into_boxed_slice(),

            io: vec![0; 0x400].into_boxed_slice(),

            pram: vec![0; 0x400].into_boxed_slice(),
            vram: vec![0; 0x18000].into_boxed_slice(),
            oam: vec![0; 0x400].into_boxed_slice(),

            cartridge,
        }
    }
}

impl Bus for Memory {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x0000_0000..=0x0000_3fff => self.bios.read(addr),
            0x0200_0000..=0x0203_ffff => self.ewram.read_byte(addr & 0x3_ffff),
            0x0300_0000..=0x0300_7fff => self.iwram.read_byte(addr & 0x7fff),

            0x0400_0000..=0x0400_03fe => self.io.read_byte(addr & 0x3ff),

            0x0500_0000..=0x0500_03ff => self.pram.read_byte(addr & 0x3ff),
            0x0600_0000..=0x0601_7fff => self.vram.read_byte(addr & 0x1_ffff),
            0x0700_0000..=0x0700_03ff => self.oam.read_byte(addr & 0x3ff),

            0x0800_0000..=0x0fff_ffff => self.cartridge.read(addr & 0xfff_ffff),
            _x => {
                eprintln!("Unreachable mem read: {:08x}", _x);
                69
            }
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x0200_0000..=0x02ff_ffff => self.ewram.write_byte(addr & 0x3_ffff, value),
            0x0300_0000..=0x03ff_ffff => self.iwram.write_byte(addr & 0x7fff, value),

            0x0400_0000..=0x0400_03fe => self.io.write_byte(addr & 0x3ff, value),

            0x0800_0000..=0x0fff_ffff => self.cartridge.write(addr & 0xfff_ffff, value),
            _x => { 
                eprintln!("Unreachable mem write: {:08x}", _x);
            }
        }
    }

    fn write_hword(&mut self, addr: u32, value: u16) {
        match addr {
            0x0200_0000..=0x02ff_ffff => self.ewram.write_hword(addr & 0x3_ffff, value),
            0x0300_0000..=0x03ff_ffff => self.iwram.write_hword(addr & 0x7fff, value),

            0x0400_0000..=0x0400_03fe => self.io.write_hword(addr & 0x3ff, value),

            0x0500_0000..=0x0500_03ff => self.pram.write_hword(addr & 0x3ff, value),
            0x0600_0000..=0x0601_7fff => self.vram.write_hword(addr & 0x1_ffff, value),
            0x0700_0000..=0x0700_03ff => self.oam.write_hword(addr & 0x3ff, value),

            _x => { 
                eprintln!("Unreachable mem write: {:08x}", _x);
            }
        }
    }
}
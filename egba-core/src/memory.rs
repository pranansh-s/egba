use crate::{bios::Bios, bus::Bus, cartridge::Cartridge};

pub struct Memory {
    pub(crate) bios: Bios,
    pub(crate) ewram: Box<[u8]>,
    pub(crate) iwram: Box<[u8]>,
    pub(crate) io: Box<[u8]>,
    pub(crate) cartridge: Cartridge,
}

impl Memory {
    pub fn new(bios: Bios, cartridge: Cartridge) -> Self {
        Self {
            bios,
            ewram: vec![0; 0x40000].into_boxed_slice(),
            iwram: vec![0; 0x8000].into_boxed_slice(),
            io: vec![0; 0x4000].into_boxed_slice(),
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
            0x0400_0000..=0x0400_03fe => self.io.read_byte(addr & 0x3fe),
            // 0x0800_0000..=0x0fff_ffff => self.cartridge.read(addr & 0xfff_ffff),
            _ => panic!("Unreachable memory read: {:08x}", addr)
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x0200_0000..=0x0203_ffff => self.ewram.write_byte(addr & 0x3_ffff, value),
            0x0300_0000..=0x0300_7fff => self.iwram.write_byte(addr & 0x7fff, value),
            0x0400_0000..=0x0400_03fe => self.io.write_byte(addr & 0x3fe, value),
            // 0x0800_0000..=0x0fff_ffff => self.cartridge.read(addr & 0xfff_ffff, value),
            _ => panic!("Unreachable memory write: {:08x}", addr)
        }
    }
}
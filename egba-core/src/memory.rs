use crate::{
    bios::Bios,
    bus::Bus,
    cartridge::Cartridge,
    control::{InterruptControl, SystemControl},
    keypad::Keypad,
    video::Video,
};

pub struct Memory {
    pub(crate) bios: Bios,
    pub(crate) ewram: Box<[u8]>,
    pub(crate) iwram: Box<[u8]>,

    pub(crate) interrupt: InterruptControl,
    pub(crate) system: SystemControl,
    pub(crate) keypad: Keypad,

    pub(crate) video: Video,

    pub(crate) cartridge: Cartridge,
}

impl Memory {
    #[must_use]
    pub(crate) fn new(bios: Bios, cartridge: Cartridge) -> Self {
        Self {
            bios,
            ewram: vec![0; 0x40000].into_boxed_slice(),
            iwram: vec![0; 0x8000].into_boxed_slice(),
            interrupt: InterruptControl::default(),
            system: SystemControl::default(),
            keypad: Keypad::default(),
            video: Video::new(),
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

            0x0400_0000..=0x0400_03fe => {
                let offset = addr & 0x3ff;
                match offset {
                    0x130..0x134 => self.keypad.read_byte(offset),
                    0x200..0x204 | 0x208..0x20A => self.interrupt.read_byte(offset),
                    0x204..0x206 => self.system.read_byte(offset),
                    _x => {
                        eprintln!("Unknown IO read: {:03x}", _x);
                        0x69
                    }
                }
            }

            // 0x0500_0000..=0x0500_03ff => self.pram.read_byte(addr & 0x3ff),
            // 0x0600_0000..=0x0601_7fff => self.vram.read_byte(addr & 0x1_ffff),
            // 0x0700_0000..=0x0700_03ff => self.oam.read_byte(addr & 0x3ff),
            0x0800_0000..=0x0fff_ffff => self.cartridge.read_byte(addr & 0xfff_ffff),
            _x => {
                eprintln!("Unreachable mem read: {:08x}", _x);
                0x69
            }
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x0200_0000..=0x0203_ffff => self.ewram.write_byte(addr & 0x3_ffff, value),
            0x0300_0000..=0x0300_7fff => self.iwram.write_byte(addr & 0x7fff, value),

            0x0400_0000..=0x0400_03fe => {
                let offset = addr & 0x3ff;
                match offset {
                    0x130..0x134 => self.keypad.write_byte(offset, value),
                    0x200..0x204 | 0x208..0x20A => self.interrupt.write_byte(offset, value),
                    0x204..0x206 | 0x301 => self.system.write_byte(offset, value),
                    _x => {
                        eprintln!("Unknown IO write: {:03x} = {:02x}", _x, value);
                    }
                }
            }

            // 0x0500_0000..=0x0500_03ff => self.pram.write_byte(addr & 0x3ff, value),
            // 0x0600_0000..=0x0601_7fff => self.vram.write_byte(addr & 0x1_ffff, value),
            // 0x0700_0000..=0x0700_03ff => self.oam.write_byte(addr & 0x3ff, value),
            0x0800_0000..=0x0fff_ffff => self.cartridge.write_byte(addr & 0xfff_ffff, value),
            _x => {
                eprintln!("Unreachable mem write: {:08x}", _x);
            }
        }
    }
}

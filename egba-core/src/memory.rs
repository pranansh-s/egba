use crate::{
    bios::Bios,
    bus::Bus,
    cartridge::Cartridge,
    control::{InterruptControl, SystemControl},
    dma::{Dma, DmaMemory},
    keypad::Keypad,
    timer::Timers,
    video::Video,
};

pub(crate) struct Memory {
    pub(crate) bios: Bios,
    pub(crate) ewram: Box<[u8]>,
    pub(crate) iwram: Box<[u8]>,

    pub(crate) interrupt: InterruptControl,
    pub(crate) system: SystemControl,
    pub(crate) keypad: Keypad,

    pub(crate) video: Video,
    pub(crate) timers: Timers,
    pub(crate) dma: Dma,

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
            timers: Timers::default(),
            dma: Dma::default(),
            cartridge,
        }
    }
}

impl Bus for Memory {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x0000_0000..=0x0000_3FFF => self.bios.read(addr),
            0x0200_0000..=0x0203_FFFF => self.ewram.read_byte(addr & 0x3_FFFF),
            0x0300_0000..=0x0300_7FFF => self.iwram.read_byte(addr & 0x7FFF),
            0x0400_0000..=0x0400_03FE => {
                let offset = addr & 0x3FF;
                match offset {
                    0x000..=0x056 => self.video.read_byte(offset),
                    0x0B0..=0x0DE => self.dma.read_byte(offset),
                    0x100..=0x10F => self.timers.read_byte(offset),
                    0x130..=0x133 => self.keypad.read_byte(offset),
                    0x200..=0x203 | 0x208..=0x209 => self.interrupt.read_byte(offset),
                    0x204..=0x205 => self.system.read_byte(offset),
                    _x => 0,
                }
            }
            0x0500_0000..=0x0500_03FF => self.video.palette[(addr & 0x3FF) as usize],
            0x0500_0400..=0x05FF_FFFF => self.video.palette[(addr & 0x3FF) as usize],
            0x0600_0000..=0x0601_7FFF => self.video.vram[(addr & 0x1_7FFF) as usize],
            0x0601_8000..=0x06FF_FFFF => {
                let mirror = addr & 0x1_FFFF;
                let effective = if mirror >= 0x1_8000 {
                    mirror - 0x8000
                } else {
                    mirror
                };
                self.video.vram[effective as usize]
            }
            0x0700_0000..=0x0700_03FF => self.video.oam[(addr & 0x3FF) as usize],
            0x0700_0400..=0x07FF_FFFF => self.video.oam[(addr & 0x3FF) as usize],
            0x0800_0000..=0x0FFF_FFFF => self.cartridge.read_byte(addr),

            _x => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x0200_0000..=0x0203_FFFF => self.ewram.write_byte(addr & 0x3_FFFF, value),
            0x0300_0000..=0x0300_7FFF => self.iwram.write_byte(addr & 0x7FFF, value),

            0x0400_0000..=0x0400_03FE => {
                let offset = addr & 0x3FF;
                match offset {
                    0x000..=0x056 => self.video.write_byte(offset, value),
                    0x0B0..=0x0DE => self.dma.write_byte(offset, value),
                    0x100..=0x10F => self.timers.write_byte(offset, value),
                    0x130..=0x133 => self.keypad.write_byte(offset, value),
                    0x200..=0x203 | 0x208..=0x209 => self.interrupt.write_byte(offset, value),
                    0x204..=0x205 | 0x301 => self.system.write_byte(offset, value),
                    _x => {}
                }
            }
            0x0500_0000..=0x05FF_FFFF => {
                let pal_addr = (addr & 0x3FE) as usize;
                self.video.palette[pal_addr] = value;
                self.video.palette[pal_addr + 1] = value;
            }

            0x0600_0000..=0x06FF_FFFF => {
                let mirror = addr & 0x1_FFFF;
                let effective = if mirror >= 0x1_8000 {
                    mirror - 0x8000
                } else {
                    mirror
                };
                let bg_mode = self.video.read_byte(0x000) & 0x7;
                if bg_mode >= 3 {
                    let aligned = (effective & !1) as usize;
                    if aligned + 1 < self.video.vram.len() {
                        self.video.vram[aligned] = value;
                        self.video.vram[aligned + 1] = value;
                    }
                }
            }
            0x0700_0000..=0x07FF_FFFF => {}
            0x0800_0000..=0x0FFF_FFFF => self.cartridge.write_byte(addr, value),
            _x => {}
        }
    }

    fn write_hword(&mut self, addr: u32, value: u16) {
        match addr {
            0x0500_0000..=0x05FF_FFFF => {
                let pal_addr = (addr & 0x3FE) as usize;
                self.video.palette[pal_addr] = value as u8;
                self.video.palette[pal_addr + 1] = (value >> 8) as u8;
            }
            0x0600_0000..=0x06FF_FFFF => {
                let mirror = addr & 0x1_FFFF;
                let effective = if mirror >= 0x1_8000 {
                    mirror - 0x8000
                } else {
                    mirror
                };
                let e = effective as usize;
                if e + 1 < self.video.vram.len() {
                    self.video.vram[e] = value as u8;
                    self.video.vram[e + 1] = (value >> 8) as u8;
                }
            }
            0x0700_0000..=0x07FF_FFFF => {
                let oam_addr = (addr & 0x3FE) as usize;
                self.video.oam[oam_addr] = value as u8;
                self.video.oam[oam_addr + 1] = (value >> 8) as u8;
            }
            _ => {
                let addr = addr & !0b1;
                self.write_byte(addr, value as u8);
                self.write_byte(addr.wrapping_add(1), (value >> 8) as u8);
            }
        }
    }

    fn write_word(&mut self, addr: u32, value: u32) {
        let addr = addr & !0b11;
        self.write_hword(addr, value as u16);
        self.write_hword(addr.wrapping_add(2), (value >> 16) as u16);
    }
}

impl DmaMemory for Memory {
    fn dma_read_hword(&self, addr: u32) -> u16 {
        self.read_hword(addr)
    }

    fn dma_read_word(&self, addr: u32) -> u32 {
        self.read_word(addr)
    }

    fn dma_write_hword(&mut self, addr: u32, val: u16) {
        self.write_hword(addr, val);
    }

    fn dma_write_word(&mut self, addr: u32, val: u32) {
        self.write_word(addr, val);
    }
}

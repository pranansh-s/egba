use bit::BitIndex;

use crate::{
    bus::Bus,
    cpu::{cpu::CPU, exception::Exception, psr::OperatingState},
};

#[derive(Default)]
pub struct InterruptControl {
    master: bool,
    enable: u16,
    request: u16,
}

#[derive(Clone, Copy)]
pub enum InterruptType {
    VBlank = 0,
    HBlank = 1,
    VCounter = 2,
    Timer0 = 3,
    Timer1 = 4,
    Timer2 = 5,
    Timer3 = 6,
    Serial = 7,
    DMA0 = 8,
    DMA1 = 9,
    DMA2 = 10,
    DMA3 = 11,
    Keypad = 12,
    Cartridge = 13,
}

impl Bus for InterruptControl {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x208 => self.master as u8,
            0x200 => self.enable as u8,
            0x201 => (self.enable >> 8) as u8,
            0x202 => self.request as u8,
            0x203 => (self.request >> 8) as u8,
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x208 => self.master = value & 1 != 0,
            0x200 => {
                self.enable.set_bit_range(0..8, value as u16);
            }
            0x201 => {
                self.enable.set_bit_range(8..16, value as u16);
            }
            0x202 => {
                self.request
                    .set_bit_range(0..8, self.request & !(value as u16));
            }
            0x203 => {
                self.request
                    .set_bit_range(8..16, self.request & !(value as u16));
            }
            _ => {}
        }
    }
}

impl InterruptControl {
    pub fn step(&mut self, cpu: &mut CPU, system: &mut SystemControl) {
        if (self.enable & self.request) == 0 {
            return;
        }

        system.update_power(PowerMode::Active);
        if self.master {
            let addr = match cpu.cpsr.operating_state {
                OperatingState::ARM => cpu.arm_pc(),
                OperatingState::THUMB => cpu.thumb_pc(),
            };
            cpu.enter_exception(Exception::IRQ, addr.wrapping_add(4));
        }
    }

    pub fn request(&mut self, interrupt: InterruptType) {
        self.request |= 1 << interrupt as usize;
    }
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum PowerMode {
    #[default]
    Active,
    Halt,
    Stop,
}

#[derive(Default)]
pub struct SystemControl {
    waintcnt: u16,
    power: PowerMode,
}

impl SystemControl {
    pub fn step(&mut self) {
        //TODO: actual cycle counting with ws and prefetch behavior
    }

    pub fn update_power(&mut self, power: PowerMode) {
        self.power = power;
    }

    pub fn get_power_mode(&self) -> PowerMode {
        self.power
    }
}

impl Bus for SystemControl {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x204 => self.waintcnt as u8,
            0x205 => (self.waintcnt >> 8) as u8,
            _ => 0x69,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x204 => {
                self.waintcnt.set_bit_range(0..8, value as u16);
            }
            0x205 => {
                self.waintcnt.set_bit_range(8..16, value as u16);
            }
            0x301 => {
                self.power = match value.bit(7) {
                    false => PowerMode::Halt,
                    true => PowerMode::Stop,
                };
            }
            _ => {}
        }
    }
}

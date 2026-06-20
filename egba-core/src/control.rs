use bit::BitIndex;

use crate::{
    bus::Bus,
    cpu::{cpu::CPU, exception::Exception, psr::OperatingState},
};

#[derive(Default)]
pub(crate) struct InterruptControl {
    master: bool,
    enable: u16,
    request: u16,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) enum InterruptType {
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
                self.request &= !(value as u16);
            }
            0x203 => {
                self.request &= !((value as u16) << 8);
            }
            _ => {}
        }
    }
}

impl InterruptControl {
    pub(crate) fn step(&mut self, cpu: &mut CPU, system: &mut SystemControl) -> bool {
        if (self.enable & self.request) == 0 {
            return false;
        }

        system.update_power(PowerMode::Active);
        if self.master {
            let return_addr = match cpu.cpsr.operating_state {
                OperatingState::ARM => cpu.arm_pc().wrapping_add(4),
                OperatingState::THUMB => cpu.thumb_pc().wrapping_add(4),
            };
            return cpu.setup_exception(Exception::IRQ, return_addr);
        }
        false
    }

    pub(crate) fn request(&mut self, interrupt: InterruptType) {
        self.request |= 1 << interrupt as usize;
    }
}

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub(crate) enum PowerMode {
    #[default]
    Active,
    Halt,
    Stop,
}

pub(crate) struct SystemControl {
    waitcnt: u16,
    power: PowerMode,
    ws_n: [u32; 3],
    ws_s: [u32; 3],
    sram_n: u32,
}

impl Default for SystemControl {
    fn default() -> Self {
        let mut s = Self {
            waitcnt: 0,
            power: PowerMode::default(),
            ws_n: [0; 3],
            ws_s: [0; 3],
            sram_n: 0,
        };
        s.recompute_waitcnt();
        s
    }
}

impl SystemControl {
    pub(crate) fn step(&mut self) {}

    pub(crate) fn update_power(&mut self, power: PowerMode) {
        self.power = power;
    }

    pub(crate) fn get_power_mode(&self) -> PowerMode {
        self.power
    }

    fn ws_n_lut(idx: u32) -> u32 {
        match idx {
            0 => 4,
            1 => 3,
            2 => 2,
            _ => 8,
        }
    }

    fn recompute_waitcnt(&mut self) {
        let w = self.waitcnt;
        self.sram_n = Self::ws_n_lut((w & 0b11) as u32);
        self.ws_n[0] = Self::ws_n_lut(((w >> 2) & 0b11) as u32);
        self.ws_s[0] = if (w >> 4) & 1 != 0 { 1 } else { 2 };
        self.ws_n[1] = Self::ws_n_lut(((w >> 5) & 0b11) as u32);
        self.ws_s[1] = if (w >> 7) & 1 != 0 { 1 } else { 4 };
        self.ws_n[2] = Self::ws_n_lut(((w >> 8) & 0b11) as u32);
        self.ws_s[2] = if (w >> 10) & 1 != 0 { 1 } else { 8 };
    }

    #[inline]
    fn rom_bank(addr: u32) -> Option<usize> {
        match (addr >> 24) & 0xF {
            0x8 | 0x9 => Some(0),
            0xA | 0xB => Some(1),
            0xC | 0xD => Some(2),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn rom_access_cycles(&self, addr: u32, width: u32) -> u32 {
        let bank = Self::rom_bank(addr).unwrap_or(0);
        let n = self.ws_n[bank];
        let s = self.ws_s[bank];
        if width >= 4 {
            (1 + n) + (1 + s)
        } else {
            1 + n
        }
    }

    #[inline]
    pub(crate) fn rom_seq_cycles(&self, addr: u32, width: u32) -> u32 {
        let bank = Self::rom_bank(addr).unwrap_or(0);
        let s = self.ws_s[bank];
        if width >= 4 {
            2 * (1 + s)
        } else {
            1 + s
        }
    }

    #[inline]
    pub(crate) fn sram_access_cycles(&self) -> u32 {
        1 + self.sram_n
    }
}

impl Bus for SystemControl {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x204 => self.waitcnt as u8,
            0x205 => (self.waitcnt >> 8) as u8,
            _ => 0x69,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x204 => {
                self.waitcnt.set_bit_range(0..8, value as u16);
                self.recompute_waitcnt();
            }
            0x205 => {
                self.waitcnt.set_bit_range(8..16, value as u16);
                self.recompute_waitcnt();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::cpu::{LR_INDEX, PC_INDEX};
    use crate::cpu::psr::OperatingMode;

    #[test]
    fn irq_arm_lr_is_next_instr_plus_4() {
        let mut intr = InterruptControl::default();
        let mut sys = SystemControl::default();
        let mut cpu = CPU::new();
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.cpsr.irq_disable_bit = false;
        let x = 0x200u32;
        cpu.reg[PC_INDEX] = x + 12;

        intr.master = true;
        intr.enable = 1;
        intr.request = 1;
        assert!(intr.step(&mut cpu, &mut sys));

        assert_eq!(cpu.reg[LR_INDEX], x + 8);
    }

    #[test]
    fn irq_thumb_lr_is_next_instr_plus_4() {
        let mut intr = InterruptControl::default();
        let mut sys = SystemControl::default();
        let mut cpu = CPU::new();
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.cpsr.operating_state = OperatingState::THUMB;
        cpu.cpsr.irq_disable_bit = false;
        let x = 0x200u32;
        cpu.reg[PC_INDEX] = x + 6;

        intr.master = true;
        intr.enable = 1;
        intr.request = 1;
        assert!(intr.step(&mut cpu, &mut sys));

        assert_eq!(cpu.reg[LR_INDEX], x + 6);
    }

    #[test]
    fn irq_masked_does_not_enter_handler_but_wakes_halt() {
        let mut intr = InterruptControl::default();
        let mut sys = SystemControl::default();
        let mut cpu = CPU::new();
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.cpsr.irq_disable_bit = true;
        intr.master = true;
        intr.enable = 1;
        intr.request = 1;
        sys.update_power(PowerMode::Halt);

        let accepted = intr.step(&mut cpu, &mut sys);

        assert!(!accepted, "IRQ must not be accepted when CPSR.I=1");
        assert_eq!(
            sys.get_power_mode(),
            PowerMode::Active,
            "HALT must wake on (IE & IF) regardless of CPSR.I"
        );
    }

    #[test]
    fn irq_master_off_does_not_enter_handler_but_wakes_halt() {
        let mut intr = InterruptControl::default();
        let mut sys = SystemControl::default();
        let mut cpu = CPU::new();
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.cpsr.irq_disable_bit = false;
        intr.master = false;
        intr.enable = 1;
        intr.request = 1;
        sys.update_power(PowerMode::Halt);

        let accepted = intr.step(&mut cpu, &mut sys);

        assert!(!accepted, "IME=0 -> no handler entry");
        assert_eq!(
            sys.get_power_mode(),
            PowerMode::Active,
            "HALT wakes on (IE & IF) even with IME=0"
        );
    }
}

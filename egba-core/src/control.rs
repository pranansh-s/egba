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

#[derive(Default)]
pub(crate) struct SystemControl {
    waitcnt: u16,
    power: PowerMode,
}

impl SystemControl {
    pub(crate) fn step(&mut self) {}

    pub(crate) fn update_power(&mut self, power: PowerMode) {
        self.power = power;
    }

    pub(crate) fn get_power_mode(&self) -> PowerMode {
        self.power
    }

    fn sram_wait(&self) -> u32 {
        Self::ws_n(self.waitcnt.bit_range(0..2) as u32)
    }

    fn ws0_n(&self) -> u32 {
        Self::ws_n(self.waitcnt.bit_range(2..4) as u32)
    }

    fn ws0_s(&self) -> u32 {
        if self.waitcnt.bit(4) {
            1
        } else {
            2
        }
    }

    fn ws1_n(&self) -> u32 {
        Self::ws_n(self.waitcnt.bit_range(5..7) as u32)
    }

    fn ws1_s(&self) -> u32 {
        if self.waitcnt.bit(7) {
            1
        } else {
            4
        }
    }

    fn ws2_n(&self) -> u32 {
        Self::ws_n(self.waitcnt.bit_range(8..10) as u32)
    }

    fn ws2_s(&self) -> u32 {
        if self.waitcnt.bit(10) {
            1
        } else {
            8
        }
    }

    fn ws_n(idx: u32) -> u32 {
        match idx {
            0 => 4,
            1 => 3,
            2 => 2,
            _ => 8,
        }
    }

    pub(crate) fn rom_access_cycles(&self, addr: u32, width: u32) -> u32 {
        let (n, s) = self.rom_ws(addr);
        if width >= 4 {
            (1 + n) + (1 + s)
        } else {
            1 + n
        }
    }

    pub(crate) fn rom_seq_cycles(&self, addr: u32, width: u32) -> u32 {
        let (_, s) = self.rom_ws(addr);
        if width >= 4 {
            2 * (1 + s)
        } else {
            1 + s
        }
    }

    fn rom_ws(&self, addr: u32) -> (u32, u32) {
        let region = (addr >> 24) & 0xF;
        match region {
            0x08 | 0x09 => (self.ws0_n(), self.ws0_s()),
            0x0A | 0x0B => (self.ws1_n(), self.ws1_s()),
            0x0C | 0x0D => (self.ws2_n(), self.ws2_s()),
            _ => (0, 0),
        }
    }

    pub(crate) fn sram_access_cycles(&self) -> u32 {
        1 + self.sram_wait()
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
            }
            0x205 => {
                self.waitcnt.set_bit_range(8..16, value as u16);
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
        // Call site is post-CPU::step: after executing instr@X, reg[PC] = X+12
        // (one fetch beyond the next instruction). arm_pc() therefore returns X+4,
        // the address of the instruction that would have executed next.
        let x = 0x200u32;
        cpu.reg[PC_INDEX] = x + 12;

        intr.master = true;
        intr.enable = 1;
        intr.request = 1;
        assert!(intr.step(&mut cpu, &mut sys));

        // LR_irq = next_instr + 4 = (X+4) + 4 = X+8 so SUBS PC,LR,#4 -> X+4.
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
        // Post-CPU::step THUMB: reg[PC] = X+6 (one fetch beyond next). thumb_pc() = X+2.
        let x = 0x200u32;
        cpu.reg[PC_INDEX] = x + 6;

        intr.master = true;
        intr.enable = 1;
        intr.request = 1;
        assert!(intr.step(&mut cpu, &mut sys));

        // THUMB LR_irq = next_instr + 4 = (X+2) + 4 = X+6.
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

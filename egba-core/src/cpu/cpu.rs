use bit::BitIndex;

use crate::bus::Bus;
use crate::cpu::psr::{OperatingMode, OperatingState, ProgramStatusRegister};

use super::bit_r;

pub const PC_INDEX: usize = 15;
pub const LR_INDEX: usize = 14;
pub const SP_INDEX: usize = 13;

#[derive(Clone, Copy)]
pub struct BankedRegisters {
    pub sp: u32,
    pub lr: u32,
    pub spsr: u32
}

impl BankedRegisters {
    pub fn new() -> Self {
        Self {
            sp: 0,
            lr: 0,
            spsr: 0,
        }
    }
}

pub struct CPU {
    pub reg: [u32; 16],
    pub fiq_r8_12_banked: [u32; 5],
    pub banks: [BankedRegisters; 6],
    pub cpsr: ProgramStatusRegister,
    pub spsr: u32,
    pub pipeline: [u32; 3],
}

impl CPU {
    #[must_use]
    pub fn new() -> Self {
        Self {
            reg: [0; 16],
            fiq_r8_12_banked: [0; 5],
            banks: [BankedRegisters::new(); 6],
            cpsr: ProgramStatusRegister::new(),
            spsr: 0,
            pipeline: [0, 0, 0]
        }
    }

    pub fn restore_spsr(&mut self) {
        let cpsr: ProgramStatusRegister = self.spsr.into();
        self.set_bank(cpsr.mode);
        self.cpsr = cpsr;
    }

    pub fn set_mode(&mut self, mode: OperatingMode) {
        self.set_bank(mode);
        self.cpsr.mode = mode;
    }

    fn set_bank(&mut self, mode: OperatingMode) {
        let old_bank_index = self.cpsr.mode.current_bank_index();
        let new_bank_index = mode.current_bank_index();

        if old_bank_index == new_bank_index {
            return;
        }

        self.banks[old_bank_index].sp = self.reg[SP_INDEX];
        self.banks[old_bank_index].lr = self.reg[LR_INDEX];
        self.banks[old_bank_index].spsr = self.spsr;
        
        self.reg[SP_INDEX] = self.banks[new_bank_index].sp;
        self.reg[LR_INDEX] = self.banks[new_bank_index].lr;
        self.spsr = self.banks[new_bank_index].spsr;

        if self.cpsr.mode == OperatingMode::fiq || mode == OperatingMode::fiq {
            self.fiq_r8_12_banked.swap_with_slice(&mut self.reg[8..=12]);
        }
    }

    pub fn fetch(&mut self, bus: &mut impl Bus) -> u32 {
        let addr = self.reg[PC_INDEX];
        let instr;
        match self.cpsr.operating_state {
            OperatingState::ARM => {
                instr = bus.read_word(addr);
                self.reg[PC_INDEX] = self.reg[PC_INDEX].wrapping_add(4);
            },
            OperatingState::THUMB => {
                instr = bus.read_hword(addr) as u32;
                self.reg[PC_INDEX] = self.reg[PC_INDEX].wrapping_add(2);
            }
        }
        instr
    }

    fn execute(&mut self, bus: &mut impl Bus, instr: u32) {
        match self.cpsr.operating_state {
            OperatingState::ARM => {
                self.arm_opcodes(bus, instr);
            },
            OperatingState::THUMB => {
                self.thumb_opcodes(bus, bit_r!(instr, 0..16) as u16);
            }
        }
    }

    pub fn step(&mut self, bus: &mut impl Bus) {
        self.pipeline[0] = self.pipeline[1];
        self.pipeline[1] = self.pipeline[2];
        
        self.execute(bus, self.pipeline[0]);

        self.pipeline[2] = self.fetch(bus);
    }

    pub fn flush_pipeline(&mut self, bus: &mut impl Bus) {
        self.reg[PC_INDEX] &= match self.cpsr.operating_state {
            OperatingState::ARM => !0b11,
            OperatingState::THUMB => !0b1,
        };

        self.pipeline[1] = self.fetch(bus);
    }
}

use bit::BitIndex;

use crate::bus::Bus;
use crate::cpu::psr::{OperatingMode, OperatingState, ProgramStatusRegister};

use super::bit_r;

pub const PC_INDEX: usize = 15;
pub const LR_INDEX: usize = 14;
pub const SP_INDEX: usize = 13;

#[derive(Clone, Copy, Default)]
pub struct BankedRegisters {
    pub(crate) sp: u32,
    pub(crate) lr: u32,
    pub(crate) spsr: u32,
}

pub struct CPU {
    pub reg: [u32; 16],
    pub(crate) fiq_r8_12_banked: [u32; 5],
    pub(crate) banks: [BankedRegisters; 6],
    pub cpsr: ProgramStatusRegister,
    pub(crate) spsr: u32,
    pub pipeline: [u32; 3],
    pub(crate) pipeline_dirty: bool,
}

impl CPU {
    #[must_use]
    pub fn new() -> Self {
        Self {
            reg: [0; 16],
            fiq_r8_12_banked: [0; 5],
            banks: [BankedRegisters::default(); 6],
            cpsr: ProgramStatusRegister::new(),
            spsr: 0,
            pipeline: [0, 0, 0],
            pipeline_dirty: false,
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

    #[inline]
    pub(crate) fn fetch(&mut self, bus: &mut impl Bus) -> u32 {
        let addr = self.reg[PC_INDEX];
        let instr;
        let width;
        match self.cpsr.operating_state {
            OperatingState::ARM => {
                instr = bus.read_word(addr);
                self.reg[PC_INDEX] = self.reg[PC_INDEX].wrapping_add(4);
                width = 4;
            }
            OperatingState::THUMB => {
                instr = bus.read_hword(addr) as u32;
                self.reg[PC_INDEX] = self.reg[PC_INDEX].wrapping_add(2);
                width = 2;
            }
        }
        let c = bus.access_cycles(addr, width);
        bus.tick(c);
        instr
    }

    fn execute(&mut self, bus: &mut impl Bus, instr: u32) {
        match self.cpsr.operating_state {
            OperatingState::ARM => {
                self.arm_opcodes(bus, instr);
            }
            OperatingState::THUMB => {
                self.thumb_opcodes(bus, bit_r!(instr, 0..16) as u16);
            }
        }
    }

    #[inline]
    pub(crate) fn step(&mut self, bus: &mut impl Bus) {
        self.pipeline[0] = self.pipeline[1];
        self.pipeline[1] = self.pipeline[2];

        self.pipeline_dirty = false;
        self.execute(bus, self.pipeline[0]);

        if !self.pipeline_dirty {
            self.pipeline[2] = self.fetch(bus);
        }
    }

    pub(crate) fn flush_pipeline(&mut self, bus: &mut impl Bus) {
        self.reg[PC_INDEX] &= match self.cpsr.operating_state {
            OperatingState::ARM => !0b11,
            OperatingState::THUMB => !0b1,
        };

        bus.invalidate_rom_seq();
        bus.notify_pc(self.reg[PC_INDEX]);
        self.pipeline[1] = self.fetch(bus);
        self.pipeline[2] = self.fetch(bus);
        self.pipeline_dirty = true;
    }
}

impl Default for CPU {
    fn default() -> Self {
        Self::new()
    }
}

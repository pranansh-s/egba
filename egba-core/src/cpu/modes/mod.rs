use std::fmt;

use bit::BitIndex;
use crate::cpu::cpu::CPU;

use super::cpu::PC_INDEX;

mod arm;
mod thumb;

#[macro_export]
macro_rules! bit_r {
    ($instr:expr, $range:expr) => {
        $instr.bit_range($range) as usize
    };
}

#[derive(Debug)]
pub enum ShiftType {
    LSL,
    LSR,
    ASR,
    ROR,
}

impl fmt::Display for ShiftType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}


impl ShiftType {
    pub fn from_bits(bits: usize) -> Self {
        match bits {
            0b00 => ShiftType::LSL,
            0b01 => ShiftType::LSR,
            0b10 => ShiftType::ASR,
            0b11 => ShiftType::ROR,
            _ => unreachable!()
        }
    }
}

impl CPU {
    pub fn condition_check(&self, cond: usize) -> bool {
        match cond {
            0b0000 => self.cpsr.z_condition_bit,
            0b0001 => !self.cpsr.z_condition_bit,
            0b0010 => self.cpsr.c_condition_bit,
            0b0011 => !self.cpsr.c_condition_bit,
            0b0100 => self.cpsr.n_condition_bit,
            0b0101 => !self.cpsr.n_condition_bit,
            0b0110 => self.cpsr.v_condition_bit,
            0b0111 => !self.cpsr.v_condition_bit,
            0b1000 => self.cpsr.c_condition_bit && !self.cpsr.z_condition_bit,
            0b1001 => !self.cpsr.c_condition_bit || self.cpsr.z_condition_bit,
            0b1010 => self.cpsr.n_condition_bit == self.cpsr.v_condition_bit,
            0b1011 => self.cpsr.n_condition_bit != self.cpsr.v_condition_bit,
            0b1100 => !self.cpsr.z_condition_bit && (self.cpsr.n_condition_bit == self.cpsr.v_condition_bit),
            0b1101 => self.cpsr.z_condition_bit || (self.cpsr.n_condition_bit != self.cpsr.v_condition_bit),
            0b1110 => true,
            _ => unreachable!(),
        }
    }

    pub fn arm_pc(&self) -> u32 {
        self.reg[PC_INDEX].wrapping_sub(8)
    }

    pub fn thumb_pc(&self) -> u32 {
        self.reg[PC_INDEX].wrapping_sub(4)
    }

    pub fn set_NZ(&mut self, val: u32) {
        self.cpsr.n_condition_bit = val.bit(31);
        self.cpsr.z_condition_bit = val == 0;
    }

    pub fn set_NZ_64(&mut self, val: u64) {
        self.cpsr.n_condition_bit = val.bit(63);
        self.cpsr.z_condition_bit = val == 0;
    }

    pub fn shift_by_reg(&mut self, inst: usize, s: bool) -> u32 {
        let rm = bit_r!(inst, 0..4);
        let shift_type = bit_r!(inst, 5..7);
        let rotate = if inst.bit(4) {
            let rs = bit_r!(inst, 8..12);
            bit_r!(self.reg[rs], 0..8)
        }
        else {
            bit_r!(inst, 7..12)
        } as u8;

        let val = if rm == PC_INDEX && inst.bit(4) { self.reg[rm].wrapping_add(4) } else { self.reg[rm] };

        match ShiftType::from_bits(shift_type) {
            ShiftType::LSL => self.LSL(val, rotate, s),
            ShiftType::LSR => self.LSR(val, rotate, s),
            ShiftType::ASR => self.ASR(val, rotate, s),
            ShiftType::ROR => self.ROR(val, rotate, s),
        }
    }

    fn ASR(&mut self, value: u32, rot: u8, set_condition: bool) -> u32 {
        match rot {
            0 => value,
            1..31 => {
                if set_condition {
                    self.cpsr.c_condition_bit = value.bit(rot as usize - 1);
                }
                let res = ((value as i32) >> rot) as u32;
                res
            },
            _ => {
                if set_condition {
                    self.cpsr.c_condition_bit = value.bit(31);
                }
                let res = if value.bit(31) { !0 } else { 0 };
                res
            }
        }
    }

    fn LSL(&mut self, value: u32, rot: u8, set_condition: bool) -> u32 {
        match rot {
            0 => value,
            1..=31 => {
                if set_condition {
                    self.cpsr.c_condition_bit = value.bit(32 - rot as usize);
                }
                value << rot
            }
            32 => {
                if set_condition {
                    self.cpsr.c_condition_bit = value.bit(0);
                }
                0
            }
            _ => {
                if set_condition {
                    self.cpsr.c_condition_bit = false;
                }
                0
            }
        }
    }
    
    fn LSR(&mut self, value: u32, rot: u8, set_condition: bool) -> u32 {
        match rot {
            0 => value,
            1..=31 => {
                if set_condition {
                    self.cpsr.c_condition_bit = value.bit(rot as usize - 1);
                }
                value >> rot
            }
            32 => {
                if set_condition {
                    self.cpsr.c_condition_bit = value.bit(31);
                }
                0
            }
            _ => {
                if set_condition {
                    self.cpsr.c_condition_bit = false;
                }
                0
            }
        }
    }
    
    pub fn ROR(&mut self, value: u32, rot: u8, set_condition: bool) -> u32 {
        let mut res: u32;
        if rot > 0 || rot == 0 {
            res = value.rotate_right(rot as u32 % 32);
            if set_condition {
                self.cpsr.c_condition_bit = res.bit(31);
            }
        }
        else {
            let carry = self.cpsr.c_condition_bit;
            res = value.rotate_right(1);
            if set_condition {
                self.cpsr.c_condition_bit = res.bit(31);
            }
            res.set_bit(31, carry);
        }
        res
    }
}
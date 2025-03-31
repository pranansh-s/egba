use bit::BitIndex;
use bitmatch::bitmatch;

use crate::{bit_r, bus::Bus, cpu::{alu::is_test, cpu::{CPU, LR_INDEX, PC_INDEX, SP_INDEX}, exception::Exception}};

impl CPU {
    #[bitmatch]
    pub fn thumb_opcodes(&mut self, bus: &mut impl Bus, inst: u16) {
        #[bitmatch]
        match inst.bit_range(0..16) {
            "0001_1???_????_????" => self.thumb_format2(inst.bit(10), inst.bit(9), bit_r!(inst, 6..9), bit_r!(inst, 3..6), bit_r!(inst, 0..3)),
            "000?_????_????_????" => self.thumb_format1(bit_r!(inst, 11..13), bit_r!(inst, 6..11), bit_r!(inst, 3..6), bit_r!(inst, 0..3)),
            "001?_????_????_????" => self.thumb_format3(bit_r!(inst, 11..13), bit_r!(inst, 8..11), bit_r!(inst, 0..8) as u8),

            "0100_00??_????_????" => self.thumb_format4(bit_r!(inst, 6..10), bit_r!(inst, 3..6), bit_r!(inst, 0..3)),
            "0100_01??_????_????" => self.thumb_format5(bus, bit_r!(inst, 8..10), inst.bit(7), inst.bit(6), bit_r!(inst, 3..6), bit_r!(inst, 0..3)),
            "0100_1???_????_????" => self.thumb_format6(bus, bit_r!(inst, 8..11), bit_r!(inst, 0..8) as u32),
            
            "0101_??0?_????_????" => self.thumb_format7(bus, inst.bit(11), inst.bit(10), bit_r!(inst, 6..9), bit_r!(inst, 3..6), bit_r!(inst, 0..3)),
            "0101_??1?_????_????" => self.thumb_format8(bus, inst.bit(11), inst.bit(10), bit_r!(inst, 6..9), bit_r!(inst, 3..6), bit_r!(inst, 0..3)),
            "011?_????_????_????" => self.thumb_format9(bus, inst.bit(12), inst.bit(11), bit_r!(inst, 6..11), bit_r!(inst, 3..6), bit_r!(inst, 0..3)),

            "1000_????_????_????" => self.thumb_format10(bus, inst.bit(11), bit_r!(inst, 6..11), bit_r!(inst, 3..6), bit_r!(inst, 0..3)),
            "1001_????_????_????" => self.thumb_format11(bus, inst.bit(11), bit_r!(inst, 8..11), bit_r!(inst, 0..8)),
            "1010_????_????_????" => self.thumb_format12(inst.bit(11), bit_r!(inst, 8..11), bit_r!(inst, 0..8) as u32),
            
            "1011_0000_????_????" => self.thumb_format13(inst.bit(7), bit_r!(inst, 0..7) as u32),
            "1011_?10?_????_????" => self.thumb_format14(bus, inst.bit(11), inst.bit(8), bit_r!(inst, 0..8) as u16),
            
            "1100_????_????_????" => self.thumb_format15(bus, inst.bit(11), bit_r!(inst, 8..11), bit_r!(inst, 0..8) as u8),
            "1101_1111_????_????" => self.enter_exception(Exception::SoftwareInterrupt, self.thumb_pc().wrapping_add(2)),
            "1101_????_????_????" => self.thumb_format16(bus, bit_r!(inst, 8..12), bit_r!(inst, 0..8) as u8),

            "1110_0???_????_????" => self.thumb_format18(bus, bit_r!(inst, 0..11)),
            "1111_????_????_????" => self.thumb_format19(bus, inst.bit(11), bit_r!(inst, 0..11)),
            _ => self.enter_exception(Exception::Undefined, self.thumb_pc().wrapping_add(2))
        }
    }

    fn thumb_format1(&mut self, op: usize, offset: usize, rs: usize, rd: usize) {
        self.reg[rd] = match op {
            0 => self.LSL(self.reg[rs], offset as u8, true),
            1 => self.LSR(self.reg[rs], offset as u8, true),
            2 => self.ASR(self.reg[rs], offset as u8, true),
            _ => unreachable!()
        };
    }

    fn thumb_format2(&mut self, i: bool, sub: bool, offset: usize, rs: usize, rd: usize) {
        self.reg[rd] = match (i, sub) {
            (false, false) => self.ADD(self.reg[rs], self.reg[offset], true),
            (false, true) => self.SUB(self.reg[rs], self.reg[offset], true),
            (true, false) => self.ADD(self.reg[rs],offset as u32, true),
            (true, true) => self.SUB(self.reg[rs], offset as u32, true),
        };

        self.set_NZ(self.reg[rd]);
    }

    fn thumb_format3(&mut self, op: usize, rd: usize, offset: u8) {
        let res = match op {
            0 => self.MOV(offset as u32),
            1 | 3 => self.SUB(self.reg[rd], offset as u32, true),
            2 => self.ADD(self.reg[rd], offset as u32, true),
            _ => unreachable!()
        };

        if op != 1 {
            self.reg[rd] = res;
        }

        self.set_NZ(res);
    }

    fn thumb_format4(&mut self, opcode: usize, rs: usize, rd: usize) {
        let op = self.reg[rd];
        let op2 = self.reg[rs];

        let res = match opcode {
            0b0000 => self.AND(op, op2),
            0b0001 => self.EOR(op, op2),
            0b0010 => self.LSL(op, op2 as u8, true),
            0b0011 => self.LSR(op, op2 as u8, true),
            0b0100 => self.ASR(op, op2 as u8, true),
            0b0101 => self.ADC(op, op2, true, self.cpsr.c_condition_bit),
            0b0110 => self.SBC(op, op2, true, self.cpsr.c_condition_bit),
            0b0111 => self.ROR(op, op2 as u8, true),
            0b1000 => self.AND(op, op2),
            0b1001 => self.SUB(0, op2, true),
            0b1010 => self.SUB(op, op2, true),
            0b1011 => self.ADD(op, op2, true),
            0b1100 => self.ORR(op, op2),
            0b1101 => op.wrapping_mul(op2),
            0b1110 => self.BIC(op, op2),
            0b1111 => self.MVN(op2),
            _ => unreachable!()
        };

        if !is_test(opcode) {
            self.reg[rd] = res;
        }

        self.set_NZ(res);
    }

    fn thumb_format5(&mut self, bus: &mut impl Bus, op: usize, h1: bool, h2: bool, rs: usize, rd: usize) {
        let rs = if h2 { rs + 8 } else { rs };
        let rd = if h1 { rd + 8 } else { rd };
        
        match op {
            0 => self.reg[rd] = self.ADD(self.reg[rd], self.reg[rs], false),
            1 => {
                let res = self.SUB(self.reg[rd], self.reg[rs], true);
                self.set_NZ(res);
            },
            2 => self.reg[rd] = self.MOV(self.reg[rs]),
            3 => self.arm_BX(bus, rs),
            _ => unreachable!()
        }

        if op != 1 && rd == PC_INDEX {
            self.flush_pipeline(bus);
        }
    }

    fn thumb_format6(&mut self, bus: &mut impl Bus, rd: usize, offset: u32) {
        let addr = (self.reg[PC_INDEX] & !0b10).wrapping_add(offset << 2);
        self.reg[rd] = bus.read_word(addr);

        if rd == PC_INDEX {
            self.flush_pipeline(bus);
        }
    }

    fn thumb_format7(&mut self, bus: &mut impl Bus, l: bool, b: bool, ro: usize, rb: usize, rd: usize) {
        self.arm_LDR_STR(bus, l, false, true, true, b, false, rb, rd, self.reg[ro] as usize);
    }

    fn thumb_format8(&mut self, bus: &mut impl Bus, h: bool, s: bool, ro: usize, rb: usize, rd: usize) {
        self.arm_LDRH_LDRSB_LDRSH_STRH(bus, true, true, false, false, h || s, rb, rd, 42069, s, h, ro);
    }

    fn thumb_format9(&mut self, bus: &mut impl Bus, b: bool, l: bool, offset: usize, rb: usize, rd: usize) {
        let offset = if b { offset } else { offset << 2 };
        self.arm_LDR_STR(bus, l, false, true, true, b, false, rb, rd, offset);
    }

    fn thumb_format10(&mut self, bus: &mut impl Bus, l: bool, offset: usize, rb: usize, rd: usize) {
        let offset = offset << 1;
        self.arm_LDRH_LDRSB_LDRSH_STRH(bus, true, true, true, false, l, rb, rd, bit_r!(offset, 4..8), false, true, bit_r!(offset, 0..4));
    }

    fn thumb_format11(&mut self, bus: &mut impl Bus, l: bool, rd: usize, offset: usize) {
        let offset = offset << 2;
        self.arm_LDR_STR(bus, l, false, true, true, false, false, SP_INDEX, rd, offset);
    }

    fn thumb_format12(&mut self, sp: bool, rd: usize, offset: u32) {
        let offset = offset << 2;
        self.reg[rd] = if sp {
            self.ADD(self.reg[SP_INDEX], offset, false)
        }
        else {
            self.ADD(self.reg[PC_INDEX] & !0b10, offset, false)
        };
    }

    fn thumb_format13(&mut self, s: bool, offset: u32) {
        let offset = offset << 2;
        self.reg[SP_INDEX] = if s {
            self.SUB(self.reg[SP_INDEX], offset, false)
        }
        else {
            self.ADD(self.reg[SP_INDEX], offset, false)
        };
    }

    fn thumb_format14(&mut self, bus: &mut impl Bus, l: bool, r: bool, r_list: u16) {
        let other = if l { PC_INDEX } else { LR_INDEX };
        let r_list = if r { r_list | (1 << other) } else { r_list };

        self.arm_LDM_STM(bus, l, !l, l, false, true, SP_INDEX, r_list);
    }

    fn thumb_format15(&mut self, bus: &mut impl Bus, l: bool, rb: usize, r_list: u8) {
        let mut addr = self.reg[rb];
        for r in 0..=7 {
            if r_list.bit(r) {
                if l {
                    self.reg[r] = bus.read_word(addr);
                }
                else {
                    bus.write_word(addr, self.reg[r]);
                }

                addr += 4;
            }
        }

        self.reg[rb] = addr;
    }

    fn thumb_format16(&mut self, bus: &mut impl Bus, cond: usize, offset: u8) {
        if !self.condition_check(cond) {
            return;
        }
        
        let offset = ((offset as i8) as i32) << 1;
        self.reg[PC_INDEX] = ((self.reg[PC_INDEX] as i32) + offset) as u32;
        self.flush_pipeline(bus);
    }

    fn thumb_format18(&mut self, bus: &mut impl Bus, offset: usize) {
        let offset = ((offset << 21) as i32) >> 20;
        self.reg[PC_INDEX] = ((self.reg[PC_INDEX] as i32) + offset) as u32;
        self.flush_pipeline(bus);
    }

    fn thumb_format19(&mut self, bus: &mut impl Bus, hi: bool, offset: usize) {
        if hi {
            let addr = self.reg[LR_INDEX].wrapping_add((offset as u32) << 1);
            self.reg[LR_INDEX] = self.thumb_pc().wrapping_add(2) | 1;
            self.reg[PC_INDEX] = addr;
            self.flush_pipeline(bus);
        }
        else {
            let offset = ((offset << 21) as i32) >> 9;
            self.reg[LR_INDEX] = ((self.reg[PC_INDEX] as i32) + offset) as u32;
        }
    }
}
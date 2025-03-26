use bit::BitIndex;
use bitmatch::bitmatch;

use crate::{bit_r, bus::Bus, cpu::{alu::is_test, cpu::{CPU, LR_INDEX, PC_INDEX}, exception::Exception, psr::{OperatingMode, OperatingState, ProgramStatusRegister}}};

impl CPU {
    #[bitmatch]
    pub fn arm_opcodes(&mut self, bus: &mut impl Bus, inst: u32) {
        if !self.condition_check(inst.bit_range(28..32) as usize) {
            return;
        }

        #[bitmatch]
        match inst.bit_range(0..28) {
            "0001_0010_1111_1111_1111_0001_????" => self.arm_BX(bus, bit_r!(inst, 0..4)),
            "1010_????_????_????_????_????_????" => self.arm_B(bus, bit_r!(inst, 0..24)),
            "1011_????_????_????_????_????_????" => self.arm_BL(bus, bit_r!(inst, 0..24)),
            "0000_00??_????_????_????_1001_????" => self.arm_MUL_MLA(inst.bit(21), inst.bit(20), bit_r!(inst, 16..20), bit_r!(inst, 12..16), bit_r!(inst, 8..12), bit_r!(inst, 0..4)),
            
            "0000_1???_????_????_????_1001_????" => self.arm_UMULL_UMLAL_SMULL_SMLAL(inst.bit(22), inst.bit(21), inst.bit(20), bit_r!(inst, 16..20), bit_r!(inst, 12..16), bit_r!(inst, 8..12), bit_r!(inst, 0..4)),
            
            "0001_0?00_1111_????_0000_0000_0000" => self.arm_MRS(inst.bit(22), bit_r!(inst, 12..16)),
            "00?1_0?10_100?_1111_????_????_????" => self.arm_MSR(inst.bit(25), inst.bit(22), !inst.bit(16), bit_r!(inst, 0..12)),

            "011?_????_????_????_????_???1_????" => self.enter_exception(Exception::Undefined, self.arm_pc().wrapping_add(4)),
            "100?_???1_????_????_????_????_????" => self.arm_LDM_STM(bus, inst.bit(20), inst.bit(24), inst.bit(23), inst.bit(22), inst.bit(21), bit_r!(inst, 16..20), bit_r!(inst, 0..16) as u16),
            
            "0001_0?00_????_????_0000_1001_????" => self.arm_SWP(bus, inst.bit(22), bit_r!(inst, 16..20), bit_r!(inst, 12..16), bit_r!(inst, 0..4)),
            "000?_????_????_????_????_1??1_????" => self.arm_LDRH_LDRSB_LDRSH_STRH(bus, inst.bit(24), inst.bit(23), inst.bit(22), inst.bit(21), inst.bit(20), bit_r!(inst, 16..20), bit_r!(inst, 12..16), bit_r!(inst, 8..12), inst.bit(6), inst.bit(5), bit_r!(inst, 0..4)),
            
            "00??_????_????_????_????_????_????" => self.arm_data_proc(bus, inst.bit(25), bit_r!(inst, 21..25), inst.bit(20), bit_r!(inst, 16..20), bit_r!(inst, 12..16), bit_r!(inst, 0..12)),
            "01??_????_????_????_????_????_????" => self.arm_LDR_STR(bus, inst.bit(20), inst.bit(25), inst.bit(24), inst.bit(23), inst.bit(22), inst.bit(21), bit_r!(inst, 16..20), bit_r!(inst, 12..16), bit_r!(inst, 0..12)),

            "1111_????_????_????_????_????_????" => self.enter_exception(Exception::SoftwareInterrupt, self.arm_pc().wrapping_add(4)),
            _ => self.enter_exception(Exception::Undefined, self.arm_pc().wrapping_add(4))
        }
    }

    pub(crate) fn arm_BX(&mut self, bus: &mut impl Bus, rn: usize) {
        self.reg[PC_INDEX] = self.reg[rn];
        self.cpsr.operating_state = if self.reg[PC_INDEX] & 1 == 0 {
            OperatingState::ARM
        } else {
            OperatingState::THUMB
        };
        self.flush_pipeline(bus);
    }

    fn arm_B(&mut self, bus: &mut impl Bus, offset: usize) {
        let offset = ((offset << 8) as i32) >> 6;
        self.reg[PC_INDEX] = ((self.reg[PC_INDEX] as i32) + offset) as u32;
        self.flush_pipeline(bus);
    }

    fn arm_BL(&mut self, bus: &mut impl Bus, offset: usize) {
        let offset = ((offset << 8) as i32) >> 6;
        self.reg[LR_INDEX] = self.arm_pc().wrapping_add(4);
        self.reg[PC_INDEX] = ((self.reg[PC_INDEX] as i32) + offset) as u32;
        self.flush_pipeline(bus);
    }

    fn arm_MRS(&mut self, p: bool, rd: usize) {
        let psr = if p {
            self.spsr
        }
        else {
            self.cpsr.into()
        };
        self.reg[rd] = psr;
    }

    fn arm_MSR(&mut self, i: bool, p: bool, f: bool, op: usize) {
        let bits = if i {
            self.ROR(bit_r!(op, 0..8) as u32, 2 * bit_r!(op, 8..12) as u8, false)
        }
        else {
            self.reg[bit_r!(op, 0..4)]
        };
        let mask = if f { 0xF000_0000 } else { 0xF000_00DF };

        if p {
            self.spsr = (self.spsr & !mask) | (bits & mask);
        }
        else {
            let cpsr = ProgramStatusRegister::from((u32::from(self.cpsr) & !mask) | (bits & mask));
            self.set_mode(cpsr.mode);
            self.cpsr = cpsr;
        }
    }

    fn arm_data_proc(&mut self, bus: &mut impl Bus, i: bool, opcode: usize, s: bool, rn: usize, rd: usize, operand2: usize) {
        let mut op = self.reg[rn];
        let update_cpsr = s && rd != PC_INDEX;
        let op2 = if i {
            let imm = bit_r!(operand2, 0..8) as u32;
            let rotate = 2 * bit_r!(operand2, 8..12) as u8;
            self.ROR(imm, rotate, update_cpsr)
        }
        else {
            self.shift_by_reg(operand2, update_cpsr)
        };

        if rn == PC_INDEX && operand2.bit(4) {
            op = op.wrapping_add(4);
        }

        let res = match opcode {
            0b0000 => self.AND(op, op2),
            0b0001 => self.EOR(op, op2),
            0b0010 => self.SUB(op, op2, update_cpsr),
            0b0011 => self.SUB(op2, op, update_cpsr),
            0b0100 => self.ADD(op, op2, update_cpsr),
            0b0101 => self.ADC(op, op2, update_cpsr, self.cpsr.c_condition_bit),
            0b0110 => self.SBC(op, op2, update_cpsr, self.cpsr.c_condition_bit),
            0b0111 => self.SBC(op2, op, update_cpsr, self.cpsr.c_condition_bit),
            0b1000 => self.AND(op, op2), 
            0b1001 => self.EOR(op, op2),
            0b1010 => self.SUB(op, op2, true),
            0b1011 => self.ADD(op, op2, true),
            0b1100 => self.ORR(op, op2),
            0b1101 => self.MOV(op2),
            0b1110 => self.BIC(op, op2),
            0b1111 => self.MVN(op2),
            _ => unreachable!()
        };

        if update_cpsr || is_test(opcode) {
            self.set_NZ(res);
        }

        if !is_test(opcode) {
            self.reg[rd] = res;
            if s && rd == PC_INDEX && self.cpsr.mode != OperatingMode::usr  && self.cpsr.mode != OperatingMode::sys {
                self.restore_spsr();
            }

            if rd == PC_INDEX {
                self.flush_pipeline(bus);
            }
        }
    }

    pub(crate) fn arm_LDR_STR(&mut self, bus: &mut impl Bus, l: bool, i: bool, p: bool, u: bool, b: bool, w: bool, rn: usize, rd: usize, offset: usize) {
        let shift = if i {
            self.shift_by_reg(offset, false)
        }
        else {
            offset as u32
        };

        let addr = match (p, u) {
            (false, _) => self.reg[rn],
            (true, true) => self.reg[rn].wrapping_add(shift),
            (true, false) => self.reg[rn].wrapping_sub(shift),
        };

        let pre_mode = self.cpsr.mode;
        if w && !p {
            self.set_mode(OperatingMode::usr);
        }

        if l {
            self.reg[rd] = if b {
                bus.read_byte(addr) as u32
            }
            else {
                bus.read_word(addr)
            };

            if rd == PC_INDEX {
                self.flush_pipeline(bus);
            }
        }
        else {
            let val = if rd == PC_INDEX {  self.reg[PC_INDEX].wrapping_add(12) } else { self.reg[rd] };
            
            if b {
                bus.write_byte(addr, bit_r!(val, 0..8) as u8);
            }
            else {
                bus.write_word(addr, val);
            }
        }

        if w && !p {
            self.set_mode(pre_mode);
        }

        if w || !p {
            self.reg[rn] = match u {
                true => self.reg[rn].wrapping_add(shift),
                false => self.reg[rn].wrapping_sub(shift),
            };
        }
    }

    pub(crate) fn arm_LDRH_LDRSB_LDRSH_STRH(&mut self, bus: &mut impl Bus, p: bool, u: bool, i: bool, w: bool, l: bool, rn: usize, rd: usize, offset_hi: usize, s: bool, h: bool, offset_lo: usize) {
        let shift = if i {
            ((offset_hi << 4) | offset_lo) as u32
        }
        else {
            self.reg[offset_lo]
        };

        let addr = match(p, u) {
            (false, true) | (false, false) => self.reg[rn],
            (true, true) => self.reg[rn].wrapping_add(shift),
            (true, false) => self.reg[rn].wrapping_sub(shift),
        };

        if l {
            self.reg[rd] = if h {
                let mut val = bus.read_hword(addr) as u32;
                val = if s {
                    let val = (val as i16) >> (8 * (addr & 0b1));
                    val as u32
                } else {
                    val.rotate_right(8 * (addr & 0b1))
                };
                val
            }
            else {
                bus.read_byte(addr) as i8 as u32
            };
        }
        else {
            bus.write_hword(addr, self.reg[rd] as u16);
        }

        if w || !p {
            self.reg[rn] = match u {
                true => self.reg[rn].wrapping_add(shift),
                false => self.reg[rn].wrapping_sub(shift),
            }
        }
    }

    pub(crate) fn arm_LDM_STM(&mut self, bus: &mut impl Bus, l: bool, p: bool, u: bool, s: bool, w: bool, rn: usize, r_list: u16) {
        let regs_length = r_list.count_ones();
        let base_address = match (p, u) {
            (false, true) => self.reg[rn],
            (true, true) => self.reg[rn].wrapping_add(4),
            (false, false) => self.reg[rn].wrapping_sub(4 * (regs_length - 1)),
            (true, false) => self.reg[rn].wrapping_sub(4 * regs_length),
        };
        
        let pre_mode = self.cpsr.mode;
        if s && !r_list.bit(PC_INDEX) {
            self.set_mode(OperatingMode::usr);
        }

        let mut addr = base_address;
        for r in 0..=PC_INDEX {
            if r_list.bit(r) {
                if l {
                    self.reg[r] = bus.read_word(addr);
                }
                else {
                    let val = if r == PC_INDEX {  self.reg[PC_INDEX].wrapping_add(12) } else { self.reg[r] };
                    bus.write_word(addr, val);
                }

                if r == PC_INDEX && s && l && self.cpsr.mode != OperatingMode::usr && self.cpsr.mode != OperatingMode::sys {
                    self.restore_spsr();
                }

                addr += 4;
            }
        }

        if s && !r_list.bit(PC_INDEX) {
            self.set_mode(pre_mode);
        }

        if w && !(l && r_list.bit(rn)) {
            self.reg[rn] = match (p, u) {
                (false, true) => addr,
                (true, true) => addr.wrapping_sub(4),
                (false, false) => addr.wrapping_sub(4 * (regs_length + 1)),
                (true, false) => addr.wrapping_sub(4 * regs_length)
            };
        } 

        if l && r_list.bit(PC_INDEX) {
            self.flush_pipeline(bus);
        }
    }

    fn arm_MUL_MLA(&mut self, a: bool, s: bool, rd: usize, rn: usize, rs: usize, rm: usize) {
        let acc = if a { self.reg[rn] } else { 0 };
        let prod = self.reg[rm].wrapping_mul(self.reg[rs]).wrapping_add(acc);
        self.reg[rd] = prod;
        
        if s {
            self.set_NZ(prod);
        }
    }

    fn arm_UMULL_UMLAL_SMULL_SMLAL(&mut self, u: bool, a: bool, s: bool, rd_hi: usize, rd_lo: usize, rs: usize, rm: usize) {
        let acc = if a { (self.reg[rd_hi] as u64) << 32 | (self.reg[rd_lo] as u64) } else { 0 };
        let prod = if u {
            ((self.reg[rs] as i32 as i64).wrapping_mul(self.reg[rm] as i32 as i64) as u64).wrapping_add(acc)
        }
        else {
            (self.reg[rs] as u64).wrapping_mul(self.reg[rm] as u64).wrapping_add(acc)
        };
        self.reg[rd_hi] = (prod >> 32) as u32;
        self.reg[rd_lo] = prod as u32;

        if s {
            self.set_NZ_64(prod);
        }
    }

    fn arm_SWP(&mut self, bus: &mut impl Bus, b: bool, rn: usize, rd: usize, rm: usize) {
        let swap_address = self.reg[rn];
        if b {
            self.reg[rd] = bus.read_byte(swap_address) as u32;
            bus.write_byte(swap_address, self.reg[rm] as u8);
        }
        else {
            self.reg[rd] = bus.read_word(swap_address).rotate_right(8 * (swap_address & 0b11));
            bus.write_word(swap_address, self.reg[rm]);
        }
    }
}
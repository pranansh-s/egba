use bit::BitIndex;
use bitmatch::bitmatch;

use crate::{
    bit_r,
    bus::Bus,
    cpu::{
        alu::is_test,
        cpu::{CPU, LR_INDEX, PC_INDEX},
        exception::Exception,
        psr::{OperatingMode, OperatingState, ProgramStatusRegister},
    },
};

#[allow(non_camel_case_types, clippy::too_many_arguments)]
impl CPU {
    #[bitmatch]
    pub(crate) fn arm_opcodes(&mut self, bus: &mut impl Bus, inst: u32) {
        if !self.condition_check(inst.bit_range(28..32) as usize) {
            return;
        }

        #[bitmatch]
        match inst.bit_range(0..28) {
            "0001_0010_1111_1111_1111_0001_????" => self.arm_BX(bus, bit_r!(inst, 0..4)),
            "101?_????_????_????_????_????_????" => {
                self.arm_B_BL(bus, inst.bit(24), bit_r!(inst, 0..24))
            }
            "0000_00??_????_????_????_1001_????" => self.arm_MUL_MLA(
                bus,
                inst.bit(21),
                inst.bit(20),
                bit_r!(inst, 16..20),
                bit_r!(inst, 12..16),
                bit_r!(inst, 8..12),
                bit_r!(inst, 0..4),
            ),

            "0000_1???_????_????_????_1001_????" => self.arm_UMULL_UMLAL_SMULL_SMLAL(
                bus,
                inst.bit(22),
                inst.bit(21),
                inst.bit(20),
                bit_r!(inst, 16..20),
                bit_r!(inst, 12..16),
                bit_r!(inst, 8..12),
                bit_r!(inst, 0..4),
            ),

            "0001_0?00_1111_????_0000_0000_0000" => {
                self.arm_MRS(inst.bit(22), bit_r!(inst, 12..16))
            }
            "00?1_0?10_????_1111_????_????_????" => self.arm_MSR(
                inst.bit(25),
                inst.bit(22),
                bit_r!(inst, 16..20),
                bit_r!(inst, 0..12),
            ),

            "011?_????_????_????_????_???1_????" => {
                self.enter_exception(bus, Exception::Undefined, self.arm_pc().wrapping_add(4))
            }
            "100?_????_????_????_????_????_????" => self.arm_LDM_STM(
                bus,
                inst.bit(20),
                inst.bit(24),
                inst.bit(23),
                inst.bit(22),
                inst.bit(21),
                bit_r!(inst, 16..20),
                bit_r!(inst, 0..16) as u16,
            ),

            "0001_0?00_????_????_0000_1001_????" => self.arm_SWP(
                bus,
                inst.bit(22),
                bit_r!(inst, 16..20),
                bit_r!(inst, 12..16),
                bit_r!(inst, 0..4),
            ),
            "000?_????_????_????_????_1??1_????" => self.arm_LDRH_LDRSB_LDRSH_STRH(
                bus,
                inst.bit(24),
                inst.bit(23),
                inst.bit(22),
                inst.bit(21),
                inst.bit(20),
                bit_r!(inst, 16..20),
                bit_r!(inst, 12..16),
                bit_r!(inst, 8..12),
                inst.bit(6),
                inst.bit(5),
                bit_r!(inst, 0..4),
            ),

            "00??_????_????_????_????_????_????" => self.arm_data_proc(
                bus,
                inst.bit(25),
                bit_r!(inst, 21..25),
                inst.bit(20),
                bit_r!(inst, 16..20),
                bit_r!(inst, 12..16),
                bit_r!(inst, 0..12),
            ),
            "01??_????_????_????_????_????_????" => self.arm_LDR_STR(
                bus,
                inst.bit(20),
                inst.bit(25),
                inst.bit(24),
                inst.bit(23),
                inst.bit(22),
                inst.bit(21),
                bit_r!(inst, 16..20),
                bit_r!(inst, 12..16),
                bit_r!(inst, 0..12),
            ),

            "1111_????_????_????_????_????_????" => {
                self.enter_exception(bus, Exception::SoftwareInterrupt, self.arm_pc().wrapping_add(4))
            }
            _ => self.enter_exception(bus, Exception::Undefined, self.arm_pc().wrapping_add(4)),
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

    fn arm_B_BL(&mut self, bus: &mut impl Bus, l: bool, offset: usize) {
        let offset = ((offset << 8) as i32) >> 6;
        if l {
            self.reg[LR_INDEX] = self.arm_pc().wrapping_add(4);
        }
        self.reg[PC_INDEX] = self.reg[PC_INDEX].wrapping_add(offset as u32);
        self.flush_pipeline(bus);
    }

    fn arm_MRS(&mut self, p: bool, rd: usize) {
        let psr = if p
            && self.cpsr.mode != OperatingMode::usr
            && self.cpsr.mode != OperatingMode::sys
        {
            self.spsr
        } else {
            self.cpsr.into()
        };
        self.reg[rd] = psr;
    }

    fn arm_MSR(&mut self, i: bool, p: bool, field_mask: usize, op: usize) {
        let bits = if i {
            let imm = bit_r!(op, 0..8) as u32;
            let rotate = 2 * bit_r!(op, 8..12) as u32;
            imm.rotate_right(rotate)
        } else {
            self.reg[bit_r!(op, 0..4)]
        };

        let mut mask: u32 = 0;
        if field_mask & 0b0001 != 0 { mask |= 0x0000_00FF; }
        if field_mask & 0b0010 != 0 { mask |= 0x0000_FF00; }
        if field_mask & 0b0100 != 0 { mask |= 0x00FF_0000; }
        if field_mask & 0b1000 != 0 { mask |= 0xFF00_0000; }

        mask &= !0x0000_0020;

        if p {
            if self.cpsr.mode != OperatingMode::usr && self.cpsr.mode != OperatingMode::sys {
                self.spsr = (self.spsr & !mask) | (bits & mask);
            }
            return;
        }

        if self.cpsr.mode == OperatingMode::usr {
            mask &= 0xF000_0000;
        }
        let new_cpsr = (u32::from(self.cpsr) & !mask) | (bits & mask);
        let new_psr: ProgramStatusRegister = new_cpsr.into();
        if new_psr.mode != self.cpsr.mode {
            self.set_mode(new_psr.mode);
        }
        self.cpsr = new_psr;
    }

    fn arm_data_proc(
        &mut self,
        bus: &mut impl Bus,
        i: bool,
        opcode: usize,
        s: bool,
        rn: usize,
        rd: usize,
        operand2: usize,
    ) {
        let update_cpsr = s && rd != PC_INDEX;
        let carry_in = self.cpsr.c_condition_bit;
        let op2 = if i {
            let imm = bit_r!(operand2, 0..8) as u32;
            let rotate = 2 * bit_r!(operand2, 8..12) as u32;
            let rotated = imm.rotate_right(rotate);
            if update_cpsr && rotate != 0 {
                self.cpsr.c_condition_bit = rotated.bit(31);
            }
            rotated
        } else {
            let v = self.shift_by_reg(operand2, update_cpsr);
            if operand2.bit(4) {
                bus.tick(1);
            }
            v
        };

        let op = if !i && rn == PC_INDEX && operand2.bit(4) {
            self.reg[rn].wrapping_add(4)
        } else {
            self.reg[rn]
        };

        let res = match opcode {
            0b0000 => self.AND(op, op2),
            0b0001 => self.EOR(op, op2),
            0b0010 => self.SUB(op, op2, update_cpsr),
            0b0011 => self.SUB(op2, op, update_cpsr),
            0b0100 => self.ADD(op, op2, update_cpsr),
            0b0101 => self.ADC(op, op2, update_cpsr, carry_in),
            0b0110 => self.SBC(op, op2, update_cpsr, carry_in),
            0b0111 => self.SBC(op2, op, update_cpsr, carry_in),
            0b1000 => self.AND(op, op2),
            0b1001 => self.EOR(op, op2),
            0b1010 => self.SUB(op, op2, true),
            0b1011 => self.ADD(op, op2, true),
            0b1100 => self.ORR(op, op2),
            0b1101 => self.MOV(op2),
            0b1110 => self.BIC(op, op2),
            0b1111 => self.MVN(op2),
            _ => unreachable!(),
        };

        if update_cpsr || is_test(opcode) {
            self.set_NZ(res);
        }

        if !is_test(opcode) {
            self.reg[rd] = res;
            if s && rd == PC_INDEX
                && self.cpsr.mode != OperatingMode::usr
                && self.cpsr.mode != OperatingMode::sys
            {
                self.restore_spsr();
            }

            if rd == PC_INDEX {
                self.flush_pipeline(bus);
            }
        }
    }

    pub(crate) fn arm_LDR_STR(
        &mut self,
        bus: &mut impl Bus,
        l: bool,
        i: bool,
        p: bool,
        u: bool,
        b: bool,
        w: bool,
        rn: usize,
        rd: usize,
        offset: usize,
    ) {
        let shift = if i {
            self.shift_by_reg(offset, false)
        } else {
            offset as u32
        };

        let original_rn = self.reg[rn];
        let writeback_value = if u {
            original_rn.wrapping_add(shift)
        } else {
            original_rn.wrapping_sub(shift)
        };

        let addr = if p { writeback_value } else { original_rn };
        let writeback = w || !p;
        let t_bit = w && !p;
        let pre_mode = self.cpsr.mode;

        let width = if b { 1 } else { 4 };
        let c = bus.access_cycles(addr, width);
        bus.tick(c);

        if l {
            if t_bit {
                self.set_mode(OperatingMode::usr);
            }
            let loaded = if b {
                bus.read_byte(addr) as u32
            } else {
                bus.read_word(addr).rotate_right(8 * (addr & 0b11))
            };
            if t_bit {
                self.set_mode(pre_mode);
            }

            bus.tick(1);

            if writeback && rn != rd {
                self.reg[rn] = writeback_value;
            }

            self.reg[rd] = loaded;

            if rd == PC_INDEX {
                self.flush_pipeline(bus);
            }
        } else {
            let val = if rd == PC_INDEX {
                self.reg[PC_INDEX].wrapping_add(4)
            } else {
                self.reg[rd]
            };

            if t_bit {
                self.set_mode(OperatingMode::usr);
            }
            if b {
                bus.write_byte(addr, val as u8);
            } else {
                bus.write_word(addr, val);
            }
            if t_bit {
                self.set_mode(pre_mode);
            }

            if writeback {
                self.reg[rn] = writeback_value;
            }
        }
    }

    pub(crate) fn arm_LDRH_LDRSB_LDRSH_STRH(
        &mut self,
        bus: &mut impl Bus,
        p: bool,
        u: bool,
        i: bool,
        w: bool,
        l: bool,
        rn: usize,
        rd: usize,
        offset_hi: usize,
        s: bool,
        h: bool,
        offset_lo: usize,
    ) {
        let shift = if i {
            ((offset_hi << 4) | offset_lo) as u32
        } else {
            self.reg[offset_lo]
        };

        let original_rn = self.reg[rn];
        let writeback_value = if u {
            original_rn.wrapping_add(shift)
        } else {
            original_rn.wrapping_sub(shift)
        };

        let addr = if p { writeback_value } else { original_rn };
        let writeback = w || !p;

        let width = if h { 2 } else { 1 };
        let c = bus.access_cycles(addr, width);
        bus.tick(c);

        if l {
            let loaded = if h {
                let raw = bus.read_hword(addr) as u32;
                let rot = 8 * (addr & 0b1);
                if s {
                    ((raw as i16) >> rot) as u32
                } else {
                    raw.rotate_right(rot)
                }
            } else {
                bus.read_byte(addr) as i8 as u32
            };
            bus.tick(1);

            if writeback && rn != rd {
                self.reg[rn] = writeback_value;
            }

            self.reg[rd] = loaded;

            if rd == PC_INDEX {
                self.flush_pipeline(bus);
            }
        } else {
            let val = if rd == PC_INDEX {
                self.reg[PC_INDEX].wrapping_add(4)
            } else {
                self.reg[rd]
            };
            bus.write_hword(addr, val as u16);

            if writeback {
                self.reg[rn] = writeback_value;
            }
        }
    }

    pub(crate) fn arm_LDM_STM(
        &mut self,
        bus: &mut impl Bus,
        l: bool,
        p: bool,
        u: bool,
        s: bool,
        w: bool,
        rn: usize,
        r_list: u16,
    ) {
        let empty = r_list == 0;
        let xfer_list: u16 = if empty { 1 << PC_INDEX } else { r_list };
        let regs_length = xfer_list.count_ones();
        let total_bytes = 4 * regs_length;
        let wb_bytes = if empty { 0x40 } else { total_bytes };

        let base_address = match (p, u) {
            (false, true) => self.reg[rn],
            (true, true) => self.reg[rn].wrapping_add(4),
            (false, false) => self.reg[rn].wrapping_sub(total_bytes.wrapping_sub(4)),
            (true, false) => self.reg[rn].wrapping_sub(total_bytes),
        };

        let writeback_value = if u {
            self.reg[rn].wrapping_add(wb_bytes)
        } else {
            self.reg[rn].wrapping_sub(wb_bytes)
        };

        let user_bank = s && !xfer_list.bit(PC_INDEX);
        let pre_mode = self.cpsr.mode;
        if user_bank {
            self.set_mode(OperatingMode::usr);
        }

        let rn_in_list = !empty && r_list.bit(rn);
        let first_reg = r_list.trailing_zeros() as usize;
        let stm_early_writeback = !l && w && rn_in_list && rn != first_reg;

        if stm_early_writeback {
            self.reg[rn] = writeback_value;
        }

        let pc_store_offset = match self.cpsr.operating_state {
            OperatingState::ARM => 4,
            OperatingState::THUMB => 2,
        };
        let mut addr = base_address;
        for r in 0..=PC_INDEX {
            if xfer_list.bit(r) {
                let c = bus.access_cycles(addr, 4);
                bus.tick(c);
                if l {
                    self.reg[r] = bus.read_word(addr);
                } else {
                    let val = if r == PC_INDEX {
                        self.reg[PC_INDEX].wrapping_add(pc_store_offset)
                    } else {
                        self.reg[r]
                    };
                    bus.write_word(addr, val);
                }

                if r == PC_INDEX
                    && s
                    && l
                    && self.cpsr.mode != OperatingMode::usr
                    && self.cpsr.mode != OperatingMode::sys
                {
                    self.restore_spsr();
                }

                addr = addr.wrapping_add(4);
            }
        }

        if l {
            bus.tick(1);
        }

        if user_bank {
            self.set_mode(pre_mode);
        }

        let do_writeback = if empty {
            true
        } else if !w {
            false
        } else if !l {
            !stm_early_writeback
        } else {
            !(rn_in_list && rn != first_reg)
        };

        if do_writeback {
            self.reg[rn] = writeback_value;
        }

        if l && xfer_list.bit(PC_INDEX) {
            self.flush_pipeline(bus);
        }
    }

    pub(super) fn mul_m_cycles(rs: u32, signed: bool) -> u32 {
        let matches = |mask: u32| -> bool {
            let m = rs & mask;
            m == 0 || (signed && m == mask)
        };
        if matches(0xFFFF_FF00) {
            1
        } else if matches(0xFFFF_0000) {
            2
        } else if matches(0xFF00_0000) {
            3
        } else {
            4
        }
    }

    fn arm_MUL_MLA(
        &mut self,
        bus: &mut impl Bus,
        a: bool,
        s: bool,
        rd: usize,
        rn: usize,
        rs: usize,
        rm: usize,
    ) {
        let rs_val = self.reg[rs];
        let acc = if a { self.reg[rn] } else { 0 };
        let prod = self.reg[rm].wrapping_mul(rs_val).wrapping_add(acc);
        self.reg[rd] = prod;

        let m = Self::mul_m_cycles(rs_val, true);
        bus.tick(m + a as u32);

        if s {
            self.set_NZ(prod);
            self.cpsr.c_condition_bit = false;
        }
    }

    fn arm_UMULL_UMLAL_SMULL_SMLAL(
        &mut self,
        bus: &mut impl Bus,
        u: bool,
        a: bool,
        s: bool,
        rd_hi: usize,
        rd_lo: usize,
        rs: usize,
        rm: usize,
    ) {
        let rs_val = self.reg[rs];
        let acc = if a {
            (self.reg[rd_hi] as u64) << 32 | (self.reg[rd_lo] as u64)
        } else {
            0
        };
        let prod = if u {
            ((rs_val as i32 as i64).wrapping_mul(self.reg[rm] as i32 as i64) as u64)
                .wrapping_add(acc)
        } else {
            (rs_val as u64)
                .wrapping_mul(self.reg[rm] as u64)
                .wrapping_add(acc)
        };
        self.reg[rd_hi] = (prod >> 32) as u32;
        self.reg[rd_lo] = prod as u32;

        let m = Self::mul_m_cycles(rs_val, u);
        bus.tick(m + 1 + a as u32);

        if s {
            self.set_NZ_64(prod);
        }
    }

    fn arm_SWP(&mut self, bus: &mut impl Bus, b: bool, rn: usize, rd: usize, rm: usize) {
        let swap_address = self.reg[rn];
        let rm_val = self.reg[rm];

        let width = if b { 1 } else { 4 };
        let c = bus.access_cycles(swap_address, width);
        bus.tick(c);

        let loaded = if b {
            bus.read_byte(swap_address) as u32
        } else {
            bus.read_word(swap_address)
                .rotate_right(8 * (swap_address & 0b11))
        };

        let c = bus.access_cycles(swap_address, width);
        bus.tick(c);

        if b {
            bus.write_byte(swap_address, rm_val as u8);
        } else {
            bus.write_word(swap_address, rm_val);
        }

        self.reg[rd] = loaded;

        bus.tick(1);
    }
}

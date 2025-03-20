use super::cpu::CPU;

pub fn is_single_operand(opcode: usize) -> bool {
    matches!(opcode, 0b1101 | 0b1111)
}

pub fn is_test(opcode: usize) -> bool {
    matches!(opcode, 0b1000 | 0b1001 | 0b1010 | 0b1011)
}

impl CPU {
    pub fn AND(&mut self, op: u32, op2: u32) -> u32 {
        op & op2
    }

    pub fn EOR(&mut self, op: u32, op2: u32) -> u32 {
        op ^ op2
    }

    pub fn SUB(&mut self, op: u32, op2: u32, update_cpsr: bool) -> u32 {
        let res = op.wrapping_sub(op2);

        if update_cpsr {
            self.cpsr.c_condition_bit = op2 > op;
            self.cpsr.v_condition_bit = (op as i32).overflowing_sub(op2 as i32).1;
        }
        res
    }

    pub fn ADD(&mut self, op: u32, op2: u32, update_cpsr: bool) ->  u32 {
        let res = op.wrapping_add(op2);

        if update_cpsr {
            self.cpsr.c_condition_bit = (op as u64 + op2 as u64) > u32::MAX as u64;
            self.cpsr.v_condition_bit = (op as i32).overflowing_add(op2 as i32).1;
        }
        res
    }

    pub fn ADC(&mut self, op: u32, op2: u32, update_cpsr: bool, carry: bool) -> u32 {
        let res = op.wrapping_add(op2).wrapping_add(carry as u32);

        if update_cpsr {
            self.cpsr.c_condition_bit = (op as u64 + op2 as u64 + carry as u64) > u32::MAX as u64;
            self.cpsr.v_condition_bit = (op as i32).overflowing_add(op2 as i32).0.overflowing_add(carry as i32).1;
        }
        res
    }

    pub fn SBC(&mut self, op: u32, op2: u32, update_cpsr: bool, carry: bool) -> u32 {
        let res = op.wrapping_sub(op2).wrapping_sub(1 - carry as u32);

        if update_cpsr {
            self.cpsr.c_condition_bit = (op as u64 - op2 as u64 + carry as u64 - 1) >> 32 == 0;
            self.cpsr.v_condition_bit = (op as i32).overflowing_sub(op2 as i32).0.overflowing_sub(1 - carry as i32).1;
        }
        res
    }

    pub fn ORR(&mut self, op: u32, op2: u32) -> u32 {
        op | op2
    }

    pub fn MOV(&mut self, op: u32) -> u32 {
        op
    }

    pub fn BIC(&mut self, op: u32, op2: u32) -> u32 {
        op & !op2
    }

    pub fn MVN(&mut self, op: u32) -> u32 {
        !op
    }
}
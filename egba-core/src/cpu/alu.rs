use super::cpu::CPU;

pub fn is_single_operand(opcode: usize) -> bool {
    matches!(opcode, 0b1101 | 0b1111)
}

pub fn is_test(opcode: usize) -> bool {
    matches!(opcode, 0b1000..=0b1011)
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
            self.cpsr.c_condition_bit = op >= op2;
            self.cpsr.v_condition_bit = (op as i32).overflowing_sub(op2 as i32).1;
        }
        res
    }

    pub fn ADD(&mut self, op: u32, op2: u32, update_cpsr: bool) -> u32 {
        let res = op.wrapping_add(op2);

        if update_cpsr {
            self.cpsr.c_condition_bit = (op as u64 + op2 as u64) > u32::MAX as u64;
            self.cpsr.v_condition_bit = (op as i32).overflowing_add(op2 as i32).1;
        }
        res
    }

    pub fn ADC(&mut self, op: u32, op2: u32, update_cpsr: bool, carry: bool) -> u32 {
        let cin = carry as u64;
        let sum = op as u64 + op2 as u64 + cin;
        let res = sum as u32;

        if update_cpsr {
            self.cpsr.c_condition_bit = sum > u32::MAX as u64;
            self.cpsr.v_condition_bit =
                (((op ^ res) & (op2 ^ res)) >> 31) & 1 == 1;
        }
        res
    }

    pub fn SBC(&mut self, op: u32, op2: u32, update_cpsr: bool, carry: bool) -> u32 {
        let nb = (!carry) as u64;
        let sum = op as u64 + (!op2) as u64 + (1 - nb);
        let res = sum as u32;

        if update_cpsr {
            self.cpsr.c_condition_bit = sum > u32::MAX as u64;
            self.cpsr.v_condition_bit =
                (((op ^ op2) & (op ^ res)) >> 31) & 1 == 1;
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

use bit::BitIndex;
use bitmatch::bitmatch;
use egba_core::{bit_r, cpu::{ShiftType, alu::is_test, alu::is_single_operand}};

#[bitmatch]
pub fn arm_decode(instr: u32) -> String {
    #[bitmatch]
    match bit_r!(instr, 0..28) {
        //B_BL
        "101?_????_????_????_????_????_????" => {
            let l = if instr.bit(24) { "L" } else { "" };
            format!("B{l} {}", ((bit_r!(instr, 0..24) as i32) << 8) >> 6)
        },
        //MRS
        "0001_0?00_1111_????_0000_0000_0000" => {
            let rd = bit_r!(instr, 12..16);
            let psr = if instr.bit(22) { "SPSR" } else { "CPSR" };
            format!("MRS R{:02}, {psr}", rd)
        },
        //MSR
        "00?1_0?10_100?_1111_????_????_????" => {
            let psr = if instr.bit(22) { "SPSR" } else { "CPSR" };
            let op = if instr.bit(25) {
                let imm = bit_r!(instr, 0..8) as u32;
                let rot = 2 * bit_r!(instr, 8..12) as u32;
                &format!("#{}", imm.rotate_right(rot))
            }
            else {
                &format!("R{:02}", bit_r!(instr, 0..4))
            };
            let f = if !instr.bit(16) { "_flg" } else { "" };
            format!("MSR {psr}{f}, {op}")
        },
        //SWP
        "0001_0?00_????_????_0000_1001_????" => {
            let b = if instr.bit(22) { "B" } else { "" };
            let rd = &format!("R{:02}", bit_r!(instr, 12..16));
            let rm = &format!("R{:02}", bit_r!(instr, 0..4));
            let rn = &format!("R{:02}", bit_r!(instr, 16..20));
            format!("SWP{b} {rd}, {rm}, [{rn}]")
        },
        //Data Processing
        "00??_????_????_????_????_????_????" => {
            let opcode_bits = bit_r!(instr, 21..25);
            let opcode = match opcode_bits {
                0b0000 => "AND",
                0b0001 => "EOR",
                0b0010 => "SUB",
                0b0011 => "RSB",
                0b0100 => "ADD",
                0b0101 => "ADC",
                0b0110 => "SBC",
                0b0111 => "RSC",
                0b1000 => "TST",
                0b1001 => "TEQ",
                0b1010 => "CMP",
                0b1011 => "CMN",
                0b1100 => "ORR",
                0b1101 => "MOV",
                0b1110 => "BIC",
                0b1111 => "MVN",
                _ => unreachable!()
            };

            let s = if instr.bit(20) { "S" } else { "" };
            let rd = if !is_test(opcode_bits) { &format!("R{:02}", bit_r!(instr, 12..16)) } else { "" };
            let rn = if !is_single_operand(opcode_bits) { &format!("R{:02}", bit_r!(instr, 16..20)) } else { "" };
            let op2 = if instr.bit(25) {
                let imm = bit_r!(instr, 0..8) as u32;
                let rot = 2 * bit_r!(instr, 8..12) as u8;
                &format!("#{}", imm.rotate_right(rot as u32))
            }
            else {
                let rm = format!("R{:02}", bit_r!(instr, 0..4));
                let shift_type = ShiftType::from_bits(bit_r!(instr, 5..7));
                let shift = if instr.bit(4) { format!("R{:02}", bit_r!(instr, 8..12)) } else { format!("#{}", bit_r!(instr, 7..12)) };
                &format!("{rm}, {shift_type} {shift}")
            };
            let operands = [rd, rn, op2].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(", ");
            format!("{opcode}{s} {operands}")
        },
        //LDM_STM
        "100?_????_????_????_????_????_????" => {
            let opcode = if instr.bit(20) { "LDM" } else { "STM" };
            let p = if instr.bit(24) { "B" } else { "A" };
            let u = if instr.bit(23) { "I" } else { "D" };
            let s = if instr.bit(22) { "^" } else { "" };
            let w = if instr.bit(21) { "!" } else { "" };
            let rlist = (0..16).filter(|&i| bit_r!(instr, 0..16) & (1 << i) != 0).map(|i| format!("R{:02}", i)).collect::<Vec<_>>().join(", ");
            format!("{opcode}{u}{p} R{:02}{w}, {{{rlist}}}{s}", bit_r!(instr, 16..20))
        },
        //LDR_STR
        "01??_????_????_????_????_????_????" => {
            let opcode = if instr.bit(20) { "LDR" } else { "STR" };
            let u = if instr.bit(23) { "+" } else { "-" };
            let b = if instr.bit(22) { "B" } else { "" };
            let t = if !instr.bit(24) && instr.bit(21) { "T" } else { "" };
            let w = if instr.bit(21) { "!" } else { "" };
            let p = if instr.bit(24) { ", " } else { "], " };
            let exp = if instr.bit(25) {
                &format!("#{}", bit_r!(instr, 0..12))
            }
            else {
                let rm = format!("R{:02}", bit_r!(instr, 0..4));
                let shift_type = ShiftType::from_bits(bit_r!(instr, 5..7));
                let shift = if instr.bit(4) { format!("R{:02}", bit_r!(instr, 8..12)) } else { format!("#{}", bit_r!(instr, 7..12)) };
                &format!("{rm}, {shift_type} {shift}")
            };
            let rd = format!("R{:02}", bit_r!(instr, 12..16));
            let rn = format!("R{:02}", bit_r!(instr, 16..20));

            let pre = if instr.bit(24) { "]" } else { "" };
            format!("{opcode}{b}{t} {rd}, [{rn}{p}{u}{exp}{pre}{w}")
        },
        //LDRH_LDRSH_LDRSB_STRH
        "000?_????_????_????_????_1??1_????" => {
            format!("LDRH/LDRSH/LDRSB/STRH ????")
        },
        //SWI
        "1111_????_????_????_????_????_????" => "SWI".to_string(),
        _ => "????".to_string()
    }
}

#[bitmatch]
pub fn thumb_decode(instr: u32) -> String {
    #[bitmatch]
    match bit_r!(instr, 0..16) {
        //2 
        "0001_11??_????_????" => {
            let opcode = if instr.bit(9) { "SUB" } else { "ADD" };
            let rd = format!("R{:02}", bit_r!(instr, 0..3));
            let rs = format!("R{:02}", bit_r!(instr, 3..6));
            let offset = if instr.bit(10) { format!("#{}", bit_r!(instr, 6..9)) } else { format!("R{:02}", bit_r!(instr, 6..9)) };
            format!("{opcode} {rd}, {rs}, {offset}")
        },
        //1
        "000?_????_????_????" => {
            let opcode = match bit_r!(instr, 11..13) {
                0 => "LSL",
                1 => "LSR",
                2 => "ASR",
                _ => unreachable!()
            };
            let rd = format!("R{:02}", bit_r!(instr, 0..3));
            let rs = format!("R{:02}", bit_r!(instr, 3..6));
            let offset = format!("#{}", bit_r!(instr, 6..11));
            format!("{opcode} {rd}, {rs}, {offset}")
        },
        //3
        "001?_????_????_????" => {
            let opcode = match bit_r!(instr, 11..13) {
                0 => "MOV",
                1 => "CMP",
                2 => "ADD",
                3 => "SUB",
                _ => unreachable!()
            };
            let rd = format!("R{:02}", bit_r!(instr, 8..11));
            let offset = format!("#{}", bit_r!(instr, 0..8));
            format!("{opcode} {rd}, {offset}")
        },
        //4
        "0100_00??_????_????" => {
            let opcode = match bit_r!(instr, 6..10) {
                0b0000 => "AND",
                0b0001 => "EOR",
                0b0010 => "LSL",
                0b0011 => "LSR",
                0b0100 => "ASR",
                0b0101 => "ADC",
                0b0110 => "SBC",
                0b0111 => "ROR",
                0b1000 => "TST",
                0b1001 => "NEQ",
                0b1010 => "CMP",
                0b1011 => "CMN",
                0b1100 => "ORR",
                0b1101 => "MUL",
                0b1110 => "BIC",
                0b1111 => "MVN",
                _ => unreachable!()
            };
            let rd = format!("R{:02}", bit_r!(instr, 0..3));
            let rs = format!("R{:02}", bit_r!(instr, 3..6));
            format!("{opcode} {rd}, {rs}")
        }
        //5 
        "0100_01??_????_????" => {
            let opcode = match bit_r!(instr, 8..10) {
                0 => "ADD",
                1 => "CMP",
                2 => "MOV",
                3 => "BX",
                _ => unreachable!()
            };
            let rd = format!("R{:02}, ", bit_r!(instr, 0..3) + (instr.bit(7) as usize) * 8);
            let rs = format!("R{:02}", bit_r!(instr, 3..6) + (instr.bit(6) as usize) * 8);
            format!("{opcode} {rd}{rs}")
        },
        //6
        "0100_1???_????_????" => {
            let rd = format!("R{:02}", bit_r!(instr, 8..11));
            let imm = format!("#{}", bit_r!(instr, 0..8) << 2);
            format!("LDR {rd}, [PC, {imm}]")
        },
        //10
        "1000_????_????_????" => {
            let opcode = if instr.bit(11) { "LDRH" } else { "STRH" };
            let rd = format!("R{:02}", bit_r!(instr, 0..3));
            let rb = format!("R{:02}", bit_r!(instr, 3..6));
            let imm = format!("#{}", bit_r!(instr, 6..11) << 1);
            format!("{opcode} {rd}, [{rb}, {imm}]")
        },
        //18
        "1110_0???_????_????" => {
            format!("B {}", (bit_r!(instr, 0..11) as i32) << 1)
        },
        //17
        "1101_1111_????_????" => {
            format!("SWI {}", bit_r!(instr, 0..8))
        },
        _ => "????".to_string()
    }
}
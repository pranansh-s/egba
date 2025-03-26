use bit::BitIndex;
use bitmatch::bitmatch;

use egba_core::{bit_r, cpu::{ShiftType, alu::is_test, alu::is_single_operand}};

use crate::{bit_check, format_reg};

#[bitmatch]
pub fn arm_decode(instr: u32) -> String {
    #[bitmatch]
    match bit_r!(instr, 0..28) {
        //BX 
        "0001_0010_1111_1111_1111_0001_????" => {
            let rn = &format_reg!(instr, 0..4);
            format!("BX {rn}")
        },
        //B_BL
        "101?_????_????_????_????_????_????" => {
            let l = bit_check!(instr, 24, "L", "");
            format!("B{l} {}", ((bit_r!(instr, 0..24) as i32) << 8) >> 6)
        },
        //MUL_MLA
        "0000_00??_????_????_????_1001_????" => {
            let opcode = bit_check!(instr, 21, "MLA", "MUL");
            let s = bit_check!(instr, 20, "S", "");
            let rd = &format_reg!(instr, 16..20);
            let rm = &format_reg!(instr, 0..4);
            let rs = &format_reg!(instr, 8..12);
            let rn = bit_check!(instr, 21, &format!(", R{:02}", bit_r!(instr, 12..16)), "");
            format!("{opcode}{s} {rd}, {rm}, {rs}{rn}")
        },
        //UMULL_UMLAL_SMULL_SMLAL
        "0000_1???_????_????_????_1001_????" => {
            let opcode = match(instr.bit(22), instr.bit(21)) {
                (false, false) => "UMULL",
                (false, true) => "UMLAL",
                (true, false) => "SMULL",
                (true, true) => "SMLAL",
            };
            let s = bit_check!(instr, 20, "S", "");
            let rd_lo = &format_reg!(instr, 12..16);
            let rd_hi = &format_reg!(instr, 16..20);
            let rm = &format_reg!(instr, 0..4);
            let rs = &format_reg!(instr, 8..12);
            format!("{opcode}{s} {rd_lo}, {rd_hi}, {rm}, {rs}")
        },
        //MRS
        "0001_0?00_1111_????_0000_0000_0000" => {
            let rd = &format_reg!(instr, 12..16);
            let psr = bit_check!(instr, 22, "SPSR", "CPSR");
            format!("MRS {rd}, {psr}")
        },
        //MSR
        "00?1_0?10_100?_1111_????_????_????" => {
            let psr = bit_check!(instr, 22, "SPSR", "CPSR");
            let op = if instr.bit(25) {
                let imm = bit_r!(instr, 0..8) as u32;
                let rot = 2 * bit_r!(instr, 8..12) as u32;
                &format!("#{}", imm.rotate_right(rot))
            }
            else {
                &format_reg!(instr, 0..4)
            };
            let f = bit_check!(instr, 16, "", "_flg");
            format!("MSR {psr}{f}, {op}")
        },
        //LDM_STM
        "100?_????_????_????_????_????_????" => {
            let opcode = bit_check!(instr, 20, "LDM", "STM");
            let p = bit_check!(instr, 24, "B", "A");
            let u = bit_check!(instr, 23, "I", "D");
            let s = bit_check!(instr, 22, "^", "");
            let w = bit_check!(instr, 21, "!", "");
            let rlist = (0..16).filter(|&i| bit_r!(instr, 0..16) & (1 << i) != 0).map(|i| format!("R{:02}", i)).collect::<Vec<_>>().join(", ");
            format!("{opcode}{u}{p} R{:02}{w}, {{{rlist}}}{s}", bit_r!(instr, 16..20))
        },
        //SWP
        "0001_0?00_????_????_0000_1001_????" => {
            let b = bit_check!(instr, 22, "B", "");
            let rd = &format_reg!(instr, 12..16);
            let rm = &format_reg!(instr, 0..4);
            let rn = &format_reg!(instr, 16..20);
            format!("SWP{b} {rd}, {rm}, [{rn}]")
        },
        //LDRH_LDRSH_LDRSB_STRH
        "000?_????_????_????_????_1??1_????" => {
            let opcode = bit_check!(instr, 20, "LDR", "STM");
            let sh = match(instr.bit(6), instr.bit(5)) {
                (false, true) => "H",
                (true, false) => "SB",
                (true, true) => "SH",
                (false, false) => unreachable!()
            };
            let rd = &format_reg!(instr, 12..16);
            let rn = &format_reg!(instr, 16..20);
            let u = bit_check!(instr, 23, "+", "-");
            let exp = bit_check!(instr, 22, &format!("#{}", (bit_r!(instr, 0..4) << 4 | bit_r!(instr, 8..12)) as u32), &format_reg!(instr, 0..4));
            let w = bit_check!(instr, 21, "!", "");

            let post = bit_check!(instr, 24, ", ", "], ");
            let pre = bit_check!(instr, 24, "]", "");
            format!("{opcode}{sh} {rd}, [{rn}{post}{u}{exp}{pre}{w}")
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

            let s = bit_check!(instr, 20, "S", "");
            let rd = if !is_test(opcode_bits) { &format_reg!(instr, 12..16) } else { "" };
            let rn = if !is_single_operand(opcode_bits) { &format_reg!(instr, 16..20) } else { "" };
            let op2 = if instr.bit(25) {
                let imm = bit_r!(instr, 0..8) as u32;
                let rot = 2 * bit_r!(instr, 8..12) as u8;
                &format!("#{}", imm.rotate_right(rot as u32))
            }
            else {
                let rm = &format_reg!(instr, 0..4);
                let shift_type = ShiftType::from_bits(bit_r!(instr, 5..7));
                let shift = bit_check!(instr, 4, &format_reg!(instr, 8..12), &format!("#{}", bit_r!(instr, 7..12)));
                &format!("{rm}, {shift_type} {shift}")
            };
            let operands = [rd, rn, op2].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(", ");
            format!("{opcode}{s} {operands}")
        },
        //LDR_STR
        "01??_????_????_????_????_????_????" => {
            let opcode = bit_check!(instr, 20, "LDR", "STR");
            let u = bit_check!(instr, 23, "+", "-");
            let b = bit_check!(instr, 22, "B", "");
            let t = if !instr.bit(24) && instr.bit(21) { "T" } else { "" };
            let w = bit_check!(instr, 21, "!", "");
            let exp = if instr.bit(25) {
                &format!("#{}", bit_r!(instr, 0..12))
            }
            else {
                let rm = &format_reg!(instr, 0..4);
                let shift_type = ShiftType::from_bits(bit_r!(instr, 5..7));
                let shift = bit_check!(instr, 4, &format_reg!(instr, 8..12), &format!("#{}", bit_r!(instr, 7..12)));
                &format!("{rm}, {shift_type} {shift}")
            };
            let rd = &format_reg!(instr, 12..16);
            let rn = &format_reg!(instr, 16..20);
            
            let post = bit_check!(instr, 24, ", ", "], ");
            let pre = bit_check!(instr, 24, "]", "");
            format!("{opcode}{b}{t} {rd}, [{rn}{post}{u}{exp}{pre}{w}")
        },
        //SWI
        "1111_????_????_????_????_????_????" => "SWI".to_string(),
        _ => "????".to_string()
    }
}
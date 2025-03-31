use bit::BitIndex;
use bitmatch::bitmatch;

use egba_core::bit_r;

use crate::{bit_check, format_reg};

#[bitmatch]
pub fn thumb_decode(instr: u32) -> String {
    #[bitmatch]
    match bit_r!(instr, 0..16) {
        //2 
        "0001_1???_????_????" => {
            let opcode = bit_check!(instr, 9, "SUB", "ADD");
            let rd = &format_reg!(instr, 0..3);
            let rs = &format_reg!(instr, 3..6);
            let offset = bit_check!(instr, 10, &format!("#{}", bit_r!(instr, 6..9)), &format_reg!(instr, 6..9));
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
            let rd = &format_reg!(instr, 0..3);
            let rs = &format_reg!(instr, 3..6);
            let offset = &format!("#{}", bit_r!(instr, 6..11));
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
            let rd = &format_reg!(instr, 8..11);
            let offset = &format!("#{}", bit_r!(instr, 0..8));
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
            let rd = &format_reg!(instr, 0..3);
            let rs = &format_reg!(instr, 3..6);
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
            let rd = if bit_r!(instr, 8..10) == 3 { "" } else { &format!("R{:02}, ", bit_r!(instr, 0..3) + (instr.bit(7) as usize) * 8) };
            let rs = &format!("R{:02}", bit_r!(instr, 3..6) + (instr.bit(6) as usize) * 8);
            format!("{opcode} {rd}{rs}")
        },
        //6
        "0100_1???_????_????" => {
            let rd =  &format_reg!(instr, 8..11);
            let imm = &format!("#{}", bit_r!(instr, 0..8) << 2);
            format!("LDR {rd}, [PC, {imm}]")
        },
        //7
        "0101_??0?_????_????" => {
            let opcode = bit_check!(instr, 11, "LDR", "STR");
            let b = bit_check!(instr, 10, "B", "");
            let rd = format_reg!(instr, 0..3);
            let rb = format_reg!(instr, 3..6);
            let ro = format_reg!(instr, 6..9);
            format!("{opcode}{b} {rd}, [{rb}, {ro}]")
        },
        //8
        "0101_??1?_????_????" => {
            let opcode = match (instr.bit(10), instr.bit(11)) {
                (false, false) => "STRH",
                (false, true) => "LDRH",
                (true, false) => "LDSB",
                (true, true) => "LDSH"
            };
            let rd = format_reg!(instr, 0..3);
            let rb = format_reg!(instr, 3..6);
            let ro = format_reg!(instr, 6..9);
            format!("{opcode} {rd}, [{rb}, {ro}]")
        },
        //9
        "011?_????_????_????" => {
            let opcode = bit_check!(instr, 11, "LDR", "STR");
            let b = bit_check!(instr, 12, "B", "");
            let rd =  &format_reg!(instr, 0..3);
            let rb =  &format_reg!(instr, 3..6);
            let offset = if instr.bit(12) { bit_r!(instr, 6..11) } else { bit_r!(instr, 6..11) << 2 };
            let imm = &format!("#{}", offset);
            format!("{opcode}{b} {rd}, [{rb}, {imm}]")
        },
        //10
        "1000_????_????_????" => {
            let opcode = bit_check!(instr, 11, "LDRH", "STRH");
            let rd =  &format_reg!(instr, 0..3);
            let rb =  &format_reg!(instr, 3..6);
            let imm = &format!("#{}", bit_r!(instr, 6..11) << 1);
            format!("{opcode} {rd}, [{rb}, {imm}]")
        },
        //11
        "1001_????_????_????" => {
            let opcode = bit_check!(instr, 11, "LDR", "STR");
            let rd =  &format_reg!(instr, 0..8);
            let imm = &format!("#{}", bit_r!(instr, 0..8) << 2);
            format!("{opcode} {rd}, [SP, {imm}]")
        },
        //12
        "1010_????_????_????" => {
            let rd =  &format_reg!(instr, 8..11);
            let rb = bit_check!(instr, 11, "SP", "PC");
            let imm = &format!("#{}", bit_r!(instr, 0..8) << 2);
            format!("ADD {rd}, {rb}, {imm}")
        },
        //13
        "1011_0000_????_????" => {
            let sign = bit_check!(instr, 7, "-", "");
            let imm = &format!("#{sign}{}", bit_r!(instr, 6..11) << 2);
            format!("ADD SP, {imm}")
        },
        //14
        "1011_?10?_????_????" => {
            let opcode = bit_check!(instr, 11, "POP", "PUSH");
            let rlist = (0..8).filter(|&i| bit_r!(instr, 0..8) & (1 << i) != 0).map(|i| format!("R{:02}", i)).collect::<Vec<_>>().join(", ");
            let other = if instr.bit(8) && instr.bit(11) { ", PC" } else { bit_check!(instr, 8, ", LR", "") };
            format!("{opcode} {{{rlist}{other}}}")
        },
        //15
        "1100_????_????_????" => {
            let opcode = bit_check!(instr, 11, "LDMIA", "STMIA");
            let rb = &format_reg!(instr, 8..11);
            let rlist = (0..8).filter(|&i| bit_r!(instr, 0..8) & (1 << i) != 0).map(|i| format!("R{:02}", i)).collect::<Vec<_>>().join(", ");
            format!("{opcode} {rb}, {{{rlist}}}")
        },
        //17
        "1101_1111_????_????" => {
            format!("SWI {}", bit_r!(instr, 0..8))
        },
        //16
        "1101_????_????_????" => {
            let cond = match bit_r!(instr, 8..12) {
                0b0000 => "EQ",
                0b0001 => "NE",
                0b0010 => "CS",
                0b0011 => "CC",
                0b0100 => "MI",
                0b0101 => "PL",
                0b0110 => "VS",
                0b0111 => "VC",
                0b1000 => "HI",
                0b1001 => "LS",
                0b1010 => "GE",
                0b1011 => "LT",
                0b1100 => "GT",
                0b1101 => "LE",
                0b1110 => "und?",
                _ => unreachable!()
            };
            let offset = ((bit_r!(instr, 0..8) as i8) as i32) << 1;
            format!("B{cond} {offset}")
        },
        //18
        "1110_0???_????_????" => {
            format!("B {}", ((bit_r!(instr, 0..11) << 21) as i32) >> 20)
        },
        //19
        "1111_????_????_????" => {
            let offset = bit_r!(instr, 0..11);
            let label = if instr.bit(11) { 
                &format!("{}", (offset as u32) << 1)
            }
            else {
                &format!("{}", ((offset << 21) as i32) >> 9)
            };
            format!("BL {label}")
        },
        _ => "????".to_string()
    }
}
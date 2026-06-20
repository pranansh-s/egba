#[cfg(test)]
mod tests {
    use crate::bus::Bus;
    use crate::cpu::cpu::{CPU, LR_INDEX, PC_INDEX, SP_INDEX};
    use crate::cpu::exception::Exception;
    use crate::cpu::psr::{OperatingMode, OperatingState, ProgramStatusRegister};

    /// A simple flat memory bus for testing
    struct TestBus {
        mem: Vec<u8>,
        ticks: u32,
    }

    impl TestBus {
        fn new(size: usize) -> Self {
            Self {
                mem: vec![0; size],
                ticks: 0,
            }
        }

        fn write_word_at(&mut self, addr: u32, val: u32) {
            let a = addr as usize;
            self.mem[a] = val as u8;
            self.mem[a + 1] = (val >> 8) as u8;
            self.mem[a + 2] = (val >> 16) as u8;
            self.mem[a + 3] = (val >> 24) as u8;
        }

        fn write_hword_at(&mut self, addr: u32, val: u16) {
            let a = addr as usize;
            self.mem[a] = val as u8;
            self.mem[a + 1] = (val >> 8) as u8;
        }
    }

    impl Bus for TestBus {
        fn read_byte(&self, addr: u32) -> u8 {
            let a = (addr as usize) % self.mem.len();
            self.mem[a]
        }

        fn write_byte(&mut self, addr: u32, value: u8) {
            let a = (addr as usize) % self.mem.len();
            self.mem[a] = value;
        }

        fn tick(&mut self, n: u32) {
            self.ticks += n;
        }
    }

    // =========================================================================
    // ALU Tests
    // =========================================================================

    #[test]
    fn alu_add_basic() {
        let mut cpu = CPU::new();
        let result = cpu.ADD(10, 20, true);
        assert_eq!(result, 30);
        assert!(!cpu.cpsr.c_condition_bit);
        assert!(!cpu.cpsr.v_condition_bit);
    }

    #[test]
    fn alu_add_overflow() {
        let mut cpu = CPU::new();
        let result = cpu.ADD(0x7FFF_FFFF, 1, true);
        assert_eq!(result, 0x8000_0000);
        assert!(!cpu.cpsr.c_condition_bit); // No unsigned carry
        assert!(cpu.cpsr.v_condition_bit); // Signed overflow
    }

    #[test]
    fn alu_add_carry() {
        let mut cpu = CPU::new();
        let result = cpu.ADD(0xFFFF_FFFF, 1, true);
        assert_eq!(result, 0);
        assert!(cpu.cpsr.c_condition_bit); // Unsigned carry
    }

    #[test]
    fn alu_sub_basic() {
        let mut cpu = CPU::new();
        let result = cpu.SUB(30, 10, true);
        assert_eq!(result, 20);
        assert!(cpu.cpsr.c_condition_bit); // No borrow = carry set
        assert!(!cpu.cpsr.v_condition_bit);
    }

    #[test]
    fn alu_sub_borrow() {
        let mut cpu = CPU::new();
        let result = cpu.SUB(0, 1, true);
        assert_eq!(result, 0xFFFF_FFFF);
        assert!(!cpu.cpsr.c_condition_bit); // Borrow = carry clear
    }

    #[test]
    fn alu_sub_signed_overflow() {
        let mut cpu = CPU::new();
        let result = cpu.SUB(0x8000_0000, 1, true);
        assert_eq!(result, 0x7FFF_FFFF);
        assert!(cpu.cpsr.v_condition_bit); // Signed overflow
    }

    #[test]
    fn alu_adc_with_carry() {
        let mut cpu = CPU::new();
        let result = cpu.ADC(10, 20, true, true);
        assert_eq!(result, 31); // 10 + 20 + 1
        assert!(!cpu.cpsr.c_condition_bit);
    }

    #[test]
    fn alu_adc_overflow_with_carry() {
        let mut cpu = CPU::new();
        let result = cpu.ADC(0xFFFF_FFFF, 0, true, true);
        assert_eq!(result, 0);
        assert!(cpu.cpsr.c_condition_bit);
    }

    #[test]
    fn alu_sbc_basic() {
        let mut cpu = CPU::new();
        // SBC: op - op2 - (1 - carry)
        let result = cpu.SBC(30, 10, true, true);
        assert_eq!(result, 20); // 30 - 10 - 0
    }

    #[test]
    fn alu_sbc_with_borrow() {
        let mut cpu = CPU::new();
        // carry = false means borrow active: op - op2 - 1
        let result = cpu.SBC(30, 10, true, false);
        assert_eq!(result, 19); // 30 - 10 - 1
    }

    #[test]
    fn alu_adc_scenarios() {
        // Table-driven ADC: rows cover carry-in / unsigned overflow / signed overflow.
        let cases: [(u32, u32, bool, u32, bool, bool, &str); 6] = [
            (10, 20, false, 30, false, false, "no overflow no carry"),
            (10, 20, true,  31, false, false, "carry in adds 1"),
            (0xFFFF_FFFF, 0, true, 0, true, false, "carry in causes unsigned wrap, V false"),
            (0x7FFF_FFFF, 1, false, 0x8000_0000, false, true,
                "positive + positive crosses sign, V true"),
            (0x8000_0000, 0x8000_0000, false, 0, true, true,
                "negative + negative wraps to 0, V true"),
            (0x7FFF_FFFF, 0, true, 0x8000_0000, false, true,
                "positive + 0 + carry crosses sign, V true"),
        ];
        for (op, op2, c_in, expected_res, expected_c, expected_v, label) in cases {
            let mut cpu = CPU::new();
            let res = cpu.ADC(op, op2, true, c_in);
            assert_eq!(res, expected_res, "{label} result");
            assert_eq!(cpu.cpsr.c_condition_bit, expected_c, "{label} C flag");
            assert_eq!(cpu.cpsr.v_condition_bit, expected_v, "{label} V flag");
        }
    }

    #[test]
    fn alu_sbc_scenarios() {
        // Table-driven SBC: rows cover borrow / no-borrow / sign-overflow / op<op2.
        // Carry-in convention: carry=true means NO borrow (C=1), carry=false means borrow (C=0).
        // C-out: 1 means no borrow occurred; 0 means borrow occurred.
        // V-out: signed overflow.
        let cases: [(u32, u32, bool, u32, bool, bool, &str); 7] = [
            (30, 10, true,  20,         true,  false, "no borrow in, op>op2"),
            (30, 10, false, 19,         true,  false, "borrow in, op>op2 by margin"),
            (10, 10, false, 0xFFFF_FFFF,false, false, "borrow in, op==op2 -> result wraps, borrow out"),
            (10, 10, true,  0,          true,  false, "no borrow in, op==op2 -> 0, no borrow"),
            (5,  10, true,  0xFFFF_FFFB, false, false,"no borrow in, op<op2 -> wraps, borrow out"),
            (5,  10, false, 0xFFFF_FFFA, false, false,"borrow in, op<op2 -> wraps further, borrow out"),
            (0x8000_0000, 0x0000_0001, true, 0x7FFF_FFFF, true, true,
                "INT_MIN - 1 signed overflow"),
        ];
        for (op, op2, c_in, expected_res, expected_c, expected_v, label) in cases {
            let mut cpu = CPU::new();
            let res = cpu.SBC(op, op2, true, c_in);
            assert_eq!(res, expected_res, "{label} result");
            assert_eq!(cpu.cpsr.c_condition_bit, expected_c, "{label} C flag");
            assert_eq!(cpu.cpsr.v_condition_bit, expected_v, "{label} V flag");
        }
    }

    #[test]
    fn alu_and() {
        let mut cpu = CPU::new();
        assert_eq!(cpu.AND(0xFF00_FF00, 0x00FF_00FF), 0);
        assert_eq!(cpu.AND(0xFFFF_FFFF, 0x1234_5678), 0x1234_5678);
    }

    #[test]
    fn alu_eor() {
        let mut cpu = CPU::new();
        assert_eq!(cpu.EOR(0xFFFF_FFFF, 0xFFFF_FFFF), 0);
        assert_eq!(cpu.EOR(0x0F0F_0F0F, 0xF0F0_F0F0), 0xFFFF_FFFF);
    }

    #[test]
    fn alu_orr() {
        let mut cpu = CPU::new();
        assert_eq!(cpu.ORR(0xFF00_0000, 0x00FF_0000), 0xFFFF_0000);
    }

    #[test]
    fn alu_bic() {
        let mut cpu = CPU::new();
        assert_eq!(cpu.BIC(0xFFFF_FFFF, 0x00FF_00FF), 0xFF00_FF00);
    }

    #[test]
    fn alu_mov() {
        let mut cpu = CPU::new();
        assert_eq!(cpu.MOV(42), 42);
    }

    #[test]
    fn alu_mvn() {
        let mut cpu = CPU::new();
        assert_eq!(cpu.MVN(0), 0xFFFF_FFFF);
        assert_eq!(cpu.MVN(0xFFFF_FFFF), 0);
    }

    // =========================================================================
    // Shift Tests
    // =========================================================================

    #[test]
    fn shift_lsl_zero() {
        let mut cpu = CPU::new();
        let result = cpu.LSL(0xDEAD_BEEF, 0, true);
        assert_eq!(result, 0xDEAD_BEEF); // LSL#0 = no change
    }

    #[test]
    fn shift_lsl_basic() {
        let mut cpu = CPU::new();
        let result = cpu.LSL(1, 4, true);
        assert_eq!(result, 16);
    }

    #[test]
    fn shift_lsl_32() {
        let mut cpu = CPU::new();
        let result = cpu.LSL(1, 32, true);
        assert_eq!(result, 0);
        assert!(cpu.cpsr.c_condition_bit); // C = bit 0 of original value
    }

    #[test]
    fn shift_lsl_33() {
        let mut cpu = CPU::new();
        let result = cpu.LSL(0xFFFF_FFFF, 33, true);
        assert_eq!(result, 0);
        assert!(!cpu.cpsr.c_condition_bit);
    }

    #[test]
    fn shift_lsr_basic() {
        let mut cpu = CPU::new();
        let result = cpu.LSR(0x80, 4, true);
        assert_eq!(result, 8);
    }

    #[test]
    fn shift_lsr_zero_means_32() {
        let mut cpu = CPU::new();
        // LSR#0 in immediate encoding means LSR#32
        let result = cpu.LSR(0x8000_0000, 0, true);
        assert_eq!(result, 0);
        assert!(cpu.cpsr.c_condition_bit); // C = bit 31
    }

    #[test]
    fn shift_asr_basic() {
        let mut cpu = CPU::new();
        let result = cpu.ASR(0x8000_0000_u32, 1, true);
        assert_eq!(result, 0xC000_0000); // Sign-extended
    }

    #[test]
    fn shift_asr_zero_positive() {
        let mut cpu = CPU::new();
        let result = cpu.ASR(0x7FFF_FFFF, 0, true);
        assert_eq!(result, 0); // All positive -> 0
        assert!(!cpu.cpsr.c_condition_bit);
    }

    #[test]
    fn shift_asr_zero_negative() {
        let mut cpu = CPU::new();
        let result = cpu.ASR(0x8000_0000, 0, true);
        assert_eq!(result, 0xFFFF_FFFF); // All negative -> -1
        assert!(cpu.cpsr.c_condition_bit);
    }

    #[test]
    fn shift_ror_basic() {
        let mut cpu = CPU::new();
        let result = cpu.ROR(0x0000_0001, 1, true);
        assert_eq!(result, 0x8000_0000);
        assert!(cpu.cpsr.c_condition_bit); // C = bit 0 before rotate
    }

    #[test]
    fn shift_ror_zero_is_rrx() {
        let mut cpu = CPU::new();
        cpu.cpsr.c_condition_bit = true;
        let result = cpu.ROR(0x0000_0001, 0, true);
        // ROR#0 = RRX: (carry << 31) | (value >> 1)
        assert_eq!(result, 0x8000_0000);
        assert!(cpu.cpsr.c_condition_bit); // New C = old bit 0
    }

    #[test]
    fn shift_ror_32_is_identity() {
        let mut cpu = CPU::new();
        let result = cpu.ROR(0xDEAD_BEEF, 32, true);
        assert_eq!(result, 0xDEAD_BEEF);
        assert!(cpu.cpsr.c_condition_bit); // C = bit 31
    }

    // =========================================================================
    // NZ Flag Tests
    // =========================================================================

    #[test]
    fn set_nz_zero() {
        let mut cpu = CPU::new();
        cpu.set_NZ(0);
        assert!(cpu.cpsr.z_condition_bit);
        assert!(!cpu.cpsr.n_condition_bit);
    }

    #[test]
    fn set_nz_negative() {
        let mut cpu = CPU::new();
        cpu.set_NZ(0x8000_0000);
        assert!(!cpu.cpsr.z_condition_bit);
        assert!(cpu.cpsr.n_condition_bit);
    }

    #[test]
    fn set_nz_positive() {
        let mut cpu = CPU::new();
        cpu.set_NZ(42);
        assert!(!cpu.cpsr.z_condition_bit);
        assert!(!cpu.cpsr.n_condition_bit);
    }

    // =========================================================================
    // Condition Code Tests
    // =========================================================================

    #[test]
    fn condition_eq() {
        let mut cpu = CPU::new();
        cpu.cpsr.z_condition_bit = true;
        assert!(cpu.condition_check(0b0000)); // EQ
        assert!(!cpu.condition_check(0b0001)); // NE
    }

    #[test]
    fn condition_cs_cc() {
        let mut cpu = CPU::new();
        cpu.cpsr.c_condition_bit = true;
        assert!(cpu.condition_check(0b0010)); // CS
        assert!(!cpu.condition_check(0b0011)); // CC
    }

    #[test]
    fn condition_mi_pl() {
        let mut cpu = CPU::new();
        cpu.cpsr.n_condition_bit = true;
        assert!(cpu.condition_check(0b0100)); // MI
        assert!(!cpu.condition_check(0b0101)); // PL
    }

    #[test]
    fn condition_vs_vc() {
        let mut cpu = CPU::new();
        cpu.cpsr.v_condition_bit = true;
        assert!(cpu.condition_check(0b0110)); // VS
        assert!(!cpu.condition_check(0b0111)); // VC
    }

    #[test]
    fn condition_hi() {
        let mut cpu = CPU::new();
        cpu.cpsr.c_condition_bit = true;
        cpu.cpsr.z_condition_bit = false;
        assert!(cpu.condition_check(0b1000)); // HI: C && !Z
    }

    #[test]
    fn condition_ls() {
        let mut cpu = CPU::new();
        cpu.cpsr.c_condition_bit = false;
        cpu.cpsr.z_condition_bit = false;
        assert!(cpu.condition_check(0b1001)); // LS: !C || Z
    }

    #[test]
    fn condition_ge() {
        let mut cpu = CPU::new();
        cpu.cpsr.n_condition_bit = true;
        cpu.cpsr.v_condition_bit = true;
        assert!(cpu.condition_check(0b1010)); // GE: N == V
    }

    #[test]
    fn condition_lt() {
        let mut cpu = CPU::new();
        cpu.cpsr.n_condition_bit = true;
        cpu.cpsr.v_condition_bit = false;
        assert!(cpu.condition_check(0b1011)); // LT: N != V
    }

    #[test]
    fn condition_gt() {
        let mut cpu = CPU::new();
        cpu.cpsr.z_condition_bit = false;
        cpu.cpsr.n_condition_bit = false;
        cpu.cpsr.v_condition_bit = false;
        assert!(cpu.condition_check(0b1100)); // GT: !Z && N==V
    }

    #[test]
    fn condition_le() {
        let mut cpu = CPU::new();
        cpu.cpsr.z_condition_bit = true;
        assert!(cpu.condition_check(0b1101)); // LE: Z || N!=V
    }

    #[test]
    fn condition_al() {
        let cpu = CPU::new();
        assert!(cpu.condition_check(0b1110)); // AL: always
    }

    // =========================================================================
    // PSR Conversion Tests
    // =========================================================================

    #[test]
    fn psr_round_trip() {
        let psr = ProgramStatusRegister {
            n_condition_bit: true,
            z_condition_bit: false,
            c_condition_bit: true,
            v_condition_bit: false,
            irq_disable_bit: true,
            fiq_disable_bit: false,
            operating_state: OperatingState::THUMB,
            mode: OperatingMode::irq,
        };

        let val: u32 = psr.into();
        let restored: ProgramStatusRegister = val.into();

        assert_eq!(restored.n_condition_bit, true);
        assert_eq!(restored.z_condition_bit, false);
        assert_eq!(restored.c_condition_bit, true);
        assert_eq!(restored.v_condition_bit, false);
        assert_eq!(restored.irq_disable_bit, true);
        assert_eq!(restored.fiq_disable_bit, false);
        assert_eq!(restored.operating_state, OperatingState::THUMB);
        assert_eq!(restored.mode, OperatingMode::irq);
    }

    #[test]
    fn psr_invalid_mode_no_panic() {
        // Should not panic — falls back to sys mode
        let mode = OperatingMode::from(0b00000);
        assert_eq!(mode, OperatingMode::sys);
    }

    // =========================================================================
    // Bank Switching Tests
    // =========================================================================

    #[test]
    fn bank_switch_preserves_registers() {
        let mut cpu = CPU::new();
        cpu.set_mode(OperatingMode::svc);
        cpu.reg[SP_INDEX] = 0x1000;
        cpu.reg[LR_INDEX] = 0x2000;

        // Switch to IRQ mode
        cpu.set_mode(OperatingMode::irq);
        // SVC registers should be banked
        assert_ne!(cpu.reg[SP_INDEX], 0x1000);
        cpu.reg[SP_INDEX] = 0x3000;
        cpu.reg[LR_INDEX] = 0x4000;

        // Switch back to SVC
        cpu.set_mode(OperatingMode::svc);
        assert_eq!(cpu.reg[SP_INDEX], 0x1000);
        assert_eq!(cpu.reg[LR_INDEX], 0x2000);

        // Switch back to IRQ, verify its values persisted
        cpu.set_mode(OperatingMode::irq);
        assert_eq!(cpu.reg[SP_INDEX], 0x3000);
        assert_eq!(cpu.reg[LR_INDEX], 0x4000);
    }

    #[test]
    fn bank_switch_fiq_banks_r8_r12() {
        let mut cpu = CPU::new();
        cpu.set_mode(OperatingMode::usr);

        // Set R8-R12 in user mode
        for i in 8..=12 {
            cpu.reg[i] = (i as u32) * 100;
        }

        // Switch to FIQ — R8-R12 should be banked
        cpu.set_mode(OperatingMode::fiq);
        for i in 8..=12 {
            assert_eq!(cpu.reg[i], 0); // FIQ bank starts at 0
            cpu.reg[i] = (i as u32) * 200;
        }

        // Switch back to user — original values should be restored
        cpu.set_mode(OperatingMode::usr);
        for i in 8..=12 {
            assert_eq!(cpu.reg[i], (i as u32) * 100);
        }
    }

    // =========================================================================
    // Exception Entry Tests
    // =========================================================================

    #[test]
    fn exception_swi_enters_svc_mode() {
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x10000);
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.cpsr.irq_disable_bit = false;
        cpu.reg[PC_INDEX] = 0x100;

        // Put NOPs at exception vector 0x08 (SWI vector)
        bus.write_word_at(0x08, 0xE1A00000); // MOV R0, R0 (NOP)
        bus.write_word_at(0x0C, 0xE1A00000);

        cpu.setup_exception(Exception::SoftwareInterrupt, 0x104);

        assert_eq!(cpu.cpsr.mode, OperatingMode::svc);
        assert_eq!(cpu.cpsr.operating_state, OperatingState::ARM);
        assert!(cpu.cpsr.irq_disable_bit);
        assert_eq!(cpu.reg[PC_INDEX], 0x08); // SWI vector
    }

    #[test]
    fn exception_irq_masked() {
        let mut cpu = CPU::new();
        cpu.cpsr.irq_disable_bit = true; // IRQ masked
        let accepted = cpu.setup_exception(Exception::IRQ, 0x100);
        assert!(!accepted);
    }

    #[test]
    fn exception_irq_accepted() {
        let mut cpu = CPU::new();
        cpu.cpsr.irq_disable_bit = false;
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.reg[PC_INDEX] = 0x200;

        let accepted = cpu.setup_exception(Exception::IRQ, 0x204);
        assert!(accepted);
        assert_eq!(cpu.cpsr.mode, OperatingMode::irq);
        assert_eq!(cpu.reg[PC_INDEX], 0x18); // IRQ vector
        assert!(cpu.cpsr.irq_disable_bit);
    }

    #[test]
    fn exception_saves_spsr() {
        let mut cpu = CPU::new();
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.cpsr.irq_disable_bit = false;
        cpu.cpsr.n_condition_bit = true;
        cpu.cpsr.z_condition_bit = true;
        let original_cpsr: u32 = cpu.cpsr.into();

        cpu.setup_exception(Exception::IRQ, 0x100);

        // SPSR should contain the old CPSR
        assert_eq!(cpu.spsr, original_cpsr);
    }

    #[test]
    fn exception_saves_return_address() {
        let mut cpu = CPU::new();
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.cpsr.irq_disable_bit = false;
        cpu.reg[PC_INDEX] = 0x200;

        cpu.setup_exception(Exception::IRQ, 0x204);

        // LR in IRQ mode should be the return address
        assert_eq!(cpu.reg[LR_INDEX], 0x204);
    }

    // =========================================================================
    // Pipeline Tests
    // =========================================================================

    #[test]
    fn pipeline_flush_refills_both_slots() {
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x10000);

        // Write 3 distinct NOP-like instructions at address 0
        bus.write_word_at(0x00, 0xAAAA_AAAA);
        bus.write_word_at(0x04, 0xBBBB_BBBB);
        bus.write_word_at(0x08, 0xCCCC_CCCC);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.flush_pipeline(&mut bus);

        // After flush, pipeline[1] and [2] should be filled
        // PC should have advanced by 8 (two fetches)
        assert_eq!(cpu.pipeline[1], 0xAAAA_AAAA);
        assert_eq!(cpu.pipeline[2], 0xBBBB_BBBB);
        assert_eq!(cpu.reg[PC_INDEX], 0x08);
    }

    #[test]
    fn pipeline_flush_thumb_mode() {
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x10000);

        bus.write_hword_at(0x100, 0x1234);
        bus.write_hword_at(0x102, 0x5678);

        cpu.cpsr.operating_state = OperatingState::THUMB;
        cpu.reg[PC_INDEX] = 0x100;
        cpu.flush_pipeline(&mut bus);

        assert_eq!(cpu.pipeline[1], 0x1234);
        assert_eq!(cpu.pipeline[2], 0x5678);
        assert_eq!(cpu.reg[PC_INDEX], 0x104);
    }

    #[test]
    fn arm_pc_offset() {
        let mut cpu = CPU::new();
        cpu.reg[PC_INDEX] = 0x108;
        // In ARM mode, the "current" instruction PC is PC - 8
        assert_eq!(cpu.arm_pc(), 0x100);
    }

    #[test]
    fn thumb_pc_offset() {
        let mut cpu = CPU::new();
        cpu.reg[PC_INDEX] = 0x104;
        // In Thumb mode, the "current" instruction PC is PC - 4
        assert_eq!(cpu.thumb_pc(), 0x100);
    }

    // =========================================================================
    // Pipeline / branch correctness
    // =========================================================================

    #[test]
    fn branch_then_executes_both_target_and_target_plus_4() {
        // 0x00: B +0x10   (EA00 0002 → branches to 0x10)
        // 0x10: MOV r0, #0x11
        // 0x14: MOV r1, #0x22
        // 0x18: MOV r2, #0x33
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x00, 0xEA00_0002);
        bus.write_word_at(0x04, 0xE3A0_0000); // filler so flush after init reads safely
        bus.write_word_at(0x10, 0xE3A0_0011); // MOV r0, #0x11
        bus.write_word_at(0x14, 0xE3A0_1022); // MOV r1, #0x22
        bus.write_word_at(0x18, 0xE3A0_2033); // MOV r2, #0x33

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.flush_pipeline(&mut bus);

        cpu.step(&mut bus); // executes B at 0x00 -> flush, lands at 0x10
        cpu.step(&mut bus); // should execute MOV r0,#0x11 at 0x10
        cpu.step(&mut bus); // should execute MOV r1,#0x22 at 0x14

        assert_eq!(cpu.reg[0], 0x11, "MOV r0,#0x11 at 0x10 must execute after branch");
        assert_eq!(
            cpu.reg[1], 0x22,
            "MOV r1,#0x22 at 0x14 must NOT be skipped by pipeline-fill"
        );
    }

    // =========================================================================
    // Shift-by-register Rs==0 edge case
    // =========================================================================

    #[test]
    fn arm_str_pc_stores_self_plus_12() {
        // ARM7TDMI quirk: STR R15 stores (address_of_STR + 12).
        // Since reg[PC] = self+8 during execute, that equals reg[PC] + 4.
        // E580F000 = STR R15, [R0]  (P=1,U=1,B=0,W=0,L=0,Rn=R0,Rd=R15,off=0)
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x100, 0xE580_F000);
        bus.write_word_at(0x104, 0xE3A0_0000);
        bus.write_word_at(0x108, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x100;
        cpu.reg[0] = 0x200;
        cpu.flush_pipeline(&mut bus);
        cpu.step(&mut bus);

        let stored = u32::from_le_bytes([
            bus.mem[0x200],
            bus.mem[0x201],
            bus.mem[0x202],
            bus.mem[0x203],
        ]);
        assert_eq!(stored, 0x10C, "STR PC must store self+12 per ARM7TDMI");
    }

    #[test]
    fn arm_stm_pc_stores_self_plus_12() {
        // STMIA R0!, {R15}   E880_8000
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x100, 0xE880_8000);
        bus.write_word_at(0x104, 0xE3A0_0000);
        bus.write_word_at(0x108, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x100;
        cpu.reg[0] = 0x200;
        cpu.flush_pipeline(&mut bus);
        cpu.step(&mut bus);

        let stored = u32::from_le_bytes([
            bus.mem[0x200],
            bus.mem[0x201],
            bus.mem[0x202],
            bus.mem[0x203],
        ]);
        assert_eq!(stored, 0x10C, "STM PC must store self+12 per ARM7TDMI");
    }

    #[test]
    fn shift_by_reg_zero_preserves_value_and_carry() {
        // MOVS r0, r1, LSR r2     with r1=0x7000_0000, r2=0, carry=true
        // Per ARM7TDMI: when shift-by-register amount is 0, value and carry pass through.
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x000, 0xE1B0_0231); // MOVS r0, r1, LSR r2
        bus.write_word_at(0x004, 0xE3A0_0000);
        bus.write_word_at(0x008, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.reg[1] = 0x7000_0000;
        cpu.reg[2] = 0;
        cpu.cpsr.c_condition_bit = true;
        cpu.flush_pipeline(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(cpu.reg[0], 0x7000_0000, "value should pass through");
        assert!(cpu.cpsr.c_condition_bit, "carry must remain set");
    }

    #[test]
    fn shift_by_reg_lsr_32_clears_value_carry_is_bit31() {
        // MOVS r0, r1, LSR r2  with r1=0x8000_0000, r2=32
        // Expected: r0 = 0, carry = bit31 of r1 = 1.
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x000, 0xE1B0_0231);
        bus.write_word_at(0x004, 0xE3A0_0000);
        bus.write_word_at(0x008, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.reg[1] = 0x8000_0000;
        cpu.reg[2] = 32;
        cpu.cpsr.c_condition_bit = false;
        cpu.flush_pipeline(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(cpu.reg[0], 0, "LSR by 32 -> 0");
        assert!(cpu.cpsr.c_condition_bit, "carry = bit31 of original");
    }

    // =========================================================================
    // ARM LDR misaligned rotate
    // =========================================================================

    #[test]
    fn arm_ldr_word_misaligned_rotates_right() {
        // LDR r0, [r1]  =  0xE591_0000
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x000, 0xE591_0000);
        bus.write_word_at(0x004, 0xE3A0_0000);
        bus.write_word_at(0x008, 0xE3A0_0000);
        bus.write_word_at(0x100, 0xDEAD_BEEF);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.reg[1] = 0x101;
        cpu.flush_pipeline(&mut bus);
        cpu.step(&mut bus);

        // Word at 0x100 = 0xDEADBEEF rotated right 8 bits = 0xEFDEADBE
        assert_eq!(cpu.reg[0], 0xEFDE_ADBE);
    }

    #[test]
    fn arm_ldr_word_aligned_no_rotate() {
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x000, 0xE591_0000);
        bus.write_word_at(0x004, 0xE3A0_0000);
        bus.write_word_at(0x008, 0xE3A0_0000);
        bus.write_word_at(0x100, 0xDEAD_BEEF);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.reg[1] = 0x100;
        cpu.flush_pipeline(&mut bus);
        cpu.step(&mut bus);

        assert_eq!(cpu.reg[0], 0xDEAD_BEEF);
    }

    // =========================================================================
    // Bus tick / cycle accounting
    // =========================================================================

    #[test]
    fn cpu_step_ticks_bus_for_instruction_fetch() {
        // ARM MOV r0, #0  (E3A00000)
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x00, 0xE3A0_0000);
        bus.write_word_at(0x04, 0xE3A0_0000);
        bus.write_word_at(0x08, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.flush_pipeline(&mut bus);

        let before = bus.ticks;
        cpu.step(&mut bus);
        let delta = bus.ticks - before;

        // Single ARM data-proc step = 1 sequential fetch = 1 cycle (no memory access in MOV imm).
        assert!(
            delta >= 1,
            "expected cpu.step to tick bus at least once, got {}",
            delta
        );
    }

    // =========================================================================
    // SPSR Restore Tests
    // =========================================================================

    #[test]
    fn restore_spsr_switches_mode() {
        let mut cpu = CPU::new();

        // Start in IRQ mode with a saved SPSR pointing to user mode
        cpu.set_mode(OperatingMode::irq);
        let usr_cpsr = ProgramStatusRegister {
            mode: OperatingMode::usr,
            operating_state: OperatingState::ARM,
            n_condition_bit: true,
            z_condition_bit: false,
            c_condition_bit: true,
            v_condition_bit: false,
            irq_disable_bit: false,
            fiq_disable_bit: false,
        };
        cpu.spsr = usr_cpsr.into();

        cpu.restore_spsr();

        assert_eq!(cpu.cpsr.mode, OperatingMode::usr);
        assert!(cpu.cpsr.n_condition_bit);
        assert!(!cpu.cpsr.z_condition_bit);
        assert!(cpu.cpsr.c_condition_bit);
        assert!(!cpu.cpsr.irq_disable_bit);
    }

    // =========================================================================
    // MRS / MSR Tests (BIOS init relies on these)
    // =========================================================================

    #[test]
    fn arm_mrs_copies_cpsr_to_rd() {
        // MRS R12, CPSR  =  0xE10F_C000
        // BIOS-relevant: MRSEQ R12, CPSR = 0x010F_C000 with cond=EQ.
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x00, 0xE10F_C000); // MRS r12, CPSR (AL)
        bus.write_word_at(0x04, 0xE3A0_0000);
        bus.write_word_at(0x08, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.cpsr.mode = OperatingMode::svc;
        cpu.cpsr.irq_disable_bit = true;
        cpu.cpsr.fiq_disable_bit = true;
        cpu.cpsr.c_condition_bit = true;
        cpu.cpsr.z_condition_bit = true;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.flush_pipeline(&mut bus);

        let expected_cpsr: u32 = cpu.cpsr.into();
        cpu.step(&mut bus);

        assert_eq!(
            cpu.reg[12], expected_cpsr,
            "MRS must copy CPSR into Rd unchanged"
        );
    }

    #[test]
    fn thumb_neg_writes_back_negated_value() {
        // THUMB format 4 opcode 0b1001 = NEG Rd, Rs => Rd = -Rs.
        // Encoding: 0100_0010_01_Rs_Rd. NEG R0, R0 = 0b 01000010_01_000_000 = 0x4240.
        // Bug: shared with ARM `is_test` table (0b1000..=0b1011) that suppresses writeback
        // for TST/TEQ/CMP/CMN. THUMB NEG must still write Rd.
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_hword_at(0x00, 0x4240);
        bus.write_hword_at(0x02, 0x46C0);
        bus.write_hword_at(0x04, 0x46C0);

        cpu.cpsr.operating_state = OperatingState::THUMB;
        cpu.cpsr.mode = OperatingMode::sys;
        cpu.reg[0] = 1;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.flush_pipeline(&mut bus);

        cpu.step(&mut bus);

        assert_eq!(cpu.reg[0], 0xFFFF_FFFF, "NEG R0, R0 with R0=1 must yield -1");
        assert!(cpu.cpsr.n_condition_bit, "NEG must set N flag for negative result");
    }

    #[test]
    fn arm_adr_immediate_uses_pc_plus_8_not_plus_12() {
        // ADD R0, PC, #0x258  =  E28F0F96.
        // ARM ADR pattern: PC is read as self+8 for immediate operand2.
        // The +12 (=self+8+4) rule applies ONLY to register-shift-by-register form (I=0, bit4=1).
        // Bug: code applied +4 whenever operand2.bit(4)=1 regardless of I, breaking BIOS init
        // (the boot-animation user IRQ vector was off by 4 -> handler skipped MOV R3 init -> IRQ never ack-ed).
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x00A0, 0xE28F_0F96);
        bus.write_word_at(0x00A4, 0xE3A0_0000);
        bus.write_word_at(0x00A8, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.cpsr.mode = OperatingMode::svc;
        cpu.reg[PC_INDEX] = 0xA0;
        cpu.flush_pipeline(&mut bus);

        cpu.step(&mut bus);

        assert_eq!(
            cpu.reg[0], 0x300,
            "ADR-style ADD R0, PC, #0x258 with PC=0xA0 must read PC as 0xA8 (self+8), yielding 0x300"
        );
    }

    #[test]
    fn arm_msr_in_user_mode_ignores_control_field() {
        // MSR CPSR_fc, #0x11 = 0xE329_F011. Tries to switch to fiq (mode bits 0b10001).
        // User mode is unprivileged: ARM ARM mandates control-field writes are dropped.
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x00, 0xE329_F011);
        bus.write_word_at(0x04, 0xE3A0_0000);
        bus.write_word_at(0x08, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.cpsr.mode = OperatingMode::usr;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.flush_pipeline(&mut bus);

        cpu.step(&mut bus);

        assert_eq!(
            cpu.cpsr.mode,
            OperatingMode::usr,
            "MSR control-field write from user mode must not switch mode"
        );
    }

    #[test]
    fn arm_msr_in_privileged_mode_changes_mode() {
        // Same MSR but from svc -> must succeed (privileged).
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x00, 0xE329_F011);
        bus.write_word_at(0x04, 0xE3A0_0000);
        bus.write_word_at(0x08, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.cpsr.mode = OperatingMode::svc;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.flush_pipeline(&mut bus);

        cpu.step(&mut bus);

        assert_eq!(
            cpu.cpsr.mode,
            OperatingMode::fiq,
            "MSR control-field write from svc must switch mode"
        );
    }

    #[test]
    fn arm_mrseq_runs_when_z_set_bios_init() {
        // MRSEQ R12, CPSR = 0x010F_C000. With Z=1 (CPSR=0x600000D3), should write R12.
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x1000);
        bus.write_word_at(0x00, 0x010F_C000);
        bus.write_word_at(0x04, 0xE3A0_0000);
        bus.write_word_at(0x08, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.cpsr.mode = OperatingMode::svc;
        cpu.cpsr.irq_disable_bit = true;
        cpu.cpsr.fiq_disable_bit = true;
        cpu.cpsr.c_condition_bit = true;
        cpu.cpsr.z_condition_bit = true;
        cpu.reg[PC_INDEX] = 0x00;
        cpu.flush_pipeline(&mut bus);

        let cpsr_before: u32 = cpu.cpsr.into();
        cpu.step(&mut bus);

        assert_eq!(
            cpu.reg[12], cpsr_before,
            "MRSEQ with Z=1 must write CPSR to R12"
        );
        let cpsr_after: u32 = cpu.cpsr.into();
        assert_eq!(cpsr_before, cpsr_after, "MRS must not alter CPSR");
    }

    // =========================================================================
    // Multiply timing (gbatek ARM7TDMI: MUL=1S+mI, MLA=1S+(m+1)I,
    // UMULL/SMULL=1S+(m+1)I, UMLAL/SMLAL=1S+(m+2)I)
    // =========================================================================

    fn run_inst_ticks(inst: u32, setup: impl FnOnce(&mut CPU)) -> u32 {
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x100);
        bus.write_word_at(0x00, inst);
        bus.write_word_at(0x04, 0xE3A0_0000);
        bus.write_word_at(0x08, 0xE3A0_0000);

        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.reg[PC_INDEX] = 0x00;
        setup(&mut cpu);
        cpu.flush_pipeline(&mut bus);

        let before = bus.ticks;
        cpu.step(&mut bus);
        bus.ticks - before
    }

    #[test]
    fn arm_mul_timing_scales_with_rs_leading_bits() {
        // MUL R3, R1, R2 = 0xE003_0291. Signed m: top 24 bits all 0 or all 1 -> 1,
        // top 16 -> 2, top 8 -> 3, else -> 4. MUL extra=0 -> total internal = m.
        let cases: [(u32, u32, &str); 5] = [
            (0x0000_0000, 1, "Rs=0 -> m=1"),
            (0xFFFF_FFFF, 1, "Rs=-1 (all ones) -> m=1 signed"),
            (0x0000_0100, 2, "Rs bit 8 -> m=2"),
            (0x0001_0000, 3, "Rs bit 16 -> m=3"),
            (0x0100_0000, 4, "Rs bit 24 -> m=4"),
        ];
        let base = run_inst_ticks(0xE003_0291, |c| {
            c.reg[1] = 1;
            c.reg[2] = cases[0].0;
        });
        for (rs_val, m, label) in cases {
            let ticks = run_inst_ticks(0xE003_0291, |c| {
                c.reg[1] = 1;
                c.reg[2] = rs_val;
            });
            let expected = base + (m - cases[0].1);
            assert_eq!(ticks, expected, "{label}");
        }
    }

    #[test]
    fn arm_mla_costs_one_more_i_cycle_than_mul() {
        // MLA R3, R1, R2, R0 = 0xE023_0291 (A=1 bit 21 set).
        let mul = run_inst_ticks(0xE003_0291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0;
        });
        let mla = run_inst_ticks(0xE023_0291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0;
            c.reg[0] = 0;
        });
        assert_eq!(mla, mul + 1, "MLA = MUL + 1 internal cycle");
    }

    #[test]
    fn arm_umull_costs_one_more_i_cycle_than_mul() {
        // UMULL RdLo=R3, RdHi=R4, Rm=R1, Rs=R2.
        // Encoding: cond 0000_1 U A S RdHi RdLo Rs 1001 Rm.
        // U=0 (unsigned), A=0, S=0, RdHi=4, RdLo=3, Rs=2, Rm=1.
        // 1110 0000 1000 0100 0011 0010 1001 0001 = 0xE084_3291.
        let mul = run_inst_ticks(0xE003_0291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0;
        });
        let umull = run_inst_ticks(0xE084_3291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0;
        });
        assert_eq!(umull, mul + 1, "UMULL = MUL + 1 internal cycle (long)");
    }

    #[test]
    fn arm_umlal_costs_two_more_i_cycles_than_mul() {
        // UMLAL: A=1, U=0. 1110 0000 1010 0100 0011 0010 1001 0001 = 0xE0A4_3291.
        let mul = run_inst_ticks(0xE003_0291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0;
        });
        let umlal = run_inst_ticks(0xE0A4_3291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0;
        });
        assert_eq!(umlal, mul + 2, "UMLAL = MUL + 2 internal cycles");
    }

    fn run_thumb_inst_ticks(inst: u16, setup: impl FnOnce(&mut CPU)) -> u32 {
        let mut cpu = CPU::new();
        let mut bus = TestBus::new(0x100);
        bus.write_hword_at(0x00, inst);
        bus.write_hword_at(0x02, 0x46C0);
        bus.write_hword_at(0x04, 0x46C0);

        cpu.cpsr.operating_state = OperatingState::THUMB;
        cpu.reg[PC_INDEX] = 0x00;
        setup(&mut cpu);
        cpu.flush_pipeline(&mut bus);

        let before = bus.ticks;
        cpu.step(&mut bus);
        bus.ticks - before
    }

    #[test]
    fn thumb_mul_timing_scales_with_rs_leading_bits() {
        // THUMB MUL R0, R1: format 4, opcode=0b1101, Rs=R1, Rd=R0.
        // 0100_00_1101_001_000 = 0x4348. ARM7TDMI: 1S + mI same as ARM MUL.
        let cases: [(u32, u32, &str); 4] = [
            (0x0000_0000, 1, "Rs=0 -> m=1"),
            (0x0000_0100, 2, "Rs bit 8 -> m=2"),
            (0x0001_0000, 3, "Rs bit 16 -> m=3"),
            (0x0100_0000, 4, "Rs bit 24 -> m=4"),
        ];
        let base = run_thumb_inst_ticks(0x4348, |c| {
            c.reg[0] = 1;
            c.reg[1] = cases[0].0;
        });
        for (rs_val, m, label) in cases {
            let ticks = run_thumb_inst_ticks(0x4348, |c| {
                c.reg[0] = 1;
                c.reg[1] = rs_val;
            });
            let expected = base + (m - cases[0].1);
            assert_eq!(ticks, expected, "{label}");
        }
    }

    #[test]
    fn arm_smull_unsigned_m_uses_only_zero_check() {
        // SMULL: U=1. 1110 0000 1100 0100 0011 0010 1001 0001 = 0xE0C4_3291.
        // Signed Rs=0xFFFF_FFFF -> m=1. Unsigned Rs=0xFFFF_FFFF -> m=4.
        // SMULL is signed so Rs=-1 should be m=1.
        let smull_neg_one = run_inst_ticks(0xE0C4_3291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0xFFFF_FFFF;
        });
        let smull_zero = run_inst_ticks(0xE0C4_3291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0;
        });
        assert_eq!(
            smull_neg_one, smull_zero,
            "SMULL with Rs=-1 must cost same as Rs=0 (both m=1 signed)"
        );

        // UMULL with the same Rs=-1 should hit m=4 -> 3 more cycles than UMULL with Rs=0.
        let umull_max = run_inst_ticks(0xE084_3291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0xFFFF_FFFF;
        });
        let umull_zero = run_inst_ticks(0xE084_3291, |c| {
            c.reg[1] = 1;
            c.reg[2] = 0;
        });
        assert_eq!(
            umull_max,
            umull_zero + 3,
            "UMULL with Rs=-1 hits m=4 vs m=1 at Rs=0"
        );
    }
}

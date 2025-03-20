use core::fmt;

use bit::BitIndex;

#[allow(non_camel_case_types)]

#[derive(Default, Clone, Copy, PartialEq)]
pub enum OperatingMode {
    usr = 0b10000,
    fiq = 0b10001,
    irq = 0b10010,
    #[default]
    svc = 0b10011,
    abt = 0b10111,
    sys = 0b11111,
    und = 0b11011
}

impl fmt::Debug for OperatingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode_str = match *self {
            OperatingMode::usr => "User Mode",
            OperatingMode::svc => "Supervisor Mode",
            OperatingMode::fiq => "FIQ Mode",
            OperatingMode::irq => "IRQ Mode",
            OperatingMode::und => "Undefined Mode",
            OperatingMode::sys => "System Mode",
            OperatingMode::abt => "Abort Mode",
        };
        write!(f, "{}", mode_str)
    }
}

impl OperatingMode {
    pub fn current_bank_index(self) -> usize {
        match self {
            OperatingMode::usr | OperatingMode::sys => 0,
            OperatingMode::fiq => 1,
            OperatingMode::irq => 2,
            OperatingMode::svc => 3,
            OperatingMode::abt => 4,
            OperatingMode::und => 5,
        }
    }
}

impl From<u32> for OperatingMode {
    fn from(val: u32) -> Self {
        match val {
            0b10000 => OperatingMode::usr,
            0b10001 => OperatingMode::fiq,
            0b10010 => OperatingMode::irq,
            0b10011 => OperatingMode::svc,
            0b10111 => OperatingMode::abt,
            0b11011 => OperatingMode::und,
            0b11111 => OperatingMode::sys,
            val @ _ => panic!("Unknown Operating mode 0b{:05b}", val),
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum OperatingState {
    #[default]
    ARM = 0,
    THUMB = 1
}

#[derive(Clone, Copy)]
pub struct ProgramStatusRegister {
    pub mode: OperatingMode,
    pub operating_state: OperatingState,
    pub fiq_disable_bit: bool,
    pub irq_disable_bit: bool,

    pub v_condition_bit: bool,
    pub c_condition_bit: bool,
    pub z_condition_bit: bool,
    pub n_condition_bit: bool
}

impl ProgramStatusRegister {
    pub fn new() -> Self {
        Self {
            mode: OperatingMode::default(),
            operating_state: OperatingState::default(),
            fiq_disable_bit: true,
            irq_disable_bit: true,
            v_condition_bit: false,
            c_condition_bit: false,
            z_condition_bit: false,
            n_condition_bit: false,
        }
    }
}

impl From<ProgramStatusRegister> for u32 {
    fn from(psr: ProgramStatusRegister) -> u32 {
        let mut val = 0u32;
        val.set_bit(31, psr.n_condition_bit);
        val.set_bit(30, psr.z_condition_bit);
        val.set_bit(29, psr.c_condition_bit);
        val.set_bit(28, psr.v_condition_bit);
        val.set_bit(7, psr.irq_disable_bit);
        val.set_bit(6, psr.fiq_disable_bit);
        val.set_bit(5, psr.operating_state == OperatingState::THUMB);
        val.set_bit_range(0..5, psr.mode as u32);
        val
    }
}

impl From<u32> for ProgramStatusRegister {
    fn from(val: u32) -> Self {
        ProgramStatusRegister {
            n_condition_bit: val.bit(31),
            z_condition_bit: val.bit(30),
            c_condition_bit: val.bit(29),
            v_condition_bit: val.bit(28),
            irq_disable_bit: val.bit(7),
            fiq_disable_bit: val.bit(6),
            operating_state: if val.bit(5) {
                OperatingState::THUMB
            } else {
                OperatingState::ARM
            },
            mode: OperatingMode::from(val.bit_range(0..5)),
        }
    }
}
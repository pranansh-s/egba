use super::{cpu::{CPU, PC_INDEX}, psr::{OperatingMode, OperatingState}};

#[derive(Clone, Copy, PartialEq)]
pub enum Exception {
    Reset,
    DataAbort,
    FIQ,
    IRQ,
    PrefetchAbort,
    Undefined,
    SoftwareInterrupt
}

impl Exception {
    fn get_vector_address(self) -> u32 {
        match self {
            Exception::Reset => 0x0000,
            Exception::DataAbort => 0x0010,
            Exception::FIQ => 0x001C,
            Exception::IRQ => 0x0018,
            Exception::PrefetchAbort => 0x000C,
            Exception::Undefined => 0x0004,
            Exception::SoftwareInterrupt => 0x0008,
        }
    }

    fn get_mode(self) -> OperatingMode {
        match self {
            Exception::Reset | Exception::SoftwareInterrupt => OperatingMode::svc,
            Exception::DataAbort | Exception::PrefetchAbort => OperatingMode::abt,
            Exception::FIQ => OperatingMode::fiq,
            Exception::IRQ => OperatingMode::irq,            
            Exception::Undefined => OperatingMode::und,
        }
    }
}

impl CPU {
    pub fn enter_exception(&mut self, exception: Exception, next_address: u32) {
        let exception_mode: OperatingMode = exception.get_mode();
        let exception_bank_index = exception_mode.current_bank_index();

        self.banks[exception_bank_index].lr = next_address;
        self.banks[exception_bank_index].spsr = self.cpsr.into();

        self.cpsr.operating_state = OperatingState::ARM;
        self.set_mode(exception_mode);
        self.cpsr.irq_disable_bit = true;
        
        if exception == Exception::FIQ || exception == Exception::Reset {
            self.cpsr.fiq_disable_bit = true;
        }

        self.reg[PC_INDEX] = exception.get_vector_address();
    }
}
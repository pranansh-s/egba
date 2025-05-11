use bit::BitIndex;

use crate::{bus::Bus, cpu::{cpu::CPU, exception::Exception, psr::OperatingState}, memory::Memory, HALTCNT, IE, IF, IME, WAITCNT};

trait Control {
    fn update(&mut self, mem: &mut Memory);
}

#[derive(Default)]
pub struct InterruptControl {
    master: bool,
    enable: Interrupt,
    request: Interrupt,
}

#[derive(Clone, Copy)]
pub enum InterruptType {
    VBlank = 0,
    HBlank = 1,
    VCounter = 2,
    Timer0 = 3,
    Timer1 = 4,
    Timer2 = 5,
    Timer3 = 6,
    Serial = 7,
    DMA0 = 8,
    DMA1 = 9,
    DMA2 = 10,
    DMA3 = 11,
    Keypad = 12,
    Cartridge = 13,
}

#[derive(Default)] 
struct Interrupt(u16);

impl Control for InterruptControl {
    fn update(&mut self, mem: &mut Memory) {
        self.master = mem.io.read_hword(IME).bit(0);
        self.enable = Interrupt(mem.io.read_hword(IE).bit_range(0..14));
        self.request = Interrupt(mem.io.read_hword(IF).bit_range(0..14));
    }
}

impl InterruptControl {
    pub fn step(&mut self, cpu: &mut CPU, mem: &mut Memory, system: &mut SystemControl) {
        self.update(mem);

        if self.master && (self.enable.0 & self.request.0) != 0 {
            system.power = PowerMode::Active;
            let addr = match cpu.cpsr.operating_state {
                OperatingState::ARM => cpu.arm_pc(),
                OperatingState::THUMB => cpu.thumb_pc(),
            };
            cpu.enter_exception(Exception::IRQ, addr.wrapping_add(4));
        }
    }

    pub fn interrupt_request(&mut self, interrupt: InterruptType) {
        self.request.0.set_bit(interrupt as usize, true);
    }
}

#[derive(Default)]
struct WaitState(usize, usize);

#[derive(Default, PartialEq, Clone, Copy)]
pub enum PowerMode {
    #[default]
    Active,
    Halt,
    Stop
}

#[derive(Default)]
pub struct SystemControl {
    sram_wait: usize,
    wait_state_0: WaitState,
    wait_state_1: WaitState,
    wait_state_2: WaitState,

    prefetch: bool,
    power: PowerMode,
}

impl Control for SystemControl {
    fn update(&mut self, mem: &mut Memory) {
        let waitcnt = mem.io.read_hword(WAITCNT);
        
        self.sram_wait = waitcnt.bit_range(0..2) as usize;
        self.wait_state_0 = WaitState(waitcnt.bit_range(2..4) as usize, waitcnt.bit(4) as usize);
        self.wait_state_1 = WaitState(waitcnt.bit_range(5..7) as usize, waitcnt.bit(7) as usize);
        self.wait_state_2 = WaitState(waitcnt.bit_range(8..10) as usize, waitcnt.bit(10) as usize);
        //TODO: Map wait states
        
        self.prefetch = waitcnt.bit(14);
        if mem.haltcnt_update {
            self.power = if mem.io.read_byte(HALTCNT).bit(7) {
                PowerMode::Stop
            }
            else {
                PowerMode::Halt
            };
        }
    }
}

impl SystemControl {
    pub fn step(&mut self, mem: &mut Memory) {
        self.update(mem);
        //TODO: actual cycle counting with ws and prefetch behavior
    }

    pub fn get_power_mode(&mut self) -> PowerMode {
        self.power
    }
}   
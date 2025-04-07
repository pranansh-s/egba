use crate::{ bios::Bios, cartridge::Cartridge, control::{InterruptControl, PowerMode, SystemControl}, cpu::cpu::CPU, memory::Memory };

pub struct GBA {
    pub(crate) cpu: CPU,
    pub(crate) memory: Memory,
    pub(crate) interrupt: InterruptControl,
    pub(crate) system: SystemControl
}

impl GBA {
    #[must_use]
    pub fn new(bios: Bios, cartridge: Cartridge) -> Self {
        let mut cpu = CPU::new();
        let mut memory = Memory::new(bios, cartridge);

        let interrupt = InterruptControl::default();
        let system = SystemControl::default();
        
        cpu.pipeline[1] = cpu.fetch(&mut memory);
        cpu.pipeline[2] = cpu.fetch(&mut memory);
        Self {
            cpu,
            memory,
            interrupt,
            system
        }
    }

    pub fn step(&mut self) {
        // let mut file = OpenOptions::new().append(true).create(true).open("data.txt").unwrap();
        // let pc_value = self.cpu.reg[PC_INDEX];
        // writeln!(file, "{pc_value}").expect("Failed to write to file");
        let power = self.system.get_power_mode();
        if power == PowerMode::Active {
            self.memory.haltcnt_update = false;
            self.cpu.step(&mut self.memory);
        }

        if power == PowerMode::Active || power == PowerMode::Halt {
            //TODO: video audio timers, dma, etc
            self.system.step(&mut self.memory);
        }
        
        self.interrupt.step(&mut self.cpu, &mut self.memory, &mut self.system);
    }
}
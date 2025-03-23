use crate::{ bios::Bios, cartridge::Cartridge, cpu::cpu::CPU, memory::Memory };

pub struct GBA {
    pub cpu: CPU,
    pub memory: Memory,
}

impl GBA {
    pub fn new(bios: Bios, cartridge: Cartridge) -> Self {
        let mut cpu = CPU::new();
        let mut memory = Memory::new(bios, cartridge);
        
        cpu.pipeline[1] = cpu.fetch(&mut memory);
        cpu.pipeline[2] = cpu.fetch(&mut memory);
        Self {
            cpu,
            memory,
        }
    }

    pub fn step(&mut self) {
        self.cpu.step(&mut self.memory);
    }
}

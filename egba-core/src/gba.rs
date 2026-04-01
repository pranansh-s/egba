use crate::{bios::Bios, cartridge::Cartridge, control::PowerMode, cpu::cpu::CPU, memory::Memory};

pub struct GBA {
    cpu: CPU,
    pub(crate) memory: Memory,
}

impl GBA {
    #[must_use]
    pub fn new(bios: Bios, cartridge: Cartridge) -> Self {
        let mut cpu = CPU::new();
        let mut memory = Memory::new(bios, cartridge);

        cpu.pipeline[1] = cpu.fetch(&mut memory);
        cpu.pipeline[2] = cpu.fetch(&mut memory);

        Self { cpu, memory }
    }

    pub fn get_cpu(&self) -> &CPU {
        &self.cpu
    }

    pub fn step(&mut self) {
        let power = self.memory.system.get_power_mode();
        if power == PowerMode::Active {
            self.cpu.step(&mut self.memory);
        }

        if power == PowerMode::Active || power == PowerMode::Halt {
            //TODO: video audio timers, dma, etc
            self.memory.video.step();
            self.memory.system.step();
        }

        self.memory
            .interrupt
            .step(&mut self.cpu, &mut self.memory.system);
    }
}

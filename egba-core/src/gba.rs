use std::{fs::OpenOptions, io::Write};

use crate::{ bios::Bios, cartridge::Cartridge, cpu::cpu::{CPU, PC_INDEX}, memory::Memory };

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
        // let mut file = OpenOptions::new().append(true).create(true).open("data.txt").unwrap();
        // let pc_value = self.cpu.reg[PC_INDEX];
        // writeln!(file, "{pc_value}").expect("Failed to write to file");
        self.cpu.step(&mut self.memory);
    }
}

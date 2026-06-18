use crate::{
    bios::Bios,
    bus::Bus,
    cartridge::Cartridge,
    control::{InterruptType, PowerMode},
    cpu::cpu::CPU,
    dma::{Dma, DmaEvent},
    memory::Memory,
    video::VideoEvent,
};

pub const CYCLES_PER_FRAME: u32 = 280896;

pub struct GBA {
    cpu: CPU,
    memory: Memory,
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

    fn drain_events(&mut self) {
        let debt = std::mem::take(&mut self.memory.video_cycle_debt);
        for _ in 0..debt {
            let (event, irq) = self.memory.video.step();
            if let Some(irq) = irq {
                self.memory.interrupt.request(irq);
            }
            match event {
                VideoEvent::HBlank => self.run_dma(DmaEvent::HBlank),
                VideoEvent::VBlank => self.run_dma(DmaEvent::VBlank),
                _ => {}
            }
        }

        let sound = std::mem::take(&mut self.memory.pending_sound_dma);
        if sound != 0 {
            self.run_dma(DmaEvent::Special);
        }

        self.run_dma(DmaEvent::Immediate);
        self.memory.system.step();
    }

    pub fn step_one_instruction(&mut self) {
        let power = self.memory.system.get_power_mode();
        if power == PowerMode::Active {
            let pc = self.cpu.reg[crate::cpu::cpu::PC_INDEX];
            self.memory.bios_readable = pc < 0x0000_4000;
            self.cpu.step(&mut self.memory);
        } else if power != PowerMode::Stop {
            <Memory as Bus>::tick(&mut self.memory, 1);
        }

        if power != PowerMode::Stop {
            self.drain_events();
        }

        let irq_accepted = self
            .memory
            .interrupt
            .step(&mut self.cpu, &mut self.memory.system);
        if irq_accepted {
            self.cpu.flush_pipeline(&mut self.memory);
        }
    }

    pub fn run_frame(&mut self) {
        let target = self.memory.bus_cycles.wrapping_add(CYCLES_PER_FRAME as u64);
        while self.memory.bus_cycles < target {
            self.step_one_instruction();
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.memory.read_byte(addr)
    }

    pub fn read_hword(&self, addr: u32) -> u16 {
        self.memory.read_hword(addr)
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        self.memory.read_word(addr)
    }

    pub fn framebuffer(&self) -> &[u32] {
        self.memory.video.framebuffer()
    }

    pub fn update_keypad(&mut self, state: u16) {
        self.memory.keypad.keystate = state;

        if self.memory.keypad.should_interrupt() {
            self.memory.interrupt.request(InterruptType::Keypad);
        }
    }

    pub fn drain_audio(&mut self) -> Vec<(i16, i16)> {
        self.memory.apu.drain_samples()
    }

    pub fn bus_cycles(&self) -> u64 {
        self.memory.bus_cycles
    }

    fn run_dma(&mut self, event: DmaEvent) {
        let mut dma = std::mem::take(&mut self.memory.dma);
        let irq_flags = dma.run(event, &mut self.memory);
        self.memory.dma = dma;

        for i in 0..4 {
            if irq_flags & (1 << i) != 0 {
                self.memory.interrupt.request(Dma::irq_type(i));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::Rom;
    use std::path::PathBuf;

    fn build_gba() -> GBA {
        let bios = Bios::new(Rom::new(&vec![0u8; 0x4000])).expect("bios");
        let cart = Cartridge::new(
            Rom::new(&vec![0u8; 0x1000]),
            &PathBuf::from("/nonexistent/no.sav"),
        )
        .expect("cart");
        GBA::new(bios, cart)
    }

    #[test]
    fn run_frame_advances_bus_at_least_one_frame() {
        let mut gba = build_gba();
        let before = gba.bus_cycles();
        gba.run_frame();
        let delta = gba.bus_cycles() - before;
        assert!(
            delta >= CYCLES_PER_FRAME as u64,
            "run_frame consumed {} cycles, expected >= {}",
            delta,
            CYCLES_PER_FRAME
        );
        // And not pathologically more than one extra instruction's worth.
        assert!(
            delta < (CYCLES_PER_FRAME as u64) + 32,
            "run_frame overshot by {} cycles",
            delta - CYCLES_PER_FRAME as u64
        );
    }
}

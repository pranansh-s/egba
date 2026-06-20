use crate::{
    bios::Bios,
    bus::Bus,
    cartridge::Cartridge,
    control::{InterruptType, PowerMode},
    cpu::{
        cpu::{CPU, PC_INDEX, SP_INDEX},
        psr::{OperatingMode, OperatingState},
    },
    dma::{Dma, DmaEvent},
    memory::Memory,
    video::VideoEvent,
};

pub const CYCLES_PER_FRAME: u32 = 280896;
pub const FB_WIDTH: usize = 240;
pub const FB_HEIGHT: usize = 160;

#[derive(Default, Clone, Copy)]
pub struct FrameProfile {
    pub instructions: u64,
    pub cycles: u64,
    pub halt_steps: u64,
}

pub struct GBA {
    cpu: CPU,
    memory: Memory,
    pub last_profile: FrameProfile,
}

impl GBA {
    #[must_use]
    pub fn new(bios: Bios, cartridge: Cartridge) -> Self {
        let mut cpu = CPU::new();
        let mut memory = Memory::new(bios, cartridge);

        cpu.pipeline[1] = cpu.fetch(&mut memory);
        cpu.pipeline[2] = cpu.fetch(&mut memory);

        Self { cpu, memory, last_profile: FrameProfile::default() }
    }

    #[must_use]
    pub fn new_skipping_bios(bios: Bios, cartridge: Cartridge) -> Self {
        let mut cpu = CPU::new();
        let mut memory = Memory::new(bios, cartridge);

        cpu.banks[OperatingMode::svc.current_bank_index()].sp = 0x0300_7FE0;
        cpu.banks[OperatingMode::irq.current_bank_index()].sp = 0x0300_7FA0;
        cpu.banks[OperatingMode::usr.current_bank_index()].sp = 0x0300_7F00;

        cpu.set_mode(OperatingMode::sys);
        cpu.cpsr.operating_state = OperatingState::ARM;
        cpu.cpsr.irq_disable_bit = false;
        cpu.cpsr.fiq_disable_bit = false;
        cpu.reg[SP_INDEX] = 0x0300_7F00;
        cpu.reg[PC_INDEX] = 0x0800_0000;

        memory.bios_readable = false;
        cpu.pipeline[1] = cpu.fetch(&mut memory);
        cpu.pipeline[2] = cpu.fetch(&mut memory);

        Self { cpu, memory, last_profile: FrameProfile::default() }
    }

    pub fn get_cpu(&self) -> &CPU {
        &self.cpu
    }

    fn drain_events(&mut self) {
        let debt = std::mem::take(&mut self.memory.video_cycle_debt);
        if debt > 0 {
            let mut events: [(VideoEvent, Option<crate::control::InterruptType>); 8] =
                [(VideoEvent::None, None); 8];
            let mut n = 0usize;
            self.memory.video.step_n(debt, |ev, irq| {
                if n < events.len() {
                    events[n] = (ev, irq);
                    n += 1;
                }
            });
            for i in 0..n {
                let (event, irq) = events[i];
                if let Some(irq) = irq {
                    self.memory.interrupt.request(irq);
                }
                match event {
                    VideoEvent::HBlank => self.run_dma(DmaEvent::HBlank),
                    VideoEvent::VBlank => self.run_dma(DmaEvent::VBlank),
                    _ => {}
                }
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
            self.memory.flush_pending_ticks();
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
        let start_cycles = self.memory.bus_cycles;
        let target = start_cycles.wrapping_add(CYCLES_PER_FRAME as u64);
        let mut prof = FrameProfile::default();
        while self.memory.bus_cycles < target {
            let power = self.memory.system.get_power_mode();
            if power == PowerMode::Active {
                let pc = self.cpu.reg[crate::cpu::cpu::PC_INDEX];
                self.memory.bios_readable = pc < 0x0000_4000;
                self.cpu.step(&mut self.memory);
                prof.instructions += 1;
            } else if power != PowerMode::Stop {
                let frame_left = (target - self.memory.bus_cycles) as u32;
                let video_left = self.memory.video.cycles_to_next_event();
                let mut batch = frame_left.min(video_left);
                if let Some(t) = self.memory.timers.cycles_to_next_overflow() {
                    batch = batch.min(t);
                }
                <Memory as Bus>::tick(&mut self.memory, batch.max(1));
                prof.halt_steps += 1;
            }
            if power != PowerMode::Stop {
                self.memory.flush_pending_ticks();
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
        prof.cycles = self.memory.bus_cycles.wrapping_sub(start_cycles);
        self.last_profile = prof;
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

    pub fn audio_samples(&self) -> &[(i16, i16)] {
        self.memory.apu.samples()
    }

    pub fn clear_audio(&mut self) {
        self.memory.apu.clear_samples();
    }

    pub fn bus_cycles(&self) -> u64 {
        self.memory.bus_cycles
    }

    pub fn save_backup(&self) {
        self.memory.cartridge.save();
    }

    fn run_dma(&mut self, event: DmaEvent) {
        if !self.memory.dma.any_running() {
            return;
        }
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
        assert!(
            delta < (CYCLES_PER_FRAME as u64) + 32,
            "run_frame overshot by {} cycles",
            delta - CYCLES_PER_FRAME as u64
        );
    }
}

use crate::{
    bios::Bios,
    bus::Bus,
    cartridge::Cartridge,
    control::{InterruptType, PowerMode},
    cpu::cpu::CPU,
    dma::{Dma, DmaEvent},
    memory::Memory,
    timer::Timers,
    video::VideoEvent,
};

const CYCLES_PER_FRAME: u32 = 280896;

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

    fn step(&mut self) {
        let power = self.memory.system.get_power_mode();
        if power == PowerMode::Active {
            self.cpu.step(&mut self.memory);
        }

        if power != PowerMode::Stop {
            let (video_event, video_irq) = self.memory.video.step();
            if let Some(irq) = video_irq {
                self.memory.interrupt.request(irq);
            }

            let timer_overflow = self.memory.timers.step(1);
            for i in 0..4 {
                if timer_overflow & (1 << i) != 0 && self.memory.timers.timer_irq_enabled(i) {
                    self.memory.interrupt.request(Timers::irq_type(i));
                }
            }

            match video_event {
                VideoEvent::HBlank => {
                    self.run_dma(DmaEvent::HBlank);
                }
                VideoEvent::VBlank => {
                    self.run_dma(DmaEvent::VBlank);
                }
                _ => {}
            }

            self.run_dma(DmaEvent::Immediate);
            self.memory.system.step();
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
        for _ in 0..CYCLES_PER_FRAME {
            self.step();
        }
    }

    /// Reads a byte from the current GBA memory map.
    ///
    /// GBATEK: GBA Memory Map
    pub fn read_byte(&self, addr: u32) -> u8 {
        self.memory.read_byte(addr)
    }

    /// Reads a halfword from the current GBA memory map.
    ///
    /// GBATEK: GBA Memory Map
    pub fn read_hword(&self, addr: u32) -> u16 {
        self.memory.read_hword(addr)
    }

    /// Reads a word from the current GBA memory map.
    ///
    /// GBATEK: GBA Memory Map
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

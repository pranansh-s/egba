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
        cpu.cpsr.fiq_disable_bit = true;
        cpu.reg[SP_INDEX] = 0x0300_7F00;
        cpu.reg[crate::cpu::cpu::LR_INDEX] = 0x0800_0000;
        cpu.reg[PC_INDEX] = 0x0800_0000;
        cpu.banks[OperatingMode::usr.current_bank_index()].lr = 0x0800_0000;

        memory.bios_readable = false;
        memory.last_bios_value = std::cell::Cell::new(0xE129F000);
        memory.write_byte(0x0400_0300, 0x01);
        memory.write_byte(0x0400_0000, 0x80);
        memory.write_byte(0x0400_0088, 0x00);
        memory.write_byte(0x0400_0089, 0x02);

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
            let mut events = std::mem::take(&mut self.memory.video_events);
            events.clear();
            self.memory.video.step_n(debt, |ev, irq| {
                events.push((ev, irq));
            });
            for (event, irq) in events.drain(..) {
                if let Some(irq) = irq {
                    self.memory.interrupt.request(irq);
                }
                match event {
                    VideoEvent::HBlank => self.run_dma(DmaEvent::HBlank),
                    VideoEvent::VBlank => self.run_dma(DmaEvent::VBlank),
                    _ => {}
                }
            }
            self.memory.video_events = events;
        }

        let sound = std::mem::take(&mut self.memory.pending_sound_dma);
        if sound != 0 {
            self.run_dma(DmaEvent::Special);
        }

        self.run_dma(DmaEvent::Immediate);
    }

    pub fn step_one_instruction(&mut self) {
        let mut prof = FrameProfile::default();
        let cap = self.memory.bus_cycles.wrapping_add(1);
        self.tick_one(&mut prof, cap);
    }

    pub fn run_frame(&mut self) {
        let start_cycles = self.memory.bus_cycles;
        let target = start_cycles.wrapping_add(CYCLES_PER_FRAME as u64);
        let mut prof = FrameProfile::default();
        while self.memory.bus_cycles < target {
            self.tick_one(&mut prof, target);
        }
        prof.cycles = self.memory.bus_cycles.wrapping_sub(start_cycles);
        self.last_profile = prof;
    }

    fn tick_one(&mut self, prof: &mut FrameProfile, halt_batch_target: u64) {
        let power = self.memory.system.get_power_mode();
        if power == PowerMode::Active {
            let pc = self.cpu.reg[crate::cpu::cpu::PC_INDEX];
            self.memory.bios_readable = pc < 0x0000_4000;
            self.cpu.step(&mut self.memory);
            prof.instructions += 1;
        } else if power != PowerMode::Stop {
            let frame_left = halt_batch_target.saturating_sub(self.memory.bus_cycles) as u32;
            let video_left = self.memory.video.cycles_to_next_event();
            let mut batch = frame_left.min(video_left);
            if let Some(t) = self.memory.timers.cycles_to_next_overflow() {
                batch = batch.min(t);
            }
            <Memory as Bus>::tick(&mut self.memory, batch.max(1));
            prof.halt_steps += 1;
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

    #[test]
    fn vblank_wakes_halt_within_one_frame() {
        let mut gba = build_gba();
        gba.memory.write_byte(0x0400_0004, 0x08);
        gba.memory.write_byte(0x0400_0200, 0x01);
        gba.memory.write_byte(0x0400_0208, 0x01);
        gba.cpu.cpsr.irq_disable_bit = false;
        gba.memory.write_byte(0x0400_0301, 0x00);
        assert_eq!(gba.memory.system.get_power_mode(), PowerMode::Halt);
        let start = gba.bus_cycles();
        let limit = start + (CYCLES_PER_FRAME as u64) + 4000;
        let mut prof = FrameProfile::default();
        while gba.bus_cycles() < limit {
            gba.tick_one(&mut prof, limit);
            if gba.memory.system.get_power_mode() == PowerMode::Active {
                break;
            }
        }
        assert_eq!(
            gba.memory.system.get_power_mode(),
            PowerMode::Active,
            "HALT must wake on VBlank IRQ within one frame; bus_cycles={}",
            gba.bus_cycles() - start
        );
    }

    #[test]
    #[ignore]
    fn diagnose_jsmolka_arm_run_state() {
        use crate::bios::Bios;
        use crate::cartridge::Cartridge;
        use crate::cpu::cpu::PC_INDEX;
        use crate::rom::Rom;
        use std::path::Path;
        let bios_path = Path::new("../roms/bios.bin");
        let rom_path = Path::new("../roms/jsmolka/arm.gba");
        if !bios_path.exists() || !rom_path.exists() {
            eprintln!("skip: ROM/BIOS not present");
            return;
        }
        let bios_bytes = std::fs::read(bios_path).expect("bios read");
        let rom_bytes = std::fs::read(rom_path).expect("rom read");
        let bios = Bios::new(Rom::new(&bios_bytes)).expect("bios");
        let cart = Cartridge::new(
            Rom::new(&rom_bytes),
            &PathBuf::from("/tmp/_jsmolka_arm_diag.sav"),
        )
        .expect("cart");
        let mut gba = GBA::new(bios, cart);
        for _ in 0..1200 {
            gba.run_frame();
        }
        eprintln!("=== jsmolka arm diag after 1200 frames ===");
        eprintln!("PC = 0x{:08X}", gba.cpu.reg[PC_INDEX]);
        eprintln!("cpsr = 0x{:08X}", u32::from(gba.cpu.cpsr));
        eprintln!("state = {:?}", gba.cpu.cpsr.operating_state);
        eprintln!("mode  = {:?}", gba.cpu.cpsr.mode);
        eprintln!("last instr count = {}", gba.last_profile.instructions);
        eprintln!("last cycle count = {}", gba.last_profile.cycles);
        eprintln!("last halt steps  = {}", gba.last_profile.halt_steps);
        for i in 0..16 {
            eprintln!("  r{:>2} = 0x{:08X}", i, gba.cpu.reg[i]);
        }
        let pc = gba.cpu.reg[PC_INDEX];
        eprintln!("mem around PC:");
        for off in [-16i32, -12, -8, -4, 0, 4, 8, 12].iter() {
            let a = pc.wrapping_add(*off as u32);
            eprintln!("  [0x{:08X}] = 0x{:08X}", a, gba.memory.read_word(a));
        }
        eprintln!("DISPCNT (0x4000000) = 0x{:04X}", gba.memory.read_hword(0x0400_0000));
        eprintln!("VCOUNT  (0x4000006) = 0x{:04X}", gba.memory.read_hword(0x0400_0006));
        eprintln!("IE      (0x4000200) = 0x{:04X}", gba.memory.read_hword(0x0400_0200));
        eprintln!("IF      (0x4000202) = 0x{:04X}", gba.memory.read_hword(0x0400_0202));
        eprintln!("IME     (0x4000208) = 0x{:04X}", gba.memory.read_hword(0x0400_0208));
        panic!("diagnostic dump only - inspect stderr above");
    }

    #[test]
    fn ags_prescaler_loop_yields_4096_cycles_in_iwram() {
        let mut gba = build_gba();

        gba.memory.write_word(0x0300_0000, 0xE250_0001);
        gba.memory.write_word(0x0300_0004, 0x1AFF_FFFD);

        gba.cpu.reg[0] = 0x400;
        gba.cpu.reg[crate::cpu::cpu::PC_INDEX] = 0x0300_0000;
        gba.cpu.cpsr.operating_state = crate::cpu::psr::OperatingState::ARM;
        gba.cpu.cpsr.mode = crate::cpu::psr::OperatingMode::sys;
        gba.cpu.flush_pipeline(&mut gba.memory);

        gba.memory.write_word(0x0400_0100, 0x0000_0000);
        let cycles_before_enable = gba.memory.bus_cycles;
        gba.memory.write_word(0x0400_0100, 0x0080_0000);

        for _ in 0..3000 {
            gba.step_one_instruction();
            if gba.cpu.reg[0] == 0 && gba.cpu.reg[crate::cpu::cpu::PC_INDEX] >= 0x0300_0008 {
                break;
            }
        }

        let counter = gba.memory.read_hword(0x0400_0100);
        let cycles_elapsed = gba.memory.bus_cycles - cycles_before_enable;
        assert!(
            (0xFF0..=0x1010).contains(&counter),
            "AGS-style 0x400 subs/bne loop should yield ~0x1000 timer ticks (got {:#x}, cycles elapsed {})",
            counter,
            cycles_elapsed
        );
    }

    #[test]
    fn timer0_overflow_irq_wakes_halt() {
        let mut gba = build_gba();
        gba.memory.write_byte(0x0400_0100, 0xF0);
        gba.memory.write_byte(0x0400_0101, 0xFF);
        gba.memory.write_byte(0x0400_0102, 0b1100_0000);
        gba.memory.write_byte(0x0400_0200, 0x08);
        gba.memory.write_byte(0x0400_0208, 0x01);
        gba.cpu.cpsr.irq_disable_bit = false;
        gba.memory.write_byte(0x0400_0301, 0x00);
        assert_eq!(gba.memory.system.get_power_mode(), PowerMode::Halt);
        let start = gba.bus_cycles();
        let limit = start + 1000;
        let mut prof = FrameProfile::default();
        while gba.bus_cycles() < limit {
            gba.tick_one(&mut prof, limit);
            if gba.memory.system.get_power_mode() == PowerMode::Active {
                break;
            }
        }
        assert_eq!(
            gba.memory.system.get_power_mode(),
            PowerMode::Active,
            "HALT must wake on Timer0 overflow IRQ within ~20 cycles (reload=0xFFF0 = 16 ticks + 2 delay); bus_cycles={}",
            gba.bus_cycles() - start
        );
    }

    #[test]
    fn hblank_dma_fires_only_on_visible_lines() {
        let mut gba = build_gba();

        for i in 0..230u32 {
            gba.memory.write_word(0x0200_0000 + i * 4, i + 1);
        }

        let src_base: u32 = 0x0200_0000;
        let dst_base: u32 = 0x0300_0000;

        for i in 0..4 {
            gba.memory.write_byte(0x0400_00B0 + i, (src_base >> (i * 8)) as u8);
        }
        for i in 0..4 {
            gba.memory.write_byte(0x0400_00B4 + i, (dst_base >> (i * 8)) as u8);
        }
        gba.memory.write_byte(0x0400_00B8, 1);
        gba.memory.write_byte(0x0400_00B9, 0);
        gba.memory.write_byte(0x0400_00BA, 0x40);
        gba.memory.write_byte(0x0400_00BB, 0xA6);

        gba.run_frame();

        let last_value = gba.memory.read_word(dst_base);
        assert_eq!(
            last_value, 160,
            "HBlank-DMA must fire exactly 160 times (visible lines), not during VBlank lines 160-227"
        );
    }

    #[test]
    fn bios_readable_flips_with_pc_via_flush_pipeline() {
        let mut gba = build_gba();
        gba.memory.bios_readable = false;
        gba.cpu.cpsr.irq_disable_bit = false;
        gba.cpu.cpsr.mode = crate::cpu::psr::OperatingMode::usr;
        gba.cpu.reg[crate::cpu::cpu::PC_INDEX] = 0x0800_0000;
        gba.cpu.flush_pipeline(&mut gba.memory);
        assert!(
            !gba.memory.bios_readable,
            "PC in cart after flush: bios_readable stays false"
        );

        let accepted = gba.cpu.setup_exception(crate::cpu::exception::Exception::IRQ, 0x0800_0004);
        assert!(accepted);
        gba.cpu.flush_pipeline(&mut gba.memory);
        assert!(
            gba.memory.bios_readable,
            "after IRQ entry: PC=0x18 in BIOS, flush_pipeline must set bios_readable"
        );
    }
}

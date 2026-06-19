use crate::{
    apu::Apu,
    bios::Bios,
    bus::Bus,
    cartridge::Cartridge,
    control::{InterruptControl, SystemControl},
    dma::{Dma, DmaMemory},
    keypad::Keypad,
    timer::Timers,
    video::Video,
};

pub(crate) struct Memory {
    pub(crate) bios: Bios,
    pub(crate) ewram: Box<[u8]>,
    pub(crate) iwram: Box<[u8]>,

    pub(crate) interrupt: InterruptControl,
    pub(crate) system: SystemControl,
    pub(crate) keypad: Keypad,

    pub(crate) video: Video,
    pub(crate) timers: Timers,
    pub(crate) dma: Dma,
    pub(crate) apu: Apu,

    pub(crate) cartridge: Cartridge,

    pub(crate) bios_readable: bool,
    last_bios_value: u8,

    last_bus_value: u8,

    pub(crate) video_cycle_debt: u32,
    pub(crate) pending_sound_dma: u8,
    pub(crate) bus_cycles: u64,

    last_rom_access: u32,
}

impl Memory {
    #[must_use]
    pub(crate) fn new(bios: Bios, cartridge: Cartridge) -> Self {
        Self {
            bios,
            ewram: vec![0; 0x40000].into_boxed_slice(),
            iwram: vec![0; 0x8000].into_boxed_slice(),
            interrupt: InterruptControl::default(),
            system: SystemControl::default(),
            keypad: Keypad::default(),
            video: Video::new(),
            timers: Timers::default(),
            dma: Dma::default(),
            apu: Apu::default(),
            cartridge,
            bios_readable: true,
            last_bios_value: 0,
            last_bus_value: 0,
            video_cycle_debt: 0,
            pending_sound_dma: 0,
            bus_cycles: 0,
            last_rom_access: !0,
        }
    }

}

impl Bus for Memory {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x0000_0000..=0x0000_3FFF => {
                if self.bios_readable {
                    let v = self.bios.read(addr);
                    v
                } else {
                    self.last_bios_value
                }
            }
            0x0200_0000..=0x02FF_FFFF => self.ewram.read_byte(addr & 0x3_FFFF),
            0x0300_0000..=0x03FF_FFFF => self.iwram.read_byte(addr & 0x7FFF),
            0x0400_0000..=0x0400_03FE => {
                let offset = addr & 0x3FF;
                match offset {
                    0x000..=0x056 => self.video.read_byte(offset),
                    0x060..=0x089 | 0x0A0..=0x0A7 => self.apu.read_byte(offset),
                    0x0B0..=0x0DE => self.dma.read_byte(offset),
                    0x100..=0x10F => self.timers.read_byte(offset),
                    0x130..=0x133 => self.keypad.read_byte(offset),
                    0x200..=0x203 | 0x208..=0x209 => self.interrupt.read_byte(offset),
                    0x204..=0x205 => self.system.read_byte(offset),
                    _x => self.last_bus_value,
                }
            }
            0x0500_0000..=0x05FF_FFFF => self.video.palette[(addr & 0x3FF) as usize],
            0x0600_0000..=0x0601_7FFF => self.video.vram[(addr & 0x1_7FFF) as usize],
            0x0601_8000..=0x06FF_FFFF => {
                let mirror = addr & 0x1_FFFF;
                let effective = if mirror >= 0x1_8000 {
                    mirror - 0x8000
                } else {
                    mirror
                };
                self.video.vram[effective as usize]
            }
            0x0700_0000..=0x07FF_FFFF => self.video.oam[(addr & 0x3FF) as usize],
            0x0800_0000..=0x0FFF_FFFF => self.cartridge.read_byte(addr),

            _x => self.last_bus_value,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x0200_0000..=0x02FF_FFFF => self.ewram.write_byte(addr & 0x3_FFFF, value),
            0x0300_0000..=0x03FF_FFFF => self.iwram.write_byte(addr & 0x7FFF, value),

            0x0400_0000..=0x0400_03FE => {
                let offset = addr & 0x3FF;
                match offset {
                    0x000..=0x056 => self.video.write_byte(offset, value),
                    0x060..=0x089 | 0x0A0..=0x0A7 => self.apu.write_byte(offset, value),
                    0x0B0..=0x0DE => self.dma.write_byte(offset, value),
                    0x100..=0x10F => self.timers.write_byte(offset, value),
                    0x130..=0x133 => self.keypad.write_byte(offset, value),
                    0x200..=0x203 | 0x208..=0x209 => self.interrupt.write_byte(offset, value),
                    0x204..=0x205 | 0x301 => self.system.write_byte(offset, value),
                    _x => {}
                }
            }
            0x0500_0000..=0x05FF_FFFF => {
                let pal_addr = (addr & 0x3FE) as usize;
                self.video.palette[pal_addr] = value;
                self.video.palette[pal_addr + 1] = value;
            }

            0x0600_0000..=0x06FF_FFFF => {
                let mirror = addr & 0x1_FFFF;
                let effective = if mirror >= 0x1_8000 {
                    mirror - 0x8000
                } else {
                    mirror
                };
                let bg_mode = self.video.read_byte(0x000) & 0x7;
                // OBJ VRAM starts at 0x10000 in tile modes (0..=2), 0x14000 in bitmap modes (3..=5).
                let obj_start = if bg_mode >= 3 { 0x14000 } else { 0x10000 };
                if (effective as usize) < obj_start {
                    // BG VRAM byte writes duplicate into the entire halfword.
                    let aligned = (effective & !1) as usize;
                    if aligned + 1 < self.video.vram.len() {
                        self.video.vram[aligned] = value;
                        self.video.vram[aligned + 1] = value;
                    }
                }
                // else: 8-bit writes to OBJ VRAM are ignored.
            }
            0x0700_0000..=0x07FF_FFFF => {}
            0x0800_0000..=0x0FFF_FFFF => self.cartridge.write_byte(addr, value),
            _x => {}
        }
    }

    fn write_hword(&mut self, addr: u32, value: u16) {
        match addr {
            0x0500_0000..=0x05FF_FFFF => {
                let pal_addr = (addr & 0x3FE) as usize;
                self.video.palette[pal_addr] = value as u8;
                self.video.palette[pal_addr + 1] = (value >> 8) as u8;
            }
            0x0600_0000..=0x06FF_FFFF => {
                let mirror = addr & 0x1_FFFF;
                let effective = if mirror >= 0x1_8000 {
                    mirror - 0x8000
                } else {
                    mirror
                };
                let e = effective as usize;
                if e + 1 < self.video.vram.len() {
                    self.video.vram[e] = value as u8;
                    self.video.vram[e + 1] = (value >> 8) as u8;
                }
            }
            0x0700_0000..=0x07FF_FFFF => {
                let oam_addr = (addr & 0x3FE) as usize;
                self.video.oam[oam_addr] = value as u8;
                self.video.oam[oam_addr + 1] = (value >> 8) as u8;
            }
            _ => {
                let addr = addr & !0b1;
                self.write_byte(addr, value as u8);
                self.write_byte(addr.wrapping_add(1), (value >> 8) as u8);
            }
        }
    }

    fn write_word(&mut self, addr: u32, value: u32) {
        match addr & 0x0FFF_FFFC {
            0x0400_00A0 => {
                self.apu.write_fifo(0, value);
                return;
            }
            0x0400_00A4 => {
                self.apu.write_fifo(1, value);
                return;
            }
            _ => {}
        }
        let addr = addr & !0b11;
        self.write_hword(addr, value as u16);
        self.write_hword(addr.wrapping_add(2), (value >> 16) as u16);
    }

    fn access_cycles(&mut self, addr: u32, width: u32) -> u32 {
        let region = (addr >> 24) & 0xF;
        let word = width >= 4;
        match region {
            0x0 | 0x3 | 0x4 | 0x7 => 1,
            0x2 => {
                if word {
                    6
                } else {
                    3
                }
            }
            0x5 | 0x6 => {
                if word {
                    2
                } else {
                    1
                }
            }
            0x8 | 0x9 | 0xA | 0xB | 0xC | 0xD => {
                let seq = addr == self.last_rom_access;
                let cycles = if seq {
                    self.system.rom_seq_cycles(addr, width)
                } else {
                    self.system.rom_access_cycles(addr, width)
                };
                self.last_rom_access = addr.wrapping_add(width);
                cycles
            }
            0xE | 0xF => self.system.sram_access_cycles(),
            _ => 1,
        }
    }

    fn invalidate_rom_seq(&mut self) {
        self.last_rom_access = u32::MAX;
    }

    fn tick(&mut self, n: u32) {
        if n == 0 {
            return;
        }

        self.bus_cycles = self.bus_cycles.wrapping_add(n as u64);

        let timer_overflow = self.timers.step(n);
        for i in 0..4 {
            if timer_overflow & (1 << i) != 0 && self.timers.timer_irq_enabled(i) {
                self.interrupt.request(Timers::irq_type(i));
            }
        }
        for timer_id in 0u8..2 {
            if timer_overflow & (1 << timer_id) != 0 {
                let refill = self.apu.on_timer_overflow(timer_id);
                if refill != 0 {
                    self.pending_sound_dma |= refill;
                }
            }
        }

        self.apu.step(n);

        self.video_cycle_debt = self.video_cycle_debt.saturating_add(n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rom::Rom;
    use std::path::PathBuf;

    fn build_memory() -> Memory {
        let bios = Bios::new(Rom::new(&vec![0u8; 0x4000])).expect("bios");
        let cart = Cartridge::new(
            Rom::new(&vec![0u8; 0x1000]),
            &PathBuf::from("/nonexistent/no.sav"),
        )
        .expect("cart");
        Memory::new(bios, cart)
    }

    #[test]
    fn obj_vram_byte_write_ignored_tile_modes() {
        // DISPCNT bg_mode = 0 (default). OBJ VRAM starts at offset 0x10000.
        let mut m = build_memory();
        // Pre-seed a value so we can detect any write.
        m.video.vram[0x10010] = 0xCD;
        m.video.vram[0x10011] = 0xEF;
        m.write_byte(0x0601_0010, 0xAB);
        // 8-bit writes to OBJ VRAM are ignored.
        assert_eq!(m.video.vram[0x10010], 0xCD);
        assert_eq!(m.video.vram[0x10011], 0xEF);
    }

    #[test]
    fn bg_vram_byte_write_duplicates_to_halfword_tile_modes() {
        let mut m = build_memory();
        m.write_byte(0x0600_1000, 0xAB);
        // Should duplicate to both bytes of the aligned halfword.
        assert_eq!(m.video.vram[0x1000], 0xAB);
        assert_eq!(m.video.vram[0x1001], 0xAB);
    }

    #[test]
    fn bg_vram_byte_write_duplicates_in_bitmap_modes() {
        let mut m = build_memory();
        // Set DISPCNT bg_mode = 3 (bitmap).
        m.write_byte(0x0400_0000, 0x03);
        m.write_byte(0x0600_2000, 0xAB);
        assert_eq!(m.video.vram[0x2000], 0xAB);
        assert_eq!(m.video.vram[0x2001], 0xAB);
    }

    #[test]
    fn obj_vram_byte_write_ignored_bitmap_modes() {
        let mut m = build_memory();
        m.write_byte(0x0400_0000, 0x03); // bg_mode 3 (bitmap)
        m.video.vram[0x14010] = 0xCD;
        m.video.vram[0x14011] = 0xEF;
        m.write_byte(0x0601_4010, 0xAB);
        assert_eq!(m.video.vram[0x14010], 0xCD);
        assert_eq!(m.video.vram[0x14011], 0xEF);
    }

    #[test]
    fn haltcnt_write_enters_halt_power_mode() {
        use crate::control::PowerMode;
        let mut m = build_memory();
        // HALTCNT bit7 = 0 -> Halt, 1 -> Stop. Write 0x00 -> Halt.
        m.write_byte(0x0400_0301, 0x00);
        assert_eq!(m.system.get_power_mode(), PowerMode::Halt);
    }

    #[test]
    fn haltcnt_stop_bit_enters_stop_mode() {
        use crate::control::PowerMode;
        let mut m = build_memory();
        m.write_byte(0x0400_0301, 0x80);
        assert_eq!(m.system.get_power_mode(), PowerMode::Stop);
    }

    #[test]
    fn io_above_0x3fe_still_routes() {
        // Bus value at addr just past current outer cap (0x0400_0400+) should
        // not panic and should be readable (returns open-bus = 0 here).
        let m = build_memory();
        let _ = m.read_byte(0x0400_0410);
    }

    #[test]
    fn ewram_mirrors_across_full_region() {
        let mut m = build_memory();
        m.write_byte(0x0200_1234, 0xAB);
        // EWRAM is 256 KB. Region 0x02000000..=0x02FFFFFF mirrors every 256 KB.
        assert_eq!(m.read_byte(0x0204_1234), 0xAB, "mirror at +256KB");
        assert_eq!(m.read_byte(0x02F0_1234), 0xAB, "mirror near top");
    }

    #[test]
    fn iwram_mirrors_across_full_region() {
        let mut m = build_memory();
        m.write_byte(0x0300_0010, 0x42);
        // IWRAM is 32 KB. Region 0x03000000..=0x03FFFFFF mirrors every 32 KB.
        assert_eq!(m.read_byte(0x0300_8010), 0x42, "mirror at +32KB");
        assert_eq!(m.read_byte(0x03FF_8010), 0x42, "mirror near top");
    }

    #[test]
    fn access_cycles_per_region() {
        let mut m = build_memory();
        // BIOS / IWRAM / IO / OAM: 1 cycle regardless of width.
        assert_eq!(m.access_cycles(0x0000_0000, 4), 1);
        assert_eq!(m.access_cycles(0x0300_0000, 4), 1);
        assert_eq!(m.access_cycles(0x0400_0000, 4), 1);
        assert_eq!(m.access_cycles(0x0700_0000, 4), 1);

        // EWRAM: 16-bit bus -> word = 2x.
        assert_eq!(m.access_cycles(0x0200_0000, 1), 3);
        assert_eq!(m.access_cycles(0x0200_0000, 2), 3);
        assert_eq!(m.access_cycles(0x0200_0000, 4), 6);

        // Palette / VRAM: byte/hword = 1, word = 2.
        assert_eq!(m.access_cycles(0x0500_0000, 2), 1);
        assert_eq!(m.access_cycles(0x0500_0000, 4), 2);
        assert_eq!(m.access_cycles(0x0600_0000, 2), 1);
        assert_eq!(m.access_cycles(0x0600_0000, 4), 2);

        // GamePak ROM: baseline 5 byte/hword, 8 word.
        assert_eq!(m.access_cycles(0x0800_0000, 2), 5);
        assert_eq!(m.access_cycles(0x0800_0000, 4), 8);

        // SRAM: byte only, 5 cycles.
        assert_eq!(m.access_cycles(0x0E00_0000, 1), 5);
    }

    #[test]
    fn waitcnt_ws0_changes_rom_cycles() {
        let mut m = build_memory();
        // Default WAITCNT=0: ws0 N=4, S=2 -> byte=5, word=8.
        assert_eq!(m.access_cycles(0x0800_0000, 2), 5);
        assert_eq!(m.access_cycles(0x0900_0000, 4), 8);

        // Set ws0 N=2 (idx=2 -> 2 waits), ws0 S=1 (bit4=1 -> 1 wait).
        // WAITCNT bits[3:2]=10, bit4=1 -> low byte = 0b0001_1000 = 0x18.
        m.write_byte(0x0400_0204, 0x18);
        assert_eq!(m.access_cycles(0x0800_0100, 2), 3, "ws0 N changed");
        assert_eq!(m.access_cycles(0x0800_0200, 4), 5, "ws0 word = N+S");
    }

    #[test]
    fn invalidate_rom_seq_forces_next_access_to_n() {
        let mut m = build_memory();
        assert_eq!(m.access_cycles(0x0800_0000, 2), 5, "first N");
        assert_eq!(m.access_cycles(0x0800_0002, 2), 3, "seq S");
        m.invalidate_rom_seq();
        assert_eq!(
            m.access_cycles(0x0800_0004, 2),
            5,
            "after invalidate, even an address contiguous with last access must charge N"
        );
    }

    #[test]
    fn rom_seq_cycles_independent_of_prefetch_flag() {
        // ROM bus always charges S on address matching last_rom_access.
        // Prefetch (WAITCNT bit 14) governs whether the prefetcher fills during
        // CPU idle, not the per-access cost.
        for (prefetch_byte, label) in [(0x00u8, "prefetch off"), (0x40u8, "prefetch on")] {
            let mut m = build_memory();
            m.write_byte(0x0400_0205, prefetch_byte);

            assert_eq!(m.access_cycles(0x0800_0000, 2), 5, "{label}: first = N");
            assert_eq!(
                m.access_cycles(0x0800_0002, 2),
                3,
                "{label}: sequential = S regardless of prefetch flag"
            );
            assert_eq!(m.access_cycles(0x0800_1000, 2), 5, "{label}: non-seq jump = N");
        }
    }

    #[test]
    fn waitcnt_sram_cycles() {
        let mut m = build_memory();
        // Default SRAM idx=0 -> 4 wait -> 5 cycles.
        assert_eq!(m.access_cycles(0x0E00_0000, 1), 5);
        // SRAM idx=2 -> 2 wait -> 3 cycles. bits[1:0]=10.
        m.write_byte(0x0400_0204, 0b10);
        assert_eq!(m.access_cycles(0x0E00_0000, 1), 3);
    }

    #[test]
    fn memory_tick_advances_enabled_timer() {
        let mut m = build_memory();
        // TM0CNT_H = 0x80 (enabled, prescaler 1, no cascade, no IRQ)
        m.write_byte(0x0400_0102, 0x80);
        // Reload+counter both start at 0
        m.tick(50);
        // TM0CNT_L low byte = current counter
        let lo = m.read_byte(0x0400_0100);
        let hi = m.read_byte(0x0400_0101);
        let counter = (lo as u16) | ((hi as u16) << 8);
        assert_eq!(counter, 50, "timer should have advanced 50 cycles");
    }
}

impl DmaMemory for Memory {
    fn dma_read_hword(&self, addr: u32) -> u16 {
        self.read_hword(addr)
    }

    fn dma_read_word(&self, addr: u32) -> u32 {
        self.read_word(addr)
    }

    fn dma_write_hword(&mut self, addr: u32, val: u16) {
        self.write_hword(addr, val);
    }

    fn dma_write_word(&mut self, addr: u32, val: u32) {
        self.write_word(addr, val);
    }
}

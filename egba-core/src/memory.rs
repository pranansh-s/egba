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
    pub(crate) pending_tick: u32,

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
            pending_tick: 0,
            last_rom_access: !0,
        }
    }

}

impl Bus for Memory {
    #[inline]
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
                    0x0B0..=0x0DF => self.dma.read_byte(offset),
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

    #[inline]
    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x0200_0000..=0x02FF_FFFF => self.ewram.write_byte(addr & 0x3_FFFF, value),
            0x0300_0000..=0x03FF_FFFF => self.iwram.write_byte(addr & 0x7FFF, value),

            0x0400_0000..=0x0400_03FE => {
                let offset = addr & 0x3FF;
                match offset {
                    0x000..=0x056 => self.video.write_byte(offset, value),
                    0x060..=0x089 | 0x0A0..=0x0A7 => self.apu.write_byte(offset, value),
                    0x0B0..=0x0DF => self.dma.write_byte(offset, value),
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
                let obj_start = if bg_mode >= 3 { 0x14000 } else { 0x10000 };
                if (effective as usize) < obj_start {
                    let aligned = (effective & !1) as usize;
                    if aligned + 1 < self.video.vram.len() {
                        self.video.vram[aligned] = value;
                        self.video.vram[aligned + 1] = value;
                    }
                }
            }
            0x0700_0000..=0x07FF_FFFF => {}
            0x0800_0000..=0x0FFF_FFFF => self.cartridge.write_byte(addr, value),
            _x => {}
        }
    }

    #[inline]
    fn read_hword(&self, addr: u32) -> u16 {
        let addr = addr & !0b1;
        match addr {
            0x0200_0000..=0x02FF_FFFF => {
                let off = (addr & 0x3_FFFF) as usize;
                u16::from_le_bytes([self.ewram[off], self.ewram[off + 1]])
            }
            0x0300_0000..=0x03FF_FFFF => {
                let off = (addr & 0x7FFF) as usize;
                u16::from_le_bytes([self.iwram[off], self.iwram[off + 1]])
            }
            0x0500_0000..=0x05FF_FFFF => {
                let off = (addr & 0x3FE) as usize;
                u16::from_le_bytes([self.video.palette[off], self.video.palette[off + 1]])
            }
            0x0600_0000..=0x06FF_FFFF => {
                let mirror = addr & 0x1_FFFF;
                let eff = if mirror >= 0x1_8000 { mirror - 0x8000 } else { mirror } as usize;
                u16::from_le_bytes([self.video.vram[eff], self.video.vram[eff + 1]])
            }
            0x0700_0000..=0x07FF_FFFF => {
                let off = (addr & 0x3FE) as usize;
                u16::from_le_bytes([self.video.oam[off], self.video.oam[off + 1]])
            }
            _ => u16::from_le_bytes([self.read_byte(addr), self.read_byte(addr.wrapping_add(1))]),
        }
    }

    #[inline]
    fn read_word(&self, addr: u32) -> u32 {
        let addr = addr & !0b11;
        match addr {
            0x0200_0000..=0x02FF_FFFF => {
                let off = (addr & 0x3_FFFF) as usize;
                u32::from_le_bytes([
                    self.ewram[off],
                    self.ewram[off + 1],
                    self.ewram[off + 2],
                    self.ewram[off + 3],
                ])
            }
            0x0300_0000..=0x03FF_FFFF => {
                let off = (addr & 0x7FFF) as usize;
                u32::from_le_bytes([
                    self.iwram[off],
                    self.iwram[off + 1],
                    self.iwram[off + 2],
                    self.iwram[off + 3],
                ])
            }
            0x0500_0000..=0x05FF_FFFF => {
                let off = (addr & 0x3FC) as usize;
                u32::from_le_bytes([
                    self.video.palette[off],
                    self.video.palette[off + 1],
                    self.video.palette[off + 2],
                    self.video.palette[off + 3],
                ])
            }
            0x0600_0000..=0x06FF_FFFF => {
                let mirror = addr & 0x1_FFFF;
                let eff = if mirror >= 0x1_8000 { mirror - 0x8000 } else { mirror } as usize;
                u32::from_le_bytes([
                    self.video.vram[eff],
                    self.video.vram[eff + 1],
                    self.video.vram[eff + 2],
                    self.video.vram[eff + 3],
                ])
            }
            0x0700_0000..=0x07FF_FFFF => {
                let off = (addr & 0x3FC) as usize;
                u32::from_le_bytes([
                    self.video.oam[off],
                    self.video.oam[off + 1],
                    self.video.oam[off + 2],
                    self.video.oam[off + 3],
                ])
            }
            _ => u32::from_le_bytes([
                self.read_byte(addr),
                self.read_byte(addr.wrapping_add(1)),
                self.read_byte(addr.wrapping_add(2)),
                self.read_byte(addr.wrapping_add(3)),
            ]),
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

    #[inline]
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

    #[inline]
    fn tick(&mut self, n: u32) {
        if n == 0 {
            return;
        }

        self.bus_cycles = self.bus_cycles.wrapping_add(n as u64);
        self.pending_tick = self.pending_tick.wrapping_add(n);

        self.video_cycle_debt = self.video_cycle_debt.saturating_add(n);
    }
}

impl Memory {
    #[inline]
    pub(crate) fn flush_pending_ticks(&mut self) {
        let n = self.pending_tick;
        if n == 0 {
            return;
        }
        self.pending_tick = 0;

        self.apu.step(n);

        let timer_overflow = self.timers.step(n);
        if timer_overflow == 0 {
            return;
        }
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
        let mut m = build_memory();
        m.video.vram[0x10010] = 0xCD;
        m.video.vram[0x10011] = 0xEF;
        m.write_byte(0x0601_0010, 0xAB);
        assert_eq!(m.video.vram[0x10010], 0xCD);
        assert_eq!(m.video.vram[0x10011], 0xEF);
    }

    #[test]
    fn bg_vram_byte_write_duplicates_to_halfword_tile_modes() {
        let mut m = build_memory();
        m.write_byte(0x0600_1000, 0xAB);
        assert_eq!(m.video.vram[0x1000], 0xAB);
        assert_eq!(m.video.vram[0x1001], 0xAB);
    }

    #[test]
    fn bg_vram_byte_write_duplicates_in_bitmap_modes() {
        let mut m = build_memory();
        m.write_byte(0x0400_0000, 0x03);
        m.write_byte(0x0600_2000, 0xAB);
        assert_eq!(m.video.vram[0x2000], 0xAB);
        assert_eq!(m.video.vram[0x2001], 0xAB);
    }

    #[test]
    fn obj_vram_byte_write_ignored_bitmap_modes() {
        let mut m = build_memory();
        m.write_byte(0x0400_0000, 0x03);
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
        let m = build_memory();
        let _ = m.read_byte(0x0400_0410);
    }

    #[test]
    fn ewram_mirrors_across_full_region() {
        let mut m = build_memory();
        m.write_byte(0x0200_1234, 0xAB);
        assert_eq!(m.read_byte(0x0204_1234), 0xAB, "mirror at +256KB");
        assert_eq!(m.read_byte(0x02F0_1234), 0xAB, "mirror near top");
    }

    #[test]
    fn iwram_mirrors_across_full_region() {
        let mut m = build_memory();
        m.write_byte(0x0300_0010, 0x42);
        assert_eq!(m.read_byte(0x0300_8010), 0x42, "mirror at +32KB");
        assert_eq!(m.read_byte(0x03FF_8010), 0x42, "mirror near top");
    }

    #[test]
    fn access_cycles_per_region() {
        let mut m = build_memory();
        assert_eq!(m.access_cycles(0x0000_0000, 4), 1);
        assert_eq!(m.access_cycles(0x0300_0000, 4), 1);
        assert_eq!(m.access_cycles(0x0400_0000, 4), 1);
        assert_eq!(m.access_cycles(0x0700_0000, 4), 1);

        assert_eq!(m.access_cycles(0x0200_0000, 1), 3);
        assert_eq!(m.access_cycles(0x0200_0000, 2), 3);
        assert_eq!(m.access_cycles(0x0200_0000, 4), 6);

        assert_eq!(m.access_cycles(0x0500_0000, 2), 1);
        assert_eq!(m.access_cycles(0x0500_0000, 4), 2);
        assert_eq!(m.access_cycles(0x0600_0000, 2), 1);
        assert_eq!(m.access_cycles(0x0600_0000, 4), 2);

        assert_eq!(m.access_cycles(0x0800_0000, 2), 5);
        assert_eq!(m.access_cycles(0x0800_0000, 4), 8);

        assert_eq!(m.access_cycles(0x0E00_0000, 1), 5);
    }

    #[test]
    fn waitcnt_ws0_changes_rom_cycles() {
        let mut m = build_memory();
        assert_eq!(m.access_cycles(0x0800_0000, 2), 5);
        assert_eq!(m.access_cycles(0x0900_0000, 4), 8);

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
        assert_eq!(m.access_cycles(0x0E00_0000, 1), 5);
        m.write_byte(0x0400_0204, 0b10);
        assert_eq!(m.access_cycles(0x0E00_0000, 1), 3);
    }

    #[test]
    fn dma3_control_high_byte_routes_to_dma() {
        let mut m = build_memory();
        m.write_byte(0x0400_00DF, 0x80);
        assert_eq!(m.read_byte(0x0400_00DF), 0x80, "DMA3 enable byte must round-trip");
    }

    fn assert_word_matches_bytes(m: &Memory, addr: u32, label: &str) {
        let aligned = addr & !0b11;
        let expected = u32::from_le_bytes([
            m.read_byte(aligned),
            m.read_byte(aligned + 1),
            m.read_byte(aligned + 2),
            m.read_byte(aligned + 3),
        ]);
        assert_eq!(m.read_word(addr), expected, "{label} read_word");
    }

    fn assert_hword_matches_bytes(m: &Memory, addr: u32, label: &str) {
        let aligned = addr & !0b1;
        let expected =
            u16::from_le_bytes([m.read_byte(aligned), m.read_byte(aligned + 1)]);
        assert_eq!(m.read_hword(addr), expected, "{label} read_hword");
    }

    #[test]
    fn fast_path_word_hword_match_byte_path_across_regions() {
        let cases: [(u32, u32, &str); 5] = [
            (0x0200_1000, 0xDEAD_BEEF, "ewram"),
            (0x0300_0010, 0x1234_5678, "iwram"),
            (0x0500_0000, 0xCAFE_BABE, "palette"),
            (0x0600_2000, 0xFACE_1337, "vram"),
            (0x0700_0000, 0xBEEF_DEAD, "oam"),
        ];
        for (addr, val, label) in cases {
            let mut m = build_memory();
            m.write_word(addr, val);
            assert_word_matches_bytes(&m, addr, label);
            assert_hword_matches_bytes(&m, addr, label);
            assert_hword_matches_bytes(&m, addr + 2, label);
        }
    }

    #[test]
    fn fast_path_misalignment_and_mirror() {
        let mut m = build_memory();
        m.write_word(0x0200_1000, 0xDEAD_BEEF);
        assert_eq!(m.read_word(0x0200_1003), m.read_word(0x0200_1000), "misaligned forces aligned");
        assert_eq!(m.read_hword(0x0200_1001), m.read_hword(0x0200_1000), "misaligned hword");
        assert_eq!(m.read_word(0x0204_1000), 0xDEAD_BEEF, "ewram mirror at +256KB");
    }

    #[test]
    fn memory_tick_advances_enabled_timer() {
        let mut m = build_memory();
        m.write_byte(0x0400_0102, 0x80);
        m.tick(50);
        m.flush_pending_ticks();
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

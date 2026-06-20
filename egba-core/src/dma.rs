use bit::BitIndex;

use crate::{bus::Bus, control::InterruptType};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DmaEvent {
    Immediate,
    VBlank,
    HBlank,
    Special,
}

#[derive(Default, Clone)]
struct DmaChannel {
    src: u32,
    dst: u32,
    count: u32,
    control: u16,

    internal_src: u32,
    internal_dst: u32,
    internal_count: u32,

    running: bool,
}

impl DmaChannel {
    fn enabled(&self) -> bool {
        self.control.bit(15)
    }

    fn irq_enabled(&self) -> bool {
        self.control.bit(14)
    }

    fn start_timing(&self) -> DmaEvent {
        match self.control.bit_range(12..14) {
            0 => DmaEvent::Immediate,
            1 => DmaEvent::VBlank,
            2 => DmaEvent::HBlank,
            3 => DmaEvent::Special,
            _ => unreachable!(),
        }
    }

    fn transfer_32bit(&self) -> bool {
        self.control.bit(10)
    }

    fn repeat(&self) -> bool {
        self.control.bit(9)
    }

    fn src_control(&self) -> u8 {
        self.control.bit_range(7..9) as u8
    }

    fn dst_control(&self) -> u8 {
        self.control.bit_range(5..7) as u8
    }
}

#[derive(Default)]
pub(crate) struct Dma {
    channels: [DmaChannel; 4],
}

impl Dma {
    pub(crate) fn any_running(&self) -> bool {
        self.channels.iter().any(|c| c.enabled() && c.running)
    }

    pub(crate) fn run(&mut self, event: DmaEvent, memory: &mut dyn DmaMemory) -> u8 {
        let mut irq_flags: u8 = 0;

        for i in 0..4 {
            if !self.channels[i].enabled() || !self.channels[i].running {
                continue;
            }

            if self.channels[i].start_timing() != event {
                continue;
            }

            let is_fifo_dma = (i == 1 || i == 2)
                && self.channels[i].start_timing() == DmaEvent::Special;

            if is_fifo_dma {
                self.execute_fifo_transfer(i, memory);
            } else {
                self.execute_transfer(i, memory);
            }

            if self.channels[i].irq_enabled() {
                irq_flags |= 1 << i;
            }

            if self.channels[i].repeat() && event != DmaEvent::Immediate {
                self.channels[i].internal_count = if self.channels[i].count == 0 {
                    if i == 3 {
                        0x10000
                    } else {
                        0x4000
                    }
                } else {
                    self.channels[i].count
                };
                if self.channels[i].dst_control() == 3 {
                    self.channels[i].internal_dst = self.channels[i].dst;
                }
            } else {
                self.channels[i].control.set_bit(15, false);
                self.channels[i].running = false;
            }
        }

        irq_flags
    }

    fn execute_transfer(&mut self, ch: usize, memory: &mut dyn DmaMemory) {
        let word32 = self.channels[ch].transfer_32bit();
        let step = if word32 { 4u32 } else { 2u32 };
        let count = self.channels[ch].internal_count;

        let src_step: i32 = match self.channels[ch].src_control() {
            0 => step as i32,
            1 => -(step as i32),
            2 => 0,
            _ => step as i32,
        };

        let dst_step: i32 = match self.channels[ch].dst_control() {
            0 | 3 => step as i32,
            1 => -(step as i32),
            2 => 0,
            _ => step as i32,
        };

        let src_mask = Self::src_addr_mask(ch);
        let dst_mask = Self::dst_addr_mask(ch);
        let mut src = self.channels[ch].internal_src & src_mask;
        let mut dst = self.channels[ch].internal_dst & dst_mask;

        for _ in 0..count {
            if word32 {
                let val = memory.dma_read_word(src);
                memory.dma_write_word(dst, val);
            } else {
                let val = memory.dma_read_hword(src);
                memory.dma_write_hword(dst, val);
            }

            src = ((src as i32).wrapping_add(src_step) as u32) & src_mask;
            dst = ((dst as i32).wrapping_add(dst_step) as u32) & dst_mask;
        }

        self.channels[ch].internal_src = src;
        self.channels[ch].internal_dst = dst;
    }

    fn execute_fifo_transfer(&mut self, ch: usize, memory: &mut dyn DmaMemory) {
        let src_mask = Self::src_addr_mask(ch);
        let mut src = self.channels[ch].internal_src & src_mask;
        let dst = self.channels[ch].internal_dst;

        for _ in 0..4 {
            let val = memory.dma_read_word(src);
            memory.dma_write_word(dst, val);
            src = src.wrapping_add(4) & src_mask;
        }

        self.channels[ch].internal_src = src;
    }

    fn src_addr_mask(ch: usize) -> u32 {
        if ch == 0 {
            0x07FF_FFFF
        } else {
            0x0FFF_FFFF
        }
    }

    fn dst_addr_mask(ch: usize) -> u32 {
        if ch == 3 {
            0x0FFF_FFFF
        } else {
            0x07FF_FFFF
        }
    }

    pub(crate) fn irq_type(index: usize) -> InterruptType {
        match index {
            0 => InterruptType::DMA0,
            1 => InterruptType::DMA1,
            2 => InterruptType::DMA2,
            3 => InterruptType::DMA3,
            _ => unreachable!(),
        }
    }
}

impl Bus for Dma {
    fn read_byte(&self, addr: u32) -> u8 {
        let ch = ((addr - 0xB0) / 12) as usize;
        let reg = (addr - 0xB0) % 12;

        if ch >= 4 {
            return 0;
        }

        match reg {
            10 => self.channels[ch].control as u8,
            11 => (self.channels[ch].control >> 8) as u8,
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        let ch = ((addr - 0xB0) / 12) as usize;
        let reg = (addr - 0xB0) % 12;

        if ch >= 4 {
            return;
        }

        match reg {
            0 => {
                self.channels[ch].src.set_bit_range(0..8, value as u32);
            }
            1 => {
                self.channels[ch].src.set_bit_range(8..16, value as u32);
            }
            2 => {
                self.channels[ch].src.set_bit_range(16..24, value as u32);
            }
            3 => {
                self.channels[ch]
                    .src
                    .set_bit_range(24..28, (value & 0x0F) as u32);
            }
            4 => {
                self.channels[ch].dst.set_bit_range(0..8, value as u32);
            }
            5 => {
                self.channels[ch].dst.set_bit_range(8..16, value as u32);
            }
            6 => {
                self.channels[ch].dst.set_bit_range(16..24, value as u32);
            }
            7 => {
                self.channels[ch]
                    .dst
                    .set_bit_range(24..28, (value & 0x0F) as u32);
            }
            8 => {
                self.channels[ch].count.set_bit_range(0..8, value as u32);
            }
            9 => {
                self.channels[ch].count.set_bit_range(8..16, value as u32);
            }
            10 => {
                self.channels[ch].control.set_bit_range(0..8, value as u16);
            }
            11 => {
                let was_enabled = self.channels[ch].enabled();
                self.channels[ch].control.set_bit_range(8..16, value as u16);
                let now_enabled = self.channels[ch].enabled();

                if !was_enabled && now_enabled {
                    self.channels[ch].internal_src = self.channels[ch].src;
                    self.channels[ch].internal_dst = self.channels[ch].dst;
                    self.channels[ch].internal_count = if self.channels[ch].count == 0 {
                        if ch == 3 {
                            0x10000
                        } else {
                            0x4000
                        }
                    } else {
                        self.channels[ch].count
                    };
                    self.channels[ch].running = true;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeMem {
        src: Vec<u32>,
        dst: Vec<u32>,
        reads: Vec<u32>,
        writes: Vec<(u32, u32)>,
    }

    impl DmaMemory for FakeMem {
        fn dma_read_hword(&self, addr: u32) -> u16 {
            (self.dma_read_word(addr) & 0xFFFF) as u16
        }
        fn dma_read_word(&self, addr: u32) -> u32 {
            let idx = ((addr - self.src[0]) / 4) as usize;
            self.src.get(idx + 1).copied().unwrap_or(0)
        }
        fn dma_write_hword(&mut self, addr: u32, val: u16) {
            self.dma_write_word(addr, val as u32);
        }
        fn dma_write_word(&mut self, addr: u32, val: u32) {
            self.writes.push((addr, val));
            self.dst.push(val);
        }
    }

    fn fake_mem(src_base: u32, words: &[u32]) -> FakeMem {
        let mut src = vec![src_base];
        src.extend_from_slice(words);
        FakeMem {
            src,
            dst: vec![],
            reads: vec![],
            writes: vec![],
        }
    }

    fn setup_dma3(dma: &mut Dma, src: u32, dst: u32, count: u16, ctrl_h: u16) {
        let base: u32 = 0xD4;
        for (i, &b) in src.to_le_bytes().iter().enumerate() {
            dma.write_byte(base + i as u32, b);
        }
        for (i, &b) in dst.to_le_bytes().iter().enumerate() {
            dma.write_byte(base + 4 + i as u32, b);
        }
        dma.write_byte(base + 8, count as u8);
        dma.write_byte(base + 9, (count >> 8) as u8);
        dma.write_byte(base + 10, ctrl_h as u8);
        dma.write_byte(base + 11, (ctrl_h >> 8) as u8);
    }

    #[test]
    fn dma3_immediate_word_copy_runs_on_immediate_event() {
        let mut dma = Dma::default();
        let mut mem = fake_mem(0x0800_0000, &[0xDEAD_BEEF, 0xCAFEBABE]);
        setup_dma3(&mut dma, 0x0800_0000, 0x0300_0000, 2, 0x8400);
        let irq = dma.run(DmaEvent::Immediate, &mut mem);
        assert_eq!(irq, 0, "DMA3 IRQ not requested (enable bit 14 = 0)");
        assert_eq!(mem.writes.len(), 2, "two words written");
        assert_eq!(mem.writes[0].0, 0x0300_0000);
        assert_eq!(mem.writes[0].1, 0xDEAD_BEEF);
        assert_eq!(mem.writes[1].0, 0x0300_0004);
        assert_eq!(mem.writes[1].1, 0xCAFEBABE);
    }

    #[test]
    fn dma_immediate_does_not_repeat() {
        let mut dma = Dma::default();
        let mut mem = fake_mem(0x0200_0000, &[1, 2, 3, 4]);
        setup_dma3(&mut dma, 0x0200_0000, 0x0300_0000, 2, 0x8400 | 0x0200);
        dma.run(DmaEvent::Immediate, &mut mem);
        assert_eq!(mem.writes.len(), 2);
        dma.run(DmaEvent::Immediate, &mut mem);
        assert_eq!(
            mem.writes.len(),
            2,
            "immediate DMA must not re-trigger even with repeat bit"
        );
    }

    #[test]
    fn dma3_vblank_repeat_reruns_on_each_vblank() {
        let mut dma = Dma::default();
        let mut mem = fake_mem(0x0200_0000, &[10, 20]);
        setup_dma3(
            &mut dma,
            0x0200_0000,
            0x0300_0000,
            1,
            0x8000 | 0x0400 | 0x1000 | 0x0200,
        );
        dma.run(DmaEvent::VBlank, &mut mem);
        assert_eq!(mem.writes.len(), 1, "first VBlank");
        dma.run(DmaEvent::VBlank, &mut mem);
        assert_eq!(mem.writes.len(), 2, "repeat: second VBlank");
    }

    #[test]
    fn dma3_irq_flag_when_enabled() {
        let mut dma = Dma::default();
        let mut mem = fake_mem(0x0200_0000, &[42]);
        setup_dma3(&mut dma, 0x0200_0000, 0x0300_0000, 1, 0x8400 | 0x4000);
        let irq = dma.run(DmaEvent::Immediate, &mut mem);
        assert_eq!(irq, 0b1000, "bit 3 = DMA3 IRQ flag");
    }

    #[test]
    fn dma0_src_addr_mask_blocks_rom_region() {
        assert_eq!(Dma::src_addr_mask(0), 0x07FF_FFFF);
        assert_eq!(Dma::src_addr_mask(1), 0x0FFF_FFFF);
        assert_eq!(Dma::src_addr_mask(2), 0x0FFF_FFFF);
        assert_eq!(Dma::src_addr_mask(3), 0x0FFF_FFFF);
    }

    #[test]
    fn dma_dst_addr_mask_only_dma3_reaches_rom() {
        assert_eq!(Dma::dst_addr_mask(0), 0x07FF_FFFF);
        assert_eq!(Dma::dst_addr_mask(1), 0x07FF_FFFF);
        assert_eq!(Dma::dst_addr_mask(2), 0x07FF_FFFF);
        assert_eq!(Dma::dst_addr_mask(3), 0x0FFF_FFFF);
    }
}

pub(crate) trait DmaMemory {
    fn dma_read_hword(&self, addr: u32) -> u16;
    fn dma_read_word(&self, addr: u32) -> u32;
    fn dma_write_hword(&mut self, addr: u32, val: u16);
    fn dma_write_word(&mut self, addr: u32, val: u32);
}

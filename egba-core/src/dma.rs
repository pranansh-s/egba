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

pub(crate) struct Dma {
    channels: [DmaChannel; 4],
}

impl Default for Dma {
    fn default() -> Self {
        Self {
            channels: [
                DmaChannel::default(),
                DmaChannel::default(),
                DmaChannel::default(),
                DmaChannel::default(),
            ],
        }
    }
}

impl Dma {
    pub(crate) fn run(&mut self, event: DmaEvent, memory: &mut dyn DmaMemory) -> u8 {
        let mut irq_flags: u8 = 0;

        for i in 0..4 {
            if !self.channels[i].enabled() || !self.channels[i].running {
                continue;
            }

            if self.channels[i].start_timing() != event {
                continue;
            }

            self.execute_transfer(i, memory);

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

        let mut src = self.channels[ch].internal_src;
        let mut dst = self.channels[ch].internal_dst;

        for _ in 0..count {
            if word32 {
                let val = memory.dma_read_word(src);
                memory.dma_write_word(dst, val);
            } else {
                let val = memory.dma_read_hword(src);
                memory.dma_write_hword(dst, val);
            }

            src = (src as i32).wrapping_add(src_step) as u32;
            dst = (dst as i32).wrapping_add(dst_step) as u32;
        }

        self.channels[ch].internal_src = src;
        self.channels[ch].internal_dst = dst;
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

    pub(crate) fn channel_irq_enabled(&self, index: usize) -> bool {
        self.channels[index].irq_enabled()
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
            0..=7 => 0,
            8 => 0,
            9 => 0,
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

pub(crate) trait DmaMemory {
    fn dma_read_hword(&self, addr: u32) -> u16;
    fn dma_read_word(&self, addr: u32) -> u32;
    fn dma_write_hword(&mut self, addr: u32, val: u16);
    fn dma_write_word(&mut self, addr: u32, val: u32);
}

use bit::BitIndex;

use crate::bus::Bus;
use crate::control::InterruptType;

mod background;
mod render;
mod sprite;

pub(crate) const WIDTH: usize = 240;
pub(crate) const HEIGHT: usize = 160;
const TOTAL_LINES: u16 = 228;
const HDRAW_CYCLES: u32 = 960;
const HBLANK_CYCLES: u32 = 272;
const SCANLINE_CYCLES: u32 = HDRAW_CYCLES + HBLANK_CYCLES;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum VideoEvent {
    None,
    HBlank,
    HBlankInVBlank,
    VBlank,
}

pub(crate) struct Video {
    frame_buffer: Box<[u32]>,

    dot_cycle: u32,
    vcount: u16,

    dispcnt: u16,
    dispstat: u16,

    bgcnt: [u16; 4],
    bgofs_x: [u16; 4],
    bgofs_y: [u16; 4],
    bgref_x: [u32; 2],
    bgref_y: [u32; 2],
    bgaffine: [[u16; 4]; 2],
    win_h: [u16; 2],
    win_v: [u16; 2],
    winin: u16,
    winout: u16,
    mosaic: u16,
    bldcnt: u16,
    bldalpha: u16,
    bldy: u16,

    pub(crate) vram: Box<[u8]>,
    pub(crate) palette: Box<[u8]>,
    pub(crate) oam: Box<[u8]>,
}

impl Video {
    pub(crate) fn new() -> Self {
        Self {
            frame_buffer: vec![0; WIDTH * HEIGHT].into_boxed_slice(),
            dot_cycle: 0,
            vcount: 0,
            dispcnt: 0,
            dispstat: 0,
            bgcnt: [0; 4],
            bgofs_x: [0; 4],
            bgofs_y: [0; 4],
            bgref_x: [0; 2],
            bgref_y: [0; 2],
            bgaffine: [[0; 4]; 2],
            win_h: [0; 2],
            win_v: [0; 2],
            winin: 0,
            winout: 0,
            mosaic: 0,
            bldcnt: 0,
            bldalpha: 0,
            bldy: 0,
            vram: vec![0; 96 * 1024].into_boxed_slice(),
            palette: vec![0; 1024].into_boxed_slice(),
            oam: vec![0; 1024].into_boxed_slice(),
        }
    }

    pub(crate) fn step(&mut self) -> (VideoEvent, Option<InterruptType>) {
        self.dot_cycle += 1;
        let mut event = VideoEvent::None;
        let mut irq: Option<InterruptType> = None;

        if self.dot_cycle == HDRAW_CYCLES {
            self.dispstat.set_bit(1, true);

            if self.vcount < HEIGHT as u16 {
                self.render_scanline();
                event = VideoEvent::HBlank;

                if self.dispstat.bit(4) {
                    irq = Some(InterruptType::HBlank);
                }
            } else {
                event = VideoEvent::HBlankInVBlank;
            }
        }

        if self.dot_cycle >= SCANLINE_CYCLES {
            self.dot_cycle = 0;
            self.dispstat.set_bit(1, false);
            self.vcount += 1;

            if self.vcount == HEIGHT as u16 {
                self.dispstat.set_bit(0, true);
                event = VideoEvent::VBlank;

                if self.dispstat.bit(3) {
                    irq = Some(InterruptType::VBlank);
                }
            }

            if self.vcount >= TOTAL_LINES {
                self.vcount = 0;
                self.dispstat.set_bit(0, false);
            }

            let lyc = self.dispstat.bit_range(8..16) as u16;
            let match_flag = self.vcount == lyc;
            self.dispstat.set_bit(2, match_flag);
            if match_flag && self.dispstat.bit(5) {
                irq = Some(InterruptType::VCounter);
            }
        }

        (event, irq)
    }

    pub(crate) fn framebuffer(&self) -> &[u32] {
        &self.frame_buffer
    }

    fn bg_mode(&self) -> u16 {
        self.dispcnt.bit_range(0..3) as u16
    }

    fn forced_blank(&self) -> bool {
        self.dispcnt.bit(7)
    }

    fn frame_select(&self) -> bool {
        self.dispcnt.bit(4)
    }

    fn palette_read_u16(&self, offset: usize) -> u16 {
        if offset + 1 < self.palette.len() {
            u16::from_le_bytes([self.palette[offset], self.palette[offset + 1]])
        } else {
            0
        }
    }

    fn rgb555_to_rgb888(&self, color: u16) -> u32 {
        let r = ((color & 0x1F) as u32) << 3;
        let g = (((color >> 5) & 0x1F) as u32) << 3;
        let b = (((color >> 10) & 0x1F) as u32) << 3;
        (r << 16) | (g << 8) | b
    }
}

impl Bus for Video {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x000 => self.dispcnt as u8,
            0x001 => (self.dispcnt >> 8) as u8,
            0x004 => self.dispstat as u8,
            0x005 => (self.dispstat >> 8) as u8,
            0x006 => self.vcount as u8,
            0x007 => (self.vcount >> 8) as u8,

            0x008..=0x00F => {
                let bg = ((addr - 0x008) / 2) as usize;
                if (addr & 1) == 0 {
                    self.bgcnt[bg] as u8
                } else {
                    (self.bgcnt[bg] >> 8) as u8
                }
            }

            0x010..=0x01F | 0x028..=0x03F | 0x020..=0x027 | 0x040..=0x04B => 0,

            0x050 => self.bldcnt as u8,
            0x051 => (self.bldcnt >> 8) as u8,
            0x052 => self.bldalpha as u8,
            0x053 => (self.bldalpha >> 8) as u8,
            0x054 | 0x055 => 0,

            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x000 => {
                self.dispcnt.set_bit_range(0..8, value as u16);
            }
            0x001 => {
                self.dispcnt.set_bit_range(8..16, value as u16);
            }
            0x004 => {
                let writable = value & 0x38;
                self.dispstat = (self.dispstat & !0x38) | (writable as u16);
            }
            0x005 => {
                self.dispstat.set_bit_range(8..16, value as u16);
            }
            0x006 | 0x007 => {}
            0x008..=0x00F => {
                let bg = ((addr - 0x008) / 2) as usize;
                if (addr & 1) == 0 {
                    self.bgcnt[bg].set_bit_range(0..8, value as u16);
                } else {
                    self.bgcnt[bg].set_bit_range(8..16, value as u16);
                }
            }

            0x010..=0x01F => {
                let reg = (addr - 0x010) as usize;
                let bg = reg / 4;
                let sub = reg % 4;
                match sub {
                    0 => {
                        self.bgofs_x[bg].set_bit_range(0..8, value as u16);
                    }
                    1 => {
                        self.bgofs_x[bg].set_bit_range(8..16, (value & 0x01) as u16);
                    }
                    2 => {
                        self.bgofs_y[bg].set_bit_range(0..8, value as u16);
                    }
                    3 => {
                        self.bgofs_y[bg].set_bit_range(8..16, (value & 0x01) as u16);
                    }
                    _ => {}
                }
            }
            0x020..=0x027 => {
                let reg = (addr - 0x020) as usize;
                let param = reg / 2;
                if (addr & 1) == 0 {
                    self.bgaffine[0][param].set_bit_range(0..8, value as u16);
                } else {
                    self.bgaffine[0][param].set_bit_range(8..16, value as u16);
                }
            }

            0x028..=0x02B => {
                let shift = ((addr - 0x028) * 8) as usize;
                self.bgref_x[0] = (self.bgref_x[0] & !(0xFF << shift)) | ((value as u32) << shift);
            }
            0x02C..=0x02F => {
                let shift = ((addr - 0x02C) * 8) as usize;
                self.bgref_y[0] = (self.bgref_y[0] & !(0xFF << shift)) | ((value as u32) << shift);
            }

            0x030..=0x037 => {
                let reg = (addr - 0x030) as usize;
                let param = reg / 2;
                if (addr & 1) == 0 {
                    self.bgaffine[1][param].set_bit_range(0..8, value as u16);
                } else {
                    self.bgaffine[1][param].set_bit_range(8..16, value as u16);
                }
            }

            0x038..=0x03B => {
                let shift = ((addr - 0x038) * 8) as usize;
                self.bgref_x[1] = (self.bgref_x[1] & !(0xFF << shift)) | ((value as u32) << shift);
            }
            0x03C..=0x03F => {
                let shift = ((addr - 0x03C) * 8) as usize;
                self.bgref_y[1] = (self.bgref_y[1] & !(0xFF << shift)) | ((value as u32) << shift);
            }

            0x040 => {
                self.win_h[0].set_bit_range(0..8, value as u16);
            }
            0x041 => {
                self.win_h[0].set_bit_range(8..16, value as u16);
            }
            0x042 => {
                self.win_h[1].set_bit_range(0..8, value as u16);
            }
            0x043 => {
                self.win_h[1].set_bit_range(8..16, value as u16);
            }

            0x044 => {
                self.win_v[0].set_bit_range(0..8, value as u16);
            }
            0x045 => {
                self.win_v[0].set_bit_range(8..16, value as u16);
            }
            0x046 => {
                self.win_v[1].set_bit_range(0..8, value as u16);
            }
            0x047 => {
                self.win_v[1].set_bit_range(8..16, value as u16);
            }

            0x048 => {
                self.winin.set_bit_range(0..8, value as u16);
            }
            0x049 => {
                self.winin.set_bit_range(8..16, value as u16);
            }

            0x04A => {
                self.winout.set_bit_range(0..8, value as u16);
            }
            0x04B => {
                self.winout.set_bit_range(8..16, value as u16);
            }

            0x04C => {
                self.mosaic.set_bit_range(0..8, value as u16);
            }
            0x04D => {
                self.mosaic.set_bit_range(8..16, value as u16);
            }

            0x050 => {
                self.bldcnt.set_bit_range(0..8, value as u16);
            }
            0x051 => {
                self.bldcnt.set_bit_range(8..16, value as u16);
            }

            0x052 => {
                self.bldalpha.set_bit_range(0..8, value as u16);
            }
            0x053 => {
                self.bldalpha.set_bit_range(8..16, value as u16);
            }

            0x054 => {
                self.bldy.set_bit_range(0..8, value as u16);
            }
            0x055 => {
                self.bldy.set_bit_range(8..16, value as u16);
            }

            _ => {}
        }
    }
}

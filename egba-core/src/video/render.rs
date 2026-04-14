use crate::video::sprite::Sprite;

use super::{Video, HEIGHT, WIDTH};
use bit::BitIndex;

impl Video {
    pub(crate) fn render_scanline(&mut self) {
        let y = self.vcount as usize;
        if y >= HEIGHT {
            return;
        }

        if self.forced_blank() {
            for x in 0..WIDTH {
                self.frame_buffer[y * WIDTH + x] = 0x00FFFFFF;
            }
            return;
        }

        let mut line_buffer = [(0u32, 4u8); WIDTH];
        let backdrop = self.rgb555_to_rgb888(self.palette_read_u16(0));
        for x in 0..WIDTH {
            line_buffer[x] = (backdrop, 4);
        }

        let mode = self.bg_mode();

        if mode == 3 || mode == 4 || mode == 5 {
            match mode {
                3 => self.render_mode3(y, &mut line_buffer),
                4 => self.render_mode4(y, &mut line_buffer),
                5 => self.render_mode5(y, &mut line_buffer),
                _ => {}
            }
        } else {
            for prio in (0..=3).rev() {
                for bg in (0..=3).rev() {
                    if self.dispcnt.bit(8 + bg) {
                        let bgcnt = self.bgcnt[bg as usize];
                        let bg_prio = (bgcnt & 3) as u8;
                        if bg_prio == prio {
                            if mode == 0 || (mode == 1 && bg <= 2) || (mode == 2 && bg >= 2) {
                                self.render_text_bg(bg as usize, y, &mut line_buffer, prio);
                            }
                        }
                    }
                }

                if self.dispcnt.bit(12) {
                    self.render_sprites(y, &mut line_buffer, prio);
                }
            }
        }

        for x in 0..WIDTH {
            self.frame_buffer[y * WIDTH + x] = line_buffer[x].0;
        }
    }

    fn render_mode3(&self, y: usize, line: &mut [(u32, u8)]) {
        for x in 0..WIDTH {
            let offset = (y * WIDTH + x) * 2;
            let color = u16::from_le_bytes([self.vram[offset], self.vram[offset + 1]]);
            line[x].0 = self.rgb555_to_rgb888(color);
        }
    }

    fn render_mode4(&self, y: usize, line: &mut [(u32, u8)]) {
        let base = if self.frame_select() { 0xA000 } else { 0 };
        for x in 0..WIDTH {
            let idx = self.vram[base + y * WIDTH + x] as usize;
            if idx != 0 {
                let color = self.palette_read_u16(idx * 2);
                line[x].0 = self.rgb555_to_rgb888(color);
            }
        }
    }

    fn render_mode5(&self, y: usize, line: &mut [(u32, u8)]) {
        let base = if self.frame_select() { 0xA000 } else { 0 };
        if y >= 128 {
            return;
        }
        for x in 0..WIDTH {
            if x >= 160 {
                continue;
            }
            let offset = base + (y * 160 + x) * 2;
            let color = u16::from_le_bytes([self.vram[offset], self.vram[offset + 1]]);
            line[x].0 = self.rgb555_to_rgb888(color);
        }
    }

    fn render_text_bg(&self, bg: usize, y: usize, line: &mut [(u32, u8)], prio: u8) {
        let bgcnt = self.bgcnt[bg];
        let char_base = ((bgcnt >> 2) & 3) as usize * 0x4000;
        let screen_base = ((bgcnt >> 8) & 0x1F) as usize * 0x800;
        let is_8bpp = (bgcnt & 0x80) != 0;

        let screen_size = (bgcnt >> 14) & 3;
        let layout_width = if screen_size == 1 || screen_size == 3 {
            512
        } else {
            256
        };
        let layout_height = if screen_size == 2 || screen_size == 3 {
            512
        } else {
            256
        };

        let scroll_x = self.bgofs_x[bg] as usize & 0x1FF;
        let scroll_y = self.bgofs_y[bg] as usize & 0x1FF;

        let map_y = (y + scroll_y) % layout_height;
        let tile_y = map_y / 8;
        let pixel_y = map_y % 8;

        for x in 0..WIDTH {
            let map_x = (x + scroll_x) % layout_width;
            let tile_x = map_x / 8;
            let pixel_x = map_x % 8;

            let mut sbb_offset = 0;
            if screen_size == 1 {
                if map_x >= 256 {
                    sbb_offset = 1;
                }
            } else if screen_size == 2 {
                if map_y >= 256 {
                    sbb_offset = 1;
                }
            } else if screen_size == 3 {
                sbb_offset = (map_y / 256) * 2 + (map_x / 256);
            }

            let local_tile_x = tile_x % 32;
            let local_tile_y = tile_y % 32;
            let map_addr =
                screen_base + sbb_offset * 0x800 + (local_tile_y * 32 + local_tile_x) * 2;

            let tile_entry = u16::from_le_bytes([self.vram[map_addr], self.vram[map_addr + 1]]);
            let tile_id = (tile_entry & 0x3FF) as usize;
            let h_flip = (tile_entry & 0x0400) != 0;
            let v_flip = (tile_entry & 0x0800) != 0;
            let pal_bank = ((tile_entry >> 12) & 0xF) as usize;

            let final_pixel_x = if h_flip { 7 - pixel_x } else { pixel_x };
            let final_pixel_y = if v_flip { 7 - pixel_y } else { pixel_y };

            if is_8bpp {
                let tile_addr = char_base + tile_id * 64 + final_pixel_y * 8 + final_pixel_x;
                let color_idx = self.vram[tile_addr] as usize;

                if color_idx != 0 {
                    let color = self.palette_read_u16(color_idx * 2);
                    line[x] = (self.rgb555_to_rgb888(color), prio);
                }
            } else {
                let tile_addr = char_base + tile_id * 32 + final_pixel_y * 4 + (final_pixel_x / 2);
                let byte = self.vram[tile_addr];
                let color_idx = if final_pixel_x % 2 == 0 {
                    byte & 0xF
                } else {
                    byte >> 4
                } as usize;

                if color_idx != 0 {
                    let pal_offset = (pal_bank * 16 + color_idx) * 2;
                    let color = self.palette_read_u16(pal_offset);
                    line[x] = (self.rgb555_to_rgb888(color), prio);
                }
            }
        }
    }

    fn render_sprites(&self, y: usize, line: &mut [(u32, u8)], prio: u8) {
        let is_1d_mapping = self.dispcnt.bit(6);
        for i in (0..128).rev() {
            let offset = i * 8;
            let attr0 = u16::from_le_bytes([self.oam[offset], self.oam[offset + 1]]);
            let attr1 = u16::from_le_bytes([self.oam[offset + 2], self.oam[offset + 3]]);
            let attr2 = u16::from_le_bytes([self.oam[offset + 4], self.oam[offset + 5]]);

            let sprite = Sprite::new(attr0, attr1, attr2);

            if sprite.priority != prio {
                continue;
            }

            if sprite.affine || sprite.double_or_disable {
                continue;
            }

            if sprite.mode == 2 {
                continue;
            }

            let (w, h) = sprite.dimensions();

            let sy = sprite.y;
            let mut ly = y as i16 - sy;

            if ly < 0 {
                ly += 256;
            }

            if ly < 0 || ly >= h {
                continue;
            }

            let local_y = if sprite.v_flip { h - 1 - ly } else { ly };

            for lx in 0..w {
                let screen_x = sprite.x + lx;
                if screen_x < 0 || screen_x >= WIDTH as i16 {
                    continue;
                }

                let local_x = if sprite.h_flip { w - 1 - lx } else { lx };

                let tile_x = local_x / 8;
                let tile_y = local_y / 8;
                let pixel_x = local_x % 8;
                let pixel_y = local_y % 8;

                let tile_id = if is_1d_mapping {
                    let tile_offset = tile_y * (w / 8) + tile_x;
                    if sprite.is_8bpp {
                        sprite.tile_id + (tile_offset * 2) as u16
                    } else {
                        sprite.tile_id + tile_offset as u16
                    }
                } else {
                    sprite.tile_id + (tile_y * 32) as u16 + tile_x as u16
                };

                let base_addr = 0x10000;

                if sprite.is_8bpp {
                    let tile_addr = base_addr
                        + (tile_id as usize & 0x3FF) * 64
                        + (pixel_y as usize) * 8
                        + (pixel_x as usize);
                    if tile_addr < self.vram.len() {
                        let color_idx = self.vram[tile_addr] as usize;
                        if color_idx != 0 {
                            let color = self.palette_read_u16(0x200 + color_idx * 2);
                            line[screen_x as usize] = (self.rgb555_to_rgb888(color), prio);
                        }
                    }
                } else {
                    let tile_addr = base_addr
                        + (tile_id as usize & 0x3FF) * 32
                        + (pixel_y as usize) * 4
                        + (pixel_x as usize / 2);
                    if tile_addr < self.vram.len() {
                        let byte = self.vram[tile_addr];
                        let color_idx = if pixel_x % 2 == 0 {
                            byte & 0xF
                        } else {
                            byte >> 4
                        } as usize;

                        if color_idx != 0 {
                            let pal_offset =
                                0x200 + (sprite.palette_bank as usize * 16 + color_idx) * 2;
                            let color = self.palette_read_u16(pal_offset);
                            line[screen_x as usize] = (self.rgb555_to_rgb888(color), prio);
                        }
                    }
                }
            }
        }
    }
}

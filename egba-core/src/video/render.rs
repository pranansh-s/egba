#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

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

        // Bitmap modes (3, 4, 5) render background differently but still support sprites
        if mode == 3 || mode == 4 || mode == 5 {
            match mode {
                3 => self.render_mode3(y, &mut line_buffer),
                4 => self.render_mode4(y, &mut line_buffer),
                5 => self.render_mode5(y, &mut line_buffer),
                _ => {}
            }

            // Bitmap modes can still display sprites if OBJ is enabled
            if self.dispcnt.bit(12) {
                for prio in (0..=3).rev() {
                    self.render_sprites(y, &mut line_buffer, prio);
                }
            }
        } else {
            // Text and affine background modes
            for prio in (0..=3).rev() {
                for bg in (0..=3).rev() {
                    if self.dispcnt.bit(8 + bg) {
                        let bgcnt = self.bgcnt[bg];
                        let bg_prio = (bgcnt & 3) as u8;
                        if bg_prio == prio
                            && (mode == 0 || (mode == 1 && bg <= 2) || (mode == 2 && bg >= 2))
                        {
                            self.render_text_bg(bg, y, &mut line_buffer, prio);
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

            // For non-affine sprites: double_or_disable bit is a disable flag
            // For affine sprites: double_or_disable bit is a "double size" flag (not disable)
            if !sprite.affine && sprite.double_or_disable {
                continue;
            }

            // Skip OBJ Window mode sprites (mode 2) - not implemented yet
            if sprite.mode == 2 {
                continue;
            }

            let (w, h) = sprite.dimensions();

            // Handle affine double-size
            let (w, h) = if sprite.affine && sprite.double_or_disable {
                (w * 2, h * 2)
            } else {
                (w, h)
            };

            let sy = sprite.y;
            let mut ly = y as i16 - sy;

            // Adjust for double-size vertical
            if sprite.affine && sprite.double_or_disable {
                ly /= 2;
            }

            if ly < 0 {
                ly += 256;
            }

            if ly < 0 || ly >= h {
                continue;
            }

            if sprite.affine {
                // Affine sprite transformation
                self.render_affine_sprite(sprite, y, line, prio, is_1d_mapping, w, h);
            } else {
                // Normal (non-affine) sprite rendering
                self.render_normal_sprite(sprite, y, line, prio, is_1d_mapping, w, h, ly);
            }
        }
    }

    fn render_normal_sprite(
        &self,
        sprite: Sprite,
        _y: usize,
        line: &mut [(u32, u8)],
        prio: u8,
        is_1d_mapping: bool,
        w: i16,
        h: i16,
        ly: i16,
    ) {
        let local_y = if sprite.v_flip { h - 1 - ly } else { ly };

        for lx in 0..w {
            let screen_x = sprite.x + lx;
            if screen_x < 0 || screen_x >= WIDTH as i16 {
                continue;
            }

            let local_x = if sprite.h_flip { w - 1 - lx } else { lx };

            self.fetch_sprite_pixel(sprite, local_x, local_y, is_1d_mapping, |color| {
                line[screen_x as usize] = (color, prio);
            });
        }
    }

    fn render_affine_sprite(
        &self,
        sprite: Sprite,
        y: usize,
        line: &mut [(u32, u8)],
        prio: u8,
        is_1d_mapping: bool,
        w: i16,
        h: i16,
    ) {
        // Affine sprites use parameters stored in OAM (attr1 contains affine_param which indexes into BG affine params)
        // Actually, each affine sprite has its own transformation matrix stored in OAM after the 128 normal sprites
        // The affine parameters are stored at OAM[512 + sprite_index * 8 ..]
        // But for simplicity, we use the BG2 affine parameters as a fallback
        // Proper implementation: read from OAM affine buffer
        let oam_affine_offset = 512 + (sprite.affine_param as usize & 31) * 8;

        // Read affine parameters from OAM (each param is 16-bit signed fixed-point)
        let pa = if oam_affine_offset + 1 < self.oam.len() {
            i16::from_le_bytes([self.oam[oam_affine_offset], self.oam[oam_affine_offset + 1]])
        } else {
            256 // Default: identity (scale 1.0)
        };
        let pb = if oam_affine_offset + 3 < self.oam.len() {
            i16::from_le_bytes([
                self.oam[oam_affine_offset + 2],
                self.oam[oam_affine_offset + 3],
            ])
        } else {
            0
        };
        let pc = if oam_affine_offset + 5 < self.oam.len() {
            i16::from_le_bytes([
                self.oam[oam_affine_offset + 4],
                self.oam[oam_affine_offset + 5],
            ])
        } else {
            0
        };
        let pd = if oam_affine_offset + 7 < self.oam.len() {
            i16::from_le_bytes([
                self.oam[oam_affine_offset + 6],
                self.oam[oam_affine_offset + 7],
            ])
        } else {
            256 // Default: identity
        };

        // Center coordinates for affine transformation
        let center_x = w / 2;
        let center_y = h / 2;

        // Start position for this scanline (accounting for sprite position)
        let start_y = sprite.y;
        let dy = (y as i16) - start_y - center_y;

        for lx in 0..w {
            let screen_x = sprite.x + lx;
            if screen_x < 0 || screen_x >= WIDTH as i16 {
                continue;
            }

            let dx = lx - center_x;

            // Affine transformation: apply rotation/scaling matrix
            // tex_x = (dx * pa + dy * pb) >> 8 (8-bit fixed point)
            // tex_y = (dx * pc + dy * pd) >> 8
            let tex_x = ((dx as i32) * (pa as i32) + (dy as i32) * (pb as i32)) >> 8;
            let tex_y = ((dx as i32) * (pc as i32) + (dy as i32) * (pd as i32)) >> 8;

            if tex_x < 0 || tex_x >= (w as i32) || tex_y < 0 || tex_y >= (h as i32) {
                continue;
            }

            self.fetch_sprite_pixel(sprite, tex_x as i16, tex_y as i16, is_1d_mapping, |color| {
                line[screen_x as usize] = (color, prio);
            });
        }
    }

    fn fetch_sprite_pixel<F: FnMut(u32)>(
        &self,
        sprite: Sprite,
        local_x: i16,
        local_y: i16,
        is_1d_mapping: bool,
        mut callback: F,
    ) {
        let tile_x = (local_x / 8) as u16;
        let tile_y = (local_y / 8) as u16;
        let pixel_x = (local_x % 8) as usize;
        let pixel_y = (local_y % 8) as usize;

        let tile_id = if is_1d_mapping {
            let (w, _h) = sprite.dimensions();
            let tile_offset = tile_y * (w / 8) as u16 + tile_x;
            if sprite.is_8bpp {
                sprite.tile_id + (tile_offset * 2)
            } else {
                sprite.tile_id + tile_offset
            }
        } else {
            sprite.tile_id + (tile_y * 32) + tile_x
        };

        let base_addr = 0x10000;

        if sprite.is_8bpp {
            let tile_addr = base_addr + (tile_id as usize & 0x3FF) * 64 + pixel_y * 8 + pixel_x;
            if tile_addr < self.vram.len() {
                let color_idx = self.vram[tile_addr] as usize;
                if color_idx != 0 {
                    let color = self.palette_read_u16(0x200 + color_idx * 2);
                    callback(self.rgb555_to_rgb888(color));
                }
            }
        } else {
            let tile_addr =
                base_addr + (tile_id as usize & 0x3FF) * 32 + pixel_y * 4 + (pixel_x / 2);
            if tile_addr < self.vram.len() {
                let byte = self.vram[tile_addr];
                let color_idx = if pixel_x.is_multiple_of(2) {
                    byte & 0xF
                } else {
                    byte >> 4
                } as usize;

                if color_idx != 0 {
                    let pal_offset = 0x200 + (sprite.palette_bank as usize * 16 + color_idx) * 2;
                    let color = self.palette_read_u16(pal_offset);
                    callback(self.rgb555_to_rgb888(color));
                }
            }
        }
    }
}

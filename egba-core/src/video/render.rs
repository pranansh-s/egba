#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use crate::video::sprite::Sprite;

use super::{Video, HEIGHT, WIDTH};
use bit::BitIndex;

/// Window mask bits: which layers are visible at a given pixel
#[allow(dead_code)]
const WIN_BG0: u8 = 1 << 0;
#[allow(dead_code)]
const WIN_BG1: u8 = 1 << 1;
#[allow(dead_code)]
const WIN_BG2: u8 = 1 << 2;
#[allow(dead_code)]
const WIN_BG3: u8 = 1 << 3;
const WIN_OBJ: u8 = 1 << 4;
const WIN_SFX: u8 = 1 << 5;
/// All layers + effects enabled (used when windows are disabled)
const WIN_ALL: u8 = 0x3F;

/// Per-pixel layer info for compositing and blending
#[derive(Clone, Copy)]
struct PixelInfo {
    color: u32,
    priority: u8,
    /// Which layer produced this pixel (0-3 for BG, 4 for OBJ, 5 for backdrop)
    layer: u8,
    /// Is this pixel semi-transparent (OBJ mode 1)?
    semi_transparent: bool,
}

impl Default for PixelInfo {
    fn default() -> Self {
        Self {
            color: 0,
            priority: 4,
            layer: 5,
            semi_transparent: false,
        }
    }
}

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

        // Two-layer compositing buffer: top and second pixel per column
        let backdrop = self.rgb555_to_rgb888(self.palette_read_u16(0));
        let backdrop_pixel = PixelInfo {
            color: backdrop,
            priority: 4,
            layer: 5,
            semi_transparent: false,
        };
        let mut top = [backdrop_pixel; WIDTH];
        let mut second = [backdrop_pixel; WIDTH];

        // Compute OBJ window mask (mode-2 sprites) before window evaluation
        let mut objwin_mask = [false; WIDTH];
        let any_window_enabled = self.win0_enabled() || self.win1_enabled() || self.objwin_enabled();
        if any_window_enabled && self.objwin_enabled() && self.dispcnt.bit(12) {
            self.render_objwin_mask(y, &mut objwin_mask);
        }

        // Compute per-pixel window mask (which layers/effects are enabled at each pixel)
        let win_mask = self.compute_window_mask(y, &objwin_mask);

        let mode = self.bg_mode();

        match mode {
            0 => {
                // Mode 0: 4 text BGs
                self.render_tiled_bgs(y, &mut top, &mut second, &[0, 1, 2, 3], &[false; 4], &win_mask);
            }
            1 => {
                // Mode 1: BG0, BG1 text; BG2 affine
                self.render_tiled_bgs(y, &mut top, &mut second, &[0, 1, 2], &[false, false, true], &win_mask);
            }
            2 => {
                // Mode 2: BG2, BG3 affine
                self.render_tiled_bgs(y, &mut top, &mut second, &[2, 3], &[true, true], &win_mask);
            }
            3 => self.render_mode3(y, &mut top),
            4 => self.render_mode4(y, &mut top),
            5 => self.render_mode5(y, &mut top),
            _ => {}
        }

        // Render sprites on top (all modes can have sprites)
        if self.dispcnt.bit(12) {
            for prio in (0..=3).rev() {
                self.render_sprites_layered(y, &mut top, &mut second, prio, &win_mask);
            }
        }

        // Apply color special effects and write to framebuffer
        self.apply_blending(y, &top, &second, &win_mask);
    }

    /// Render tiled backgrounds (both text and affine) with priority compositing
    fn render_tiled_bgs(
        &self,
        y: usize,
        top: &mut [PixelInfo; WIDTH],
        second: &mut [PixelInfo; WIDTH],
        bgs: &[usize],
        is_affine: &[bool],
        win_mask: &[u8; WIDTH],
    ) {
        // Render from lowest to highest priority so higher-priority layers overwrite
        for prio in (0..=3u8).rev() {
            for (idx, &bg) in bgs.iter().enumerate().rev() {
                if !self.dispcnt.bit(8 + bg) {
                    continue;
                }
                let bg_prio = (self.bgcnt[bg] & 3) as u8;
                if bg_prio != prio {
                    continue;
                }

                if is_affine.get(idx).copied().unwrap_or(false) {
                    self.render_affine_bg(bg, y, top, second, prio, win_mask);
                } else {
                    self.render_text_bg(bg, y, top, second, prio, win_mask);
                }
            }
        }
    }

    fn render_mode3(&self, y: usize, top: &mut [PixelInfo; WIDTH]) {
        for x in 0..WIDTH {
            let offset = (y * WIDTH + x) * 2;
            if offset + 1 < self.vram.len() {
                let color = u16::from_le_bytes([self.vram[offset], self.vram[offset + 1]]);
                top[x] = PixelInfo {
                    color: self.rgb555_to_rgb888(color),
                    priority: 0,
                    layer: 2, // BG2 in bitmap modes
                    semi_transparent: false,
                };
            }
        }
    }

    fn render_mode4(&self, y: usize, top: &mut [PixelInfo; WIDTH]) {
        let base = if self.frame_select() { 0xA000 } else { 0 };
        for x in 0..WIDTH {
            let idx = self.vram[base + y * WIDTH + x] as usize;
            if idx != 0 {
                let color = self.palette_read_u16(idx * 2);
                top[x] = PixelInfo {
                    color: self.rgb555_to_rgb888(color),
                    priority: 0,
                    layer: 2,
                    semi_transparent: false,
                };
            }
        }
    }

    fn render_mode5(&self, y: usize, top: &mut [PixelInfo; WIDTH]) {
        let base = if self.frame_select() { 0xA000 } else { 0 };
        if y >= 128 {
            return;
        }
        for x in 0..WIDTH {
            if x >= 160 {
                continue;
            }
            let offset = base + (y * 160 + x) * 2;
            if offset + 1 < self.vram.len() {
                let color = u16::from_le_bytes([self.vram[offset], self.vram[offset + 1]]);
                top[x] = PixelInfo {
                    color: self.rgb555_to_rgb888(color),
                    priority: 0,
                    layer: 2,
                    semi_transparent: false,
                };
            }
        }
    }

    fn render_text_bg(
        &self,
        bg: usize,
        y: usize,
        top: &mut [PixelInfo; WIDTH],
        second: &mut [PixelInfo; WIDTH],
        prio: u8,
        win_mask: &[u8; WIDTH],
    ) {
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

        let bg_win_bit = 1u8 << bg;
        for x in 0..WIDTH {
            // Window check: skip this pixel if the BG is masked out
            if win_mask[x] & bg_win_bit == 0 {
                continue;
            }
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

            if map_addr + 1 >= self.vram.len() {
                continue;
            }

            let tile_entry = u16::from_le_bytes([self.vram[map_addr], self.vram[map_addr + 1]]);
            let tile_id = (tile_entry & 0x3FF) as usize;
            let h_flip = (tile_entry & 0x0400) != 0;
            let v_flip = (tile_entry & 0x0800) != 0;
            let pal_bank = ((tile_entry >> 12) & 0xF) as usize;

            let final_pixel_x = if h_flip { 7 - pixel_x } else { pixel_x };
            let final_pixel_y = if v_flip { 7 - pixel_y } else { pixel_y };

            let color_rgb = if is_8bpp {
                let tile_addr = char_base + tile_id * 64 + final_pixel_y * 8 + final_pixel_x;
                if tile_addr >= self.vram.len() {
                    continue;
                }
                let color_idx = self.vram[tile_addr] as usize;
                if color_idx == 0 {
                    continue;
                }
                let color = self.palette_read_u16(color_idx * 2);
                self.rgb555_to_rgb888(color)
            } else {
                let tile_addr = char_base + tile_id * 32 + final_pixel_y * 4 + (final_pixel_x / 2);
                if tile_addr >= self.vram.len() {
                    continue;
                }
                let byte = self.vram[tile_addr];
                let color_idx = if final_pixel_x % 2 == 0 {
                    byte & 0xF
                } else {
                    byte >> 4
                } as usize;
                if color_idx == 0 {
                    continue;
                }
                let pal_offset = (pal_bank * 16 + color_idx) * 2;
                let color = self.palette_read_u16(pal_offset);
                self.rgb555_to_rgb888(color)
            };

            let pixel = PixelInfo {
                color: color_rgb,
                priority: prio,
                layer: bg as u8,
                semi_transparent: false,
            };

            // Compositing: if this pixel has higher or equal priority than the current top,
            // push top to second and replace
            if prio <= top[x].priority {
                second[x] = top[x];
                top[x] = pixel;
            } else if prio <= second[x].priority {
                second[x] = pixel;
            }
        }
    }

    /// Render an affine (rotation/scaling) background
    fn render_affine_bg(
        &self,
        bg: usize,
        _y: usize,
        top: &mut [PixelInfo; WIDTH],
        second: &mut [PixelInfo; WIDTH],
        prio: u8,
        win_mask: &[u8; WIDTH],
    ) {
        let bgcnt = self.bgcnt[bg];
        let char_base = ((bgcnt >> 2) & 3) as usize * 0x4000;
        let screen_base = ((bgcnt >> 8) & 0x1F) as usize * 0x800;
        let wrap = bgcnt.bit(13);

        // Affine BG sizes: 0=128, 1=256, 2=512, 3=1024
        let size = match (bgcnt >> 14) & 3 {
            0 => 128usize,
            1 => 256,
            2 => 512,
            3 => 1024,
            _ => 128,
        };
        let tiles_per_row = size / 8;

        // Use internal reference points and affine parameters
        // BG2 uses index 0, BG3 uses index 1
        let affine_idx = bg - 2;

        // The reference points are 28-bit signed fixed-point (20.8)
        let ref_x = self.internal_ref_x[affine_idx];
        let ref_y = self.internal_ref_y[affine_idx];

        // PA, PB, PC, PD are 16-bit signed fixed-point (8.8)
        let pa = self.bgaffine[affine_idx][0] as i16 as i32;
        let pc = self.bgaffine[affine_idx][2] as i16 as i32;

        let bg_win_bit = 1u8 << bg;
        for x in 0..WIDTH {
            // Window check: skip this pixel if the BG is masked out
            if win_mask[x] & bg_win_bit == 0 {
                continue;
            }
            // Texture coordinates from reference point + PA * x (PB was already added per-scanline)
            let tex_x = (ref_x + pa * (x as i32)) >> 8;
            let tex_y = (ref_y + pc * (x as i32)) >> 8;

            let (tx, ty) = if wrap {
                (
                    ((tex_x % size as i32) + size as i32) as usize % size,
                    ((tex_y % size as i32) + size as i32) as usize % size,
                )
            } else {
                if tex_x < 0 || tex_x >= size as i32 || tex_y < 0 || tex_y >= size as i32 {
                    continue;
                }
                (tex_x as usize, tex_y as usize)
            };

            let tile_x = tx / 8;
            let tile_y = ty / 8;
            let pixel_x = tx % 8;
            let pixel_y = ty % 8;

            // Affine BGs always use 8bpp, single screenblock map (1 byte per tile entry)
            let map_addr = screen_base + tile_y * tiles_per_row + tile_x;
            if map_addr >= self.vram.len() {
                continue;
            }

            let tile_id = self.vram[map_addr] as usize;
            let tile_addr = char_base + tile_id * 64 + pixel_y * 8 + pixel_x;
            if tile_addr >= self.vram.len() {
                continue;
            }

            let color_idx = self.vram[tile_addr] as usize;
            if color_idx == 0 {
                continue;
            }

            let color = self.palette_read_u16(color_idx * 2);
            let pixel = PixelInfo {
                color: self.rgb555_to_rgb888(color),
                priority: prio,
                layer: bg as u8,
                semi_transparent: false,
            };

            if prio <= top[x].priority {
                second[x] = top[x];
                top[x] = pixel;
            } else if prio <= second[x].priority {
                second[x] = pixel;
            }
        }
    }

    /// Render sprites with layered compositing
    fn render_sprites_layered(
        &self,
        y: usize,
        top: &mut [PixelInfo; WIDTH],
        second: &mut [PixelInfo; WIDTH],
        prio: u8,
        win_mask: &[u8; WIDTH],
    ) {
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

            if !sprite.affine && sprite.double_or_disable {
                continue;
            }

            // OBJ Window mode sprites (mode 2) are rendered separately in render_objwin_mask
            if sprite.mode == 2 {
                continue;
            }

            let is_semi_transparent = sprite.mode == 1;
            let (orig_w, orig_h) = sprite.dimensions();

            // For affine double-size, the bounding box is doubled but texture dims stay the same
            let (bound_w, bound_h) = if sprite.affine && sprite.double_or_disable {
                (orig_w * 2, orig_h * 2)
            } else {
                (orig_w, orig_h)
            };

            let sy = sprite.y;
            let mut ly = y as i16 - sy;

            if ly < 0 {
                ly += 256;
            }

            if ly < 0 || ly >= bound_h {
                continue;
            }

            if sprite.affine {
                self.render_affine_sprite_layered(
                    sprite,
                    y,
                    top,
                    second,
                    prio,
                    is_1d_mapping,
                    orig_w,
                    orig_h,
                    bound_w,
                    bound_h,
                    is_semi_transparent,
                    win_mask,
                );
            } else {
                self.render_normal_sprite_layered(
                    sprite,
                    top,
                    second,
                    prio,
                    is_1d_mapping,
                    orig_w,
                    orig_h,
                    ly,
                    is_semi_transparent,
                    win_mask,
                );
            }
        }
    }

    fn render_normal_sprite_layered(
        &self,
        sprite: Sprite,
        top: &mut [PixelInfo; WIDTH],
        second: &mut [PixelInfo; WIDTH],
        prio: u8,
        is_1d_mapping: bool,
        w: i16,
        h: i16,
        ly: i16,
        semi_transparent: bool,
        win_mask: &[u8; WIDTH],
    ) {
        let local_y = if sprite.v_flip { h - 1 - ly } else { ly };

        for lx in 0..w {
            let screen_x = sprite.x + lx;
            if screen_x < 0 || screen_x >= WIDTH as i16 {
                continue;
            }
            // Window check: skip this pixel if OBJ layer is masked out
            if win_mask[screen_x as usize] & WIN_OBJ == 0 {
                continue;
            }

            let local_x = if sprite.h_flip { w - 1 - lx } else { lx };
            let sx = screen_x as usize;

            self.fetch_sprite_pixel(sprite, local_x, local_y, is_1d_mapping, |color| {
                let pixel = PixelInfo {
                    color,
                    priority: prio,
                    layer: 4, // OBJ layer
                    semi_transparent,
                };
                if prio <= top[sx].priority {
                    second[sx] = top[sx];
                    top[sx] = pixel;
                } else if prio <= second[sx].priority {
                    second[sx] = pixel;
                }
            });
        }
    }

    fn render_affine_sprite_layered(
        &self,
        sprite: Sprite,
        y: usize,
        top: &mut [PixelInfo; WIDTH],
        second: &mut [PixelInfo; WIDTH],
        prio: u8,
        is_1d_mapping: bool,
        orig_w: i16,
        orig_h: i16,
        bound_w: i16,
        bound_h: i16,
        semi_transparent: bool,
        win_mask: &[u8; WIDTH],
    ) {
        // Affine sprite parameters are stored interleaved in OAM:
        // OAM entry N (8 bytes each): bytes 6-7 contain one affine parameter
        // Group G has PA at entry G*4+0 byte 6, PB at G*4+1 byte 6, etc.
        let group = sprite.affine_param as usize;
        let pa = self.read_oam_affine_param(group, 0);
        let pb = self.read_oam_affine_param(group, 1);
        let pc = self.read_oam_affine_param(group, 2);
        let pd = self.read_oam_affine_param(group, 3);

        // Center coordinates use the original (un-doubled) dimensions
        let center_x = bound_w / 2;
        let center_y = bound_h / 2;

        let half_orig_w = orig_w / 2;
        let half_orig_h = orig_h / 2;

        let dy = (y as i16) - sprite.y - center_y;

        for lx in 0..bound_w {
            let screen_x = sprite.x + lx;
            if screen_x < 0 || screen_x >= WIDTH as i16 {
                continue;
            }
            // Window check: skip this pixel if OBJ layer is masked out
            if win_mask[screen_x as usize] & WIN_OBJ == 0 {
                continue;
            }

            let dx = lx - center_x;

            // Apply affine transformation to get texture coordinates
            let tex_x = ((dx as i32 * pa as i32 + dy as i32 * pb as i32) >> 8) + half_orig_w as i32;
            let tex_y = ((dx as i32 * pc as i32 + dy as i32 * pd as i32) >> 8) + half_orig_h as i32;

            if tex_x < 0 || tex_x >= orig_w as i32 || tex_y < 0 || tex_y >= orig_h as i32 {
                continue;
            }

            let sx = screen_x as usize;

            self.fetch_sprite_pixel(
                sprite,
                tex_x as i16,
                tex_y as i16,
                is_1d_mapping,
                |color| {
                    let pixel = PixelInfo {
                        color,
                        priority: prio,
                        layer: 4,
                        semi_transparent,
                    };
                    if prio <= top[sx].priority {
                        second[sx] = top[sx];
                        top[sx] = pixel;
                    } else if prio <= second[sx].priority {
                        second[sx] = pixel;
                    }
                },
            );
        }
    }

    /// Read one of the 4 affine parameters (PA/PB/PC/PD) for a given affine group.
    /// Parameters are stored at OAM[group*32 + param*8 + 6] (interleaved in OAM entries).
    fn read_oam_affine_param(&self, group: usize, param: usize) -> i16 {
        let offset = group * 32 + param * 8 + 6;
        if offset + 1 < self.oam.len() {
            i16::from_le_bytes([self.oam[offset], self.oam[offset + 1]])
        } else {
            if param == 0 || param == 3 {
                256 // Identity: PA=PD=1.0 in 8.8 fixed point
            } else {
                0 // PB=PC=0 (no rotation/shear)
            }
        }
    }

    /// Apply color special effects (blending) and write to framebuffer
    fn apply_blending(
        &mut self,
        y: usize,
        top: &[PixelInfo; WIDTH],
        second: &[PixelInfo; WIDTH],
        win_mask: &[u8; WIDTH],
    ) {
        let blend_mode = (self.bldcnt >> 6) & 3;
        let first_targets = self.bldcnt & 0x3F; // bits 0-5: BG0-3, OBJ, BD
        let second_targets = (self.bldcnt >> 8) & 0x3F;

        let eva = (self.bldalpha & 0x1F).min(16) as u32;
        let evb = ((self.bldalpha >> 8) & 0x1F).min(16) as u32;
        let evy = (self.bldy & 0x1F).min(16) as u32;

        for x in 0..WIDTH {
            let tp = top[x];
            let sp = second[x];

            // Window mask: only apply special effects if WIN_SFX is set for this pixel
            let sfx_enabled = win_mask[x] & WIN_SFX != 0;

            let is_first = self.layer_in_target(tp.layer, first_targets);
            let is_second = self.layer_in_target(sp.layer, second_targets);

            let final_color = if sfx_enabled && tp.semi_transparent && is_second {
                // Semi-transparent OBJ always uses alpha blend if second target exists
                self.alpha_blend(tp.color, sp.color, eva, evb)
            } else if sfx_enabled && is_first {
                match blend_mode {
                    1 if is_second => self.alpha_blend(tp.color, sp.color, eva, evb),
                    2 => self.brightness_increase(tp.color, evy),
                    3 => self.brightness_decrease(tp.color, evy),
                    _ => tp.color,
                }
            } else {
                tp.color
            };

            self.frame_buffer[y * WIDTH + x] = final_color;
        }
    }

    fn layer_in_target(&self, layer: u8, targets: u16) -> bool {
        match layer {
            0..=3 => targets.bit(layer as usize),
            4 => targets.bit(4), // OBJ
            5 => targets.bit(5), // Backdrop
            _ => false,
        }
    }

    fn alpha_blend(&self, color1: u32, color2: u32, eva: u32, evb: u32) -> u32 {
        let r1 = (color1 >> 16) & 0xFF;
        let g1 = (color1 >> 8) & 0xFF;
        let b1 = color1 & 0xFF;

        let r2 = (color2 >> 16) & 0xFF;
        let g2 = (color2 >> 8) & 0xFF;
        let b2 = color2 & 0xFF;

        let r = ((r1 * eva + r2 * evb) >> 4).min(255);
        let g = ((g1 * eva + g2 * evb) >> 4).min(255);
        let b = ((b1 * eva + b2 * evb) >> 4).min(255);

        (r << 16) | (g << 8) | b
    }

    fn brightness_increase(&self, color: u32, evy: u32) -> u32 {
        let r = (color >> 16) & 0xFF;
        let g = (color >> 8) & 0xFF;
        let b = color & 0xFF;

        let r = r + (((255 - r) * evy) >> 4);
        let g = g + (((255 - g) * evy) >> 4);
        let b = b + (((255 - b) * evy) >> 4);

        (r.min(255) << 16) | (g.min(255) << 8) | b.min(255)
    }

    fn brightness_decrease(&self, color: u32, evy: u32) -> u32 {
        let r = (color >> 16) & 0xFF;
        let g = (color >> 8) & 0xFF;
        let b = color & 0xFF;

        let r = r - ((r * evy) >> 4);
        let g = g - ((g * evy) >> 4);
        let b = b - ((b * evy) >> 4);

        (r << 16) | (g << 8) | b
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

// Window mask computation
impl Video {
    /// Check if WIN0 is enabled (DISPCNT bit 13)
    fn win0_enabled(&self) -> bool {
        self.dispcnt.bit(13)
    }

    /// Check if WIN1 is enabled (DISPCNT bit 14)
    fn win1_enabled(&self) -> bool {
        self.dispcnt.bit(14)
    }

    /// Check if OBJ Window is enabled (DISPCNT bit 15)
    fn objwin_enabled(&self) -> bool {
        self.dispcnt.bit(15)
    }

    /// Compute per-pixel window mask for the current scanline.
    /// Returns a [u8; 240] where each byte contains the layer-enable bits
    /// for that pixel (WIN_BG0..WIN_SFX).
    /// If no windows are enabled, all pixels get WIN_ALL (everything enabled).
    fn compute_window_mask(&self, y: usize, objwin_mask: &[bool; WIDTH]) -> [u8; WIDTH] {
        let win0 = self.win0_enabled();
        let win1 = self.win1_enabled();
        let objwin = self.objwin_enabled();

        if !win0 && !win1 && !objwin {
            return [WIN_ALL; WIDTH];
        }

        // Pre-extract window enable bits
        let win0_enables = (self.winin & 0x3F) as u8;
        let win1_enables = ((self.winin >> 8) & 0x3F) as u8;
        let outside_enables = (self.winout & 0x3F) as u8;
        let objwin_enables = ((self.winout >> 8) & 0x3F) as u8;

        // WIN0 vertical bounds
        let win0_y1 = (self.win_v[0] >> 8) as usize;
        let win0_y2 = (self.win_v[0] & 0xFF) as usize;
        let win0_in_y = if win0_y1 <= win0_y2 {
            y >= win0_y1 && y < win0_y2
        } else {
            // Wraps around: y >= y1 OR y < y2
            y >= win0_y1 || y < win0_y2
        };

        // WIN1 vertical bounds
        let win1_y1 = (self.win_v[1] >> 8) as usize;
        let win1_y2 = (self.win_v[1] & 0xFF) as usize;
        let win1_in_y = if win1_y1 <= win1_y2 {
            y >= win1_y1 && y < win1_y2
        } else {
            y >= win1_y1 || y < win1_y2
        };

        // WIN0 horizontal bounds
        let win0_x1 = (self.win_h[0] >> 8) as usize;
        let win0_x2 = (self.win_h[0] & 0xFF) as usize;

        // WIN1 horizontal bounds
        let win1_x1 = (self.win_h[1] >> 8) as usize;
        let win1_x2 = (self.win_h[1] & 0xFF) as usize;

        let mut mask = [0u8; WIDTH];

        for x in 0..WIDTH {
            // Priority order: WIN0 > WIN1 > OBJWIN > Outside
            if win0 && win0_in_y {
                let in_x = if win0_x1 <= win0_x2 {
                    x >= win0_x1 && x < win0_x2
                } else {
                    x >= win0_x1 || x < win0_x2
                };
                if in_x {
                    mask[x] = win0_enables;
                    continue;
                }
            }

            if win1 && win1_in_y {
                let in_x = if win1_x1 <= win1_x2 {
                    x >= win1_x1 && x < win1_x2
                } else {
                    x >= win1_x1 || x < win1_x2
                };
                if in_x {
                    mask[x] = win1_enables;
                    continue;
                }
            }

            if objwin && objwin_mask[x] {
                mask[x] = objwin_enables;
                continue;
            }

            mask[x] = outside_enables;
        }

        mask
    }

    /// Render OBJ Window sprites (mode 2) into a boolean mask.
    /// A pixel is true if any mode-2 sprite covers it (non-transparent pixel).
    fn render_objwin_mask(&self, y: usize, mask: &mut [bool; WIDTH]) {
        let is_1d_mapping = self.dispcnt.bit(6);

        for i in (0..128).rev() {
            let offset = i * 8;
            let attr0 = u16::from_le_bytes([self.oam[offset], self.oam[offset + 1]]);
            let attr1 = u16::from_le_bytes([self.oam[offset + 2], self.oam[offset + 3]]);
            let attr2 = u16::from_le_bytes([self.oam[offset + 4], self.oam[offset + 5]]);

            let sprite = Sprite::new(attr0, attr1, attr2);

            // Only process mode-2 (OBJ Window) sprites
            if sprite.mode != 2 {
                continue;
            }
            if !sprite.affine && sprite.double_or_disable {
                continue;
            }

            let (orig_w, orig_h) = sprite.dimensions();
            let (bound_w, bound_h) = if sprite.affine && sprite.double_or_disable {
                (orig_w * 2, orig_h * 2)
            } else {
                (orig_w, orig_h)
            };

            let sy = sprite.y;
            let mut ly = y as i16 - sy;
            if ly < 0 {
                ly += 256;
            }
            if ly < 0 || ly >= bound_h {
                continue;
            }

            if sprite.affine {
                // Affine OBJ window rendering
                let group = sprite.affine_param as usize;
                let pa = self.read_oam_affine_param(group, 0);
                let pb = self.read_oam_affine_param(group, 1);
                let pc = self.read_oam_affine_param(group, 2);
                let pd = self.read_oam_affine_param(group, 3);

                let center_x = bound_w / 2;
                let center_y = bound_h / 2;
                let half_orig_w = orig_w / 2;
                let half_orig_h = orig_h / 2;

                let dy = (y as i16) - sprite.y - center_y;

                for lx in 0..bound_w {
                    let screen_x = sprite.x + lx;
                    if screen_x < 0 || screen_x >= WIDTH as i16 {
                        continue;
                    }

                    let dx = lx - center_x;
                    let tex_x = ((dx as i32 * pa as i32 + dy as i32 * pb as i32) >> 8) + half_orig_w as i32;
                    let tex_y = ((dx as i32 * pc as i32 + dy as i32 * pd as i32) >> 8) + half_orig_h as i32;

                    if tex_x < 0 || tex_x >= orig_w as i32 || tex_y < 0 || tex_y >= orig_h as i32 {
                        continue;
                    }

                    self.fetch_sprite_pixel(sprite, tex_x as i16, tex_y as i16, is_1d_mapping, |_color| {
                        mask[screen_x as usize] = true;
                    });
                }
            } else {
                // Normal OBJ window rendering
                let local_y = if sprite.v_flip { orig_h - 1 - ly } else { ly };

                for lx in 0..orig_w {
                    let screen_x = sprite.x + lx;
                    if screen_x < 0 || screen_x >= WIDTH as i16 {
                        continue;
                    }

                    let local_x = if sprite.h_flip { orig_w - 1 - lx } else { lx };

                    self.fetch_sprite_pixel(sprite, local_x, local_y, is_1d_mapping, |_color| {
                        mask[screen_x as usize] = true;
                    });
                }
            }
        }
    }
}

// Keep the old render_sprites method signature for backward compatibility
// (the old API is no longer called from render_scanline, but keeping it
// avoids breaking any external references)
impl Video {
    #[allow(dead_code)]
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
            if !sprite.affine && sprite.double_or_disable {
                continue;
            }
            if sprite.mode == 2 {
                continue;
            }

            let (w, h) = sprite.dimensions();
            let (w, h) = if sprite.affine && sprite.double_or_disable {
                (w * 2, h * 2)
            } else {
                (w, h)
            };

            let sy = sprite.y;
            let mut ly = y as i16 - sy;
            if ly < 0 {
                ly += 256;
            }
            if ly < 0 || ly >= h {
                continue;
            }

            // Normal sprite rendering for backward compat
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
    }
}

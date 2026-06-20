#[cfg(test)]
mod tests {
    use bit::BitIndex;
    use crate::bus::Bus;
    use crate::video::{Video, WIDTH};

    fn make_video() -> Video {
        Video::new()
    }

    // =========================================================================
    // Palette / Color Conversion Tests
    // =========================================================================

    #[test]
    fn rgb555_to_rgb888_black() {
        let v = make_video();
        assert_eq!(v.rgb555_to_rgb888(0x0000), 0x000000);
    }

    #[test]
    fn rgb555_to_rgb888_white() {
        let v = make_video();
        let result = v.rgb555_to_rgb888(0x7FFF);
        assert_eq!((result >> 16) & 0xFF, 0xF8);
        assert_eq!((result >> 8) & 0xFF, 0xF8);
        assert_eq!(result & 0xFF, 0xF8);
    }

    #[test]
    fn rgb555_to_rgb888_pure_red() {
        let v = make_video();
        let result = v.rgb555_to_rgb888(0x001F);
        assert_eq!((result >> 16) & 0xFF, 0xF8);
        assert_eq!((result >> 8) & 0xFF, 0x00);
        assert_eq!(result & 0xFF, 0x00);
    }

    #[test]
    fn rgb555_to_rgb888_pure_green() {
        let v = make_video();
        let result = v.rgb555_to_rgb888(0x03E0);
        assert_eq!((result >> 16) & 0xFF, 0x00);
        assert_eq!((result >> 8) & 0xFF, 0xF8);
        assert_eq!(result & 0xFF, 0x00);
    }

    #[test]
    fn rgb555_to_rgb888_pure_blue() {
        let v = make_video();
        let result = v.rgb555_to_rgb888(0x7C00);
        assert_eq!((result >> 16) & 0xFF, 0x00);
        assert_eq!((result >> 8) & 0xFF, 0x00);
        assert_eq!(result & 0xFF, 0xF8);
    }

    // =========================================================================
    // Sign Extension Tests
    // =========================================================================

    #[test]
    fn sign_extend_28_positive() {
        let v = make_video();
        assert_eq!(v.sign_extend_28(0x0000_0100), 256);
    }

    #[test]
    fn sign_extend_28_negative() {
        let v = make_video();
        let val = 0x0FFF_FF00;
        let result = v.sign_extend_28(val);
        assert_eq!(result, -256);
    }

    #[test]
    fn sign_extend_28_zero() {
        let v = make_video();
        assert_eq!(v.sign_extend_28(0), 0);
    }

    // =========================================================================
    // Blending Calculation Tests
    // =========================================================================

    /// Helper: compute alpha blend inline (same formula as Video::alpha_blend)
    fn alpha_blend(color1: u32, color2: u32, eva: u32, evb: u32) -> u32 {
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

    fn brightness_increase(color: u32, evy: u32) -> u32 {
        let r = (color >> 16) & 0xFF;
        let g = (color >> 8) & 0xFF;
        let b = color & 0xFF;
        let r = r + (((255 - r) * evy) >> 4);
        let g = g + (((255 - g) * evy) >> 4);
        let b = b + (((255 - b) * evy) >> 4);
        (r.min(255) << 16) | (g.min(255) << 8) | b.min(255)
    }

    fn brightness_decrease(color: u32, evy: u32) -> u32 {
        let r = (color >> 16) & 0xFF;
        let g = (color >> 8) & 0xFF;
        let b = color & 0xFF;
        let r = r - ((r * evy) >> 4);
        let g = g - ((g * evy) >> 4);
        let b = b - ((b * evy) >> 4);
        (r << 16) | (g << 8) | b
    }

    #[test]
    fn blend_alpha_equal_weights() {
        let c1 = 0xFF0000;
        let c2 = 0x00FF00;
        let result = alpha_blend(c1, c2, 8, 8);
        let r = (result >> 16) & 0xFF;
        let g = (result >> 8) & 0xFF;
        assert_eq!(r, 127);
        assert_eq!(g, 127);
    }

    #[test]
    fn blend_alpha_full_first() {
        let c1 = 0xFF8040;
        let result = alpha_blend(c1, 0x000000, 16, 0);
        assert_eq!(result, c1);
    }

    #[test]
    fn blend_alpha_full_second() {
        let c2 = 0xFF8040;
        let result = alpha_blend(0x000000, c2, 0, 16);
        assert_eq!(result, c2);
    }

    #[test]
    fn blend_alpha_clamps_to_255() {
        let result = alpha_blend(0xFFFFFF, 0xFFFFFF, 16, 16);
        assert_eq!(result, 0xFFFFFF);
    }

    #[test]
    fn blend_brightness_increase_zero() {
        let color = 0x804020;
        assert_eq!(brightness_increase(color, 0), color);
    }

    #[test]
    fn blend_brightness_increase_full() {
        assert_eq!(brightness_increase(0x000000, 16), 0xFFFFFF);
    }

    #[test]
    fn blend_brightness_decrease_zero() {
        let color = 0x804020;
        assert_eq!(brightness_decrease(color, 0), color);
    }

    #[test]
    fn blend_brightness_decrease_full() {
        assert_eq!(brightness_decrease(0xFFFFFF, 16), 0x000000);
    }

    // =========================================================================
    // Video Timing Tests
    // =========================================================================

    #[test]
    fn scanline_timing_cycles() {
        let mut v = make_video();

        for _ in 0..959 {
            let (event, _) = v.step();
            assert_eq!(event, crate::video::VideoEvent::None);
        }
        let (event, _) = v.step();
        assert_eq!(event, crate::video::VideoEvent::HBlank);
    }

    #[test]
    fn vblank_at_line_160() {
        let mut v = make_video();

        // VBlank fires when vcount increments from 159 to 160.
        // Each scanline = 1232 cycles. Step through 159 scanlines first.
        for _ in 0..(159 * 1232) {
            v.step();
        }

        // Now we're at the start of scanline 159. Step through the rest
        // and look for VBlank within the next 2 scanlines.
        let mut found_vblank = false;
        for _ in 0..(2 * 1232) {
            let (event, _) = v.step();
            if event == crate::video::VideoEvent::VBlank {
                found_vblank = true;
                break;
            }
        }
        assert!(found_vblank, "VBlank should fire when vcount reaches 160");
    }

    // =========================================================================
    // Forced Blank Tests
    // =========================================================================

    #[test]
    fn forced_blank_produces_white() {
        let mut v = make_video();
        v.dispcnt.set_bit(7, true);
        v.render_scanline();

        for x in 0..WIDTH {
            assert_eq!(v.frame_buffer[x], 0x00FFFFFF);
        }
    }

    // =========================================================================
    // Affine Reference Point Tests
    // =========================================================================

    #[test]
    fn affine_ref_latched_on_register_write() {
        let mut v = make_video();

        v.write_byte(0x028, 0x00);
        v.write_byte(0x029, 0x01);
        v.write_byte(0x02A, 0x00);
        v.write_byte(0x02B, 0x00);

        assert_eq!(v.internal_ref_x[0], 256);
    }

    #[test]
    fn affine_ref_advanced_per_scanline() {
        let mut v = make_video();

        // Set BG2 reference point X to 0
        v.write_byte(0x028, 0x00);
        v.write_byte(0x029, 0x00);
        v.write_byte(0x02A, 0x00);
        v.write_byte(0x02B, 0x00);

        // Set PB (dmx) for BG2 to 256 (1.0 in 8.8)
        v.write_byte(0x022, 0x00);
        v.write_byte(0x023, 0x01);

        assert_eq!(v.internal_ref_x[0], 0);

        // Run one scanline (960 cycles triggers render + PB advance)
        for _ in 0..960 {
            v.step();
        }

        assert_eq!(v.internal_ref_x[0], 256);
    }

    // =========================================================================
    // step_n parity vs step (cycle-batched fast path must match per-cycle)
    // =========================================================================

    fn run_frame_one_cycle_at_a_time(v: &mut Video) {
        // One frame = 228 scanlines × 1232 cycles = 280896 cycles.
        for _ in 0..280896u32 {
            v.step();
        }
    }

    fn run_frame_via_step_n(v: &mut Video, chunk: u32) {
        let mut left = 280896u32;
        while left > 0 {
            let take = left.min(chunk);
            v.step_n(take, |_, _| {});
            left -= take;
        }
    }

    #[test]
    fn step_n_matches_step_in_chunks_of_100() {
        let mut a = make_video();
        let mut b = make_video();
        run_frame_one_cycle_at_a_time(&mut a);
        run_frame_via_step_n(&mut b, 100);
        assert_eq!(a.vcount, b.vcount, "vcount diverged");
        assert_eq!(a.dot_cycle, b.dot_cycle, "dot_cycle diverged");
        assert_eq!(a.dispstat, b.dispstat, "dispstat diverged");
    }

    #[test]
    fn step_n_matches_step_in_chunks_of_1232() {
        let mut a = make_video();
        let mut b = make_video();
        run_frame_one_cycle_at_a_time(&mut a);
        run_frame_via_step_n(&mut b, 1232);
        assert_eq!(a.vcount, b.vcount);
        assert_eq!(a.dot_cycle, b.dot_cycle);
        assert_eq!(a.dispstat, b.dispstat);
    }

    #[test]
    fn step_n_emits_hblank_and_vblank_event_counts() {
        // A full frame = 228 HBlank-edge events (one per scanline) + 1 VBlank.
        // Note: 160 of the HBlank edges happen during HDraw (-> HBlank event);
        // 68 happen during VBlank (-> HBlankInVBlank event).
        let mut v = make_video();
        let mut hblank = 0u32;
        let mut hblank_in_vblank = 0u32;
        let mut vblank = 0u32;
        v.step_n(280896, |ev, _| {
            use crate::video::VideoEvent::*;
            match ev {
                HBlank => hblank += 1,
                HBlankInVBlank => hblank_in_vblank += 1,
                VBlank => vblank += 1,
                None => {}
            }
        });
        assert_eq!(hblank, 160, "one HBlank event per visible line");
        assert_eq!(hblank_in_vblank, 68, "one HBlankInVBlank per vblank line");
        assert_eq!(vblank, 1, "single VBlank per frame");
    }

    #[test]
    fn step_n_handles_takes_smaller_than_boundary() {
        let mut a = make_video();
        let mut b = make_video();
        for _ in 0..961u32 {
            a.step();
        }
        // Drive b in chunks of 7 — uneven divisor of HDRAW_CYCLES.
        let mut left = 961u32;
        while left > 0 {
            let take = left.min(7);
            b.step_n(take, |_, _| {});
            left -= take;
        }
        assert_eq!(a.vcount, b.vcount);
        assert_eq!(a.dot_cycle, b.dot_cycle);
        assert_eq!(a.dispstat, b.dispstat);
    }
}

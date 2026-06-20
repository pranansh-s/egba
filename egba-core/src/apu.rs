use crate::bus::Bus;

const FIFO_CAPACITY: usize = 32;

pub(crate) const SAMPLE_RATE: u32 = 32768;

const CPU_CLOCK: u32 = 16_777_216;

const CYCLES_PER_SAMPLE: u32 = CPU_CLOCK / SAMPLE_RATE;

#[derive(Clone)]
struct Fifo {
    data: [i8; FIFO_CAPACITY],
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

impl Default for Fifo {
    fn default() -> Self {
        Self {
            data: [0; FIFO_CAPACITY],
            read_pos: 0,
            write_pos: 0,
            count: 0,
        }
    }
}

impl Fifo {
    fn push(&mut self, sample: i8) {
        if self.count < FIFO_CAPACITY {
            self.data[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % FIFO_CAPACITY;
            self.count += 1;
        }
    }

    fn pop(&mut self) -> i8 {
        if self.count > 0 {
            let sample = self.data[self.read_pos];
            self.read_pos = (self.read_pos + 1) % FIFO_CAPACITY;
            self.count -= 1;
            sample
        } else {
            0
        }
    }

    fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.count = 0;
    }

    fn len(&self) -> usize {
        self.count
    }
}

#[derive(Clone, Default)]
struct DirectSound {
    fifo: Fifo,
    volume_shift: u8,
    timer_sel: u8,
    enable_r: bool,
    enable_l: bool,
    current_sample: i8,
}

pub(crate) struct Apu {
    ds_a: DirectSound,
    ds_b: DirectSound,

    soundcnt_l: u16,
    soundcnt_h: u16,
    soundcnt_x: u16,
    soundbias: u16,

    sample_buffer: Vec<(i16, i16)>,
    sample_clock: u32,
}

impl Default for Apu {
    fn default() -> Self {
        Self {
            ds_a: DirectSound::default(),
            ds_b: DirectSound::default(),
            soundcnt_l: 0,
            soundcnt_h: 0,
            soundcnt_x: 0,
            soundbias: 0x0200,
            sample_buffer: Vec::with_capacity(1024),
            sample_clock: 0,
        }
    }
}

impl Apu {
    pub(crate) fn on_timer_overflow(&mut self, timer_id: u8) -> u8 {
        let mut refill = 0u8;

        if self.ds_a.timer_sel == timer_id {
            self.ds_a.current_sample = self.ds_a.fifo.pop();
            if self.ds_a.fifo.len() <= 16 {
                refill |= 1;
            }
        }

        if self.ds_b.timer_sel == timer_id {
            self.ds_b.current_sample = self.ds_b.fifo.pop();
            if self.ds_b.fifo.len() <= 16 {
                refill |= 2;
            }
        }

        refill
    }

    pub(crate) fn write_fifo(&mut self, fifo_id: usize, value: u32) {
        let ds = if fifo_id == 0 {
            &mut self.ds_a
        } else {
            &mut self.ds_b
        };
        for shift in [0, 8, 16, 24] {
            ds.fifo.push(((value >> shift) & 0xFF) as i8);
        }
    }

    pub(crate) fn step(&mut self, cycles: u32) {
        if self.soundcnt_x & 0x80 == 0 {
            return;
        }

        self.sample_clock += cycles;
        while self.sample_clock >= CYCLES_PER_SAMPLE {
            self.sample_clock -= CYCLES_PER_SAMPLE;
            let (left, right) = self.mix_sample();
            self.sample_buffer.push((left, right));
        }
    }

    fn mix_sample(&self) -> (i16, i16) {
        let ds_a = self.ds_a.current_sample as i16;
        let ds_b = self.ds_b.current_sample as i16;

        let ds_a_scaled = if self.ds_a.volume_shift == 1 {
            ds_a
        } else {
            ds_a >> 1
        };
        let ds_b_scaled = if self.ds_b.volume_shift == 1 {
            ds_b
        } else {
            ds_b >> 1
        };

        let mut left: i16 = 0;
        let mut right: i16 = 0;

        if self.ds_a.enable_l {
            left += ds_a_scaled;
        }
        if self.ds_a.enable_r {
            right += ds_a_scaled;
        }
        if self.ds_b.enable_l {
            left += ds_b_scaled;
        }
        if self.ds_b.enable_r {
            right += ds_b_scaled;
        }

        let left = (left.clamp(-0x200, 0x1FF) as i32 * 64) as i16;
        let right = (right.clamp(-0x200, 0x1FF) as i32 * 64) as i16;
        (left, right)
    }

    pub(crate) fn samples(&self) -> &[(i16, i16)] {
        &self.sample_buffer
    }

    pub(crate) fn clear_samples(&mut self) {
        self.sample_buffer.clear();
    }

    fn update_soundcnt_h(&mut self) {
        let h = self.soundcnt_h;
        self.ds_a.volume_shift = ((h >> 2) & 1) as u8;
        self.ds_b.volume_shift = ((h >> 3) & 1) as u8;
        self.ds_a.enable_r = (h >> 8) & 1 != 0;
        self.ds_a.enable_l = (h >> 9) & 1 != 0;
        self.ds_a.timer_sel = ((h >> 10) & 1) as u8;
        self.ds_b.enable_r = (h >> 12) & 1 != 0;
        self.ds_b.enable_l = (h >> 13) & 1 != 0;
        self.ds_b.timer_sel = ((h >> 14) & 1) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_apu(soundcnt_h: u16) -> Apu {
        let mut apu = Apu::default();
        apu.write_byte(0x084, 0x80);
        apu.write_byte(0x082, soundcnt_h as u8);
        apu.write_byte(0x083, (soundcnt_h >> 8) as u8);
        apu
    }

    #[test]
    fn apu_volume_shift_zero_halves_dsa() {
        let h = (1u16 << 8) | (1u16 << 9);
        let mut apu = setup_apu(h);
        apu.ds_a.current_sample = 100;
        let (l, r) = apu.mix_sample();
        assert_eq!(l, 50 * 64, "left scaled 50% then i16-amplified");
        assert_eq!(r, 50 * 64, "right scaled 50% then i16-amplified");
    }

    #[test]
    fn apu_volume_shift_one_keeps_dsa() {
        let h = (1u16 << 2) | (1u16 << 8) | (1u16 << 9);
        let mut apu = setup_apu(h);
        apu.ds_a.current_sample = 100;
        let (l, r) = apu.mix_sample();
        assert_eq!(l, 100 * 64, "left at 100% then i16-amplified");
        assert_eq!(r, 100 * 64, "right at 100% then i16-amplified");
    }

    #[test]
    fn apu_timer_overflow_pops_fifo() {
        let mut apu = setup_apu((1u16 << 8) | (1u16 << 9));
        apu.write_fifo(0, 0x04030201);
        let _ = apu.on_timer_overflow(0);
        assert_eq!(apu.ds_a.current_sample, 0x01, "first byte popped on T0 overflow");
        assert_eq!(apu.ds_a.fifo.len(), 3, "three samples remain");
    }
}

impl Bus for Apu {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x080 => self.soundcnt_l as u8,
            0x081 => (self.soundcnt_l >> 8) as u8,
            0x082 => self.soundcnt_h as u8,
            0x083 => (self.soundcnt_h >> 8) as u8,
            0x084 => (self.soundcnt_x & 0x80) as u8,
            0x088 => self.soundbias as u8,
            0x089 => (self.soundbias >> 8) as u8,
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x080 => {
                self.soundcnt_l = (self.soundcnt_l & 0xFF00) | value as u16;
            }
            0x081 => {
                self.soundcnt_l = (self.soundcnt_l & 0x00FF) | ((value as u16) << 8);
            }
            0x082 => {
                self.soundcnt_h = (self.soundcnt_h & 0xFF00) | value as u16;
                self.update_soundcnt_h();
            }
            0x083 => {
                self.soundcnt_h = (self.soundcnt_h & 0x00FF) | ((value as u16) << 8);
                if (self.soundcnt_h >> 11) & 1 != 0 {
                    self.ds_a.fifo.clear();
                    self.soundcnt_h &= !(1 << 11);
                }
                if (self.soundcnt_h >> 15) & 1 != 0 {
                    self.ds_b.fifo.clear();
                    self.soundcnt_h &= !(1 << 15);
                }
                self.update_soundcnt_h();
            }
            0x084 => {
                self.soundcnt_x = (self.soundcnt_x & !0x80) | (value as u16 & 0x80);
                if self.soundcnt_x & 0x80 == 0 {
                    self.ds_a.fifo.clear();
                    self.ds_b.fifo.clear();
                    self.ds_a.current_sample = 0;
                    self.ds_b.current_sample = 0;
                }
            }
            0x088 => {
                self.soundbias = (self.soundbias & 0xFF00) | value as u16;
            }
            0x089 => {
                self.soundbias = (self.soundbias & 0x00FF) | ((value as u16) << 8);
            }
            _ => {}
        }
    }
}

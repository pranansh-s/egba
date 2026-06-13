use crate::bus::Bus;

/// GBA Direct Sound FIFO capacity (hardware is 32 bytes / 8 words)
const FIFO_CAPACITY: usize = 32;

/// Output sample rate: ~32768 Hz is the native GBA rate.
/// We generate samples at this rate; the host can resample as needed.
pub(crate) const SAMPLE_RATE: u32 = 32768;

/// CPU clock frequency (16.78 MHz)
const CPU_CLOCK: u32 = 16_777_216;

/// Cycles per output sample (CPU_CLOCK / SAMPLE_RATE ≈ 512)
const CYCLES_PER_SAMPLE: u32 = CPU_CLOCK / SAMPLE_RATE;

/// A ring buffer FIFO for Direct Sound sample data.
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

/// State for one Direct Sound channel (A or B)
#[derive(Clone, Default)]
struct DirectSound {
    fifo: Fifo,
    /// 0 = half volume (>>1), 1 = full volume
    volume_shift: u8,
    /// 0 = TM0, 1 = TM1
    timer_sel: u8,
    enable_r: bool,
    enable_l: bool,
    current_sample: i8,
}

/// Audio Processing Unit
///
/// Handles Direct Sound channels A and B (PCM audio via DMA + timer).
/// PSG channels 1-4 are not yet implemented.
pub(crate) struct Apu {
    ds_a: DirectSound,
    ds_b: DirectSound,

    /// SOUNDCNT_L (0x4000080) — legacy PSG volume/enable (stored but not processed)
    soundcnt_l: u16,
    /// SOUNDCNT_H (0x4000082) — Direct Sound control
    soundcnt_h: u16,
    /// SOUNDCNT_X (0x4000084) — master enable
    soundcnt_x: u16,
    /// SOUNDBIAS (0x4000088)
    soundbias: u16,

    /// Output sample buffer: (left, right) pairs at SAMPLE_RATE Hz.
    /// The UI layer drains this each frame.
    sample_buffer: Vec<(i16, i16)>,
    /// Cycle accumulator for output sample generation
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
            soundbias: 0x0200, // Default bias
            sample_buffer: Vec::with_capacity(1024),
            sample_clock: 0,
        }
    }
}

impl Apu {
    /// Called when a timer overflows. Consumes one sample from matching FIFO(s).
    /// Returns a bitmask indicating which FIFOs need DMA refill:
    ///   bit 0 = FIFO-A needs refill (DMA1)
    ///   bit 1 = FIFO-B needs refill (DMA2)
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

    /// Write 4 bytes (one word) to a FIFO.
    /// fifo_id: 0 = FIFO-A, 1 = FIFO-B
    pub(crate) fn write_fifo(&mut self, fifo_id: usize, value: u32) {
        let ds = if fifo_id == 0 {
            &mut self.ds_a
        } else {
            &mut self.ds_b
        };
        // Push 4 signed bytes, oldest first (LE: byte0 is first sample)
        for shift in [0, 8, 16, 24] {
            ds.fifo.push(((value >> shift) & 0xFF) as i8);
        }
    }

    /// Advance the sample clock by `cycles` CPU cycles.
    /// Generates output samples into the sample buffer at SAMPLE_RATE.
    pub(crate) fn step(&mut self, cycles: u32) {
        // Only generate output if master sound is enabled
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

    /// Mix Direct Sound channels A and B into a stereo sample.
    fn mix_sample(&self) -> (i16, i16) {
        let ds_a = self.ds_a.current_sample as i16;
        let ds_b = self.ds_b.current_sample as i16;

        // Volume: shift=1 means full (multiply by 2 relative to half), shift=0 means half
        let ds_a_scaled = if self.ds_a.volume_shift == 1 {
            ds_a * 2
        } else {
            ds_a
        };
        let ds_b_scaled = if self.ds_b.volume_shift == 1 {
            ds_b * 2
        } else {
            ds_b
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

        // Clamp to 10-bit range (-512..511), matching GBA DAC range
        (left.clamp(-0x200, 0x1FF), right.clamp(-0x200, 0x1FF))
    }

    /// Get the current sample buffer for the UI layer to consume.
    pub(crate) fn drain_samples(&mut self) -> Vec<(i16, i16)> {
        std::mem::take(&mut self.sample_buffer)
    }

    /// Update SOUNDCNT_H fields into DirectSound channel state
    fn update_soundcnt_h(&mut self) {
        let h = self.soundcnt_h;
        self.ds_a.volume_shift = ((h >> 2) & 1) as u8;
        self.ds_b.volume_shift = ((h >> 3) & 1) as u8;
        self.ds_a.enable_r = (h >> 8) & 1 != 0;
        self.ds_a.enable_l = (h >> 9) & 1 != 0;
        self.ds_a.timer_sel = ((h >> 10) & 1) as u8;
        // bit 11: FIFO-A reset (handled on write)
        self.ds_b.enable_r = (h >> 12) & 1 != 0;
        self.ds_b.enable_l = (h >> 13) & 1 != 0;
        self.ds_b.timer_sel = ((h >> 14) & 1) as u8;
        // bit 15: FIFO-B reset (handled on write)
    }
}

impl Bus for Apu {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            // SOUNDCNT_L (0x4000080)
            0x080 => self.soundcnt_l as u8,
            0x081 => (self.soundcnt_l >> 8) as u8,
            // SOUNDCNT_H (0x4000082)
            0x082 => self.soundcnt_h as u8,
            0x083 => (self.soundcnt_h >> 8) as u8,
            // SOUNDCNT_X (0x4000084)
            0x084 => (self.soundcnt_x & 0x80) as u8, // only bit 7 readable
            0x085 => 0,
            // SOUNDBIAS (0x4000088)
            0x088 => self.soundbias as u8,
            0x089 => (self.soundbias >> 8) as u8,
            // PSG channel registers (not implemented, return 0)
            0x060..=0x07F => 0,
            // FIFO registers are write-only
            0x0A0..=0x0A7 => 0,
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            // SOUNDCNT_L (0x4000080)
            0x080 => {
                self.soundcnt_l = (self.soundcnt_l & 0xFF00) | value as u16;
            }
            0x081 => {
                self.soundcnt_l = (self.soundcnt_l & 0x00FF) | ((value as u16) << 8);
            }
            // SOUNDCNT_H (0x4000082) — low byte
            0x082 => {
                self.soundcnt_h = (self.soundcnt_h & 0xFF00) | value as u16;
                self.update_soundcnt_h();
            }
            // SOUNDCNT_H (0x4000082) — high byte
            0x083 => {
                self.soundcnt_h = (self.soundcnt_h & 0x00FF) | ((value as u16) << 8);
                // Handle FIFO reset bits
                if (self.soundcnt_h >> 11) & 1 != 0 {
                    self.ds_a.fifo.clear();
                    // Clear the reset bit after processing
                    self.soundcnt_h &= !(1 << 11);
                }
                if (self.soundcnt_h >> 15) & 1 != 0 {
                    self.ds_b.fifo.clear();
                    self.soundcnt_h &= !(1 << 15);
                }
                self.update_soundcnt_h();
            }
            // SOUNDCNT_X (0x4000084) — only bit 7 writable (master enable)
            0x084 => {
                self.soundcnt_x = (self.soundcnt_x & !0x80) | (value as u16 & 0x80);
                if self.soundcnt_x & 0x80 == 0 {
                    // Master disable: reset all sound
                    self.ds_a.fifo.clear();
                    self.ds_b.fifo.clear();
                    self.ds_a.current_sample = 0;
                    self.ds_b.current_sample = 0;
                }
            }
            0x085 => {} // read-only upper byte
            // SOUNDBIAS (0x4000088)
            0x088 => {
                self.soundbias = (self.soundbias & 0xFF00) | value as u16;
            }
            0x089 => {
                self.soundbias = (self.soundbias & 0x00FF) | ((value as u16) << 8);
            }
            // PSG channel registers (stored but not processed)
            0x060..=0x07F => {}
            // FIFO writes are handled via write_word in the memory layer
            // (byte writes to FIFO are unusual but we accept them silently)
            0x0A0..=0x0A7 => {}
            _ => {}
        }
    }
}

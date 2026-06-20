use bit::BitIndex;

use crate::{bus::Bus, control::InterruptType};

const PRESCALER_DIVS: [u32; 4] = [1, 64, 256, 1024];

#[derive(Default, Clone)]
struct Timer {
    counter: u16,
    reload: u16,
    control: u16,
    internal_counter: u32,
}

impl Timer {
    fn enabled(&self) -> bool {
        self.control.bit(7)
    }

    fn cascade(&self) -> bool {
        self.control.bit(2)
    }

    fn irq_enabled(&self) -> bool {
        self.control.bit(6)
    }

    fn prescaler(&self) -> u32 {
        PRESCALER_DIVS[self.control.bit_range(0..2) as usize]
    }
}

#[derive(Default)]
pub(crate) struct Timers {
    timers: [Timer; 4],
    any_active: bool,
}

impl Timers {
    fn refresh_any_active(&mut self) {
        self.any_active = self
            .timers
            .iter()
            .enumerate()
            .any(|(i, t)| t.enabled() && !(i > 0 && t.cascade()));
    }

    #[inline]
    pub(crate) fn step(&mut self, cycles: u32) -> u8 {
        if !self.any_active {
            return 0;
        }
        let mut overflow_flags: u8 = 0;

        for i in 0..4 {
            if !self.timers[i].enabled() || (i > 0 && self.timers[i].cascade()) {
                continue;
            }

            self.timers[i].internal_counter += cycles;
            let prescaler = self.timers[i].prescaler();

            while self.timers[i].internal_counter >= prescaler {
                self.timers[i].internal_counter -= prescaler;

                let (new_val, overflowed) = self.timers[i].counter.overflowing_add(1);
                if overflowed {
                    self.timers[i].counter = self.timers[i].reload;
                    overflow_flags |= 1 << i;

                    if i < 3 {
                        self.cascade_overflow(i + 1, &mut overflow_flags);
                    }
                } else {
                    self.timers[i].counter = new_val;
                }
            }
        }

        overflow_flags
    }

    fn cascade_overflow(&mut self, index: usize, overflow_flags: &mut u8) {
        if index >= 4 || !self.timers[index].enabled() || !self.timers[index].cascade() {
            return;
        }

        let (new_val, overflowed) = self.timers[index].counter.overflowing_add(1);
        if overflowed {
            self.timers[index].counter = self.timers[index].reload;
            *overflow_flags |= 1 << index;

            if index < 3 {
                self.cascade_overflow(index + 1, overflow_flags);
            }
        } else {
            self.timers[index].counter = new_val;
        }
    }

    pub(crate) fn irq_type(index: usize) -> InterruptType {
        match index {
            0 => InterruptType::Timer0,
            1 => InterruptType::Timer1,
            2 => InterruptType::Timer2,
            3 => InterruptType::Timer3,
            _ => unreachable!(),
        }
    }

    pub(crate) fn timer_irq_enabled(&self, index: usize) -> bool {
        self.timers[index].irq_enabled()
    }

    pub(crate) fn cycles_to_next_overflow(&self) -> Option<u32> {
        if !self.any_active {
            return None;
        }
        let mut best: Option<u32> = None;
        for i in 0..4 {
            let t = &self.timers[i];
            if !t.enabled() || (i > 0 && t.cascade()) {
                continue;
            }
            let prescaler = t.prescaler();
            let ticks_left = (0x10000u32 - t.counter as u32).saturating_mul(prescaler);
            let cycles = ticks_left.saturating_sub(t.internal_counter).max(1);
            best = Some(best.map_or(cycles, |b| b.min(cycles)));
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enable_timer(t: &mut Timers, idx: usize, control_low: u8, reload: u16) {
        let base = 0x100 + (idx * 4) as u32;
        t.write_byte(base, reload as u8);
        t.write_byte(base + 1, (reload >> 8) as u8);
        t.write_byte(base + 2, control_low);
    }

    #[test]
    fn timer0_cascade_bit_is_ignored() {
        let cases: [(u8, u32, u16, &str); 2] = [
            (0b1000_0000, 100, 100, "no cascade bit, prescaler 1"),
            (0b1000_0100, 100, 100, "cascade bit set on T0 -> still ticks"),
        ];
        for (ctrl, cycles, expected, label) in cases {
            let mut t = Timers::default();
            enable_timer(&mut t, 0, ctrl, 0);
            t.step(cycles);
            assert_eq!(t.timers[0].counter, expected, "{label}");
        }
    }

    #[test]
    fn timer_overflow_reloads_and_cascades() {
        let mut t = Timers::default();
        enable_timer(&mut t, 0, 0b1000_0000, 0xFFFE);
        enable_timer(&mut t, 1, 0b1000_0100, 0);
        let of = t.step(2);
        assert_ne!(of & 1, 0, "T0 must overflow after 2 ticks");
        assert_eq!(t.timers[0].counter, 0xFFFE, "T0 reloaded");
        assert_eq!(t.timers[1].counter, 1, "T1 cascades on T0 overflow");
    }

    #[test]
    fn timer_prescaler_64_emits_overflow_every_64_cycles() {
        let mut t = Timers::default();
        enable_timer(&mut t, 2, 0b1000_0001, 0xFFFF);
        let of = t.step(64);
        assert_ne!(of & (1 << 2), 0, "T2 must overflow once at 64 cycles");
        let of = t.step(63);
        assert_eq!(of & (1 << 2), 0, "T2 must not overflow within next 63 cycles");
        let of = t.step(1);
        assert_ne!(of & (1 << 2), 0, "T2 overflows at next 64-cycle boundary");
    }
}

impl Bus for Timers {
    fn read_byte(&self, addr: u32) -> u8 {
        let timer_idx = ((addr - 0x100) / 4) as usize;
        let reg_offset = (addr - 0x100) % 4;

        if timer_idx >= 4 {
            return 0;
        }

        match reg_offset {
            0 => self.timers[timer_idx].counter as u8,
            1 => (self.timers[timer_idx].counter >> 8) as u8,
            2 => self.timers[timer_idx].control as u8,
            3 => (self.timers[timer_idx].control >> 8) as u8,
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        let timer_idx = ((addr - 0x100) / 4) as usize;
        let reg_offset = (addr - 0x100) % 4;

        if timer_idx >= 4 {
            return;
        }

        match reg_offset {
            0 => {
                self.timers[timer_idx]
                    .reload
                    .set_bit_range(0..8, value as u16);
            }
            1 => {
                self.timers[timer_idx]
                    .reload
                    .set_bit_range(8..16, value as u16);
            }
            2 => {
                let was_enabled = self.timers[timer_idx].enabled();
                self.timers[timer_idx]
                    .control
                    .set_bit_range(0..8, value as u16);
                let now_enabled = self.timers[timer_idx].enabled();

                if !was_enabled && now_enabled {
                    self.timers[timer_idx].counter = self.timers[timer_idx].reload;
                    self.timers[timer_idx].internal_counter = 0;
                }
                self.refresh_any_active();
            }
            3 => {
                self.timers[timer_idx]
                    .control
                    .set_bit_range(8..16, value as u16);
                self.refresh_any_active();
            }
            _ => {}
        }
    }
}

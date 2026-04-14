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

pub(crate) struct Timers {
    timers: [Timer; 4],
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            timers: [
                Timer::default(),
                Timer::default(),
                Timer::default(),
                Timer::default(),
            ],
        }
    }
}

impl Timers {
    pub(crate) fn step(&mut self, cycles: u32) -> u8 {
        let mut overflow_flags: u8 = 0;

        for i in 0..4 {
            if !self.timers[i].enabled() || self.timers[i].cascade() {
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

                    // Cascade to next timer
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
}

impl Bus for Timers {
    fn read_byte(&self, addr: u32) -> u8 {
        let timer_idx = ((addr - 0x100) / 4) as usize;
        let reg_offset = (addr - 0x100) % 4;

        if timer_idx >= 4 {
            return 0;
        }

        match reg_offset {
            // TMxCNT_L: read returns current counter
            0 => self.timers[timer_idx].counter as u8,
            1 => (self.timers[timer_idx].counter >> 8) as u8,
            // TMxCNT_H: read returns control
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
            }
            3 => {
                self.timers[timer_idx]
                    .control
                    .set_bit_range(8..16, value as u16);
            }
            _ => {}
        }
    }
}

use bit::BitIndex;

use crate::bus::Bus;

pub(crate) struct Keypad {
    pub(crate) keystate: u16,
    pub(crate) keycnt: u16,
}

impl Default for Keypad {
    fn default() -> Self {
        Keypad {
            keystate: 0x03FF,
            keycnt: 0x0000,
        }
    }
}

impl Keypad {
    pub(crate) fn should_interrupt(&self) -> bool {
        if !self.keycnt.bit(14) {
            return false;
        }

        let pressed = !self.keystate.bit_range(0..10);
        let selection = self.keycnt.bit_range(0..10);

        if self.keycnt.bit(15) {
            pressed > 0 && (pressed & selection) == selection
        } else {
            (pressed & selection) > 0
        }
    }
}

impl Bus for Keypad {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x130 => self.keystate as u8,
            0x131 => (self.keystate >> 8) as u8,
            0x132 => self.keycnt as u8,
            0x133 => (self.keycnt >> 8) as u8,
            _ => 0x69,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x132 => {
                self.keycnt.set_bit_range(0..8, value as u16);
            }
            0x133 => {
                self.keycnt.set_bit_range(8..16, value as u16);
            }
            _ => {}
        }
    }
}

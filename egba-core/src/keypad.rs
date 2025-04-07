use bit::BitIndex;

use crate::{bus::Bus, control::InterruptType, gba::GBA, KEYCNT, KEYINPUT};

pub struct Keypad {
    pub a: bool,
    pub b: bool,
    pub l: bool,
    pub r: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub select: bool,
    pub start: bool,
}

impl Default for Keypad {
    fn default() -> Self {
        Keypad {
            a: true,
            b: true,
            l: true,
            r: true,
            up: true,
            down: true,
            left: true,
            right: true,
            select: true,
            start: true,
        }
    }
}

impl Into<u16> for Keypad {
    fn into(self) -> u16 {
        (self.a as u16) |
        ((self.b as u16) << 1) |
        ((self.select as u16) << 2) |
        ((self.start as u16) << 3) |
        ((self.right as u16) << 4) |
        ((self.left as u16) << 5) |
        ((self.up as u16) << 6) |
        ((self.down as u16) << 7) |
        ((self.r as u16) << 8) |
        ((self.l as u16) << 9)
    }
}

impl GBA {
    pub fn update_keypad(&mut self, keypad: u16) {
        self.memory.io.write_hword(KEYINPUT, keypad);

        let keycnt = self.memory.io.read_hword(KEYCNT);
        if keycnt.bit(14) {
            let pressed = !keypad.bit_range(0..10);
            let selection = keycnt.bit_range(0..10);
            
            let interrupt = if keycnt.bit(15) {
                pressed > 0 && pressed & selection == selection
            }
            else {
                pressed & selection > 0
            };

            if interrupt {
                self.interrupt.interrupt_request(InterruptType::Keypad);
            }
        }
    }
}
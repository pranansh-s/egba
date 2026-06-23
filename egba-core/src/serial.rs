use bit::BitIndex;

use crate::bus::Bus;

const SIO_TRANSFER_CYCLES: u32 = 256;

#[derive(Default)]
pub(crate) struct Serial {
    siodata32: u32,
    siomulti: [u16; 4],
    siocnt: u16,
    siomlt_send: u16,
    rcnt: u16,
    pending_cycles: u32,
}

impl Serial {
    pub(crate) fn step(&mut self, cycles: u32) -> bool {
        if self.pending_cycles == 0 {
            return false;
        }
        if cycles >= self.pending_cycles {
            self.pending_cycles = 0;
            self.siocnt.set_bit(7, false);
            self.siocnt.bit(14)
        } else {
            self.pending_cycles -= cycles;
            false
        }
    }
}

impl Bus for Serial {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x120 => self.siodata32 as u8,
            0x121 => (self.siodata32 >> 8) as u8,
            0x122 => (self.siodata32 >> 16) as u8,
            0x123 => (self.siodata32 >> 24) as u8,
            0x124 => self.siomulti[2] as u8,
            0x125 => (self.siomulti[2] >> 8) as u8,
            0x126 => self.siomulti[3] as u8,
            0x127 => (self.siomulti[3] >> 8) as u8,
            0x128 => self.siocnt as u8,
            0x129 => (self.siocnt >> 8) as u8,
            0x12A => self.siomlt_send as u8,
            0x12B => (self.siomlt_send >> 8) as u8,
            0x12C => self.rcnt as u8,
            0x12D => (self.rcnt >> 8) as u8,
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x120 => { self.siodata32.set_bit_range(0..8, value as u32); }
            0x121 => { self.siodata32.set_bit_range(8..16, value as u32); }
            0x122 => { self.siodata32.set_bit_range(16..24, value as u32); }
            0x123 => { self.siodata32.set_bit_range(24..32, value as u32); }
            0x124 => { self.siomulti[2].set_bit_range(0..8, value as u16); }
            0x125 => { self.siomulti[2].set_bit_range(8..16, value as u16); }
            0x126 => { self.siomulti[3].set_bit_range(0..8, value as u16); }
            0x127 => { self.siomulti[3].set_bit_range(8..16, value as u16); }
            0x128 => {
                let was_busy = self.siocnt.bit(7);
                self.siocnt.set_bit_range(0..8, value as u16);
                if !was_busy && self.siocnt.bit(7) {
                    self.pending_cycles = SIO_TRANSFER_CYCLES;
                }
            }
            0x129 => {
                self.siocnt.set_bit_range(8..16, value as u16);
            }
            0x12A => { self.siomlt_send.set_bit_range(0..8, value as u16); }
            0x12B => { self.siomlt_send.set_bit_range(8..16, value as u16); }
            0x12C => { self.rcnt.set_bit_range(0..8, value as u16); }
            0x12D => { self.rcnt.set_bit_range(8..16, value as u16); }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_siocnt_start_with_irq_enabled_fires_after_transfer_cycles() {
        let cases: [(u16, u32, bool, &str); 3] = [
            (0x4080, SIO_TRANSFER_CYCLES, true, "start + irq -> fires at exact cycles"),
            (0x4080, SIO_TRANSFER_CYCLES - 1, false, "one cycle early -> no fire"),
            (0x0080, SIO_TRANSFER_CYCLES, false, "start without irq -> no fire"),
        ];
        for (siocnt_val, cycles, expected, label) in cases {
            let mut s = Serial::default();
            s.write_byte(0x128, siocnt_val as u8);
            s.write_byte(0x129, (siocnt_val >> 8) as u8);
            assert!(s.pending_cycles > 0, "{label}: transfer must be scheduled");
            let fired = s.step(cycles);
            assert_eq!(fired, expected, "{label}");
        }
    }

    #[test]
    fn writing_start_bit_again_while_busy_does_not_reset_counter() {
        let mut s = Serial::default();
        s.write_byte(0x128, 0x80);
        s.write_byte(0x129, 0x40);
        let _ = s.step(100);
        let before = s.pending_cycles;
        s.write_byte(0x128, 0x80);
        assert_eq!(s.pending_cycles, before, "double-write of start while busy must not reset");
    }
}

use bit::BitIndex;


pub trait Bus {
    fn read_byte(&self, addr: u32) -> u8;
    fn write_byte(&mut self, addr: u32, value: u8) {}

    fn read_hword(&self, addr: u32) -> u16 {
        let addr = addr & !0b1;
        u16::from_le_bytes([self.read_byte(addr), self.read_byte(addr.wrapping_add(1))].try_into().unwrap_or_default())
    }
    fn write_hword(&mut self, addr: u32, value: u16) {
        let addr = addr & !0b1;
        self.write_byte(addr, value.bit_range(0..8).try_into().unwrap());
        self.write_byte(addr.wrapping_add(1), value.bit_range(8..16).try_into().unwrap());
    }

    fn read_word(&self, addr: u32) -> u32 {
        let addr = addr & !0b11;
        u32::from_le_bytes([self.read_byte(addr), self.read_byte(addr.wrapping_add(1)), self.read_byte(addr.wrapping_add(2)), self.read_byte(addr.wrapping_add(3))].try_into().unwrap_or_default())
    }
    
    fn write_word(&mut self, addr: u32, value: u32) {
        let addr = addr & !0b11;
        self.write_hword(addr, value.bit_range(0..16).try_into().unwrap());
        self.write_hword(addr.wrapping_add(2), value.bit_range(16..32).try_into().unwrap());
    }
}

impl Bus for [u8] {
    fn read_byte(&self, addr: u32) -> u8 {
        self[addr as usize]
    }
    fn write_byte(&mut self, addr: u32, value: u8) {
        self[addr as usize] = value;
    }
}
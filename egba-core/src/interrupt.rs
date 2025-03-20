pub struct InterruptControl {
    pub master: bool,
    pub enable: Interrupt,
    pub request: Interrupt,
}

pub struct Interrupt {
    VBlank: bool,
    HBlank: bool,
    VCounter: bool,
    Timer0: bool,
    Timer1: bool,
    Timer2: bool,
    Timer3: bool,
    Serial: bool,
    DMA0: bool,
    DMA1: bool,
    DMA2: bool,
    DMA3: bool,
    Keypad: bool,
    Cartridge: bool,
}

impl Into<u16> for Interrupt {
    fn into(self) -> u16 {
        (self.VBlank as u16) |
        ((self.HBlank as u16) << 1) |
        ((self.VCounter as u16) << 2) |
        ((self.Timer0 as u16) << 3) |
        ((self.Timer1 as u16) << 4) |
        ((self.Timer2 as u16) << 5) |
        ((self.Timer3 as u16) << 6) |
        ((self.Serial as u16) << 7) |
        ((self.DMA0 as u16) << 8) |
        ((self.DMA1 as u16) << 9) |
        ((self.DMA2 as u16) << 10) |
        ((self.DMA3 as u16) << 11) |
        ((self.Keypad as u16) << 12) |
        ((self.Cartridge as u16) << 13)
    }
}
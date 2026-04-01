use self::registers::VideoRegisters;

mod background;
mod registers;

const WIDTH: usize = 240;
const HEIGHT: usize = 160;

pub(crate) struct Video {
    frame_buffer: Box<[u32]>,
    registers: VideoRegisters,
    vram: Box<[u8]>,
    oam: Box<[u8]>,
    palette: Box<[u8]>,
}

impl Video {
    pub(crate) fn new() -> Self {
        Self {
            frame_buffer: vec![0; WIDTH * HEIGHT].into_boxed_slice(),
            registers: VideoRegisters::new(),
            vram: vec![0; 96 * 1024].into_boxed_slice(),
            oam: vec![0; 1024].into_boxed_slice(),
            palette: vec![0; 1024].into_boxed_slice(),
        }
    }

    pub(crate) fn step(&self) {}
}

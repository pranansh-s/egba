use super::{Video, WIDTH};

#[derive(Clone, Copy)]
pub(crate) struct Sprite {
    pub y: i16,
    pub affine: bool,
    pub double_or_disable: bool,
    pub mode: u8,
    pub mosaic: bool,
    pub is_8bpp: bool,
    pub shape: u8,

    pub x: i16,
    pub affine_param: u8,
    pub h_flip: bool,
    pub v_flip: bool,
    pub size: u8,

    pub tile_id: u16,
    pub priority: u8,
    pub palette_bank: u8,
}

impl Sprite {
    pub fn new(attr0: u16, attr1: u16, attr2: u16) -> Self {
        let mut y = (attr0 & 0xFF) as i16;
        if y >= 160 {
            y -= 256;
        }

        let affine = (attr0 & 0x100) != 0;
        let double_or_disable = (attr0 & 0x200) != 0;

        let mut x = (attr1 & 0x1FF) as i16;
        if x >= 256 {
            x -= 512;
        }

        Self {
            y,
            affine,
            double_or_disable,
            mode: ((attr0 >> 10) & 3) as u8,
            mosaic: (attr0 & 0x1000) != 0,
            is_8bpp: (attr0 & 0x2000) != 0,
            shape: ((attr0 >> 14) & 3) as u8,

            x,
            affine_param: ((attr1 >> 9) & 0x1F) as u8,
            h_flip: !affine && (attr1 & 0x1000) != 0,
            v_flip: !affine && (attr1 & 0x2000) != 0,
            size: ((attr1 >> 14) & 3) as u8,

            tile_id: attr2 & 0x3FF,
            priority: ((attr2 >> 10) & 3) as u8,
            palette_bank: ((attr2 >> 12) & 0xF) as u8,
        }
    }

    pub fn dimensions(&self) -> (i16, i16) {
        match (self.shape, self.size) {
            (0, 0) => (8, 8),
            (0, 1) => (16, 16),
            (0, 2) => (32, 32),
            (0, 3) => (64, 64),
            (1, 0) => (16, 8),
            (1, 1) => (32, 8),
            (1, 2) => (32, 16),
            (1, 3) => (64, 32),
            (2, 0) => (8, 16),
            (2, 1) => (8, 32),
            (2, 2) => (16, 32),
            (2, 3) => (32, 64),
            _ => (8, 8),
        }
    }
}

#![allow(dead_code)]

#[derive(Clone, Copy, Default)]
pub enum ScreenSize {
    #[default]
    Size256x256,
    Size512x256,
    Size256x512,
    Size512x512,
}

impl From<u8> for ScreenSize {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Size256x256,
            1 => Self::Size512x256,
            2 => Self::Size256x512,
            3 => Self::Size512x512,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct BGControl {
    priority: u8,
    character_block: u8,
    screen_block: u8,
    mosaic: bool,
    color_mode: bool,
    wrap: bool,
    screen_size: ScreenSize,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct BGOffset(u16, u16);

#[derive(Clone, Copy, Default)]
pub(crate) struct BGReference(u16, u16);

#[derive(Clone, Copy, Default)]
pub(crate) struct BGAffine {
    dx: u16,
    dmx: u16,
    dy: u16,
    dmy: u16,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct WindowDimension(u16, u16);

#[derive(Clone, Copy, Default)]
pub(crate) struct BlendControl;

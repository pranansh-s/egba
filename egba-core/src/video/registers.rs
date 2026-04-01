use crate::video::background::{BlendControl, WindowDimension};

use super::background::{BGAffine, BGControl, BGOffset, BGReference};

pub(crate) struct VideoRegisters {
    dispcnt: u16,
    dispstat: u16,
    vcount: u8,
    bgcnt: [BGControl; 4],
    bgofs: [BGOffset; 4],
    bgref: [BGReference; 2],
    bgaffine: [BGAffine; 2],
    windim: [WindowDimension; 2],
    winin: u16,
    winout: u16,
    mosaic: u16,
    bld: BlendControl,
}

impl VideoRegisters {
    #[must_use]
    pub fn new() -> Self {
        Self {
            dispcnt: 0x0000,
            dispstat: 0x0000,
            vcount: 0x0000,
            bgcnt: [BGControl::default(); 4],
            bgofs: [BGOffset::default(); 4],
            bgref: [BGReference::default(); 2],
            bgaffine: [BGAffine::default(); 2],
            windim: [WindowDimension::default(); 2],
            winin: 0x0000,
            winout: 0x0000,
            mosaic: 0x0000,
            bld: BlendControl::default(),
        }
    }
}

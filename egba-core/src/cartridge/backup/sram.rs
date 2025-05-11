pub struct Sram(Box<[u8]>);

impl Sram {
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self(data.clone().into_boxed_slice())
    }
}
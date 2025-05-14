use super::BackupBuffer;

pub struct EEPROM {
    data: Box<[u8]>
}

impl From<Vec<u8>> for EEPROM {
    fn from(value: Vec<u8>) -> Self {
        Self {
            data: value.clone().into_boxed_slice()
        }
    }
}

impl BackupBuffer for EEPROM {}

impl EEPROM {
    pub fn new(size: usize) -> Self {
        let mut data = <Self as BackupBuffer>::init(size).into_vec();
        if size == 1 {
            data.truncate(512);
        }

        Self {
            data: data.into_boxed_slice()
        }
    }
}
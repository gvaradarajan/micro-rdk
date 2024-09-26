#[derive(Debug, Clone, Copy)]
pub enum WaterReference {
    PaddleWheel = 0,
    PitotTube,
    Doppler,
    Correlation,
    Electromagnetic,
    Unknown,
}

impl WaterReference {
    pub fn from_byte(data: u8) -> Self {
        match data {
            0 => Self::PaddleWheel,
            1 => Self::PitotTube,
            2 => Self::Doppler,
            3 => Self::Correlation,
            4 => Self::Electromagnetic,
            _ => Self::Unknown,
        }
    }
}

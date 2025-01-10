use micro_rdk_nmea_macros::PgnMessageDerive;

#[derive(PgnMessageDerive, Debug)]
pub struct WaterDepth {
    source_id: u8,
    #[scale = 0.01]
    depth: u32,
    #[scale = 0.001]
    offset: i16,
    #[scale = 10.0]
    range: u8,
}

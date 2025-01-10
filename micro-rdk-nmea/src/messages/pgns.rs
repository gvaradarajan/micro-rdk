use crate::parse_helpers::enums::{
    DirectionReference, Gns, GnsIntegrity, GnsMethod, Lookup, MagneticVariationSource,
    SystemTimeSource, TemperatureSource, WaterReference,
};
use crate::parse_helpers::parsers::{FieldReader, FieldSet};
use micro_rdk_nmea_macros::{FieldsetDerive, PgnMessageDerive};

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

#[derive(PgnMessageDerive, Debug)]
pub struct Speed {
    source_id: u8,
    #[scale = 0.01]
    speed_water_ref: u16,
    #[scale = 0.01]
    speed_ground_ref: u16,
    #[lookup]
    #[bits = 8]
    speed_water_ref_type: WaterReference,
}

#[derive(PgnMessageDerive, Debug)]
pub struct TemperatureExtendedRange {
    source_id: u8,
    instance: u8,
    #[lookup]
    source: TemperatureSource,
    #[scale = 0.001]
    temperature: [u8; 3],
    #[scale = 0.1]
    set_temperature: u16,
}

#[derive(PgnMessageDerive, Debug)]
pub struct SystemTime {
    source_id: u8,
    #[lookup]
    #[bits = 4]
    source: SystemTimeSource,
    #[offset = 4]
    date: u16,
    #[scale = 0.0001]
    time: u32,
}

#[derive(PgnMessageDerive, Debug)]
pub struct MagneticVariation {
    source_id: u8,
    #[lookup]
    #[bits = 4]
    source: MagneticVariationSource,
    #[offset = 4]
    age_of_service: u16,
    #[scale = 0.0001]
    #[unit = "deg"]
    variation: i16,
}

#[derive(PgnMessageDerive, Debug)]
pub struct VesselHeading {
    source_id: u8,
    #[scale = 0.0001]
    heading: u16,
    #[scale = 0.0001]
    deviation: i16,
    #[scale = 0.0001]
    variation: i16,
    #[lookup]
    #[bits = 2]
    reference: DirectionReference,
}

#[derive(PgnMessageDerive, Debug)]
pub struct Attitude {
    source_id: u8,
    #[scale = 0.0001]
    yaw: i16,
    #[scale = 0.0001]
    pitch: i16,
    #[scale = 0.0001]
    roll: i16,
}

// pub struct AisClassAPositionReport {
//     source_id: u8,
//     message_id: AisMessageId,
//     repeat_indicator: RepeatIndicator,
//     user_id: u32,
//     longitude: i32,
//     latitude: i32,
//     position_accuracy: PositionAccuracy,
//     raim: RaimFlag,
//     time_stamp: TimeStamp,
//     cog: u16,
//     sog: u16,
//     communication_state: [u8; 19],
//     ais_transceiver_information: AisTransceiver,
//     heading: u16,
//     rate_of_turn: i16,
//     nav_status: NavStatus,
//     special_maneuver_indicator: AisSpecialManeuver,
//     sequence_id: u8
// }

#[derive(FieldsetDerive, Clone, Debug)]
pub struct ReferenceStation {
    #[bits = 12]
    reference_station_id: u16,
    #[scale = 0.01]
    age_of_dgnss_corrections: u16,
}

#[derive(PgnMessageDerive, Clone, Debug)]
pub struct GnssPositionData {
    source_id: u8,
    date: u16,
    #[scale = 0.0001]
    time: u32,
    #[scale = 1e-16]
    latitude: i64,
    #[scale = 1e-16]
    longitude: i64,
    #[scale = 1e-06]
    altitude: i64,
    #[lookup]
    #[bits = 4]
    gnss_type: Gns,
    #[lookup]
    #[bits = 4]
    method: GnsMethod,
    #[lookup]
    #[bits = 2]
    integrity: GnsIntegrity,
    #[offset = 6]
    number_of_svs: u8,
    #[scale = 0.01]
    hdop: i16,
    #[scale = 0.01]
    pdop: i16,
    #[scale = 0.01]
    geoidal_separation: i32,
    reference_stations: u8,
    #[fieldset]
    #[length_field = "reference_stations"]
    reference_station_structs: Vec<ReferenceStation>,
}

// macro_rules! define_pgns {
//     ( $(($pgndef:ident, $pgn:expr)),* ) => {
//         #[derive(Clone, Debug)]
//         pub enum Nmea2000Message {
//             $(Pgn{{$pgn($pgndef)}}),*,
//             Unsupported(u32)
//         }

//         impl Nmea2000Message {
//             pub fn pgn(&self) -> u32 {
//                 match self {
//                     $(Self::Pgn{{$pgn(msg)}} => $pgn),*,
//                     Self::Unsupported(pgn) => pgn
//                 }
//             }

//             pub fn key(&self) -> Result<String, NmeaParseError> {
//                 match self {
//                     $(Self::Pgn{{$pgn(msg)}} => Ok(format!("{:#x}-{}", self.pgn(), msg.source_id()))),*,
//                     Self::Unsupported(pgn) => Err(NmeaParseError::UnsupportedPgn(pgn))
//                 }
//             }

//             pub fn from_bytes(pgn: u32, source_id: u8, bytes: Vec<u8>) -> Result<Self, crate::parse_helpers::errors::NmeaParseError> {
//                 Ok(match pgn {
//                     $($pgn => Self::Pgn{{$pgn($pgndef::from_bytes(bytes.as_slice(), Some(source_id))?.0)}}),*,
//                     x => Self::Unsupported(pgn)
//                 })
//             }

//             pub fn to_readings(self) -> Result<GenericReadingsResult, crate::parse_helpers::errors::NmeaParseError> {
//                 match self {
//                     $(Self::Pgn$pgn(msg) => msg.to_readings()),*,
//                     Self::Unsupported(pgn) => Err(NmeaParseError::UnsupportedPgn(pgn))
//                 }
//             }
//         }
//     };
// }

// define_pgns!((VesselHeading, 127250), (Attitude, 12727));

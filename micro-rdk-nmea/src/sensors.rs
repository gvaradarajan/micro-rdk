// use base64::{engine::general_purpose, Engine};
// use micro_rdk::{
//     common::sensor::{GenericReadingsResult, Readings, Sensor, SensorError, SensorType},
//     google::protobuf::{value::Kind, Value},
// };
// use std::collections::HashMap;

// use crate::messages::pgns::Nmea2000Message;

// pub struct AllPgns(SensorType);

// impl Readings for AllPgns {
//     fn get_generic_readings(&mut self) -> Result<GenericReadingsResult, SensorError> {
//         let utf8_encoded = self.0.lock().unwrap().get_generic_readings()?;
//         let mapped: Result<Vec<String, Value>, SensorError> = utf8_encoded
//             .into_iter()
//             .map(|(k, v)| {
//                 let (pgn_str, source_id_str) = k
//                     .split_once("-")
//                     .ok_or(Err(SensorError::SensorGenericError("improper key format")))?;
//                 let source_id_str = source_id_str.to_string();
//                 let source_id = source_id_str
//                     .parse::<u8>()
//                     .map_err(|err| SensorError::SensorGenericError(err.to_string().as_str()))?;
//                 let pgn_str = pgn_str.to_string();
//                 let pgn = pgn_str
//                     .parse::<u32>()
//                     .map_err(|err| SensorError::SensorGenericError(err.to_string().as_str()))?;
//                 let new_value = if let Some(inner_val) = v.kind.as_ref() {
//                     match inner_val {
//                         ProtoKind::StringValue(inner_val) => {
//                             let mut data = Vec::<u8>::new();
//                             general_purpose::STANDARD
//                                 .decode_vec(inner_val, &mut data)
//                                 .map_err(|err| {
//                                     SensorError::SensorGenericError(err.to_string().as_str())
//                                 })?;
//                             let msg = Nmea2000Message::from_bytes(pgn, source_id, data).map_err(
//                                 |err| SensorError::SensorGenericError(err.to_string().as_str()),
//                             )?;
//                             Value {
//                                 kind: Some(Kind::StructValue(msg.to_readings()?)),
//                             }
//                         }
//                     }
//                 } else {
//                     return Err(SensorError::SensorGenericError("empty value in raw data"));
//                 };
//                 Ok((k, new_value))
//             })
//             .collect();
//         Ok(HashMap::from_iter(mapped?.into_iter()))
//     }
// }

// impl Sensor for AllPgns {}

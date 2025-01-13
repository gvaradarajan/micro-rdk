pub mod messages;
pub mod parse_helpers;

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose, Engine};

    use crate::{messages::pgns::WaterDepth, parse_helpers::errors::NumberFieldError};

    #[test]
    fn water_depth_parse() {
        let water_depth_str = "C/UBAHg+gD/TL/RmAAAAAFZODAAAAAAACAD/ABMAAwD/1AAAAAAA/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(water_depth_str, &mut data);
        assert!(res.is_ok());
        let thing2 = WaterDepth::from_bytes(&data[33..], Some(13));
        assert!(thing2.is_ok());
        let (thing2, _) = thing2.unwrap();
        assert_eq!(thing2.source_id(), 13);
        let depth = thing2.depth();
        assert!(depth.is_ok());
        assert_eq!(depth.unwrap(), 2.12);
        let offset = thing2.offset();
        assert!(offset.is_ok());
        assert_eq!(offset.unwrap(), 0.0);
        let range = thing2.range();
        assert!(range.is_err_and(|err| {
            matches!(err, NumberFieldError::FieldNotPresent(x) if x.as_str() == "range")
        }));
    }

    #[test]
    fn water_depth_parse_2() {
        let water_depth_str = "C/UBAHg+gD8l2A2A/////40fszsAAAAACAD/AAIAAwAAhgEAALwC/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(water_depth_str, &mut data);
        assert!(res.is_ok());
        let thing2 = WaterDepth::from_bytes(&data[33..], Some(13));
        assert!(thing2.is_ok());
        let (thing2, _) = thing2.unwrap();
        assert_eq!(thing2.source_id(), 13);
        let depth = thing2.depth();
        assert!(depth.is_ok());
        assert_eq!(depth.unwrap(), 3.9);
        let offset = thing2.offset();
        assert!(offset.is_ok());
        assert_eq!(offset.unwrap(), 0.7000000000000001);
        let range = thing2.range();
        assert!(range.is_err_and(|err| {
            matches!(err, NumberFieldError::FieldNotPresent(x) if x.as_str() == "range")
        }));
    }
}

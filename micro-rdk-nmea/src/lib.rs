pub mod messages;
pub mod parse_helpers;

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose, Engine};

    use crate::messages::pgns::WaterDepth;

    #[test]
    fn water_depth_parse() {
        let water_depth_str = "C/UBAHg+gD/TL/RmAAAAAFZODAAAAAAACAD/ABMAAwD/1AAAAAAA/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(water_depth_str, &mut data);
        assert!(res.is_ok());

        let thing2 = WaterDepth::from_bytes(&data, Some(13));
        assert!(thing2.is_ok());
        let (thing2, _) = thing2.unwrap();
        assert_eq!(thing2.source_id(), 13);
        let depth = thing2.depth();
        assert!(depth.is_ok());
        assert_eq!(depth.unwrap(), 1282.67);
        let offset = thing2.offset();
        assert!(offset.is_ok());
        assert_eq!(offset.unwrap(), 15.992);
        let range = thing2.range();
        assert!(range.is_ok());
        assert_eq!(range.unwrap(), 1280.0);
    }
}

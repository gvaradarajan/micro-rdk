pub mod messages;
pub mod parse_helpers;

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose, Engine};
    use chrono::DateTime;

    use crate::{
        messages::pgns::{
            MagneticVariation, SystemTime, TemperatureExtendedRange, VesselHeading, WaterDepth,
        },
        parse_helpers::enums::MagneticVariationSource,
    };

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

    #[test]
    fn temperature_parse() {
        // let temp_str = "DP0BAHg+gD9+kDZnAAAAALrFBAAAAAAACAD/ACMABQD/AAD3mwT//w==";
        // let temp_str = "DP0BAHg+gD90kDZnAAAAABTHBAAAAAAACAD/ACMABQD/AADwmwT//w==";
        let temp_str = "DP0BAHg+gD8QkDZnAAAAABLFBAAAAAAACAD/ACMABQD/AADzmwT//w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(temp_str, &mut data);
        assert!(res.is_ok());
        println!("data: {:?}", data);

        let thing2 = TemperatureExtendedRange::from_bytes(&data, Some(23));
        assert!(thing2.is_ok());
        let (thing2, _) = thing2.unwrap();
        assert_eq!(thing2.source_id(), 23);
        println!("temp: {:?}", thing2.temperature());
        println!("set temp: {:?}", thing2.set_temperature());
        println!("instance: {:?}", thing2.instance());
        println!("source {:?}", thing2.source());
        assert!(false);
    }

    #[test]
    fn system_time_parse() {
        let temp_str = "EPABAHg+gD+Dv0NnAAAAAOUGBgAAAAAACAD/AAQAAwA68FROQDQ7AA==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(temp_str, &mut data);
        assert!(res.is_ok());
        println!("data: {:?}", data);

        let thing2 = SystemTime::from_bytes(&data, Some(4));
        assert!(thing2.is_ok());
        let (thing2, _) = thing2.unwrap();
        assert_eq!(thing2.source_id(), 4);

        println!("source: {:?}", thing2.source());
        println!("days: {:?}", thing2.date());
        println!("time: {:?}", thing2.time());

        let seconds = thing2.time();
        assert!(seconds.is_ok());
        let seconds = seconds.unwrap();
        let seconds_i = seconds.floor();
        let nanos = ((seconds - seconds_i) * 1e9).floor() as u32;
        let days = thing2.date();
        assert!(days.is_ok());
        let days = days.unwrap();

        // let tz = chrono_tz::Tz::UTC;
        let total_seconds = (days as i64) + (seconds_i as i64);

        let datetime = DateTime::from_timestamp(total_seconds, nanos);
        println!("date: {:?}", datetime);
        // assert!(false);
    }

    #[test]
    fn magnetic_variation_parse() {
        let temp_str = "GvEBAHg+gD+Dv0NnAAAAADsTBgAAAAAACAD/AAQABgA68lROFfv//w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(temp_str, &mut data);
        assert!(res.is_ok());

        let thing2 = MagneticVariation::from_bytes(&data, Some(4));
        assert!(thing2.is_ok());
        let (thing2, _) = thing2.unwrap();
        assert_eq!(thing2.source_id(), 4);

        assert!(matches!(
            thing2.source(),
            MagneticVariationSource::AutomaticChart
        ));
        let age_of_service = thing2.age_of_service();
        assert!(age_of_service.is_ok());
        let age_of_service = age_of_service.unwrap();
        assert_eq!(age_of_service, 497);

        let var = thing2.variation();
        assert!(var.is_ok());
        let var = var.unwrap();
        assert_eq!(var, 176.0126346641889);
    }

    #[test]
    fn vessel_heading_parse() {
        let temp_str = "EvEBAHg+gD+Dv0NnAAAAAHwJCwAAAAAACAD/AAMAAgD/3oD/f/9//Q==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(temp_str, &mut data);
        assert!(res.is_ok());
        println!("data: {:?}", data);

        let thing2 = VesselHeading::from_bytes(&data, Some(16));
        assert!(thing2.is_ok());
        let (thing2, _) = thing2.unwrap();
        assert_eq!(thing2.source_id(), 16);

        // println!("heading: {:?}", thing2.heading());
        // println!("variation: {:?}", thing2.variation());
        // println!("deviation: {:?}", thing2.deviation());
        // println!("ref: {:?}", thing2.reference());
        // assert!(thing2.to_readings().is_ok());
        // assert!(false);
    }

    #[test]
    fn did_it_work() {
        // let thing = TestThing
        assert!(true);
    }
}

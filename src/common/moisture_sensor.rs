use crate::common::analog::AnalogReader;
use crate::common::sensor::GenericReadingsResult;
use crate::common::sensor::Sensor;
use crate::common::sensor::SensorResult;
use crate::common::sensor::SensorT;
use crate::common::sensor::TypedReadingsResult;
use crate::common::status::Status;
use crate::google;
// use std::cell::RefCell;
use std::collections::HashMap;
// use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::borrow::BorrowMut;

use super::sensor::Readings;

#[derive(DoCommand)]
pub struct MoistureSensor {
    analog: Arc<Mutex<dyn AnalogReader<u16, Error = anyhow::Error> + Send>>,
}

impl MoistureSensor {
    pub fn new(analog: Arc<Mutex<dyn AnalogReader<u16, Error = anyhow::Error> + Send>>) -> Self {
        MoistureSensor { analog }
    }
}

impl Sensor for MoistureSensor {}

impl Readings for MoistureSensor {
    fn get_generic_readings(&mut self) -> anyhow::Result<GenericReadingsResult> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

impl SensorT<f64> for MoistureSensor {
    fn get_readings(&mut self) -> anyhow::Result<TypedReadingsResult<f64>> {
        let reading = self.analog.read()?;
        let mut x = HashMap::new();
        x.insert("millivolts".to_string(), reading as f64);
        Ok(x)
    }
}

impl Status for MoistureSensor {
    fn get_status(&mut self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

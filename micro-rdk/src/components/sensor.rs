#![allow(dead_code)]

use crate::common::status::Status;
use crate::google;

use std::sync::{Arc, Mutex};

use crate::common::generic::DoCommand;

pub static COMPONENT_NAME: &str = "sensor";

pub type GenericReadingsResult =
    ::std::collections::HashMap<::prost::alloc::string::String, google::protobuf::Value>;

pub type TypedReadingsResult<T> = ::std::collections::HashMap<String, T>;

pub trait Readings {
    fn get_generic_readings(&mut self) -> anyhow::Result<GenericReadingsResult>;
}

pub trait Sensor: Readings + Status + DoCommand {}

pub type SensorType = Arc<Mutex<dyn Sensor>>;

pub trait SensorT<T>: Sensor {
    fn get_readings(&self) -> anyhow::Result<TypedReadingsResult<T>>;
}

// A local wrapper type we can use to specialize `From` for `google::protobuf::Value``
pub struct SensorResult<T> {
    pub value: T,
}

impl From<SensorResult<f64>> for google::protobuf::Value {
    fn from(value: SensorResult<f64>) -> google::protobuf::Value {
        google::protobuf::Value {
            kind: Some(google::protobuf::value::Kind::NumberValue(value.value)),
        }
    }
}

impl<A> Sensor for Mutex<A> where A: ?Sized + Sensor {}

impl<A> Sensor for Arc<Mutex<A>> where A: ?Sized + Sensor {}

impl<A> Readings for Mutex<A>
where
    A: ?Sized + Readings,
{
    fn get_generic_readings(&mut self) -> anyhow::Result<GenericReadingsResult> {
        self.get_mut().unwrap().get_generic_readings()
    }
}

impl<A> Readings for Arc<Mutex<A>>
where
    A: ?Sized + Readings,
{
    fn get_generic_readings(&mut self) -> anyhow::Result<GenericReadingsResult> {
        self.lock().unwrap().get_generic_readings()
    }
}

#![allow(dead_code)]
use crate::common::generic::DoCommand;
use crate::common::math_utils::Vector3;
use crate::components::sensor::{GenericReadingsResult, Readings};
use crate::common::status::Status;
use crate::google;
use crate::google::protobuf::{value::Kind, Struct, Value};
use crate::proto::common::v1::GeoPoint;
use crate::proto::component::movement_sensor;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub static COMPONENT_NAME: &str = "movement_sensor";

// A local struct representation of the supported methods indicated by the
// GetProperties method of the Movement Sensor API. TODO: add a boolean for
// orientation when it is supportable.
pub struct MovementSensorSupportedMethods {
    pub position_supported: bool,
    pub linear_velocity_supported: bool,
    pub angular_velocity_supported: bool,
    pub linear_acceleration_supported: bool,
    pub compass_heading_supported: bool,
}

impl From<MovementSensorSupportedMethods> for movement_sensor::v1::GetPropertiesResponse {
    fn from(props: MovementSensorSupportedMethods) -> movement_sensor::v1::GetPropertiesResponse {
        movement_sensor::v1::GetPropertiesResponse {
            position_supported: props.position_supported,
            linear_velocity_supported: props.linear_velocity_supported,
            angular_velocity_supported: props.angular_velocity_supported,
            linear_acceleration_supported: props.linear_acceleration_supported,
            compass_heading_supported: props.compass_heading_supported,
            orientation_supported: false,
        }
    }
}

// A struct representing geographic coordinates (latitude-longitude-altitude)
#[derive(Clone, Copy, Debug, Default)]
pub struct GeoPosition {
    pub lat: f64,
    pub lon: f64,
    pub alt: f32,
}

impl From<GeoPosition> for Value {
    fn from(value: GeoPosition) -> Self {
        let mut fields = HashMap::new();
        fields.insert(
            "lat".to_string(),
            Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(value.lat)),
            },
        );
        fields.insert(
            "lon".to_string(),
            Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(value.lon)),
            },
        );
        fields.insert(
            "alt".to_string(),
            Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(value.alt as f64)),
            },
        );
        Value {
            kind: Some(google::protobuf::value::Kind::StructValue(Struct {
                fields,
            })),
        }
    }
}

impl From<GeoPosition> for movement_sensor::v1::GetPositionResponse {
    fn from(pos: GeoPosition) -> movement_sensor::v1::GetPositionResponse {
        let pt = GeoPoint {
            latitude: pos.lat,
            longitude: pos.lon,
        };
        movement_sensor::v1::GetPositionResponse {
            coordinate: Some(pt),
            altitude_m: pos.alt,
        }
    }
}

// A trait for implementing a movement sensor component driver. TODO: add
// get_orientation and get_accuracy if/when they become supportable.
pub trait MovementSensor: Status + Readings + DoCommand {
    fn get_position(&mut self) -> anyhow::Result<GeoPosition>;
    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3>;
    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3>;
    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3>;
    fn get_compass_heading(&mut self) -> anyhow::Result<f64>;
    fn get_properties(&self) -> MovementSensorSupportedMethods;
}

pub type MovementSensorType = Arc<Mutex<dyn MovementSensor>>;

pub fn get_movement_sensor_generic_readings(
    ms: &mut dyn MovementSensor,
) -> anyhow::Result<GenericReadingsResult> {
    let mut res = std::collections::HashMap::new();
    let supported_methods = ms.get_properties();
    if supported_methods.position_supported {
        res.insert("position".to_string(), ms.get_position()?.into());
    }
    if supported_methods.linear_velocity_supported {
        res.insert(
            "linear_velocity".to_string(),
            ms.get_linear_velocity()?.into(),
        );
    }
    if supported_methods.linear_acceleration_supported {
        res.insert(
            "linear_acceleration".to_string(),
            ms.get_linear_acceleration()?.into(),
        );
    }
    if supported_methods.angular_velocity_supported {
        res.insert(
            "angular_velocity".to_string(),
            ms.get_angular_velocity()?.into(),
        );
    }
    if supported_methods.compass_heading_supported {
        res.insert(
            "compass_heading".to_string(),
            Value {
                kind: Some(Kind::NumberValue(ms.get_compass_heading()?)),
            },
        );
    }
    Ok(res)
}

impl<A> MovementSensor for Mutex<A>
where
    A: ?Sized + MovementSensor,
{
    fn get_position(&mut self) -> anyhow::Result<GeoPosition> {
        self.get_mut().unwrap().get_position()
    }

    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        self.get_mut().unwrap().get_linear_acceleration()
    }

    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3> {
        self.get_mut().unwrap().get_linear_velocity()
    }

    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        self.get_mut().unwrap().get_angular_velocity()
    }

    fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
        self.get_mut().unwrap().get_compass_heading()
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        self.lock().unwrap().get_properties()
    }
}

impl<A> MovementSensor for Arc<Mutex<A>>
where
    A: ?Sized + MovementSensor,
{
    fn get_position(&mut self) -> anyhow::Result<GeoPosition> {
        self.lock().unwrap().get_position()
    }

    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        self.lock().unwrap().get_linear_acceleration()
    }

    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3> {
        self.lock().unwrap().get_linear_velocity()
    }

    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        self.lock().unwrap().get_angular_velocity()
    }

    fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
        self.lock().unwrap().get_compass_heading()
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        self.lock().unwrap().get_properties()
    }
}

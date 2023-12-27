use crate::proto::app::data_sync::v1::SensorData;

use super::{
    config::{AttributeError, Kind},
    movement_sensor::MovementSensor,
    power_sensor::PowerSensor,
    robot::ResourceType,
    sensor::get_sensor_readings_data,
};

#[derive(Debug, Clone, Copy)]
pub struct DataCollectorConfig {
    pub method: CollectionMethod,
    pub capture_frequency_hz: f32,
}

impl TryFrom<&Kind> for DataCollectorConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("method")? {
            return Err(AttributeError::KeyNotFound("method".to_string()));
        }
        if !value.contains_key("capture_frequency_hz")? {
            return Err(AttributeError::KeyNotFound(
                "capture_frequency_hz".to_string(),
            ));
        }
        let method_str: String = value.get("method")?.ok_or(AttributeError::KeyNotFound("method".to_string()))?.try_into()?;
        let capture_frequency_hz = value.get("capture_frequency_hz")?.ok_or(AttributeError::KeyNotFound("capture_frequency_hz".to_string()))?.try_into()?;
        Ok(DataCollectorConfig {
            method: method_str.try_into()?,
            capture_frequency_hz,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CollectionMethod {
    Readings,
    // MovementSensor methods
    AngularVelocity,
    LinearAcceleration,
    LinearVelocity,
    Voltage,
    Current,
}

impl CollectionMethod {
    fn method_str(&self) -> String {
        match self {
            Self::Readings => "readings".to_string(),
            Self::AngularVelocity => "angularvelocity".to_string(),
            Self::LinearAcceleration => "linearacceleration".to_string(),
            Self::LinearVelocity => "linearvelocity".to_string(),
            Self::Voltage => "voltage".to_string(),
            Self::Current => "current".to_string()
        }
    }
}

impl TryFrom<String> for CollectionMethod {
    type Error = AttributeError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "Readings" => Ok(Self::Readings),
            "AngularVelocity" => Ok(Self::AngularVelocity),
            "LinearAcceleration" => Ok(Self::LinearAcceleration),
            "LinearVelocity" => Ok(Self::LinearVelocity),
            "Voltage" => Ok(Self::Voltage),
            "Current" => Ok(Self::Current),
            _ => Err(AttributeError::ConversionImpossibleError),
        }
    }
}

pub struct DataCollector {
    name: String,
    component_type: String,
    resource: ResourceType,
    method: CollectionMethod,
}

fn resource_method_pair_is_valid(resource: &ResourceType, method: CollectionMethod) -> bool {
    match resource {
        ResourceType::Sensor(_) => matches!(method, CollectionMethod::Readings),
        ResourceType::MovementSensor(_) => matches!(method, CollectionMethod::Readings | CollectionMethod::AngularVelocity | CollectionMethod::LinearAcceleration | CollectionMethod::LinearVelocity),
        ResourceType::PowerSensor(_) => matches!(method, CollectionMethod::Readings | CollectionMethod::Voltage | CollectionMethod::Current),
        _ => false,
    }
}

impl DataCollector {
    pub fn new(
        name: String,
        resource: ResourceType,
        method: CollectionMethod,
    ) -> anyhow::Result<Self> {
        let component_type = resource.component_type();
        if !resource_method_pair_is_valid(&resource, method) {
            anyhow::bail!(
                "cannot collect data on method {:?} for {:?} named {:?}",
                method,
                component_type,
                name
            )
        }
        Ok(DataCollector {
            name,
            component_type,
            resource,
            method,
        })
    }

    pub fn name(&self) -> String {
        self.name.to_string()
    }

    pub fn component_type(&self) -> String {
        self.component_type.to_string()
    }

    pub fn method_str(&self) -> String {
        self.method.method_str()
    }

    pub fn collect_data(&mut self) -> anyhow::Result<SensorData> {
        Ok(match &mut self.resource {
            ResourceType::Sensor(ref mut res) => match self.method {
                CollectionMethod::Readings => get_sensor_readings_data(res)?,
                _ => unreachable!(),
            },
            ResourceType::MovementSensor(ref mut res) => match self.method {
                CollectionMethod::Readings => get_sensor_readings_data(res)?,
                CollectionMethod::AngularVelocity => res
                    .get_angular_velocity()?
                    .to_sensor_data("angular_velocity"),
                CollectionMethod::LinearAcceleration => res
                    .get_linear_acceleration()?
                    .to_sensor_data("linear_acceleration"),
                CollectionMethod::LinearVelocity => {
                    res.get_linear_velocity()?.to_sensor_data("linear_velocity")
                }
                _ => unreachable!(),
            },
            ResourceType::PowerSensor(ref mut res) => match self.method {
                CollectionMethod::Voltage => res.get_voltage()?.into(),
                CollectionMethod::Current => res.get_current()?.into(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        })
    }
}

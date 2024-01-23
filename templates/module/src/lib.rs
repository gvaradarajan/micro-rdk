use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use micro_rdk::DoCommand;
use micro_rdk::common::status::Status;
use micro_rdk::common::registry::{ComponentRegistry, RegistryError};
{% if starting_component == "Motor" %}
use std::time::Duration;
use micro_rdk::common::actuator::Actuator;
use micro_rdk::common::motor::{Motor, MotorType, MotorSupportedProperties};
{% elsif starting_component == "Base" %}
use micro_rdk::common::actuator::Actuator;
use micro_rdk::common::base::{Base, BaseType};
use micro_rdk::proto::common::v1::Vector3;
{% elsif starting_component == "MovementSensor" %}
use micro_rdk::math_utils::Vector3;
use micro_rdk::MovementSensorReadings;
use micro_rdk::common::movement_sensor::{MovementSensor, MovementSensorType, GeoPosition, MovementSensorSupportedMethods};
{% elsif starting_component == "PowerSensor" %}
use micro_rdk::PowerSensorReadings;
use micro_rdk::common::power_sensor::{PowerSensor, PowerSensorType, PowerSupplyType, Voltage, Current};
{% elsif starting_component == "Sensor" %}
use micro_rdk::common::sensor::{Sensor, SensorType, Readings};
{% elsif starting_component == "Servo" %}
use micro_rdk::common::actuator::Actuator;
use micro_rdk::common::servo::{Servo, ServoType};
{% elsif starting_component == "GenericComponent" %}
use micro_rdk::common::generic::{GenericComponent, GenericComponentType};
{% elsif starting_component == "Encoder" %}
use micro_rdk::common::encoder::{Encoder, EncoderType, EncoderPosition, EncoderPositionType, EncoderSupportedRepresentations};
{% else %}
{% endif %}

pub fn register_models(registry: &mut ComponentRegistry) -> anyhow::Result<(), RegistryError> {
    {% if starting_component == "Motor" %}registry.register_motor("my_motor", &My{{starting_component}}::from_config){% elsif starting_component == "Base" %}registry.register_base("my_base", &My{{starting_component}}::from_config){% elsif starting_component == "MovementSensor" %}registry.register_movement_sensor("my_movement_sensor", &My{{starting_component}}::from_config){% elsif starting_component == "PowerSensor" %}registry.register_power_sensor("my_power_sensor", &My{{starting_component}}::from_config){% elsif starting_component == "Sensor" %}registry.register_sensor("my_sensor", &My{{starting_component}}::from_config){% elsif starting_component == "Servo" %}registry.register_servo("my_servo", &My{{starting_component}}::from_config){% elsif starting_component == "GenericComponent" %}registry.register_generic_component("my_generic_component", &My{{starting_component}}::from_config){% elsif starting_component == "Encoder" %}registry.register_encoder("my_encoder", &My{{starting_component}}::from_config){% else %}Ok(()){% endif %}
}

{% if starting_component != "None" %}
#[derive(DoCommand{% if starting_component == "MovementSensor" %}, MovementSensorReadings{% elsif starting_component == "PowerSensor" %}, PowerSensorReadings{% else %}{% endif %})]
pub struct My{{starting_component}} {}

impl My{{starting_component}} {
    pub fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> anyhow::Result<{{starting_component}}Type> {
        Ok(Arc::new(Mutex::new(My{{starting_component}} {})))
    }
}

impl Status for My{{starting_component}} {
    fn get_status(&self) -> anyhow::Result<Option<micro_rdk::google::protobuf::Struct>> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
{% endif %}

{% if starting_component == "Motor" or starting_component == "Base" or starting_component == "Servo" %}
impl Actuator for My{{starting_component}} {
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}{% else %}{% endif %}

{% if starting_component == "Motor" %}
impl Motor for My{{starting_component}} {
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        anyhow::bail!("position reporting not supported")
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        anyhow::bail!("go_for unimplemented for My{{starting_component}}")
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: false,
        }
    }
}
{% elsif starting_component == "Base" %}
impl Base for My{{starting_component}} {
    fn set_power(&mut self, lin: &Vector3, ang: &Vector3) -> anyhow::Result<()> {
        Ok(())
    }
}
{% elsif starting_component == "MovementSensor" %}
impl MovementSensor for My{{starting_component}} {
    fn get_position(&mut self) -> anyhow::Result<GeoPosition> {
        anyhow::bail!("get_position not supported")
    }
    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("get_linear_velocity not supported")
    }
    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("get_angular_velocity not supported")
    }
    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("get_linear_acceleration not supported")
    }
    fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
        anyhow::bail!("get_compass_heading not supported")
    }
    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: false,
            linear_acceleration_supported: false,
            linear_velocity_supported: false,
            angular_velocity_supported: false,
            compass_heading_supported: false,
        }
    }
}
{% elsif starting_component == "PowerSensor" %}
impl PowerSensor for My{{starting_component}} {
    fn get_voltage(&mut self) -> anyhow::Result<Voltage> {
        Ok(Voltage {
            volts: 0,
            power_supply_type: PowerSupplyType::AC
        })
    }
    fn get_current(&mut self) -> anyhow::Result<Current> {
        Ok(Current {
            amperes: 0,
            power_supply_type: PowerSupplyType::AC
        })
    }
}
{% elsif starting_component == "Sensor" %}
impl Sensor for My{{starting_component}} {}

impl Readings for My{{starting_component}} {
    fn get_generic_readings(&mut self) -> anyhow::Result<GenericReadingsResult> {
        Ok(HashMap::new())
    }
}
{% elsif starting_component == "Servo" %}
impl Servo for My{{starting_component}} {
    fn move_to(&mut self, angle_deg: u32) -> anyhow::Result<()> {
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<u32> {
        Ok(0)
    }
}
{% elsif starting_component == "Encoder" %}
impl Encoder for My{{starting_component}} {
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        EncoderSupportedRepresentations {
            ticks_count_supported: true,
            angle_degrees_supported: false,
        }
    }
    fn get_position(&self, position_type: EncoderPositionType) -> anyhow::Result<EncoderPosition> {
        Ok(EncoderPositionType::TICKS.wrap_value(0.0))
    }
}
{% else %}{% endif %}
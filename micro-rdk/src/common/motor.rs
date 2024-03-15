#![allow(dead_code)]
use crate::common::status::Status;
use crate::proto::component::motor::v1::GetPropertiesResponse;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::config::{AttributeError, Kind};
use super::actuator::Actuator;
use super::generic::DoCommand;

pub static COMPONENT_NAME: &str = "motor";

pub struct MotorSupportedProperties {
    pub position_reporting: bool,
}

impl From<MotorSupportedProperties> for GetPropertiesResponse {
    fn from(value: MotorSupportedProperties) -> Self {
        GetPropertiesResponse {
            position_reporting: value.position_reporting,
        }
    }
}

pub trait Motor: Status + Actuator + DoCommand {
    /// Sets the percentage of the motor's total power that should be employed.
    /// expressed a value between `-1.0` and `1.0` where negative values indicate a backwards
    /// direction and positive values a forward direction.
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()>;
    /// Reports the position of the robot's motor relative to its zero position.
    /// This method will return an error if position reporting is not supported.
    fn get_position(&mut self) -> anyhow::Result<i32>;
    /// Instructs the motor to turn at a specified speed, which is expressed in RPM,
    /// for a specified number of rotations relative to its starting position.
    /// This method will return an error if position reporting is not supported.
    /// If revolutions is 0, this will run the motor at rpm indefinitely.
    /// If revolutions != 0, this will block until the number of revolutions has been completed or another operation comes in.
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>>;
    /// Returns an instance of MotorSupportedProperties indicating the optional properties
    /// supported by this motor
    fn get_properties(&mut self) -> MotorSupportedProperties;
}

pub type MotorType = Arc<Mutex<dyn Motor>>;

#[derive(Debug)]
pub enum MotorPinType {
    PwmAB,
    PwmDirection,
    AB,
}

#[derive(Debug, Default)]
pub struct MotorPinsConfig {
    pub(crate) a: Option<i32>,
    pub(crate) b: Option<i32>,
    pub(crate) dir: Option<i32>,
    pub(crate) pwm: Option<i32>,
}

impl MotorPinsConfig {
    pub fn detect_motor_type(&self) -> anyhow::Result<MotorPinType> {
        match self {
            x if (x.a.is_some() && x.b.is_some()) => match x.pwm {
                Some(_) => Ok(MotorPinType::PwmAB),
                None => Ok(MotorPinType::AB),
            },
            x if x.dir.is_some() => Ok(MotorPinType::PwmDirection),
            _ => Err(anyhow::anyhow!("invalid pin parameters for motor")),
        }
    }
}

impl TryFrom<&Kind> for MotorPinsConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        let a = match value.get("a") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let b = match value.get("b") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let dir = match value.get("dir") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        let pwm = match value.get("pwm") {
            Ok(opt) => match opt {
                Some(val) => Some(val.try_into()?),
                None => None,
            },
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => None,
                _ => {
                    return Err(err);
                }
            },
        };
        Ok(Self { a, b, dir, pwm })
    }
}

impl<L> Motor for Mutex<L>
where
    L: ?Sized + Motor,
{
    fn get_position(&mut self) -> anyhow::Result<i32> {
        self.get_mut().unwrap().get_position()
    }
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        self.get_mut().unwrap().set_power(pct)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        self.get_mut().unwrap().go_for(rpm, revolutions)
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        self.get_mut().unwrap().get_properties()
    }
}

impl<A> Motor for Arc<Mutex<A>>
where
    A: ?Sized + Motor,
{
    fn get_position(&mut self) -> anyhow::Result<i32> {
        self.lock().unwrap().get_position()
    }
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        self.lock().unwrap().set_power(pct)
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        self.lock().unwrap().go_for(rpm, revolutions)
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        self.lock().unwrap().get_properties()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::common::config::{Component, DynamicComponentConfig, Kind};
    use crate::common::motor::{ConfigType, FakeMotor, MotorPinType, MotorPinsConfig};
    #[test_log::test]
    fn test_motor_config() -> anyhow::Result<()> {
        let robot_config: [Option<DynamicComponentConfig>; 1] = [Some(DynamicComponentConfig {
            name: "motor".to_owned(),
            namespace: "rdk".to_owned(),
            r#type: "motor".to_owned(),
            model: "gpio".to_owned(),
            attributes: Some(HashMap::from([
                ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                ("board".to_owned(), Kind::StringValue("board".to_owned())),
                (
                    "pins".to_owned(),
                    Kind::StructValue(HashMap::from([
                        ("a".to_owned(), Kind::StringValue("11".to_owned())),
                        ("b".to_owned(), Kind::StringValue("12".to_owned())),
                        ("pwm".to_owned(), Kind::StringValue("13".to_owned())),
                        ("dir".to_owned(), Kind::StringValue("14".to_owned())),
                    ])),
                ),
            ])),
        })];

        let val = robot_config[0]
            .as_ref()
            .unwrap()
            .get_attribute::<MotorPinsConfig>("pins");
        assert!(&val.is_ok());

        let val = val.unwrap();

        assert!(val.a.is_some());
        assert_eq!(val.a.unwrap(), 11);
        assert!(val.b.is_some());
        assert_eq!(val.b.unwrap(), 12);
        assert!(val.pwm.is_some());
        assert_eq!(val.pwm.unwrap(), 13);
        assert!(val.dir.is_some());
        assert_eq!(val.dir.unwrap(), 14);

        let dyn_conf = ConfigType::Dynamic(robot_config[0].as_ref().unwrap());
        assert!(FakeMotor::from_config(dyn_conf, Vec::new()).is_ok());

        Ok(())
    }

    #[test_log::test]
    fn test_detect_motor_type_from_cfg() {
        let robot_config: [Option<DynamicComponentConfig>; 4] = [
            Some(DynamicComponentConfig {
                name: "motor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                    ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([
                            ("a".to_owned(), Kind::StringValue("11".to_owned())),
                            ("b".to_owned(), Kind::StringValue("12".to_owned())),
                            ("pwm".to_owned(), Kind::StringValue("13".to_owned())),
                        ])),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "motor".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                    ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([
                            ("dir".to_owned(), Kind::StringValue("11".to_owned())),
                            ("pwm".to_owned(), Kind::StringValue("13".to_owned())),
                        ])),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "motor2".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                    ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([(
                            "pwm".to_owned(),
                            Kind::StringValue("13".to_owned()),
                        )])),
                    ),
                ])),
            }),
            Some(DynamicComponentConfig {
                name: "motor3".to_owned(),
                namespace: "rdk".to_owned(),
                r#type: "motor".to_owned(),
                model: "gpio".to_owned(),
                attributes: Some(HashMap::from([
                    ("max_rpm".to_owned(), Kind::NumberValue(10000f64)),
                    ("fake_position".to_owned(), Kind::NumberValue(10f64)),
                    ("board".to_owned(), Kind::StringValue("board".to_owned())),
                    (
                        "pins".to_owned(),
                        Kind::StructValue(HashMap::from([
                            ("a".to_owned(), Kind::StringValue("11".to_owned())),
                            ("b".to_owned(), Kind::StringValue("13".to_owned())),
                        ])),
                    ),
                ])),
            }),
        ];

        let dyn_cfg = ConfigType::Dynamic(robot_config[0].as_ref().unwrap());
        let pin_cfg_result = dyn_cfg.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result.is_ok());
        let motor_type = pin_cfg_result.unwrap().detect_motor_type();
        assert!(motor_type.is_ok());
        assert!(matches!(motor_type.unwrap(), MotorPinType::PwmAB));

        let dyn_cfg_2 = ConfigType::Dynamic(robot_config[1].as_ref().unwrap());
        let pin_cfg_result_2 = dyn_cfg_2.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result_2.is_ok());
        let motor_type_2 = pin_cfg_result_2.unwrap().detect_motor_type();
        assert!(motor_type_2.is_ok());
        assert!(matches!(motor_type_2.unwrap(), MotorPinType::PwmDirection));

        let dyn_cfg_3 = ConfigType::Dynamic(robot_config[2].as_ref().unwrap());
        let pin_cfg_result_3 = dyn_cfg_3.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result_3.is_ok());
        let motor_type_3 = pin_cfg_result_3.unwrap().detect_motor_type();
        assert!(motor_type_3.is_err());

        let dyn_cfg_4 = ConfigType::Dynamic(robot_config[3].as_ref().unwrap());
        let pin_cfg_result_4 = dyn_cfg_4.get_attribute::<MotorPinsConfig>("pins");
        assert!(pin_cfg_result_4.is_ok());
        let motor_type_4 = pin_cfg_result_4.unwrap().detect_motor_type();
        assert!(motor_type_4.is_ok());
        assert!(matches!(motor_type_4.unwrap(), MotorPinType::AB));
    }
}

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use crate::google;
use crate::proto::component::encoder::v1::GetPositionResponse;
use crate::proto::component::encoder::v1::GetPropertiesResponse;
use crate::proto::component::encoder::v1::PositionType;

use crate::common::actuator::Actuator;
use crate::common::config::{AttributeError, ConfigType, Kind};
use crate::common::generic::{GenericComponent, GenericComponentType, DoCommand};
use crate::common::math_utils::{Vector3, go_for_math};
use crate::common::registry::{ComponentRegistry, Dependency, ResourceKey};
use crate::common::robot::Resource;
use crate::common::status::Status;

use crate::common::encoder::{Encoder, EncoderType, EncoderPositionType, EncoderPosition, EncoderSupportedRepresentations, COMPONENT_NAME as EncoderCompName};
use crate::common::motor::{
    Motor, MotorPinType, MotorPinsConfig, MotorSupportedProperties, MotorType,
    COMPONENT_NAME as MotorCompName,
};
use crate::common::movement_sensor::{MovementSensor, MovementSensorSupportedMethods, MovementSensorType, GeoPosition};
use crate::common::sensor::{Sensor, SensorT, SensorType, Readings, GenericReadingsResult, SensorResult, TypedReadingsResult};

use log::*;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_encoder("fake", &FakeEncoder::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
    if registry
        .register_encoder("fake_incremental", &FakeIncrementalEncoder::from_config)
        .is_err()
    {
        log::error!("fake_incremental type is already registered");
    }
    if registry
        .register_generic_component("fake", &FakeGenericComponent::from_config)
        .is_err()
    {
        log::error!("model fake is already registered")
    }
    if registry
        .register_motor("fake", &FakeMotor::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
    if registry
        .register_motor("fake_with_dep", &FakeMotorWithDependency::from_config)
        .is_err()
    {
        log::error!("fake_with_dep type is already registered");
    }
    if registry
        .register_dependency_getter(
            MotorCompName,
            "fake_with_dep",
            &FakeMotorWithDependency::dependencies_from_config,
        )
        .is_err()
    {
        log::error!("fake_with_dep type dependency function is already registered");
    }
    if registry
        .register_movement_sensor("fake", &FakeMovementSensor::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
    if registry
        .register_sensor("fake", &FakeSensor::from_config)
        .is_err()
    {
        log::error!("fake sensor type is already registered");
    }
}

#[derive(DoCommand)]
pub struct FakeIncrementalEncoder {
    pub ticks: f32,
}

impl Default for FakeIncrementalEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeIncrementalEncoder {
    pub fn new() -> Self {
        Self { ticks: 0.0 }
    }
    pub(crate) fn from_config(cfg: ConfigType, _: Vec<Dependency>) -> anyhow::Result<EncoderType> {
        let mut enc: FakeIncrementalEncoder = Default::default();
        if let Ok(fake_ticks) = cfg.get_attribute::<f32>("fake_ticks") {
            enc.ticks = fake_ticks;
        }
        Ok(Arc::new(Mutex::new(enc)))
    }
}

impl Encoder for FakeIncrementalEncoder {
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        EncoderSupportedRepresentations {
            ticks_count_supported: true,
            angle_degrees_supported: false,
        }
    }
    fn get_position(&self, position_type: EncoderPositionType) -> anyhow::Result<EncoderPosition> {
        match position_type {
            EncoderPositionType::TICKS | EncoderPositionType::UNSPECIFIED => {
                Ok(EncoderPositionType::TICKS.wrap_value(self.ticks))
            }
            EncoderPositionType::DEGREES => {
                anyhow::bail!("FakeIncrementalEncoder does not support returning angular position")
            }
        }
    }
    fn reset_position(&mut self) -> anyhow::Result<()> {
        self.ticks = 0.0;
        Ok(())
    }
}

impl Status for FakeIncrementalEncoder {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

#[derive(DoCommand)]
pub struct FakeEncoder {
    pub angle_degrees: f32,
    pub ticks_per_rotation: u32,
}

impl Default for FakeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeEncoder {
    pub fn new() -> Self {
        Self {
            angle_degrees: 0.0,
            ticks_per_rotation: 1,
        }
    }

    pub(crate) fn from_config(cfg: ConfigType, _: Vec<Dependency>) -> anyhow::Result<EncoderType> {
        let mut enc: FakeEncoder = Default::default();
        if let Ok(ticks_per_rotation) = cfg.get_attribute::<u32>("ticks_per_rotation") {
            enc.ticks_per_rotation = ticks_per_rotation;
        }
        if let Ok(fake_deg) = cfg.get_attribute::<f32>("fake_deg") {
            enc.angle_degrees = fake_deg;
        }
        Ok(Arc::new(Mutex::new(enc)))
    }
}

impl Encoder for FakeEncoder {
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        EncoderSupportedRepresentations {
            ticks_count_supported: true,
            angle_degrees_supported: true,
        }
    }
    fn get_position(&self, position_type: EncoderPositionType) -> anyhow::Result<EncoderPosition> {
        match position_type {
            EncoderPositionType::UNSPECIFIED => {
                anyhow::bail!("must specify position_type to get FakeEncoder position")
            }
            EncoderPositionType::DEGREES => Ok(position_type.wrap_value(self.angle_degrees)),
            EncoderPositionType::TICKS => {
                let value: f32 = (self.angle_degrees / 360.0) * (self.ticks_per_rotation as f32);
                Ok(position_type.wrap_value(value))
            }
        }
    }
}

impl Status for FakeEncoder {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

pub struct FakeGenericComponent {}

impl FakeGenericComponent {
    pub(crate) fn from_config(
        _: ConfigType,
        _: Vec<Dependency>,
    ) -> anyhow::Result<GenericComponentType> {
        Ok(Arc::new(Mutex::new(FakeGenericComponent {})))
    }
}

impl GenericComponent for FakeGenericComponent {}

impl DoCommand for FakeGenericComponent {
    fn do_command(&mut self, command_struct: Option<google::protobuf::Struct>) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut res = HashMap::new();
        if let Some(command_struct) = command_struct.as_ref() {
            for (key, val) in &command_struct.fields {
                match key.as_str() {
                    "ping" => {
                        res.insert(
                            "ping".to_string(),
                            google::protobuf::Value {
                                kind: Some(google::protobuf::value::Kind::StringValue("pinged".to_string())),
                            },
                        );
                    }
                    "echo" => {
                        res.insert("echoed".to_string(), val.to_owned());
                    }
                    _ => {}
                };
            }
        }
        Ok(Some(google::protobuf::Struct { fields: res }))
    }
}

impl Status for FakeGenericComponent {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

#[derive(DoCommand)]
pub struct FakeMotor {
    pos: f64,
    power: f64,
    max_rpm: f64,
}

impl FakeMotor {
    pub fn new() -> Self {
        Self {
            pos: 10.0,
            power: 0.0,
            max_rpm: 100.0,
        }
    }
    pub(crate) fn from_config(cfg: ConfigType, _: Vec<Dependency>) -> anyhow::Result<MotorType> {
        let mut motor = FakeMotor::default();
        if let Ok(pos) = cfg.get_attribute::<f64>("fake_position") {
            motor.pos = pos
        }
        if let Ok(max_rpm) = cfg.get_attribute::<f64>("max_rpm") {
            motor.max_rpm = max_rpm
        }
        Ok(Arc::new(Mutex::new(motor)))
    }
}
impl Default for FakeMotor {
    fn default() -> Self {
        Self::new()
    }
}

impl Motor for FakeMotor {
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(self.pos as i32)
    }
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        debug!("setting power to {}", pct);
        self.power = pct;
        Ok(())
    }
    fn go_for(&mut self, rpm: f64, revolutions: f64) -> anyhow::Result<Option<Duration>> {
        // get_max_rpm
        let (pwr, dur) = go_for_math(self.max_rpm, rpm, revolutions)?;
        self.set_power(pwr)?;
        Ok(dur)
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: true,
        }
    }
}
impl Status for FakeMotor {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        hm.insert(
            "position".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::NumberValue(self.pos)),
            },
        );
        hm.insert(
            "position_reporting".to_string(),
            google::protobuf::Value {
                kind: Some(google::protobuf::value::Kind::BoolValue(true)),
            },
        );

        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

impl Actuator for FakeMotor {
    fn stop(&mut self) -> anyhow::Result<()> {
        debug!("stopping motor");
        self.set_power(0.0)?;
        Ok(())
    }
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        Ok(self.power > 0.0)
    }
}

#[derive(DoCommand)]
pub struct FakeMotorWithDependency {
    encoder: Option<EncoderType>,
    power: f64,
}

impl FakeMotorWithDependency {
    pub fn new(encoder: Option<EncoderType>) -> Self {
        Self {
            encoder,
            power: 0.0,
        }
    }

    pub(crate) fn dependencies_from_config(cfg: ConfigType) -> Vec<ResourceKey> {
        let mut r_keys = Vec::new();
        log::info!("getting deps");
        if let Ok(enc_name) = cfg.get_attribute::<String>("encoder") {
            let r_key = ResourceKey(EncoderCompName, enc_name);
            r_keys.push(r_key)
        }
        r_keys
    }

    pub(crate) fn from_config(_: ConfigType, deps: Vec<Dependency>) -> anyhow::Result<MotorType> {
        let mut enc: Option<EncoderType> = None;
        for Dependency(_, dep) in deps {
            match dep {
                Resource::Encoder(found_enc) => {
                    enc = Some(found_enc.clone());
                    break;
                }
                _ => {
                    continue;
                }
            };
        }
        Ok(Arc::new(Mutex::new(Self::new(enc))))
    }
}

impl Motor for FakeMotorWithDependency {
    fn get_position(&mut self) -> anyhow::Result<i32> {
        match &self.encoder {
            Some(enc) => Ok(enc.get_position(EncoderPositionType::DEGREES)?.value as i32),
            None => Ok(0),
        }
    }
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        debug!("setting power to {}", pct);
        self.power = pct;
        Ok(())
    }
    fn go_for(&mut self, _: f64, _: f64) -> anyhow::Result<Option<Duration>> {
        anyhow::bail!("go_for unimplemented")
    }
    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: true,
        }
    }
}

impl Status for FakeMotorWithDependency {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let hm = HashMap::new();
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

impl Actuator for FakeMotorWithDependency {
    fn stop(&mut self) -> anyhow::Result<()> {
        self.power = 0.0;
        Ok(())
    }
    fn is_moving(&mut self) -> anyhow::Result<bool> {
        Ok(self.power > 0.0)
    }
}

#[derive(DoCommand, MovementSensorReadings)]
pub struct FakeMovementSensor {
    pos: GeoPosition,
    linear_acc: Vector3,
}

impl Default for FakeMovementSensor {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeMovementSensor {
    pub fn new() -> Self {
        FakeMovementSensor {
            pos: GeoPosition {
                lat: 27.33,
                lon: 29.45,
                alt: 4572.2,
            },
            linear_acc: Vector3 {
                x: 5.0,
                y: 2.0,
                z: 3.0,
            },
        }
    }
    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> anyhow::Result<MovementSensorType> {
        let mut fake_pos: GeoPosition = Default::default();
        if let Ok(fake_lat) = cfg.get_attribute::<f64>("fake_lat") {
            fake_pos.lat = fake_lat
        }
        if let Ok(fake_lon) = cfg.get_attribute::<f64>("fake_lon") {
            fake_pos.lon = fake_lon
        }
        if let Ok(fake_alt) = cfg.get_attribute::<f32>("fake_alt") {
            fake_pos.alt = fake_alt
        }

        let mut lin_acc: Vector3 = Default::default();
        if let Ok(x) = cfg.get_attribute::<f64>("lin_acc_x") {
            lin_acc.x = x
        }
        if let Ok(y) = cfg.get_attribute::<f64>("lin_acc_y") {
            lin_acc.y = y
        }
        if let Ok(z) = cfg.get_attribute::<f64>("lin_acc_z") {
            lin_acc.z = z
        }

        Ok(Arc::new(Mutex::new(FakeMovementSensor {
            pos: fake_pos,
            linear_acc: lin_acc,
        })))
    }
}

impl MovementSensor for FakeMovementSensor {
    fn get_position(&mut self) -> anyhow::Result<GeoPosition> {
        Ok(self.pos)
    }

    fn get_linear_acceleration(&mut self) -> anyhow::Result<Vector3> {
        Ok(self.linear_acc)
    }

    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: true,
            linear_acceleration_supported: true,
            linear_velocity_supported: false,
            angular_velocity_supported: false,
            compass_heading_supported: false,
        }
    }

    fn get_linear_velocity(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_linear_velocity")
    }

    fn get_angular_velocity(&mut self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_angular_velocity")
    }

    fn get_compass_heading(&mut self) -> anyhow::Result<f64> {
        anyhow::bail!("unimplemented: movement_sensor_get_compass_heading")
    }
}

impl Status for FakeMovementSensor {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

#[derive(DoCommand)]
pub struct FakeSensor {
    fake_reading: f64,
}

impl FakeSensor {
    pub fn new() -> Self {
        FakeSensor {
            fake_reading: 42.42,
        }
    }
    pub(crate) fn from_config(cfg: ConfigType, _: Vec<Dependency>) -> anyhow::Result<SensorType> {
        if let Ok(val) = cfg.get_attribute::<f64>("fake_value") {
            return Ok(Arc::new(Mutex::new(FakeSensor { fake_reading: val })));
        }
        Ok(Arc::new(Mutex::new(FakeSensor::new())))
    }
}

impl Default for FakeSensor {
    fn default() -> Self {
        Self::new()
    }
}

impl Sensor for FakeSensor {}

impl Readings for FakeSensor {
    fn get_generic_readings(&mut self) -> anyhow::Result<GenericReadingsResult> {
        Ok(self
            .get_readings()?
            .into_iter()
            .map(|v| (v.0, SensorResult::<f64> { value: v.1 }.into()))
            .collect())
    }
}

impl SensorT<f64> for FakeSensor {
    fn get_readings(&self) -> anyhow::Result<TypedReadingsResult<f64>> {
        let mut x = HashMap::new();
        x.insert("fake_sensor".to_string(), self.fake_reading);
        Ok(x)
    }
}

impl Status for FakeSensor {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

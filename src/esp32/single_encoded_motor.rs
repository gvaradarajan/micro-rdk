use super::single_encoder::SingleEncoderType;
use crate::common::encoder::{
    Encoder, EncoderPositionType, EncoderSupportedRepresentations, SingleEncoder,
};
use crate::common::motor::{Motor, MotorType};

use crate::common::status::Status;
use std::collections::BTreeMap;

pub struct SingleEncodedMotor {
    encoder: SingleEncoderType,
    motor: MotorType,
}

impl SingleEncodedMotor {
    pub fn new(motor: MotorType, encoder: SingleEncoderType) -> Self {
        Self { encoder, motor }
    }
}

impl Motor for SingleEncodedMotor {
    fn set_power(&mut self, power_pct: f64) -> anyhow::Result<()> {
        let dir = (power_pct != 0.0) && (power_pct > 0.0);
        self.motor.set_power(power_pct)?;
        self.encoder.set_direction(dir)
    }

    fn get_position(&mut self) -> anyhow::Result<i32> {
        let props = self.encoder.get_properties();
        let pos_type = match props {
            EncoderSupportedRepresentations {
                ticks_count_supported: true,
                ..
            } => EncoderPositionType::TICKS,
            EncoderSupportedRepresentations {
                angle_degrees_supported: true,
                ..
            } => EncoderPositionType::DEGREES,
            _ => {
                return Err(anyhow::anyhow!(
                    "encoder for this motor does not support any known position representations"
                ));
            }
        };
        let pos = self.encoder.get_position(pos_type)?;
        Ok(pos.value as i32)
    }
}

impl Status for SingleEncodedMotor {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let pos = self
            .encoder
            .get_position(EncoderPositionType::UNSPECIFIED)?
            .value as f64;
        bt.insert(
            "position".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

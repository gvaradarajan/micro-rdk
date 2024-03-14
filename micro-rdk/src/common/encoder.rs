use std::sync::Arc;
use std::sync::Mutex;

use crate::proto::component::encoder::v1::GetPositionResponse;
use crate::proto::component::encoder::v1::GetPropertiesResponse;
use crate::proto::component::encoder::v1::PositionType;

use super::generic::DoCommand;
use super::status::Status;

pub static COMPONENT_NAME: &str = "encoder";

pub struct EncoderSupportedRepresentations {
    pub ticks_count_supported: bool,
    pub angle_degrees_supported: bool,
}

impl From<EncoderSupportedRepresentations> for GetPropertiesResponse {
    fn from(repr_struct: EncoderSupportedRepresentations) -> Self {
        GetPropertiesResponse {
            ticks_count_supported: repr_struct.ticks_count_supported,
            angle_degrees_supported: repr_struct.angle_degrees_supported,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum EncoderPositionType {
    UNSPECIFIED,
    TICKS,
    DEGREES,
}

impl EncoderPositionType {
    pub fn wrap_value(self, value: f32) -> EncoderPosition {
        EncoderPosition {
            position_type: self,
            value,
        }
    }
}

impl From<EncoderPositionType> for PositionType {
    fn from(pt: EncoderPositionType) -> Self {
        match pt {
            EncoderPositionType::UNSPECIFIED => PositionType::Unspecified,
            EncoderPositionType::TICKS => PositionType::TicksCount,
            EncoderPositionType::DEGREES => PositionType::AngleDegrees,
        }
    }
}

impl From<PositionType> for EncoderPositionType {
    fn from(mpt: PositionType) -> Self {
        match mpt {
            PositionType::Unspecified => EncoderPositionType::UNSPECIFIED,
            PositionType::AngleDegrees => EncoderPositionType::DEGREES,
            PositionType::TicksCount => EncoderPositionType::TICKS,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct EncoderPosition {
    pub position_type: EncoderPositionType,
    pub value: f32,
}

impl From<EncoderPosition> for GetPositionResponse {
    fn from(pos: EncoderPosition) -> Self {
        GetPositionResponse {
            value: pos.value,
            position_type: PositionType::from(pos.position_type).into(),
        }
    }
}

pub trait Encoder: Status + DoCommand {
    fn get_properties(&mut self) -> EncoderSupportedRepresentations;
    fn get_position(&self, position_type: EncoderPositionType) -> anyhow::Result<EncoderPosition>;
    fn reset_position(&mut self) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: encoder_reset_position")
    }
}

#[derive(Clone, Copy)]
pub enum Direction {
    Forwards,
    Backwards,
    StoppedForwards,
    StoppedBackwards,
}

impl Direction {
    pub fn is_forwards(&self) -> bool {
        matches!(self, Self::Forwards) || matches!(self, Self::StoppedForwards)
    }
}

pub trait SingleEncoder: Encoder {
    fn set_direction(&mut self, dir: Direction) -> anyhow::Result<()>;
    fn get_direction(&self) -> anyhow::Result<Direction>;
}

pub(crate) type EncoderType = Arc<Mutex<dyn Encoder>>;

impl<A> Encoder for Mutex<A>
where
    A: ?Sized + Encoder,
{
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        self.get_mut().unwrap().get_properties()
    }
    fn reset_position(&mut self) -> anyhow::Result<()> {
        self.get_mut().unwrap().reset_position()
    }
    fn get_position(&self, position_type: EncoderPositionType) -> anyhow::Result<EncoderPosition> {
        self.lock().unwrap().get_position(position_type)
    }
}

impl<A> Encoder for Arc<Mutex<A>>
where
    A: ?Sized + Encoder,
{
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        self.lock().unwrap().get_properties()
    }
    fn reset_position(&mut self) -> anyhow::Result<()> {
        self.lock().unwrap().reset_position()
    }
    fn get_position(&self, position_type: EncoderPositionType) -> anyhow::Result<EncoderPosition> {
        self.lock().unwrap().get_position(position_type)
    }
}

impl<A> SingleEncoder for Mutex<A>
where
    A: ?Sized + SingleEncoder,
{
    fn set_direction(&mut self, dir: Direction) -> anyhow::Result<()> {
        self.get_mut().unwrap().set_direction(dir)
    }

    fn get_direction(&self) -> anyhow::Result<Direction> {
        self.lock().unwrap().get_direction()
    }
}

impl<A> SingleEncoder for Arc<Mutex<A>>
where
    A: ?Sized + SingleEncoder,
{
    fn set_direction(&mut self, dir: Direction) -> anyhow::Result<()> {
        self.lock().unwrap().set_direction(dir)
    }

    fn get_direction(&self) -> anyhow::Result<Direction> {
        self.lock().unwrap().get_direction()
    }
}

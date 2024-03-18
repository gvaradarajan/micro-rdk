//! Abstraction of a general-purpose compute board

#![allow(dead_code, unused_imports)]
use crate::{
    common::status::Status,
    google,
    proto::{common, component},
};
#[cfg(feature = "analog")]
use core::cell::RefCell;

#[cfg(feature = "analog")]
use std::{collections::HashMap, rc::Rc};

use log::*;
use std::{sync::Arc, sync::Mutex, time::Duration};

#[cfg(feature = "analog")]
use super::analog::{AnalogError, FakeAnalogReader, AnalogReader};

#[cfg(feature = "i2c")]
use super::i2c::{FakeI2CHandle, FakeI2cConfig, I2CErrors, I2CHandle, I2cHandleType};

use super::{
    config::ConfigType,
    generic::DoCommand,
    registry::ComponentRegistry,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BoardError {
    #[cfg(feature = "gpio")]
    #[error("pin {0} error: {1}")]
    GpioPinError(u32, &'static str),
    #[cfg(feature = "gpio")]
    #[error("pin {0} error: {1}")]
    GpioPinOtherError(u32, Box<dyn std::error::Error + Send + Sync>),
    #[cfg(feature = "analog")]
    #[error("analog reader {0} not found")]
    AnalogReaderNotFound(String),
    #[error("board unsupported argument {0} ")]
    BoardUnsupportedArgument(&'static str),
    #[cfg(feature = "i2c")]
    #[error("i2c bus {0} not found")]
    I2CBusNotFound(String),
    #[error(transparent)]
    OtherBoardError(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("method: {0} not supported")]
    BoardMethodNotSupported(&'static str),
    #[cfg(feature = "i2c")]
    #[error(transparent)]
    BoardI2CError(#[from] I2CErrors),
}

pub static COMPONENT_NAME: &str = "board";

#[cfg(feature = "analog")]
pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_board("fake", &FakeBoard::from_config)
        .is_err()
    {
        log::error!("model fake is already registered")
    }
}

/// Represents the functionality of a general purpose compute board that contains various components such as analog readers and digital interrupts.
pub trait Board: Status + DoCommand {
    #[cfg(feature = "gpio")]
    /// Set a pin to high or low
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> Result<(), BoardError>;

    /// Return the current [BoardStatus](common::v1::BoardStatus) of the board
    fn get_board_status(&self) -> Result<common::v1::BoardStatus, BoardError>;

    #[cfg(feature = "gpio")]
    /// Get the state of a pin, high(`true`) or low(`false`)
    fn get_gpio_level(&self, pin: i32) -> Result<bool, BoardError>;

    #[cfg(feature = "analog")]
    /// Get an [AnalogReader] by name
    fn get_analog_reader_by_name(
        &self,
        name: String,
    ) -> Result<Rc<RefCell<dyn AnalogReader<u16, Error = AnalogError>>>, BoardError>;

    /// Set the board to the indicated [PowerMode](component::board::v1::PowerMode)
    fn set_power_mode(
        &self,
        mode: component::board::v1::PowerMode,
        duration: Option<Duration>,
    ) -> Result<(), BoardError>;

    #[cfg(feature = "i2c")]
    /// Get a wrapped [I2CHandle] by name.
    fn get_i2c_by_name(&self, name: String) -> Result<I2cHandleType, BoardError>;

    #[cfg(feature = "gpio")]
    /// Return the amount of detected interrupt events on a pin. Should error if the
    /// pin has not been configured as an interrupt
    fn get_digital_interrupt_value(&self, _pin: i32) -> Result<u32, BoardError> {
        Err(BoardError::BoardMethodNotSupported(
            "get_digital_interupt_value",
        ))
    }

    #[cfg(feature = "gpio")]
    /// Get the pin's given duty cycle, returns percentage as float between 0.0 and 1.0
    fn get_pwm_duty(&self, pin: i32) -> f64;

    #[cfg(feature = "gpio")]
    /// Set the pin to the given duty cycle , `duty_cycle_pct` is a float between 0.0 and 1.0.
    fn set_pwm_duty(&mut self, pin: i32, duty_cycle_pct: f64) -> Result<(), BoardError>;

    #[cfg(feature = "gpio")]
    /// Get the PWM frequency of the pin
    fn get_pwm_frequency(&self, pin: i32) -> Result<u64, BoardError>;

    #[cfg(feature = "gpio")]
    /// Set the pin to the given PWM frequency (in Hz).
    /// When frequency is 0, the board will unregister the pin and PWM channel from
    /// the timer and removes the PWM signal.
    fn set_pwm_frequency(&mut self, pin: i32, frequency_hz: u64) -> Result<(), BoardError>;
}

/// An alias for a thread-safe handle to a struct that implements the [Board] trait
pub type BoardType = Arc<Mutex<dyn Board>>;

#[cfg(all(feature = "analog", feature = "i2c"))]
#[doc(hidden)]
/// A test implementation of a generic compute board
#[derive(DoCommand)]
pub struct FakeBoard {
    #[cfg(all(feature = "analog", feature = "i2c"))]
    analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = AnalogError>>>>,
    i2cs: HashMap<String, Arc<Mutex<FakeI2CHandle>>>,
    pin_pwms: HashMap<i32, f64>,
    pin_pwm_freq: HashMap<i32, u64>,
}

#[cfg(all(feature = "analog", feature = "i2c"))]
impl FakeBoard {
    pub fn new(analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = AnalogError>>>>) -> Self {
        let mut i2cs: HashMap<String, Arc<Mutex<FakeI2CHandle>>> = HashMap::new();
        let i2c0 = Arc::new(Mutex::new(FakeI2CHandle::new("i2c0".to_string())));
        i2cs.insert(i2c0.name(), i2c0);
        let i2c1 = Arc::new(Mutex::new(FakeI2CHandle::new("i2c1".to_string())));
        i2cs.insert(i2c1.name(), i2c1);
        FakeBoard {
            analogs,
            i2cs,
            pin_pwms: HashMap::new(),
            pin_pwm_freq: HashMap::new(),
        }
    }

    pub(crate) fn from_config(cfg: ConfigType) -> Result<BoardType, BoardError> {
        let analogs = if let Ok(analog_confs) = cfg.get_attribute::<HashMap<&str, f64>>("analogs") {
            analog_confs
                .iter()
                .map(|(k, v)| {
                    let a: Rc<RefCell<dyn AnalogReader<u16, Error = AnalogError>>> = Rc::new(
                        RefCell::new(FakeAnalogReader::new(k.to_string(), *v as u16)),
                    );
                    a
                })
                .collect()
        } else {
            vec![]
        };

        let i2cs = if let Ok(i2c_confs) = cfg.get_attribute::<Vec<FakeI2cConfig>>("i2cs") {
            let name_to_i2c = i2c_confs.iter().map(|v| {
                let name = v.name.to_string();
                let value: [u8; 3] = [v.value_1, v.value_2, v.value_3];
                (
                    name.to_string(),
                    Arc::new(Mutex::new(FakeI2CHandle::new_with_value(name, value))),
                )
            });
            HashMap::from_iter(name_to_i2c)
        } else {
            HashMap::new()
        };

        Ok(Arc::new(Mutex::new(FakeBoard {
            analogs,
            i2cs,
            pin_pwms: HashMap::new(),
            pin_pwm_freq: HashMap::new(),
        })))
    }
}

#[cfg(all(feature = "analog", feature = "i2c"))]
impl Board for FakeBoard {
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> Result<(), BoardError> {
        info!("set pin {} to {}", pin, is_high);
        Ok(())
    }

    fn get_board_status(&self) -> Result<common::v1::BoardStatus, BoardError> {
        let mut b = common::v1::BoardStatus {
            analogs: HashMap::new(),
            digital_interrupts: HashMap::new(),
        };
        self.analogs.iter().for_each(|a| {
            let mut borrowed = a.borrow_mut();
            b.analogs.insert(
                borrowed.name(),
                common::v1::AnalogStatus {
                    value: borrowed.read().unwrap_or(0).into(),
                },
            );
        });
        Ok(b)
    }

    fn get_gpio_level(&self, pin: i32) -> Result<bool, BoardError> {
        info!("get pin {}", pin);
        Ok(true)
    }

    fn get_analog_reader_by_name(
        &self,
        name: String,
    ) -> Result<Rc<RefCell<dyn AnalogReader<u16, Error = AnalogError>>>, BoardError> {
        match self.analogs.iter().find(|a| a.borrow().name() == name) {
            Some(reader) => Ok(reader.clone()),
            None => Err(BoardError::AnalogReaderNotFound(name)),
        }
    }

    fn set_power_mode(
        &self,
        mode: component::board::v1::PowerMode,
        duration: Option<Duration>,
    ) -> Result<(), BoardError> {
        info!(
            "set power mode to {} for {} milliseconds",
            mode.as_str_name(),
            match duration {
                Some(dur) => dur.as_millis().to_string(),
                None => "<forever>".to_string(),
            }
        );
        Ok(())
    }

    fn get_i2c_by_name(&self, name: String) -> Result<I2cHandleType, BoardError> {
        if let Some(i2c_handle) = self.i2cs.get(&name) {
            return Ok((*i2c_handle).clone());
        }
        Err(BoardError::I2CBusNotFound(name))
    }

    fn get_pwm_duty(&self, pin: i32) -> f64 {
        *self.pin_pwms.get(&pin).unwrap_or(&0.0)
    }

    fn set_pwm_duty(&mut self, pin: i32, duty_cycle_pct: f64) -> Result<(), BoardError> {
        self.pin_pwms.insert(pin, duty_cycle_pct);
        Ok(())
    }

    fn get_pwm_frequency(&self, pin: i32) -> Result<u64, BoardError> {
        Ok(*self.pin_pwm_freq.get(&pin).unwrap_or(&0))
    }

    fn set_pwm_frequency(&mut self, pin: i32, frequency_hz: u64) -> Result<(), BoardError> {
        self.pin_pwm_freq.insert(pin, frequency_hz);
        Ok(())
    }
}

#[cfg(all(feature = "analog", feature = "i2c"))]
impl Status for FakeBoard {
    fn get_status(&self) -> anyhow::Result<Option<google::protobuf::Struct>> {
        let mut hm = HashMap::new();
        let mut analogs = HashMap::new();
        self.analogs.iter().for_each(|a| {
            let mut borrowed = a.borrow_mut();
            analogs.insert(
                borrowed.name(),
                google::protobuf::Value {
                    kind: Some(google::protobuf::value::Kind::StructValue(
                        google::protobuf::Struct {
                            fields: HashMap::from([(
                                "value".to_string(),
                                google::protobuf::Value {
                                    kind: Some(google::protobuf::value::Kind::NumberValue(
                                        borrowed.read().unwrap_or(0).into(),
                                    )),
                                },
                            )]),
                        },
                    )),
                },
            );
        });
        if !analogs.is_empty() {
            hm.insert(
                "analogs".to_string(),
                google::protobuf::Value {
                    kind: Some(google::protobuf::value::Kind::StructValue(
                        google::protobuf::Struct { fields: analogs },
                    )),
                },
            );
        }
        Ok(Some(google::protobuf::Struct { fields: hm }))
    }
}

impl<A> Board for Arc<Mutex<A>>
where
    A: ?Sized + Board,
{
    fn get_board_status(&self) -> Result<common::v1::BoardStatus, BoardError> {
        self.lock().unwrap().get_board_status()
    }

    #[cfg(feature = "gpio")]
    fn get_gpio_level(&self, pin: i32) -> Result<bool, BoardError> {
        self.lock().unwrap().get_gpio_level(pin)
    }

    #[cfg(feature = "gpio")]
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> Result<(), BoardError> {
        self.lock().unwrap().set_gpio_pin_level(pin, is_high)
    }

    #[cfg(feature = "analog")]
    fn get_analog_reader_by_name(
        &self,
        name: String,
    ) -> Result<Rc<RefCell<dyn AnalogReader<u16, Error = AnalogError>>>, BoardError> {
        self.lock().unwrap().get_analog_reader_by_name(name)
    }

    fn set_power_mode(
        &self,
        mode: component::board::v1::PowerMode,
        duration: Option<Duration>,
    ) -> Result<(), BoardError> {
        self.lock().unwrap().set_power_mode(mode, duration)
    }

    #[cfg(feature = "i2c")]
    fn get_i2c_by_name(&self, name: String) -> Result<I2cHandleType, BoardError> {
        self.lock().unwrap().get_i2c_by_name(name)
    }

    #[cfg(feature = "gpio")]
    fn get_digital_interrupt_value(&self, pin: i32) -> Result<u32, BoardError> {
        self.lock().unwrap().get_digital_interrupt_value(pin)
    }

    #[cfg(feature = "gpio")]
    fn get_pwm_duty(&self, pin: i32) -> f64 {
        self.lock().unwrap().get_pwm_duty(pin)
    }

    #[cfg(feature = "gpio")]
    fn set_pwm_duty(&mut self, pin: i32, duty_cycle_pct: f64) -> Result<(), BoardError> {
        self.lock().unwrap().set_pwm_duty(pin, duty_cycle_pct)
    }

    #[cfg(feature = "gpio")]
    fn get_pwm_frequency(&self, pin: i32) -> Result<u64, BoardError> {
        self.lock().unwrap().get_pwm_frequency(pin)
    }

    #[cfg(feature = "gpio")]
    fn set_pwm_frequency(&mut self, pin: i32, frequency_hz: u64) -> Result<(), BoardError> {
        self.lock().unwrap().set_pwm_frequency(pin, frequency_hz)
    }
}

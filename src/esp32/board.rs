#![allow(dead_code)]
use crate::common::analog::AnalogReader;
use crate::common::analog::AnalogReaderConfig;
use crate::common::board::Board;
use crate::common::board::BoardType;
use crate::common::config::ConfigType;
use crate::common::digital_interrupt::{InterruptEventType, InterruptEvent, PinEventTransmitter, DigitalInterruptConfig};
use crate::common::i2c::I2cHandleType;
use crate::common::registry::ComponentRegistry;
use crate::common::status::Status;
use crate::proto::common;
use crate::proto::component;
use anyhow::Context;
use core::cell::RefCell;
use esp_idf_hal::adc::config::Config;
use esp_idf_hal::adc::AdcChannelDriver;
use esp_idf_hal::adc::AdcDriver;
use esp_idf_hal::adc::Atten11dB;
use esp_idf_hal::adc::ADC1;
use esp_idf_hal::gpio::{AnyIOPin, InputOutput, PinDriver};
use esp_idf_sys::{esp, gpio_config, gpio_config_t, gpio_mode_t_GPIO_MODE_INPUT, 
    gpio_int_type_t, 
    gpio_int_type_t_GPIO_INTR_DISABLE,
    gpio_int_type_t_GPIO_INTR_POSEDGE,
    gpio_int_type_t_GPIO_INTR_NEGEDGE,
    gpio_int_type_t_GPIO_INTR_ANYEDGE,
    gpio_int_type_t_GPIO_INTR_LOW_LEVEL,
    gpio_int_type_t_GPIO_INTR_HIGH_LEVEL, 
    gpio_install_isr_service, 
    gpio_isr_handler_add,
    ESP_INTR_FLAG_IRAM};
use log::*;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use super::analog::Esp32AnalogReader;
use super::i2c::{Esp32I2C, Esp32I2cConfig};

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_board("esp32", &EspBoard::from_config)
        .is_err()
    {
        log::error!("esp32 board type already registered");
    }
}

lazy_static::lazy_static! {
    static ref GPIO_ISR_SERVICE_INSTALLED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

fn install_gpio_isr_service() -> anyhow::Result<()> {
    if !GPIO_ISR_SERVICE_INSTALLED.fetch_or(true, Ordering::SeqCst) {
        unsafe {
            esp!(gpio_install_isr_service(ESP_INTR_FLAG_IRAM as i32))?;
        }
    }
    Ok(())
}

impl TryFrom<gpio_int_type_t> for InterruptEventType {
    type Error = anyhow::Error;
    fn try_from(value: gpio_int_type_t) -> Result<Self, Self::Error> {
        #![allow(non_upper_case_globals)]
        Ok(match value {
            gpio_int_type_t_GPIO_INTR_POSEDGE => InterruptEventType::PosEDGE,
            gpio_int_type_t_GPIO_INTR_NEGEDGE => InterruptEventType::NegEDGE,
            gpio_int_type_t_GPIO_INTR_HIGH_LEVEL => InterruptEventType::HIGH,
            gpio_int_type_t_GPIO_INTR_LOW_LEVEL => InterruptEventType::LOW,
            gpio_int_type_t_GPIO_INTR_ANYEDGE => InterruptEventType::AnyEDGE,
            _ => {
                anyhow::bail!("invalid esp32 interrupt event type {:?} encountered", value)
            }
        }) 
    }
}

impl From<InterruptEventType> for gpio_int_type_t {
    fn from(value: InterruptEventType) -> Self {
        match value {
            InterruptEventType::PosEDGE => gpio_int_type_t_GPIO_INTR_POSEDGE,
            InterruptEventType::NegEDGE => gpio_int_type_t_GPIO_INTR_NEGEDGE,
            InterruptEventType::AnyEDGE => gpio_int_type_t_GPIO_INTR_ANYEDGE,
            InterruptEventType::HIGH => gpio_int_type_t_GPIO_INTR_HIGH_LEVEL,
            InterruptEventType::LOW => gpio_int_type_t_GPIO_INTR_LOW_LEVEL,
        }
    }
}

pub struct GPIOPin {
    pin: i32,
    driver: PinDriver<'static, AnyIOPin, InputOutput>,
    config: gpio_config_t,
    transmitter: Option<Box<PinEventTransmitter>>,
    interrupt_type: Option<InterruptEventType>
}

impl GPIOPin {
    pub fn new(pin: i32, config: Option<gpio_config_t>, pull_up: Option<bool>) -> anyhow::Result<Self> {
        let pull_up_en = pull_up.unwrap_or_default();
        let config = config;
        let config = match config {
            Some(cfg) => {
                let mut cfg = cfg;
                cfg.pin_bit_mask = 1 << pin;
                cfg
            },
            None => {
                gpio_config_t {
                    pin_bit_mask: 1 << pin,
                    mode: gpio_mode_t_GPIO_MODE_INPUT,
                    pull_up_en: pull_up_en.into(),
                    pull_down_en: (!pull_up_en).into(),
                    intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
                }
            }
        };
        unsafe {
            esp!(gpio_config(&config))?;
        }
        let driver = PinDriver::input_output(unsafe { AnyIOPin::new(pin) })?;
        Ok(Self { pin, driver, config, transmitter: None, interrupt_type: None })
    }

    pub fn pin(&self) -> i32 {
        self.pin
    }

    pub fn is_high(&self) -> bool {
        self.driver.is_high()
    }

    pub fn set_high(&mut self) -> anyhow::Result<()> {
        self.driver.set_high().map_err(|e| anyhow::anyhow!("couldn't set pin {} high {}", self.pin, e))
    }

    pub fn set_low(&mut self) -> anyhow::Result<()> {
        self.driver.set_low().map_err(|e| anyhow::anyhow!("couldn't set pin {} low {}", self.pin, e))
    }

    pub fn is_interrupt(&self) -> bool {
        self.interrupt_type.is_some()
    }

    pub fn setup_interrupt(&mut self, intr_type: InterruptEventType) -> anyhow::Result<()> {
        match &self.interrupt_type {
            Some(existing_type) => {
                if *existing_type == intr_type {
                    return Ok(())
                }
            }
            None => {}
        };
        install_gpio_isr_service()?;
        self.config.intr_type = intr_type.into();
        
        let initially_high = self.driver.is_high();
        self.transmitter = Some(Box::new(PinEventTransmitter::new(intr_type, initially_high)));
        unsafe {
            esp!(gpio_config(&self.config))?;
            esp!(gpio_isr_handler_add(
                self.pin,
                Some(Self::interrupt),
                self.transmitter.as_mut().unwrap().as_mut() as *mut PinEventTransmitter as *mut _
            ))?;
        }
        Ok(())
    }

    pub fn get_interrupt_channel(&mut self) -> anyhow::Result<Receiver<InterruptEvent>> {
        Ok(self.transmitter.as_mut().ok_or_else(|| anyhow::Error::msg(
            format!("interrupt not set up for GPIO pin {:?}", self.pin)
        ))?.subscribe())
    }

    pub fn get_event_count(&self) -> anyhow::Result<i64> {
        Ok(self.transmitter.as_ref().ok_or_else(|| anyhow::Error::msg(
            format!("interrupt not set up for GPIO pin {:?}", self.pin)
        ))?.get_event_count())
    }

    #[inline(always)]
    #[link_section = ".iram0.text"]
    unsafe extern "C" fn interrupt(arg: *mut core::ffi::c_void) {
        let arg: &mut PinEventTransmitter = &mut *(arg as *mut _);
        match arg.emit_event() {
            Ok(_) => {},
            Err(err) => {
                log::error!("failed to send interrupt event: {:?}", err)
            }
        }
    }
}

pub struct EspBoard {
    // pins: Vec<PinDriver<'static, AnyIOPin, InputOutput>>,
    pins: Vec<GPIOPin>,
    analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>,
    i2cs: HashMap<String, I2cHandleType>,
}

impl EspBoard {
    pub fn new(
        // pins: Vec<PinDriver<'static, AnyIOPin, InputOutput>>,
        pins: Vec<GPIOPin>,
        analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>>,
        i2cs: HashMap<String, I2cHandleType>,
    ) -> Self {
        EspBoard {
            pins,
            analogs,
            i2cs,
        }
    }
    /// This is a temporary approach aimed at ensuring a good POC for runtime config consumption by the ESP32,
    /// Down the road we will need to wrap the Esp32Board in a singleton instance owning the peripherals and giving them as requested.
    /// The potential approach is described in esp32/motor.rs:383
    pub(crate) fn from_config(cfg: ConfigType) -> anyhow::Result<BoardType> {
        let (analogs, mut pins, i2c_confs) = {
            let analogs = if let Ok(analogs) =
                cfg.get_attribute::<Vec<AnalogReaderConfig>>("analogs")
            {
                let analogs: Vec<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>> =
                    analogs
                        .iter()
                        .filter_map(|v| {
                            let adc1 = Rc::new(RefCell::new(
                                AdcDriver::new(
                                    unsafe { ADC1::new() },
                                    &Config::new().calibration(true),
                                )
                                .ok()?,
                            ));
                            let chan: Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>> =
                                match v.pin {
                                    32 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio32::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    33 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio33::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    34 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio34::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    35 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio35::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    36 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio36::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    37 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio37::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    38 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio38::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    39 => {
                                        let p: Rc<
                                            RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>,
                                        > = Rc::new(RefCell::new(Esp32AnalogReader::new(
                                            v.name.to_string(),
                                            AdcChannelDriver::<_, Atten11dB<ADC1>>::new(unsafe {
                                                esp_idf_hal::gpio::Gpio39::new()
                                            })
                                            .ok()?,
                                            adc1,
                                        )));
                                        Some(p)
                                    }
                                    _ => {
                                        log::error!("pin {} is not an ADC1 pin", v.pin);
                                        None
                                    }
                                }?;

                            Some(chan)
                        })
                        .collect();
                analogs
            } else {
                vec![]
            };
            let pins = if let Ok(pins) = cfg.get_attribute::<Vec<i32>>("pins") {
                // pins.iter()
                //     .filter_map(|pin| {
                //         let p = PinDriver::input_output(unsafe { AnyIOPin::new(*pin) });
                //         if let Ok(p) = p {
                //             Some(p)
                //         } else {
                //             None
                //         }
                //     })
                //     .collect()
                pins.iter()
                    .filter_map(|pin| {
                        let p = GPIOPin::new(*pin, None, None);
                        if let Ok(p) = p {
                            Some(p)
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                vec![]
            };

            let i2c_confs = if let Ok(i2c_confs) = cfg.get_attribute::<Vec<Esp32I2cConfig>>("i2cs")
            {
                i2c_confs
            } else {
                vec![]
            };
            (analogs, pins, i2c_confs)
        };
        let mut i2cs = HashMap::new();
        for conf in i2c_confs.iter() {
            let name = conf.name.to_string();
            let i2c = Esp32I2C::new_from_config(*conf)?;
            let i2c_wrapped: I2cHandleType = Arc::new(Mutex::new(i2c));
            i2cs.insert(name.to_string(), i2c_wrapped);
        }
        if let Ok(interrupt_confs) = cfg.get_attribute::<Vec<DigitalInterruptConfig>>("digital_interrupts") {
            for conf in interrupt_confs {
                let p = pins.iter_mut().find(|p| p.pin() == conf.pin);
                if let Some(p) = p {
                    // TODO: make configurable
                    p.setup_interrupt(InterruptEventType::PosEDGE)?
                } else {
                    let mut p = GPIOPin::new(conf.pin, None, None)?;
                    p.setup_interrupt(InterruptEventType::PosEDGE)?;
                    pins.push(p);
                }
            }
        }
        Ok(Arc::new(Mutex::new(Self {
            pins,
            analogs,
            i2cs,
        })))
    }
}

impl Board for EspBoard {
    fn set_gpio_pin_level(&mut self, pin: i32, is_high: bool) -> anyhow::Result<()> {
        let p = self.pins.iter_mut().find(|p| p.pin() == pin);
        if let Some(p) = p {
            if p.is_interrupt() {
                anyhow::bail!("cannot set level for pin {:?}, it is registered as an interrupt", pin)
            }
            if is_high {
                return p.set_high();
            } else {
                return p.set_low();
            }
        }
        Err(anyhow::anyhow!("pin {} is not set as an output pin", pin))
    }
    fn get_gpio_level(&self, pin: i32) -> anyhow::Result<bool> {
        let pin = self
            .pins
            .iter()
            .find(|p| p.pin() == pin)
            .context(format!("pin {pin} not registered on board"))?;
        Ok(pin.is_high())
    }
    fn get_board_status(&self) -> anyhow::Result<common::v1::BoardStatus> {
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
    fn get_analog_reader_by_name(
        &self,
        name: String,
    ) -> anyhow::Result<Rc<RefCell<dyn AnalogReader<u16, Error = anyhow::Error>>>> {
        match self.analogs.iter().find(|a| a.borrow().name() == name) {
            Some(reader) => Ok(reader.clone()),
            None => Err(anyhow::anyhow!("couldn't find analog reader {}", name)),
        }
    }
    fn set_power_mode(
        &self,
        mode: component::board::v1::PowerMode,
        duration: Option<Duration>,
    ) -> anyhow::Result<()> {
        info!(
            "Esp32 received request to set power mode to {} for {} milliseconds",
            mode.as_str_name(),
            match duration {
                Some(dur) => dur.as_millis().to_string(),
                None => "<forever>".to_string(),
            }
        );

        anyhow::ensure!(
            mode == component::board::v1::PowerMode::OfflineDeep,
            "unimplemented: EspBoard::set_power_mode: modes other than 'OfflineDeep' are not currently supported"
        );

        if let Some(dur) = duration {
            let dur_micros = dur.as_micros() as u64;
            let result: esp_idf_sys::esp_err_t;
            unsafe {
                result = esp_idf_sys::esp_sleep_enable_timer_wakeup(dur_micros);
            }
            anyhow::ensure!(
                result == esp_idf_sys::ESP_OK,
                "unimplemented: EspBoard::set_power_mode: sleep duration {:?} rejected as unsupportedly long", dur
            );
            warn!("Esp32 entering deep sleep for {} microseconds!", dur_micros);
        } else {
            warn!("Esp32 entering deep sleep without scheduled wakeup!");
        }

        unsafe {
            esp_idf_sys::esp_deep_sleep_start();
        }
    }
    fn get_i2c_by_name(&self, name: String) -> anyhow::Result<I2cHandleType> {
        match self.i2cs.get(&name) {
            Some(i2c_handle) => Ok(Arc::clone(i2c_handle)),
            None => Err(anyhow::anyhow!("no i2c found with name {}", name)),
        }
    }
    fn subscribe_to_pin(&mut self, pin: i32) -> anyhow::Result<Receiver<InterruptEvent>> {
        let p = self.pins.iter_mut().find(|p| p.pin() == pin);
        if let Some(p) = p {
            p.setup_interrupt(InterruptEventType::PosEDGE)?;
            return p.get_interrupt_channel()
        }
        Err(anyhow::anyhow!("pin {} is not configured on the board instance", pin))
    }
    fn get_digital_interrupt_value(&self, pin: i32) -> anyhow::Result<i64> {
        let p = self.pins.iter().find(|p| p.pin() == pin);
        if let Some(p) = p {
            if !p.is_interrupt() {
                return Err(anyhow::anyhow!("pin {} is not configured as an interrupt", pin))
            }
            return p.get_event_count()
        }
        Err(anyhow::anyhow!("pin {} is not configured on the board instance", pin))
    }
}

impl Status for EspBoard {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let mut analogs = BTreeMap::new();
        self.analogs.iter().for_each(|a| {
            let mut borrowed = a.borrow_mut();
            analogs.insert(
                borrowed.name(),
                prost_types::Value {
                    kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                        fields: BTreeMap::from([(
                            "value".to_string(),
                            prost_types::Value {
                                kind: Some(prost_types::value::Kind::NumberValue(
                                    borrowed.read().unwrap_or(0).into(),
                                )),
                            },
                        )]),
                    })),
                },
            );
        });
        if !analogs.is_empty() {
            bt.insert(
                "analogs".to_string(),
                prost_types::Value {
                    kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                        fields: analogs,
                    })),
                },
            );
        }
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

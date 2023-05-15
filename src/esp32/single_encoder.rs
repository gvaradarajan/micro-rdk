use embedded_hal::digital::v2::InputPin;

use super::encoder::PulseStorage;
use super::pin::PinExt;
use super::pulse_counter::{get_unit, isr_install, isr_uninstall, isr_installed};

use crate::common::encoder::{
    Encoder, EncoderPosition, EncoderPositionType, EncoderSupportedRepresentations, SingleEncoder
};

use core::ffi::{c_short, c_ulong};
use esp_idf_sys as espsys;
use espsys::pcnt_channel_edge_action_t_PCNT_CHANNEL_EDGE_ACTION_DECREASE as pcnt_count_dec;
use espsys::pcnt_channel_edge_action_t_PCNT_CHANNEL_EDGE_ACTION_INCREASE as pcnt_count_inc;
use espsys::pcnt_channel_level_action_t_PCNT_CHANNEL_LEVEL_ACTION_KEEP as pcnt_mode_keep;
use espsys::pcnt_channel_t_PCNT_CHANNEL_0 as pcnt_channel_0;
use espsys::pcnt_config_t;
use espsys::pcnt_evt_type_t_PCNT_EVT_H_LIM as pcnt_evt_h_lim;
use espsys::pcnt_evt_type_t_PCNT_EVT_L_LIM as pcnt_evt_l_lim;
use espsys::{esp, EspError, ESP_OK};

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::common::status::Status;

// TODO: Move this type to common once we have a single encoder 
// implementation for another board
pub(crate) type SingleEncoderType = Arc<Mutex<dyn SingleEncoder>>;

pub struct Esp32SingleEncoder {
    pulse_counter: Box<PulseStorage>,
    config: pcnt_config_t,
    forwards: bool,
}

impl Esp32SingleEncoder {
    pub fn new(encoder_pin: impl InputPin + PinExt) -> anyhow::Result<Self> {
        let unit = get_unit()?;
        println!("unit received: {:?}", unit);
        let pcnt = Box::new(PulseStorage {
            acc: Arc::new(AtomicI32::new(0)),
            unit,
        });
        let mut enc = Esp32SingleEncoder {
            pulse_counter: pcnt,
            config: pcnt_config_t {
                pulse_gpio_num: encoder_pin.pin(),
                ctrl_gpio_num: -1,
                pos_mode: pcnt_count_inc,
                neg_mode: pcnt_count_inc,
                lctrl_mode: pcnt_mode_keep,
                hctrl_mode: pcnt_mode_keep,
                counter_h_lim: 100,
                counter_l_lim: -100,
                channel: pcnt_channel_0,
                unit,
            },
            forwards: true,
        };
        enc.setup_pcnt()?;
        enc.start()?;
        Ok(enc)
    }

    pub fn start(&self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_counter_resume(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        Ok(())
    }
    pub fn stop(&self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_counter_pause(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        Ok(())
    }
    pub fn reset(&self) -> anyhow::Result<()> {
        self.stop()?;
        unsafe {
            match esp_idf_sys::pcnt_counter_clear(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        self.pulse_counter.acc.store(0, Ordering::Relaxed);
        self.start()?;
        Ok(())
    }
    pub fn get_counter_value(&self) -> anyhow::Result<i32> {
        let mut ctr: i16 = 0;
        unsafe {
            match esp_idf_sys::pcnt_get_counter_value(self.config.unit, &mut ctr as *mut c_short) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        let sign: i32 = match self.forwards {
            true => 1,
            false => -1,
        };
        let tot = self.pulse_counter.acc.load(Ordering::Relaxed) * 100 + (i32::from(ctr) * sign);
        Ok(tot)
    }
    pub fn setup_pcnt(&mut self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_unit_config(&self.config as *const pcnt_config_t) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }

        unsafe {
            match esp_idf_sys::pcnt_counter_pause(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
            match esp_idf_sys::pcnt_counter_clear(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }

        isr_install(self.config.unit as i32)?;

        esp!(unsafe {
            esp_idf_sys::pcnt_isr_handler_add(
                self.config.unit,
                Some(Self::irq_handler_increment),
                self.pulse_counter.as_mut() as *mut PulseStorage as *mut _,
            )
        })?;

        unsafe {
            match esp_idf_sys::pcnt_event_enable(self.config.unit, pcnt_evt_h_lim) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
            match esp_idf_sys::pcnt_event_enable(self.config.unit, pcnt_evt_l_lim) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }

        Ok(())
    }

    #[inline(always)]
    #[link_section = ".iram1.pcnt_srv"]
    unsafe extern "C" fn irq_handler_increment(arg: *mut core::ffi::c_void) {
        let arg: &mut PulseStorage = &mut *(arg as *mut _);
        let mut status = 0;
        esp_idf_sys::pcnt_get_event_status(arg.unit, &mut status as *mut c_ulong);
        if status & pcnt_evt_h_lim != 0 {
            arg.acc.fetch_add(1, Ordering::SeqCst);
        }
        if status & pcnt_evt_l_lim != 0 {
            arg.acc.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[inline(always)]
    #[link_section = ".iram1.pcnt_srv"]
    unsafe extern "C" fn irq_handler_decrement(arg: *mut core::ffi::c_void) {
        let arg: &mut PulseStorage = &mut *(arg as *mut _);
        let mut status = 0;
        esp_idf_sys::pcnt_get_event_status(arg.unit, &mut status as *mut c_ulong);
        if status & pcnt_evt_h_lim != 0 {
            arg.acc.fetch_sub(1, Ordering::SeqCst);
        }
        if status & pcnt_evt_l_lim != 0 {
            arg.acc.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Encoder for Esp32SingleEncoder {
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        EncoderSupportedRepresentations {
            ticks_count_supported: true,
            angle_degrees_supported: false,
        }
    }
    fn get_position(&self, position_type: EncoderPositionType) -> anyhow::Result<EncoderPosition> {
        match position_type {
            EncoderPositionType::TICKS | EncoderPositionType::UNSPECIFIED => {
                let count = self.get_counter_value()?;
                Ok(EncoderPositionType::TICKS.wrap_value(count as f32))
            }
            EncoderPositionType::DEGREES => {
                anyhow::bail!("Esp32SingleEncoder does not support returning angular position")
            }
        }
    }
    fn reset_position(&mut self) -> anyhow::Result<()> {
        self.reset()
    }
}

impl SingleEncoder for Esp32SingleEncoder {
    fn set_direction(&mut self, forwards: bool) -> anyhow::Result<()> {
        let mut reconfigure = false;
        if self.forwards && !forwards {
            self.config.neg_mode = pcnt_count_inc;
            self.config.pos_mode = pcnt_count_inc;
            reconfigure = true;
        } else if !self.forwards && forwards {
            self.config.neg_mode = pcnt_count_dec;
            self.config.pos_mode = pcnt_count_dec;
            reconfigure = true;
        }
        self.forwards = forwards;
        if reconfigure {
            unsafe {
                match esp_idf_sys::pcnt_counter_pause(self.config.unit) {
                    ESP_OK => {}
                    err => return Err(EspError::from(err).unwrap().into()),
                }

                match esp_idf_sys::pcnt_unit_config(&self.config as *const pcnt_config_t) {
                    ESP_OK => {}
                    err => return Err(EspError::from(err).unwrap().into()),
                }
            }
            esp!(unsafe {
                esp_idf_sys::pcnt_isr_handler_remove(self.config.unit)
            })?;
            esp!(unsafe {
                esp_idf_sys::pcnt_isr_handler_add(
                    self.config.unit,
                    Some(Self::irq_handler_decrement),
                    self.pulse_counter.as_mut() as *mut PulseStorage as *mut _,
                )
            })?;
            unsafe {
                match esp_idf_sys::pcnt_event_enable(self.config.unit, pcnt_evt_h_lim) {
                    ESP_OK => {}
                    err => return Err(EspError::from(err).unwrap().into()),
                }
                match esp_idf_sys::pcnt_event_enable(self.config.unit, pcnt_evt_l_lim) {
                    ESP_OK => {}
                    err => return Err(EspError::from(err).unwrap().into()),
                }
                match esp_idf_sys::pcnt_counter_resume(self.config.unit) {
                    ESP_OK => {}
                    err => return Err(EspError::from(err).unwrap().into()),
                }
            }
        }
        Ok(())
    }
}

impl Status for Esp32SingleEncoder {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}

impl Drop for Esp32SingleEncoder {
    fn drop(&mut self) {
        if isr_installed() {
            unsafe {
                esp_idf_sys::pcnt_isr_handler_remove(self.config.unit);
            }
            isr_uninstall();
        }
    }
}

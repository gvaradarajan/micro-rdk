#![allow(dead_code)]
use crate::common::i2c::I2cHandleType;
use crate::common::math_utils::Vector3;
use crate::common::movement_sensor::{MovementSensor, MovementSensorSupportedMethods};

use super::board::{Board, BoardType};
use super::config::Kind::BoolValue;
use super::config::{Component, ConfigType};
use super::i2c::I2CHandle;
use super::movement_sensor::MovementSensorType;
use super::registry::ComponentRegistry;
use super::status::Status;

use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// This module represents an implementation of the MPU-6050 gyroscope/accelerometer
// as a Movement Sensor component

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_movement_sensor("accel-adxl345", &ADXL345::from_config)
        .is_err()
    {
        log::error!("accel-adxl345 type is already registered");
    }
}


// This is a struct meant to hold the most recently polled readings value
// and the most recent success or error state in acquiring readings. This
// is internal to this driver and should be accessed via Mutex.
#[derive(Clone, Debug)]
struct ADXL345State {
    linear_acceleration: Vector3,
    // temperature: f64,
    error: Option<Arc<anyhow::Error>>,
}

impl ADXL345State {
    fn new() -> Self {
        Self {
            linear_acceleration: Vector3::new(),
            // temperature: 0.0,
            error: None,
        }
    }

    fn get_linear_acceleration(&self) -> Vector3 {
        self.linear_acceleration
    }

    fn set_error(&mut self, err: Option<Arc<anyhow::Error>>) {
        self.error = err;
    }

    fn set_linear_acceleration_from_reading(&mut self, reading: &[u8; 6]) {
        let mut slice_copy = vec![0; 6];
        slice_copy.clone_from_slice(&(reading[0..6]));
        let mut rdr = Cursor::new(slice_copy);
        let unscaled_x = rdr.read_i16::<LittleEndian>().unwrap();
        let unscaled_y = rdr.read_i16::<LittleEndian>().unwrap();
        let unscaled_z = rdr.read_i16::<LittleEndian>().unwrap();

        let max_acceleration: f64 = 2.0 * 9.81 * 1000.0;

        let x = f64::from(unscaled_x) * max_acceleration / 512.0;
        let y = f64::from(unscaled_y) * max_acceleration / 512.0;
        let z = f64::from(unscaled_z) * max_acceleration / 512.0;
        self.linear_acceleration = Vector3 { x, y, z };
    }
}

pub struct ADXL345 {
    state: Arc<Mutex<ADXL345State>>,
    i2c_handle: I2cHandleType,
    i2c_address: u8,
    canceller: Sender<bool>,
}

impl ADXL345 {
    pub fn new(mut i2c_handle: I2cHandleType, i2c_address: u8) -> anyhow::Result<Self> {
        let bytes: [u8; 2] = [45, 8];
        i2c_handle.write_i2c(i2c_address, &bytes)?;
        let i2c_address_copy = i2c_address;
        let raw_state = ADXL345State::new();
        let (canceller, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(raw_state));
        // reference copies for sending memory into thread
        let mut i2c_handle_copy = Arc::clone(&i2c_handle);
        let state_copy = Arc::clone(&state);
        // start a polling thread that reads from the ADXL every millisecond and mutates state.
        // This allows multi-read access to the state for the functions satisfying the
        // Movement Sensor API
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(1));
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    log::debug!("ADXL-345: Terminating polling thread.");
                    break;
                }
                Err(TryRecvError::Empty) => {
                    let register_write: [u8; 1] = [0x32];
                    let mut result: [u8; 6] = [0; 6];
                    let mut internal_state = state_copy.lock().unwrap();
                    let res = i2c_handle_copy.write_read_i2c(
                        i2c_address_copy,
                        &register_write,
                        &mut result,
                    );
                    match res {
                        Ok(_) => {
                            internal_state.set_linear_acceleration_from_reading(&result);
                            internal_state.set_error(None);
                        }
                        Err(err) => {
                            log::error!("ADXL I2C error: {:?}", err);
                            internal_state.set_error(Some(Arc::new(err)));
                        }
                    };
                }
            }
        });
        Ok(Self {
            state,
            i2c_handle,
            i2c_address,
            canceller,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn from_config(
        cfg: ConfigType,
        board: Option<BoardType>,
    ) -> anyhow::Result<MovementSensorType> {
        if board.is_none() {
            return Err(anyhow::anyhow!(
                "actual board is required to be passed to configure ADXL-345"
            ));
        }
        let board_unwrapped = board.unwrap();
        match cfg {
            ConfigType::Static(cfg) => {
                let i2c_handle: I2cHandleType;
                if let Ok(i2c_name) = cfg.get_attribute::<&'static str>("i2c_bus") {
                    i2c_handle = board_unwrapped.get_i2c_by_name(i2c_name.to_string())?;
                } else {
                    return Err(anyhow::anyhow!(
                        "i2c_bus is a required config attribute for ADXL-345"
                    ));
                };
                if match &cfg.attributes {
                    None => false,
                    Some(attrs) => match attrs.get("use_alt_i2c_address") {
                        Some(BoolValue(value)) => *value,
                        _ => false,
                    },
                } {
                    return Ok(Arc::new(Mutex::new(ADXL345::new(i2c_handle, 29)?)));
                }
                Ok(Arc::new(Mutex::new(ADXL345::new(i2c_handle, 83)?)))
            }
        }
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        // close the polling thread
        if let Err(err) = self.canceller.send(true) {
            return Err(anyhow::anyhow!(
                "adxl-345 failed to close polling thread: {:?}",
                err
            ));
        };
        // put the MPU in the sleep state
        let off_data: [u8; 2] = [45, 0];
        if let Err(err) = self.i2c_handle.write_i2c(self.i2c_address, &off_data) {
            return Err(anyhow::anyhow!("adxl-345 sleep command failed: {:?}", err));
        };
        Ok(())
    }
}

impl Drop for ADXL345 {
    fn drop(&mut self) {
        if let Err(err) = self.close() {
            log::error!("adxl-345 close failure: {:?}", err)
        };
    }
}

impl MovementSensor for ADXL345 {
    fn get_properties(&self) -> MovementSensorSupportedMethods {
        MovementSensorSupportedMethods {
            position_supported: false,
            linear_velocity_supported: false,
            angular_velocity_supported: false,
            linear_acceleration_supported: true,
            compass_heading_supported: false,
        }
    }

    fn get_linear_acceleration(&self) -> anyhow::Result<Vector3> {
        let state = self.state.lock().unwrap();
        match &state.error {
            None => Ok(state.get_linear_acceleration()),
            Some(error_arc) => {
                let inner_err = error_arc.as_ref();
                Err(anyhow::anyhow!("{}", *inner_err))
            }
        }
    }

    fn get_angular_velocity(&self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_angular_velocity")
    }

    fn get_position(&self) -> anyhow::Result<super::movement_sensor::GeoPosition> {
        anyhow::bail!("unimplemented: movement_sensor_get_position")
    }

    fn get_linear_velocity(&self) -> anyhow::Result<Vector3> {
        anyhow::bail!("unimplemented: movement_sensor_get_linear_velocity")
    }

    fn get_compass_heading(&self) -> anyhow::Result<f64> {
        anyhow::bail!("unimplemented: movement_sensor_get_compass_heading")
    }
}

impl Status for ADXL345 {
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}


//! ESP32-specific implementations of components and tools

pub mod analog;
pub mod board;
#[cfg(feature = "camera")]
pub mod camera;
pub mod certificate;
pub mod dtls;
pub mod entry;
pub mod esp_idf_svc;
pub mod exec;
pub mod i2c;
pub mod pin;
pub mod pulse_counter;
pub mod pwm;
pub mod tcp;
pub mod tls;
pub mod utils;
pub mod conn {
    pub mod mdns;
}
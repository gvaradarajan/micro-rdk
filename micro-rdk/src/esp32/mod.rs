//! ESP32-specific implementations of components and tools
#[cfg(feature = "analog")]
pub mod analog;
pub mod board;
#[cfg(feature = "camera")]
pub mod camera;
pub mod certificate;
pub mod dtls;
pub mod entry;
pub mod esp_idf_svc;
pub mod exec;
#[cfg(feature = "i2c")]
pub mod i2c;
#[cfg(feature = "gpio")]
pub mod pin;
pub mod pulse_counter;
#[cfg(feature = "gpio")]
pub mod pwm;
pub mod tcp;
pub mod tls;
pub mod utils;
pub mod conn {
    pub mod mdns;
}

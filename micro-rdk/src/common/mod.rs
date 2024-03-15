//! Structs, traits, and utils to develop [component](https://docs.viam.com/components/)
//! drivers.
//!
//! # Components
//! - [actuator]
//! - [base]
//! - [board]
//! - [camera]
//! - [encoder]
//! - [motor]
//! - [movement_sensor]
//! - [sensor]
//! - [servo]
//!
//! # Utils
//! - [grpc]
//! - [grpc_client]
//! - [i2c]
//! - [webrtc]
//! - [conn]
//!
//!
//! General Purpose Drivers
//! - [adxl345]
//! - [gpio_motor]
//! - [ina]
//! - [mpu6050]

pub mod actuator;
pub mod analog;
pub mod app_client;
pub mod base;
pub mod board;
pub mod camera;
pub mod config;
pub mod digital_interrupt;
pub mod encoder;
pub mod entry;
pub mod generic;
pub mod grpc;
pub mod grpc_client;
pub mod i2c;
pub mod log;
pub mod math_utils;
pub mod motor;
pub mod movement_sensor;
pub mod registry;
pub mod robot;
pub mod sensor;
pub mod status;
pub mod webrtc {
    pub mod api;
    pub mod candidates;
    pub mod certificate;
    pub mod dtls;
    pub mod exec;
    pub mod grpc;
    pub mod ice;
    pub mod io;
    pub mod sctp;
}
pub mod conn {
    pub mod errors;
    pub mod mdns;
    pub mod server;
    mod utils;
}

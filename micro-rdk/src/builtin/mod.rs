#[cfg(feature = "movement_sensor")]
pub mod adxl345;
#[cfg(feature = "esp32")]
pub mod esp32_encoder;
pub mod fake;
pub mod gpio_motor;
#[cfg(feature = "servo")]
pub mod gpio_servo;
#[cfg(feature = "power_sensor")]
pub mod ina;
#[cfg(feature = "sensor")]
pub mod moisture_sensor;
#[cfg(feature = "movement_sensor")]
pub mod mpu6050;
#[cfg(all(feature = "esp32", feature = "sensor"))]
pub mod hcsr04;
#[cfg(feature = "encoder")]
pub mod single_encoded_motor;
#[cfg(all(feature = "esp32", feature = "encoder"))]
pub mod single_encoder;
pub mod wheeled_base;

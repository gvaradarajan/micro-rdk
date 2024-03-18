#[cfg(all(feature = "movement_sensor", feature = "i2c"))]
pub mod adxl345;
#[cfg(all(feature = "esp32", feature = "encoder"))]
pub mod esp32_encoder;
pub mod fake;
#[cfg(all(feature = "motor", feature = "gpio"))]
pub mod gpio_motor;
#[cfg(all(feature = "servo", feature = "gpio"))]
pub mod gpio_servo;
#[cfg(feature = "power_sensor")]
pub mod ina;
#[cfg(all(feature = "sensor", feature = "analog"))]
pub mod moisture_sensor;
#[cfg(all(feature = "movement_sensor", feature = "i2c"))]
pub mod mpu6050;
#[cfg(all(feature = "esp32", feature = "sensor"))]
pub mod hcsr04;
#[cfg(all(feature = "encoder", feature = "motor"))]
pub mod single_encoded_motor;
#[cfg(all(feature = "esp32", feature = "encoder"))]
pub mod single_encoder;
#[cfg(all(feature = "base", feature = "motor"))]
pub mod wheeled_base;

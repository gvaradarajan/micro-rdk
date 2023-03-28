#![allow(dead_code)]

use std::sync::{Arc, Mutex};

// A trait representing I2C communication for a board. TODO: replace with the
// embedded_hal I2C trait when supporting boards beyond ESP32. AddressType is
// either u8 (indicating support for 7-bit addresses) or u16 (for supporting 10-bit addresses)
pub trait BoardI2C<AddressType> {
    fn read_i2c(&self, _address: AddressType, _buffer: &mut [u8]) -> anyhow::Result<()> {
        anyhow::bail!("read_i2c unimplemented")
    }

    fn write_i2c(&mut self, _address: AddressType, _bytes: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("write_i2c unimplemented")
    }

    // write_read_i2c represents a transactional write and read to an I2C address
    fn write_read_i2c(
        &mut self,
        _address: AddressType,
        _bytes: &[u8],
        _buffer: &mut [u8],
    ) -> anyhow::Result<()> {
        anyhow::bail!("write_read_i2c unimplemented")
    }
}

impl<A> BoardI2C<u8> for Arc<Mutex<A>>
where
    A: ?Sized + BoardI2C<u8>,
{
    fn read_i2c(&self, address: u8, buffer: &mut [u8]) -> anyhow::Result<()> {
        self.lock().unwrap().read_i2c(address, buffer)
    }

    fn write_i2c(&mut self, address: u8, bytes: &[u8]) -> anyhow::Result<()> {
        self.lock().unwrap().write_i2c(address, bytes)
    }

    fn write_read_i2c(
        &mut self,
        address: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> anyhow::Result<()> {
        self.lock().unwrap().write_read_i2c(address, bytes, buffer)
    }
}

impl<A> BoardI2C<u16> for Arc<Mutex<A>>
where
    A: ?Sized + BoardI2C<u16>,
{
    fn read_i2c(&self, address: u16, buffer: &mut [u8]) -> anyhow::Result<()> {
        self.lock().unwrap().read_i2c(address, buffer)
    }

    fn write_i2c(&mut self, address: u16, bytes: &[u8]) -> anyhow::Result<()> {
        self.lock().unwrap().write_i2c(address, bytes)
    }

    fn write_read_i2c(
        &mut self,
        address: u16,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> anyhow::Result<()> {
        self.lock().unwrap().write_read_i2c(address, bytes, buffer)
    }
}

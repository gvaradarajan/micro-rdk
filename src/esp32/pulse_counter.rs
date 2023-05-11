use esp_idf_sys::{pcnt_isr_service_install, pcnt_isr_service_uninstall, EspError, ESP_OK};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/*
This module exists because we want to ensure uniqueness of unit number
across instances of an Esp32 Pulse Counter unit and make sure the isr service
is only installed once.

THIS MODULE IS A TEMPORARY MEASURE. There are two circumstances that would
allow for its removal

1) Abstracting the atomicity of Esp32 peripherals to board

2) Technically the pulse counter API we are interacting with in
our encoder implementations is deprecated and the new pulse counter
manages the below for us. However the esp-idf-sys package has not been updated
to include the new headers for this pulse counter implementation. If/when
we are able to make that update, this may be deleted.

*/

static NEXT_UNIT: AtomicU32 = AtomicU32::new(0);

static ISR_INSTALLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn get_unit() -> u32 {
    NEXT_UNIT.fetch_add(1, Ordering::SeqCst)
}

pub(crate) fn isr_install() -> anyhow::Result<()> {
    if !ISR_INSTALLED.fetch_or(true, Ordering::Relaxed) {
        unsafe {
            match pcnt_isr_service_install(0) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
    }
    Ok(())
}

pub(crate) fn isr_uninstall() {
    if ISR_INSTALLED.fetch_xor(false, Ordering::Relaxed) {
        unsafe {
            pcnt_isr_service_uninstall();
        }
    }
}

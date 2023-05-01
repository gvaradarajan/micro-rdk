use esp_idf_hal::gpio::{InputMode, OutputMode, Pin, PinDriver};

pub trait PinExt {
    fn pin(&self) -> i32;
}

impl<'d, T: Pin, MODE> PinExt for PinDriver<'d, T, MODE>
{
    fn pin(&self) -> i32 {
        self.pin()
    }
}

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

use super::config::{AttributeError, Kind};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum InterruptEventType {
    PosEDGE,
    NegEDGE,
    AnyEDGE,
    LOW,
    HIGH,
}

/// Represents an interrupt event on a pin. Includes the state
/// of the pin immediately after the event (is_high) and the timestamp
/// of the event. NOTE: we use Instant because the timestamp is meant
/// for computing *time differences between events* and using duration_since
/// on SystemTime instances is subject to more inaccuracy (see Rust documentation
/// at https://doc.rust-lang.org/std/time/struct.SystemTime.html for more
/// information)
#[derive(Debug, Clone, Copy)]
pub struct InterruptEvent {
    is_high: bool,
    timestamp: Instant,
}

impl InterruptEvent {
    pub fn new(is_high: bool) -> Self {
        Self {
            is_high,
            timestamp: Instant::now(),
        }
    }

    pub fn is_high(&self) -> bool {
        self.is_high
    }

    pub fn timestamp(&self) -> Instant {
        self.timestamp
    }
}

pub(crate) struct PinEventTransmitter {
    broadcasters: Vec<Sender<InterruptEvent>>,
    intr_type: InterruptEventType,
    previously_high: bool,
    in_use: Arc<AtomicBool>,
    event_count: Arc<AtomicI64>,
}

impl PinEventTransmitter {
    pub fn new(intr_type: InterruptEventType, initially_high: bool) -> Self {
        Self {
            broadcasters: vec![],
            intr_type,
            previously_high: initially_high,
            in_use: Arc::new(AtomicBool::new(false)),
            event_count: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn emit_event(&mut self) -> anyhow::Result<()> {
        self.in_use.store(true, Ordering::Release);
        self.event_count.fetch_add(1, Ordering::Relaxed);
        let event = self.create_event();
        for broadcaster in self.broadcasters.iter_mut() {
            broadcaster.send(event)?;
        }
        self.in_use.store(false, Ordering::Release);
        Ok(())
    }

    fn create_event(&mut self) -> InterruptEvent {
        match self.intr_type {
            InterruptEventType::AnyEDGE => {
                self.previously_high = !self.previously_high;
                InterruptEvent::new(!self.previously_high)
            }
            InterruptEventType::LOW | InterruptEventType::NegEDGE => InterruptEvent::new(false),
            InterruptEventType::HIGH | InterruptEventType::PosEDGE => InterruptEvent::new(true),
        }
    }

    pub fn subscribe(&mut self) -> Receiver<InterruptEvent> {
        while self.in_use.load(Ordering::Acquire) {}
        let (broadcaster, receiver) = channel();
        self.broadcasters.push(broadcaster);
        receiver
    }

    pub fn get_event_count(&self) -> i64 {
        self.event_count.load(Ordering::Relaxed)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct DigitalInterruptConfig {
    pub pin: i32,
}

impl TryFrom<Kind> for DigitalInterruptConfig {
    type Error = AttributeError;
    fn try_from(value: Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("pin")? {
            return Err(AttributeError::KeyNotFound("pin".to_string()));
        }
        let pin = value.get("pin")?.unwrap().try_into()?;
        Ok(DigitalInterruptConfig { pin })
    }
}

impl TryFrom<&Kind> for DigitalInterruptConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if !value.contains_key("pin")? {
            return Err(AttributeError::KeyNotFound("pin".to_string()));
        }
        let pin = value.get("pin")?.unwrap().try_into()?;
        Ok(DigitalInterruptConfig { pin })
    }
}

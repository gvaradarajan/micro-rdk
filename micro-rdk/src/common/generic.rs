use std::sync::{Arc, Mutex};

use crate::google::protobuf::Struct;

use super::status::Status;

pub static COMPONENT_NAME: &str = "generic";

pub trait DoCommand {
    /// do_command custom commands outside of a strict API. Takes a command struct that can be interpreted
    /// as a map of method name keys to argument values.
    fn do_command(&mut self, _command_struct: Option<Struct>) -> anyhow::Result<Option<Struct>> {
        anyhow::bail!("do_command unimplemented")
    }
}

impl<L> DoCommand for Mutex<L>
where
    L: ?Sized + DoCommand,
{
    fn do_command(&mut self, command_struct: Option<Struct>) -> anyhow::Result<Option<Struct>> {
        self.get_mut().unwrap().do_command(command_struct)
    }
}

impl<A> DoCommand for Arc<Mutex<A>>
where
    A: ?Sized + DoCommand,
{
    fn do_command(&mut self, command_struct: Option<Struct>) -> anyhow::Result<Option<Struct>> {
        self.lock().unwrap().do_command(command_struct)
    }
}

pub trait GenericComponent: DoCommand + Status {}

pub type GenericComponentType = Arc<Mutex<dyn GenericComponent>>;

impl<L> GenericComponent for Mutex<L> where L: ?Sized + GenericComponent {}

impl<A> GenericComponent for Arc<Mutex<A>> where A: ?Sized + GenericComponent {}

use futures::sync::mpsc;

use crate::message;
use crate::thread::forward;

pub type In = message::P1A;


pub struct Scout<O> {
    rx: mpsc::UnboundedReceiver<In>,
    tx: mpsc::UnboundedSender<forward::In<O>>,
}

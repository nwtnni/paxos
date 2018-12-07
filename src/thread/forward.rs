use futures::sync::mpsc;

use crate::message;

#[derive(Clone, Debug)]
pub enum In<O> {
    P1B(usize, message::P1B<O>),
    P2B(usize, message::P2B),
}

pub struct Forward<O> {
    rx: mpsc::UnboundedReceiver<(usize, In<O>)>
}

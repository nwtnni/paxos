use futures::sync::mpsc;

use crate::message;

#[derive(Clone, Debug)]
pub enum In<O> {
    P1B(message::P1B<O>),
    P2B(message::P2B),
}

pub type SendResult<O> = Result<(), mpsc::SendError<In<O>>>;

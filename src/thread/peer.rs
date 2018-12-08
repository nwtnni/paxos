use futures::sync::mpsc;

use crate::message;

#[derive(Clone, Debug)]
pub enum In<O> {
    P1A(message::P1A),
    P2A(message::P2A<O>),
    P1B(message::P1B<O>),
    P2B(message::P2B),
}

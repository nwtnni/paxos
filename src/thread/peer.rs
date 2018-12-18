use futures::sync::mpsc;

use crate::message;

#[derive(Clone, Debug)]
pub enum In<I> {
    P1A(message::P1A),
    P2A(message::P2A<I>),
    P1B(message::P1B<I>),
    P2B(message::P2B),
}

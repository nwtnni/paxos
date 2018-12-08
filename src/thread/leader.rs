use std::sync::Arc;

use hashbrown::HashMap as Map;
use parking_lot::Mutex;
use futures::sync::mpsc;

use crate::message;
use crate::thread::peer;
use crate::shared;

#[derive(Clone, Debug)]
pub enum In<O> {
    P1A(Vec<usize>, message::P1A),
    P1B(usize, message::P1B<O>),
    P2A(Vec<usize>, message::P2A<O>),
    P2B(usize, message::P2B),
    Preempt(message::BallotID),
    Adopt(message::BallotID, Vec<message::PValue<O>>),
    Decide(message::PValue<O>),
}

pub struct Leader<O> {
    id: usize,
    rx: mpsc::UnboundedReceiver<In<O>>,
    tx: shared::Shared<O>,
}

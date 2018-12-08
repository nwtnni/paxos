use std::sync::Arc;

use hashbrown::HashMap as Map;
use parking_lot::Mutex;
use futures::sync::mpsc;

use crate::message;
use crate::thread::{Tx, Rx};
use crate::thread::{commander, scout};
use crate::shared;

#[derive(Clone, Debug)]
pub enum In<O> {
    P1A(Vec<usize>, message::P1A),
    P2A(Vec<usize>, message::P2A<O>),
    Preempt(message::BallotID),
    Adopt(message::BallotID, Vec<message::PValue<O>>),
    Decide(message::PValue<O>),
}

pub type SendResult<O> = Result<(), mpsc::SendError<In<O>>>;

pub struct Leader<O> {
    id: usize,
    rx: mpsc::UnboundedReceiver<In<O>>,
    tx: shared::Shared<O>,
    scout_tx: Option<Tx<scout::In<O>>>,
    comms_tx: Map<commander::ID, Tx<commander::In>>,
}

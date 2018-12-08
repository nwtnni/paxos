use hashbrown::HashSet as Set;

use crate::message;

#[derive(Clone, Debug)]
pub enum In<O> {
    P1A(Vec<usize>, message::P1A),
    P1B(usize, message::P1B<O>),
    P2B(usize, message::P2B),
    Preempt(message::BallotID),
    Adopt(message::BallotID, Vec<message::PValue<O>>),
}

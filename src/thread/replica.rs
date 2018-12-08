use crate::message;

pub enum In<O> {
    Propose(message::Proposal<O>),
    Decide(message::Proposal<O>),
}

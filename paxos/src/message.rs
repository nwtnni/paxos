//! # Summary
//!
//! This module defines message and identifier types for
//! server-to-server communication. Almost all types, like `P1A`,
//! `P1B`, `P2A`, and `P2B`, are equivalent to those described in
//! Paxos Made Moderately Complex. `Command` is a Rust implementation
//! detail.

use serde_derive::{Deserialize, Serialize};

use crate::state;

/// Wrapper around `state::Command` that defines
/// equality based on a command's client ID and
/// its client-local ID.
#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Clone, Debug)]
pub struct Command<C: state::Command>(C);

impl<C: state::Command> Command<C> {
    pub fn inner(self) -> C {
        self.0
    }
}

impl<C: state::Command> From<C> for Command<C> {
    fn from(command: C) -> Self {
        Command(command)
    }
}

impl<C: state::Command> Eq for Command<C> {}

impl<C: state::Command> PartialEq for Command<C> {
    fn eq(&self, rhs: &Self) -> bool {
        self.0.client_id() == rhs.0.client_id() &&
        self.0.local_id() == rhs.0.local_id()
    }
}

impl<C: state::Command> std::hash::Hash for Command<C> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.client_id().hash(state);
        self.0.local_id().hash(state);
    }
}

impl<C: state::Command> std::ops::Deref for Command<C> {
    type Target = C;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A ballot is uniquely determined by its leader's ID
/// and its local sequence number.
#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ballot {
    /// Leader-local sequence number
    pub b_id: usize,
    /// Leader ID
    pub l_id: usize,
}

/// A commander is uniquely determined by its leader's ballot
/// when it was created and the slot it is targeting.
#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommanderID {
    /// Associated ballot
    pub b_id: Ballot,
    /// Targeted slot
    pub s_id: usize,
}

/// Represents a proposed binding from slot to command using the associated ballot.
#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""), Hash(bound = ""), PartialEq(bound = ""), Eq(bound = ""))]
pub struct PValue<C: state::Command> {
    /// Targeted slot
    pub s_id: usize,
    /// Associated ballot
    pub b_id: Ballot,
    /// Proposed command
    pub command: Command<C>,
}

/// Query from scout to acceptor.
pub type P1A = Ballot;

/// Response from acceptor to scout.
#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""), Hash(bound = ""), PartialEq(bound = ""), Eq(bound = ""))]
pub struct P1B<C: state::Command> {
    pub a_id: usize,
    pub b_id: Ballot,
    pub pvalues: Vec<PValue<C>>,
}

/// Query from commander to acceptor.
pub type P2A<C> = PValue<C>;

/// Response from acceptor to commander.
#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2B {
    /// Acceptor ID
    pub a_id: usize,
    /// Acceptor's currently adopted ballot
    pub b_id: Ballot,
}

/// Represents a proposed binding from slot to command.
#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""), Debug(bound = ""), Hash(bound = ""), PartialEq(bound = ""), Eq(bound = ""))]
pub struct Proposal<C: state::Command> {
    /// Targeted slot
    pub s_id: usize,
    /// Proposed command
    pub command: Command<C>,
}

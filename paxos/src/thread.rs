//! # Summary
//!
//! This module contains all of the necessary sub-threads for the Paxos protocol.
//!
//! With the exception of `client`, which handles message forwarding between server
//! and client, and `peer`, which handles message forwarding between servers, each
//! module directly correlates to a sub-thread described in
//! [Paxos Made Moderately Complex][1], which this implementation is based on.
//!
//! [1]: http://paxos.systems/index.html

/// Distributed memory.
pub(crate) mod acceptor;

/// Client communication.
pub(crate) mod client;

/// Command proposer.
pub(crate) mod commander;

/// Replica ambassador.
pub(crate) mod leader;

/// Peer server communication.
pub(crate) mod peer;

/// Replicated state machine.
pub(crate) mod replica;

/// Ballot proposer.
pub(crate) mod scout;

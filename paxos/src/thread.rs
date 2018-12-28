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

pub(crate) mod acceptor;
pub(crate) mod client;
pub(crate) mod commander;
pub(crate) mod leader;
pub(crate) mod peer;
pub(crate) mod replica;
pub(crate) mod scout;

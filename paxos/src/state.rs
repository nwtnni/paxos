//! # Summary
//!
//! This module defines several traits that together define a state machine,
//! which the Paxos protocol then replicates. Library users must implement
//! the `State` trait for their state machine, and then create a `paxos`
//! instance using the `config` module.

/// Unique identifier
pub trait Identifier: std::hash::Hash
    + std::fmt::Debug
    + Clone
    + Eq
    + Send
    + Sync
{
}

impl<T> Identifier for T where T: std::hash::Hash
    + std::fmt::Debug
    + Clone
    + Eq
    + Send
    + Sync
{
}

/// Operation that can be applied to a state machine
pub trait Command: Send
    + Clone
    + std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
    type ClientID: Identifier;
    type LocalID: Identifier;
    fn client_id(&self) -> Self::ClientID;
    fn local_id(&self) -> Self::LocalID;
}

/// Result of applying an operation to a state machine
pub trait Response: Send
    + std::fmt::Debug
    + serde::Serialize
{
}

impl<T> Response for T where T: Send
    + std::fmt::Debug
    + serde::Serialize
{
}

/// Replicated state machine
pub trait State: Default + Send + 'static {
    type Command: Command;
    type Response: Response;
    fn execute(&mut self, slot: usize, command: Self::Command) -> Option<Self::Response>;
}

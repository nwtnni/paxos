pub trait Identifier: std::fmt::Debug
    + std::hash::Hash
    + std::marker::Unpin
    + serde::Serialize
    + serde::de::DeserializeOwned
    + Clone
    + Eq
    + PartialEq
    + Send
    + Sync
    + 'static
{
}

/// Operation that can be applied to a state machine
pub trait Command: Clone
    + std::marker::Unpin
    + serde::Serialize
    + serde::de::DeserializeOwned
    + Send
    + Sync
    + 'static
{
    type ClientID: Identifier;
    type LocalID: Identifier;
    fn client_id(&self) -> Self::ClientID;
    fn local_id(&self) -> Self::LocalID;
}

pub trait Response: std::marker::Unpin 
    + serde::Serialize
    + serde::de::DeserializeOwned
    + Send
    + 'static
{
    fn connected(replica_id: usize) -> Self;
}

/// Replicated state machine
pub trait State: 'static + Default + Send {
    type Command: Command;
    type Response: Response;
    fn execute(&mut self, command: Self::Command) -> Self::Response;
}

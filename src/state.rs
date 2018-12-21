/// Unique identifier
pub trait Identifier: std::hash::Hash
    + std::marker::Unpin
    + serde::Serialize
    + serde::de::DeserializeOwned
    + Clone
    + Eq
    + Send
    + Sync
{
}

/// Operation that can be applied to a state machine
pub trait Command: Send
    + std::marker::Unpin
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
    + serde::Serialize
    + serde::de::DeserializeOwned
{
    fn connected(replica_id: usize) -> Self;
}

/// Replicated state machine
pub trait State: Default + Send + 'static {
    type Command: Command;
    type Response: Response;
    fn execute(&mut self, slot: usize, command: Self::Command) -> Self::Response;
}

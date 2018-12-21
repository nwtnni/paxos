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

pub trait Serializable: std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
}

pub trait CommandID: Identifier {
    type Client: Identifier;
    type Command: Identifier;
    fn client_id(&self) -> Self::Client;
}

pub trait Command: Clone
    + std::marker::Unpin
    + Serializable
{
    type ID: CommandID;
    type Response: Serializable;
    fn id(&self) -> Self::ID;
}

pub trait State {
    type Command: Command;
    fn execute(&mut self, command: Self::Command) -> <Self::Command as Command>::Response;
}

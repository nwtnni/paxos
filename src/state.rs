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

pub trait CommandID: Identifier {
    type Client: Identifier;
    type Command: Identifier;
    fn client_id(&self) -> Self::Client;
}

pub trait Command: std::fmt::Debug
    + std::marker::Unpin
    + serde::Serialize
    + serde::de::DeserializeOwned
    + Clone
{
    type ID: CommandID;

    fn id(&self) -> Self::ID;
}

pub trait Response: std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
}

pub trait State<C: Command, R: Response> {
    fn execute(&mut self, command: C) -> R;
}

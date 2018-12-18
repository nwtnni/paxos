pub trait Identifier: std::fmt::Debug
    + std::hash::Hash
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

#[rustfmt::skip]
pub trait Command: std::fmt::Debug
    + std::marker::Unpin
    + serde::Serialize
    + serde::de::DeserializeOwned
    + Clone
{
    type ID: Identifier;

    fn id(&self) -> Self::ID;
}

#[rustfmt::skip]
pub trait Response: std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
}

pub trait State<C: Command, R: Response> {
    fn execute(&mut self, command: C) -> R;
}

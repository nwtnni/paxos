#[rustfmt::skip]
pub trait Operation: std::fmt::Debug
    + std::hash::Hash
    + serde::Serialize
    + serde::de::DeserializeOwned
    + Clone
    + Eq
    + PartialEq
    + Send
    + Sync
{
}

#[rustfmt::skip]
pub trait Response: std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
}

pub trait State<O: Operation, R: Response> {
    fn execute(&mut self, operation: O) -> R;
}

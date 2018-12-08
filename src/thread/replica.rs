use std::marker;

use futures::{stream, sink};
use tokio_async_await::stream::StreamExt;
use tokio_serde_bincode::{ReadBincode, WriteBincode};
use tokio_serde_json::{ReadJson, WriteJson};
use tokio::codec::{FramedRead, LengthDelimitedCodec};
use tokio::net;

use crate::message;
use crate::state;
use crate::thread::*;

pub enum In<O> {
    Propose(message::Proposal<O>),
    Decide(message::Proposal<O>),
}

pub struct Replica<O, R, S> {
    client_rx: SocketRx<O>,
    client_tx: SocketTx<O>,
    state: S,
    _phantom: marker::PhantomData<R>,
}

impl<O: state::Operation + marker::Unpin, R: state::Response, S: state::State<O, R>> Replica<O, R, S> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.client_rx.next()) {


            }
        }
    }
}

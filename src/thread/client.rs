use tokio_serde_bincode::WriteBincode;
use tokio::prelude::*;

use crate::message;
use crate::shared;
use crate::state;
use crate::thread::{SocketRx, SocketTx, Rx, Tx, replica};

pub type In<C> = message::Proposal<C>;

pub struct Client<S: state::State> {
    socket_rx: SocketRx<S::Command>,
    socket_tx: SocketTx,
    replica_tx: Tx<replica::In<S::Command>>,
    shared_tx: shared::Shared<S>,
    rx: Rx<S::Response>,
}

impl<S: state::State> Client<S> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.socket_rx.next()) {
                self.replica_tx.unbounded_send(replica::In::Request(message))
                    .expect("[INTERNAL ERROR]: failed to send to replica")
            }

            while let Some(Ok(message)) = await!(self.rx.next()) {
                if let Err(_) = WriteBincode::new(&mut self.socket_tx)
                    .send(message)
                    .wait() {
                    return
                }
            }
        }
    }
}

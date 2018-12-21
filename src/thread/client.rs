use tokio_serde_bincode::WriteBincode;
use tokio::prelude::*;

use crate::message;
use crate::shared;
use crate::state;
use crate::thread::{SocketRx, SocketTx, Rx, Tx, replica};

type In<C> = message::Proposal<C>;

pub struct Client<C: state::Command> {
    socket_rx: SocketRx<In<C::ID>>,
    socket_tx: SocketTx,
    replica_tx: Tx<replica::In<C::ID>>,
    shared_tx: shared::Shared<C::ID>,
    rx: Rx<In<C::ID>>,
}

impl<C: state::Command> Client<C> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.socket_rx.next()) {
                self.replica_tx.unbounded_send(message)
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

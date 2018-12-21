use futures::sync::mpsc;
use tokio_serde_bincode::WriteBincode;
use tokio::prelude::*;
use tokio::net;

use crate::shared;
use crate::socket;
use crate::state;
use crate::state::Command;
use crate::thread::{Rx, Tx, replica};

pub struct Connecting<S: state::State> {
    client_rx: socket::Rx<S::Command>,
    client_tx: socket::Tx,
    replica_tx: Tx<replica::In<S::Command>>,
    shared_tx: shared::Shared<S>,
}

impl<S: state::State> Connecting<S> {
    pub fn new(
        stream: net::tcp::TcpStream,
        replica_tx: Tx<replica::In<S::Command>>,
        shared_tx: shared::Shared<S>,
    ) -> Self {
        let (client_rx, client_tx) = socket::split(stream);
        Connecting {
            client_rx,
            client_tx,
            replica_tx,
            shared_tx,
        }
    }

    pub async fn run(mut self) -> Client<S> {
        loop {
            while let Some(Ok(message)) = await!(self.client_rx.next()) {
                let client_id = message.client_id();
                let (tx, rx) = mpsc::unbounded();
                self.shared_tx.write().connect_client(client_id.clone(), tx);
                self.replica_tx.unbounded_send(replica::In::Request(message))
                    .expect("[INTERNAL ERROR]: failed to send to replica");
                debug!("connected to {:?}", client_id);
                return Client {
                    client_id,
                    client_rx: self.client_rx,
                    client_tx: self.client_tx,
                    replica_tx: self.replica_tx,
                    shared_tx: self.shared_tx,
                    rx,
                }
            }
        }
    }
}

pub struct Client<S: state::State> {
    client_id: <S::Command as state::Command>::ClientID,
    client_rx: socket::Rx<S::Command>,
    client_tx: socket::Tx,
    replica_tx: Tx<replica::In<S::Command>>,
    shared_tx: shared::Shared<S>,
    rx: Rx<S::Response>,
}

impl<S: state::State> Client<S> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.client_rx.next()) {
                trace!("received command {:?}", message);
                self.replica_tx.unbounded_send(replica::In::Request(message))
                    .expect("[INTERNAL ERROR]: failed to send to replica")
            }

            while let Some(Ok(message)) = await!(self.rx.next()) {
                trace!("sending message {:?} to {:?}", message, self.client_id);
                if let Err(_) = WriteBincode::new(&mut self.client_tx)
                    .send(message)
                    .wait() {
                    return
                }
            }
        }
    }
}

impl<S: state::State> Drop for Client<S> {
    fn drop(&mut self) {
        debug!("disconnected from {:?}", self.client_id);
        self.shared_tx.write().disconnect_client(&self.client_id);
    }
}

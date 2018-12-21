use futures::sync::mpsc;
use tokio_serde_bincode::{ReadBincode, WriteBincode};
use tokio::prelude::*;
use tokio::{codec, net};

use crate::shared;
use crate::state;
use crate::state::{Command, Response};
use crate::thread::{SocketRx, SocketTx, Rx, Tx, replica};

pub struct Connecting<S: state::State> {
    self_id: usize,
    client_rx: SocketRx<S::Command>,
    client_tx: SocketTx,
    replica_tx: Tx<replica::In<S::Command>>,
    shared_tx: shared::Shared<S>,
}

impl<S: state::State> Connecting<S> {
    pub fn new(
        self_id: usize,
        stream: net::tcp::TcpStream,
        replica_tx: Tx<replica::In<S::Command>>,
        shared_tx: shared::Shared<S>,
    ) -> Self {
        let (client_rx, client_tx) = stream.split();
        let client_rx = ReadBincode::new(
            codec::length_delimited::Builder::new()
                .new_read(client_rx)
                .from_err::<bincode::Error>()
        );
        let client_tx = codec::length_delimited::Builder::new()
            .new_write(client_tx)
            .sink_from_err::<bincode::Error>();
        Connecting {
            self_id,
            client_rx,
            client_tx,
            replica_tx,
            shared_tx,
        }
    }

    pub async fn run(mut self) -> Result<Client<S>, Box<std::error::Error + 'static>> {
        loop {
            while let Some(Ok(message)) = await!(self.client_rx.next()) {
                let client_id = message.client_id();
                let (tx, rx) = mpsc::unbounded();
                self.shared_tx.write().connect_client(client_id.clone(), tx);
                WriteBincode::new(&mut self.client_tx)
                    .send(S::Response::connected(self.self_id))
                    .wait()?;
                return Ok(Client {
                    client_id,
                    client_rx: self.client_rx,
                    client_tx: self.client_tx,
                    replica_tx: self.replica_tx,
                    shared_tx: self.shared_tx,
                    rx,
                })
            }
        }
    }
}

pub struct Client<S: state::State> {
    client_id: <S::Command as state::Command>::ClientID,
    client_rx: SocketRx<S::Command>,
    client_tx: SocketTx,
    replica_tx: Tx<replica::In<S::Command>>,
    shared_tx: shared::Shared<S>,
    rx: Rx<S::Response>,
}

impl<S: state::State> Client<S> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.client_rx.next()) {
                self.replica_tx.unbounded_send(replica::In::Request(message))
                    .expect("[INTERNAL ERROR]: failed to send to replica")
            }

            while let Some(Ok(message)) = await!(self.rx.next()) {
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
        self.shared_tx.write().disconnect_client(&self.client_id);
    }
}

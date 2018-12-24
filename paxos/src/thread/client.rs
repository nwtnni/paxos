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
    client_rx: Option<socket::Rx<S::Command>>,
    client_tx: Option<socket::Tx>,
    replica_tx: Option<Tx<replica::In<S::Command>>>,
    shared_tx: Option<shared::Shared<S>>,
}

impl<S: state::State> Connecting<S> {
    pub fn new(
        stream: net::tcp::TcpStream,
        replica_tx: Tx<replica::In<S::Command>>,
        shared_tx: shared::Shared<S>,
    ) -> Self {
        let (client_rx, client_tx) = socket::split(stream);
        Connecting {
            client_rx: Some(client_rx),
            client_tx: Some(client_tx),
            replica_tx: Some(replica_tx),
            shared_tx: Some(shared_tx),
        }
    }
}

impl<S: state::State> Future for Connecting<S> {
    type Item = Client<S>;
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Async::Ready(Some(message)) = self.client_rx.as_mut()
            .unwrap()
            .poll()
            .map_err(|_| ())?
        {
            debug!("connected to {:?}", message.client_id());
            let client_id = message.client_id();
            let (tx, rx) = mpsc::unbounded();
            self.shared_tx.as_mut()
                .unwrap()
                .write()
                .connect_client(client_id.clone(), tx);
            self.replica_tx.as_mut()
                .unwrap()
                .unbounded_send(replica::In::Request(message))
                .expect("[INTERNAL ERROR]: failed to send to replica");
            return Ok(Async::Ready(Client {
                client_id,
                client_rx: self.client_rx.take().unwrap(),
                client_tx: self.client_tx.take().unwrap(),
                replica_tx: self.replica_tx.take().unwrap(),
                shared_tx: self.shared_tx.take().unwrap(),
                rx,
            }))
        }
        Ok(Async::NotReady)
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

impl<S: state::State> Future for Client<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Async::Ready(Some(message)) = self.client_rx
            .poll()
            .map_err(|_| ())?
        {
            info!("received {:?}", message);
            self.replica_tx.unbounded_send(replica::In::Request(message))
                .expect("[INTERNAL ERROR]: failed to send to replica")
        }

        while let Async::Ready(Some(message)) = self.rx
            .poll()
            .map_err(|_| ())?
        {
            WriteBincode::new(&mut self.client_tx)
                .send(message)
                .wait()
                .map_err(|_| ())?;
        }

        Ok(Async::NotReady)
    }
}

impl<S: state::State> Drop for Client<S> {
    fn drop(&mut self) {
        debug!("disconnected from {:?}", self.client_id);
        self.shared_tx.write().disconnect_client(&self.client_id);
    }
}

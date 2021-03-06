//! # Summary
//!
//! This module defines external connections to clients.
//! Responsible for forwarding messages to and from connected clients.

use tokio::prelude::*;
use tokio::net;

use crate::external;
use crate::internal;
use crate::shared;
use crate::state;
use crate::state::Command;
use crate::thread::replica;

/// Represents a client that has not yet sent a message, so we don't know its ID.
pub struct Connecting<S: state::State> {
    /// External client receiving channel
    client_rx: Option<external::Rx<S::Command>>,

    /// External client transmitting channel
    client_tx: Option<external::Tx<S::Response>>,

    /// Internal replica transmitting channel
    replica_tx: Option<internal::Tx<replica::In<S::Command>>>,
    
    /// Internal shared transmitting channels
    shared_tx: Option<shared::Shared<S>>,
}

impl<S: state::State> Connecting<S> {
    pub fn new(
        stream: net::tcp::TcpStream,
        replica_tx: internal::Tx<replica::In<S::Command>>,
        shared_tx: shared::Shared<S>,
    ) -> Self {
        let (client_rx, client_tx) = external::new(stream);
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
        while let Async::Ready(Some(message)) = self.client_rx.as_mut().unwrap().poll()?  {
            // After we receive a message, we can read off the client's ID,
            // register it with the shared transmission hub, and
            // promote it to a Client struct. Safe to unwrap here because
            // we always initialize with Some, and always return after moving
            // out of the option.
            info!("connected to {:?}", message.client_id());
            let client_id = message.client_id();
            let (rx, tx) = internal::new();
            self.shared_tx.as_mut()
                .unwrap()
                .write()
                .connect_client(client_id.clone(), tx);
            self.replica_tx.as_mut()
                .unwrap()
                .send(replica::In::Request(message));
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

/// Represents a client with known ID that is registered with
/// the shared transmission hub.
pub struct Client<S: state::State> {
    /// Internal receiving channel
    rx: internal::Rx<S::Response>,

    /// Client ID
    client_id: <S::Command as state::Command>::ClientID,

    /// External client receiving channel
    client_rx: external::Rx<S::Command>,

    /// External client transmitting channel
    client_tx: external::Tx<S::Response>,

    /// Internal replica transmitting channel
    replica_tx: internal::Tx<replica::In<S::Command>>,

    /// Internal shared transmitting channels
    shared_tx: shared::Shared<S>,
}

impl<S: state::State> Future for Client<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {

        // Forward incoming requests
        while let Async::Ready(Some(message)) = self.client_rx.poll()?  {
            trace!("received {:?}", message);
            self.replica_tx.send(replica::In::Request(message));
        }

        // Forward outgoing responses
        while let Async::Ready(Some(message)) = self.rx.poll()?  {
            trace!("sending {:?}", message);
            self.client_tx.start_send(message)?;
        }

        // Complete sends
        if let Async::NotReady = self.client_tx.poll_complete()? {
            return Ok(Async::NotReady)
        }

        Ok(Async::NotReady)
    }
}

impl<S: state::State> Drop for Client<S> {
    fn drop(&mut self) {
        info!("disconnected from {:?}", self.client_id);
        self.shared_tx.write().disconnect_client(&self.client_id);
    }
}

//! # Summary
//!
//! This module defines external connections to other servers.
//! Responsible for forwarding messages to and from connected servers.

use serde_derive::{Serialize, Deserialize};
use tokio::prelude::*;
use tokio::net;

use crate::external;
use crate::internal;
use crate::message;
use crate::shared::Shared;
use crate::state;
use crate::thread::acceptor;

/// Peer servers can receive messages between
/// scouts, commanders, and acceptors, decisions
/// from commanders, and pings to detect failed
/// servers.
#[derive(Debug, Derivative, Deserialize, Serialize)]
#[derivative(Clone(bound = ""))]
#[serde(bound(serialize = "", deserialize = ""))]
pub enum In<C: state::Command> {
    P1A(message::P1A),
    P1B(message::P1B<C>),
    P2A(message::CommanderID, message::P2A<C>),
    P2B(message::CommanderID, message::P2B),
    Decision(message::Proposal<C>),
    Ping(usize),
}

/// Represents a peer that has not yet sent a ping, so we don't know its ID.
pub struct Connecting<S: state::State> {
    /// ID of the current server (not the peer)
    self_id: usize,

    /// External peer receiving channel
    peer_rx: Option<external::Rx<In<S::Command>>>,

    /// External peer transmitting channel
    peer_tx: Option<external::Tx<In<S::Command>>>,

    /// Internal acceptor transmitting channel
    acceptor_tx: Option<internal::Tx<acceptor::In<S::Command>>>,

    /// Internal shared transmitting channels
    shared_tx: Option<Shared<S>>,

    /// Ping interval for detecting failed connections
    timeout: std::time::Duration,
}

impl<S: state::State> Connecting<S> {
    pub fn new(
        self_id: usize,
        stream: net::tcp::TcpStream,
        acceptor_tx: internal::Tx<acceptor::In<S::Command>>,
        shared_tx: Shared<S>,
        timeout: std::time::Duration,
    ) -> Self {
        let (peer_rx, peer_tx) = external::new(stream);
        Connecting {
            self_id,
            peer_rx: Some(peer_rx),
            peer_tx: Some(peer_tx),
            acceptor_tx: Some(acceptor_tx),
            shared_tx: Some(shared_tx),
            timeout,
        }
    }
}

impl<S: state::State> Future for Connecting<S> {
    type Item = Peer<S>;
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Async::Ready(Some(message)) = self.peer_rx.as_mut().unwrap().poll()?  {
            match message {
            | In::Ping(peer_id) => {
                // After we receiving a ping, we can read off the connected server's ID,
                // register it with the shared transmission hub, and promote it to a
                // Peer struct. Safe to unwrap here because we always initialize with Some
                // and always return after moving out of the option.
                info!("connected to {}", peer_id);
                let (rx, tx) = internal::new();
                self.shared_tx.as_mut()
                    .unwrap()
                    .write()
                    .connect_peer(peer_id, tx);
                return Ok(Async::Ready(Peer {
                    self_id: self.self_id,
                    peer_id,
                    rx,
                    peer_rx: self.peer_rx.take().unwrap(),
                    peer_tx: self.peer_tx.take().unwrap(),
                    acceptor_tx: self.acceptor_tx.take().unwrap(),
                    shared_tx: self.shared_tx.take().unwrap(),
                    timeout: tokio::timer::Interval::new_interval(self.timeout),
                }))
            }
            | _ => (),
            }
        }
        Ok(Async::NotReady)
    }
}

/// Represents a peer server with known ID that is registered
/// with the shared transmission hub.
pub struct Peer<S: state::State> {
    /// ID of connected server
    peer_id: usize,

    /// ID of this server
    self_id: usize,
    
    /// Internal receiving channel
    rx: internal::Rx<In<S::Command>>,

    /// External peer receiving channel
    peer_rx: external::Rx<In<S::Command>>,

    /// External peer transmitting channel
    peer_tx: external::Tx<In<S::Command>>,

    /// Internal acceptor transmitting channel
    acceptor_tx: internal::Tx<acceptor::In<S::Command>>,

    /// Internal shared transmitting channels
    shared_tx: Shared<S>,

    /// Ping interval for detecting failed connections
    timeout: tokio::timer::Interval,
}

impl<S: state::State> Peer<S> {
    pub fn new(
        self_id: usize,
        peer_id: usize,
        stream: net::tcp::TcpStream,
        acceptor_tx: internal::Tx<acceptor::In<S::Command>>,
        shared_tx: Shared<S>,
        timeout: std::time::Duration,
    ) -> Self {
        let (peer_rx, peer_tx) = external::new(stream);
        let (rx, tx) = internal::new();
        shared_tx.write().connect_peer(peer_id, tx);
        info!("connected to {}", peer_id);
        Peer {
            self_id,
            peer_id,
            peer_rx,
            peer_tx,
            acceptor_tx,
            shared_tx,
            timeout: tokio::timer::Interval::new_interval(timeout),
            rx,
        }
    }
}

impl<S: state::State> Peer<S> {
    /// Forward incoming messages to appropriate thread.
    fn respond_incoming(&self, message: In<S::Command>) {
        match message {
        | In::P1A(p1a)       => self.acceptor_tx.send(acceptor::In::P1A(p1a)),
        | In::P2A(c_id, p2a) => self.acceptor_tx.send(acceptor::In::P2A(c_id, p2a)),
        | message            => self.shared_tx.read().forward(message),
        }
    }
}

impl<S: state::State> Future for Peer<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {

        // Drop connections to unresponsive peers
        while let Async::Ready(Some(_)) = self.timeout.poll().map_err(|_| ())?  {
            self.peer_tx.start_send(In::Ping(self.self_id))?;
        }

        // Forward incoming messages
        while let Async::Ready(Some(message)) = self.peer_rx.poll()?  {
            if let In::Ping(_) = &message {} else {
                trace!("received {:?}", message);
                self.respond_incoming(message);
            }
        }

        // Forward outgoing messages
        while let Async::Ready(Some(message)) = self.rx.poll()? {
            trace!("sending {:?}", message);
            self.peer_tx.start_send(message)?;
        }

        // Complete sends
        if let Async::NotReady = self.peer_tx.poll_complete()? {
            return Ok(Async::NotReady)
        }

        Ok(Async::NotReady)
    }
}

impl<S: state::State> Drop for Peer<S> {
    fn drop(&mut self) {
        info!("disconnected from {}", self.peer_id);
        self.shared_tx.write().disconnect_peer(self.peer_id);
    }
}

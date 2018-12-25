use futures::sync::mpsc;
use serde_derive::{Serialize, Deserialize};
use tokio::prelude::*;
use tokio::net;

use crate::message;
use crate::shared::Shared;
use crate::state;
use crate::socket;
use crate::thread::*;

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

pub struct Connecting<S: state::State> {
    self_id: usize,
    peer_rx: Option<socket::Rx<In<S::Command>>>,
    peer_tx: Option<socket::Tx<In<S::Command>>>,
    acceptor_tx: Option<Tx<acceptor::In<S::Command>>>,
    shared_tx: Option<Shared<S>>,
    timeout: std::time::Duration,
}

impl<S: state::State> Connecting<S> {
    pub fn new(
        self_id: usize,
        stream: net::tcp::TcpStream,
        acceptor_tx: Tx<acceptor::In<S::Command>>,
        shared_tx: Shared<S>,
        timeout: std::time::Duration,
    ) -> Self {
        let (peer_rx, peer_tx) = socket::split(stream);
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
                info!("connected to {}", peer_id);
                let (tx, rx) = mpsc::unbounded();
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

pub struct Peer<S: state::State> {
    peer_id: usize,
    self_id: usize,
    rx: Rx<In<S::Command>>,
    peer_rx: socket::Rx<In<S::Command>>,
    peer_tx: socket::Tx<In<S::Command>>,
    acceptor_tx: Tx<acceptor::In<S::Command>>,
    shared_tx: Shared<S>,
    timeout: tokio::timer::Interval,
}

impl<S: state::State> Peer<S> {
    pub fn new(
        self_id: usize,
        peer_id: usize,
        stream: net::tcp::TcpStream,
        acceptor_tx: Tx<acceptor::In<S::Command>>,
        shared_tx: Shared<S>,
        timeout: std::time::Duration,
    ) -> Self {
        let (peer_rx, peer_tx) = socket::split(stream);
        let (tx, rx) = mpsc::unbounded();
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
    fn respond_incoming(&self, message: In<S::Command>) {
        match message {
        | In::P1A(p1a) => {
            self.acceptor_tx
                .unbounded_send(acceptor::In::P1A(p1a))
                .expect("[INTERNAL ERROR]: failed to send to acceptor");
        }
        | In::P2A(c_id, p2a) => {
            self.acceptor_tx
                .unbounded_send(acceptor::In::P2A(c_id, p2a))
                .expect("[INTERNAL ERROR]: failed to send to acceptor");
        }
        | In::P1B(p1b) => {
            self.shared_tx
                .read()
                .send_scout(p1b);
        }
        | In::P2B(c_id, p2b) => {
            self.shared_tx
                .read()
                .send_commander(c_id, p2b);
        }
        | In::Decision(proposal) => {
            self.shared_tx
                .read()
                .send_replica(replica::In::Decision(proposal));
        }
        | In::Ping(_) => (),
        }
    }
}

impl<S: state::State> Future for Peer<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {

        // Drop connections to unresponsive peers
        while let Async::Ready(Some(_)) = self.timeout.poll().map_err(|_| ())?  {
            self.peer_tx.start_send(In::Ping(self.self_id)).map_err(|_| ())?;
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
            self.peer_tx.start_send(message).map_err(|_| ())?;
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

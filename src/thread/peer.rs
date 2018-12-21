use futures::sync::mpsc;
use serde_derive::{Serialize, Deserialize};
use tokio::prelude::*;
use tokio_serde_bincode::WriteBincode;
use tokio::{codec, net};

use crate::message;
use crate::shared::Shared;
use crate::state;
use crate::thread::*;

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub enum In<C: state::Command> {
    P1A(message::P1A),
    P1B(message::P1B<C>),
    P2A(message::CommanderID, message::P2A<C>),
    P2B(message::CommanderID, message::P2B),
    Ping(usize),
}

pub struct Connecting<S: state::State> {
    self_id: usize,
    peer_rx: SocketRx<In<S::Command>>,
    peer_tx: SocketTx,
    acceptor_tx: Tx<acceptor::In<S::Command>>,
    shared_tx: Shared<S>,
}

impl<S: state::State> Connecting<S> {

    pub fn new(
        self_id: usize,
        stream: net::tcp::TcpStream,
        acceptor_tx: Tx<acceptor::In<S::Command>>,
        shared_tx: Shared<S>,
    ) -> Self {
        let (peer_rx, peer_tx) = stream.split();
        let peer_rx = ReadBincode::new(
            codec::length_delimited::Builder::new()
                .new_read(peer_rx)
                .from_err::<bincode::Error>()
        );
        let peer_tx = codec::length_delimited::Builder::new()
            .new_write(peer_tx)
            .sink_from_err::<bincode::Error>();
        Connecting {
            self_id,
            peer_rx,
            peer_tx,
            acceptor_tx,
            shared_tx,
        }
    }

    pub async fn run(mut self) -> Peer<S> {
        loop {
            while let Some(Ok(message)) = await!(self.peer_rx.next()) {
                match message {
                | In::Ping(peer_id) => {
                    let (tx, rx) = mpsc::unbounded();
                    self.shared_tx.write().connect_peer(peer_id, tx);
                    return Peer {
                        self_id: self.self_id,
                        peer_id,
                        rx,
                        peer_rx: self.peer_rx,
                        peer_tx: self.peer_tx,
                        acceptor_tx: self.acceptor_tx,
                        shared_tx: self.shared_tx,
                        ping: tokio::timer::Interval::new_interval(
                            std::time::Duration::from_millis(500)
                        ),
                    }
                }
                | _ => (),
                }
            }
        }
    }
}

pub struct Peer<S: state::State> {
    peer_id: usize,
    self_id: usize,
    rx: Rx<In<S::Command>>,
    peer_rx: SocketRx<In<S::Command>>,
    peer_tx: SocketTx,
    acceptor_tx: Tx<acceptor::In<S::Command>>,
    shared_tx: Shared<S>,
    ping: tokio::timer::Interval,
}

impl<S: state::State> Peer<S> {

    pub async fn run(mut self) {
        loop {
            // Drop connection to unresponsive peers
            while let Some(_) = await!(self.ping.next()) {
                if let Err(_) = self.send(In::Ping(self.self_id)) {
                    return
                }
            }

            while let Some(Ok(message)) = await!(self.peer_rx.next()) {
                self.respond_incoming(message);
            }

            while let Some(Ok(message)) = await!(self.rx.next()) {
                if let Err(_) = self.send(message) {
                    return
                }
            }
        }
    }

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
            self.shared_tx.read().send_scout(p1b);
        }
        | In::P2B(c_id, p2b) => {
            self.shared_tx.read().send_commander(c_id, p2b);
        }
        | In::Ping(_) => (),
        }
    }

    fn send(&mut self, message: In<S::Command>) -> Result<(), ()> {
        WriteBincode::new(&mut self.peer_tx)
            .send(message)
            .wait()
            .map(|_| ())
            .map_err(|_| ())
    }
}

impl<S: state::State> Drop for Peer<S> {
    fn drop(&mut self) {
        self.shared_tx.write().disconnect_peer(self.peer_id);
    }
}

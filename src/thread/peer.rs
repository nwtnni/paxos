use futures::sync::mpsc;
use serde_derive::{Serialize, Deserialize};
use tokio::prelude::*;
use tokio_serde_bincode::WriteBincode;

use crate::message;
use crate::shared::Shared;
use crate::state;
use crate::thread::*;

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub enum In<I> {
    P1A(message::P1A),
    P1B(message::P1B<I>),
    P2A(commander::ID, message::P2A<I>),
    P2B(commander::ID, message::P2B),
    Ping(usize),
}

pub struct Connecting<I: state::CommandID> {
    self_id: usize,
    peer_rx: SocketRx<In<I>>,
    peer_tx: SocketTx,
    acceptor_tx: Tx<acceptor::In<I>>,
    shared_tx: Shared<I>,
}

impl<I: state::CommandID> Connecting<I> {

    pub fn new(
        self_id: usize,
        peer_rx: SocketRx<In<I>>,
        peer_tx: SocketTx,
        acceptor_tx: Tx<acceptor::In<I>>,
        shared_tx: Shared<I>,
    ) -> Self {
        Connecting {
            self_id,
            peer_rx,
            peer_tx,
            acceptor_tx,
            shared_tx,
        }
    }

    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.peer_rx.next()) {
                match message {
                | In::Ping(peer_id) => {
                    let (tx, rx) = mpsc::unbounded();
                    self.shared_tx.write().connect_peer(peer_id, tx);
                    let peer = Peer {
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
                    };
                    
                    tokio::spawn_async(async {
                        peer.run();
                    });

                    return
                }
                | _ => (),
                }
            }
        }
    }
}

pub struct Peer<I: state::CommandID> {
    peer_id: usize,
    self_id: usize,
    rx: Rx<In<I>>,
    peer_rx: SocketRx<In<I>>,
    peer_tx: SocketTx,
    acceptor_tx: Tx<acceptor::In<I>>,
    shared_tx: Shared<I>,
    ping: tokio::timer::Interval,
}

impl<I: state::CommandID> Peer<I> {

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

    fn respond_incoming(&self, message: In<I>) {
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

    fn send(&mut self, message: In<I>) -> Result<(), ()> {
        WriteBincode::new(&mut self.peer_tx)
            .send(message)
            .wait()
            .map(|_| ())
            .map_err(|_| ())
    }
}

impl<I: state::CommandID> Drop for Peer<I> {
    fn drop(&mut self) {
        self.shared_tx.write().disconnect_peer(self.peer_id);
    }
}

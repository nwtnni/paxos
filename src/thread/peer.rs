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
    Ping,
}

pub struct Peer<I> {
    rx: Rx<In<I>>,
    peer_rx: SocketRx<In<I>>,
    peer_tx: SocketTx,
    acceptor_tx: Tx<acceptor::In<I>>,
    shared: Shared<I>,
    ping: tokio::timer::Interval,
}

impl<I: state::Identifier> Peer<I> {

    pub async fn run(mut self) {
        loop {
            // Drop connection to unresponsive peers
            while let Some(_) = await!(self.ping.next()) {
                if let Err(_) = self.send(In::Ping) {
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
            self.shared.read().send_scout(p1b);
        }
        | In::P2B(c_id, p2b) => {
            self.shared.read().send_commander(c_id, p2b);
        }
        | In::Ping => (),
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

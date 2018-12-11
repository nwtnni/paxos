use std::marker;
use std::collections::HashMap as Map;

use tokio::prelude::*;

use crate::message;
use crate::state;
use crate::thread::*;

pub type In<O> = message::Proposal<O>;

pub struct Replica<O, R, S> {
    client_rx: SocketRx<O>,
    client_tx: SocketTx<R>,
    leader_tx: Tx<leader::In<O>>,
    rx: Rx<In<O>>,
    state: S,
    slot: usize,
    proposals: Map<O, usize>,
    decisions: Map<O, usize>,  
}

impl<O: state::Operation + marker::Unpin, R: state::Response, S: state::State<O, R>> Replica<O, R, S> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(op)) = await!(self.client_rx.next()) {
                self.propose(op);
            }

            while let Some(Ok(decision)) = await!(self.rx.next()) {

            }
        }
    }

    fn propose(&mut self, op: O) {
        if self.decisions.contains_key(&op) { return }

        let next = 1 + std::cmp::max(
            self.proposals.values().max().unwrap_or(&0),
            self.decisions.values().max().unwrap_or(&0),
        );

        self.proposals.insert(op.clone(), next);

        let proposal = leader::In::Propose(message::Proposal {
            s_id: next,
            op: op,
        });

        self.leader_tx.unbounded_send(proposal)
            .expect("[INTERNAL ERROR]: failed to send proposal");
    }

}

use std::marker;
use std::collections::HashMap as Map;

use tokio::prelude::*;

use crate::message;
use crate::state;
use crate::thread::*;

pub type In<C> = message::Proposal<C>;

pub struct Replica<C: state::Command, R, S> {
    client_rx: SocketRx<C>,
    client_tx: SocketTx<R>,
    leader_tx: Tx<leader::In<C::ID>>,
    rx: Rx<In<C>>,
    state: S,
    slot: usize,
    proposals: Map<C::ID, usize>,
    decisions: Map<C::ID, usize>,  
}

impl<C: state::Command, R: state::Response, S: state::State<C, R>> Replica<C, R, S> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(op)) = await!(self.client_rx.next()) {
                self.propose(op.id());
            }

            while let Some(Ok(decision)) = await!(self.rx.next()) {

            }
        }
    }

    fn propose(&mut self, c_id: C::ID) {
        if self.decisions.contains_key(&c_id) { return }

        let next = 1 + std::cmp::max(
            self.proposals.values().max().unwrap_or(&0),
            self.decisions.values().max().unwrap_or(&0),
        );

        self.proposals.insert(c_id.clone(), next);

        let proposal = leader::In::Propose(message::Proposal {
            s_id: next,
            c_id: c_id,
        });

        self.leader_tx.unbounded_send(proposal)
            .expect("[INTERNAL ERROR]: failed to send proposal");
    }

}

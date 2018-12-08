use std::time;

use hashbrown::HashMap as Map;
use futures::sync::mpsc;
use tokio::prelude::*;

use crate::message;
use crate::thread::{Tx, Rx};
use crate::thread::{commander, replica, scout};
use crate::shared;
use crate::state;

#[derive(Clone, Debug)]
pub enum In<O> {
    Propose(message::Proposal<O>),
    Preempt(message::BallotID),
    Adopt(message::BallotID, Vec<message::PValue<O>>),
    Decide(commander::ID, message::Proposal<O>),
}

pub struct Leader<O> {
    id: usize,
    count: usize,
    self_rx: Rx<In<O>>,
    self_tx: Tx<In<O>>,
    replica_tx: Tx<replica::In<O>>,
    scout_tx: Option<Tx<scout::In<O>>>,
    commander_txs: Map<commander::ID, Tx<commander::In>>,
    peer_txs: shared::Shared<O>,
    active: bool,
    ballot: message::BallotID,
    backoff: time::Duration,
    proposals: Map<usize, message::Proposal<O>>,
}

impl<O: state::Operation> Leader<O> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.self_rx.next()) {
                match message {
                | In::Propose(proposal) => {

                }
                | In::Preempt(ballot) => {

                }
                | In::Adopt(ballot, pvalues) => {

                }
                | In::Decide(commander, proposal) => self.send_decide(commander, proposal),
                }
            }
        }
    }

    fn send_decide(&mut self, commander: commander::ID, proposal: message::Proposal<O>) {
        let decide = replica::In::Decide(proposal);
        self.commander_txs.remove(&commander); 
        self.replica_tx
            .unbounded_send(decide)
            .expect("[INTERNAL ERROR]: failed to send decision");
    }

    async fn spawn_commander(&mut self, ballot: message::BallotID, proposal: message::Proposal<O>) {
        let id = (ballot, proposal.s_id);
        let pvalue = message::PValue {
            s_id: proposal.s_id, 
            b_id: ballot,
            op: proposal.op,
        };
        let (commander, commander_tx) = commander::Commander::new(
            self.self_tx.clone(),
            self.peer_txs.clone(),
            pvalue,
            self.count,
        );
        self.commander_txs.insert(id, commander_tx);
        await!(commander.run());
    }

    async fn spawn_scout(&mut self) {
        let (scout, scout_tx) = scout::Scout::new(
            self.self_tx.clone(),
            self.peer_txs.clone(),
            self.ballot,
            self.count,
            self.backoff,
        );
        self.scout_tx = Some(scout_tx);
        await!(scout.run());
    }
}

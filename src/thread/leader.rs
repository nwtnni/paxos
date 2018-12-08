use std::time;

use hashbrown::HashMap as Map;
use tokio::prelude::*;

use crate::message;
use crate::thread::{Tx, Rx};
use crate::thread::{commander, replica, scout};
use crate::shared;
use crate::state;

#[derive(Clone, Debug)]
pub enum In<O> {
    Propose(message::Proposal<O>),
    CommanderPreempt(commander::ID, message::BallotID),
    ScoutPreempt(message::BallotID),
    Adopt(Vec<message::PValue<O>>),
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
    proposals: Map<O, usize>,
}

impl<O: state::Operation> Leader<O> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.self_rx.next()) {
                match message {
                | In::Propose(proposal) => {
                    self.respond_propose(proposal);
                }
                | In::ScoutPreempt(ballot) => {
                    self.respond_scout_preempt(ballot);
                }
                | In::Adopt(pvalues) => {
                    self.respond_adopt(pvalues);
                }
                | In::CommanderPreempt(commander, ballot) => {
                    self.respond_commander_preempt(commander, ballot);
                }
                | In::Decide(commander, proposal) => {
                    self.respond_decide(commander, proposal);
                }
                }
            }
        }
    }

    fn respond_propose(&mut self, proposal: message::Proposal<O>) {
        if self.proposals.contains_key(&proposal.op) {
            return
        }
        self.proposals.insert(proposal.op.clone(), proposal.s_id);
        if self.active {
            self.spawn_commander(proposal);
        }
    }

    fn respond_scout_preempt(&mut self, ballot: message::BallotID) {
        if ballot < self.ballot { return }
        self.active = false;
        self.ballot = message::BallotID {
            b_id: ballot.b_id + 1,
            l_id: self.id,
        };
        self.spawn_scout();
    }

    fn respond_adopt(&mut self, pvalues: Vec<message::PValue<O>>) {
        for (op, s_id) in Self::pmax(pvalues) {
            self.proposals.insert(op, s_id);
        }
        let proposals = std::mem::replace(
            &mut self.proposals,
            Map::with_capacity(0)
        );
        for (op, s_id) in &proposals {
            let proposal = message::Proposal {
                s_id: s_id.clone(),
                op: op.clone(),
            };
            self.spawn_commander(proposal);
        }
        self.proposals = proposals;
        self.active = true;
    }

    fn respond_commander_preempt(&mut self, commander: commander::ID, ballot: message::BallotID) {
        self.commander_txs.remove(&commander);
        self.respond_scout_preempt(ballot);
    }

    fn respond_decide(&mut self, commander: commander::ID, proposal: message::Proposal<O>) {
        let decide = replica::In::Decide(proposal);
        self.commander_txs.remove(&commander);
        self.replica_tx
            .unbounded_send(decide)
            .expect("[INTERNAL ERROR]: failed to send decision");
    }

    fn pmax<I>(pvalues: I) -> impl Iterator<Item = (O, usize)>
        where I: IntoIterator<Item = message::PValue<O>> {
        let mut pmax: Map<usize, (message::BallotID, O)> = Map::default();
        for pvalue in pvalues.into_iter() {
            pmax.entry(pvalue.s_id)
                .and_modify(|(b_id, op)| {
                    if pvalue.b_id > *b_id {
                        *b_id = pvalue.b_id;
                        *op = pvalue.op;
                    }
                });
        }
        pmax.into_iter().map(|(s_id, (_, op))| (op, s_id))
    }

    fn spawn_commander(&mut self, proposal: message::Proposal<O>) {
        let id = (self.ballot, proposal.s_id);
        let pvalue = message::PValue {
            s_id: proposal.s_id,
            b_id: self.ballot,
            op: proposal.op,
        };
        let (commander, commander_tx) = commander::Commander::new(
            self.self_tx.clone(),
            self.peer_txs.clone(),
            pvalue,
            self.count,
        );
        self.commander_txs.insert(id, commander_tx);
        tokio::spawn_async(async move {
            commander.run();
        })
    }

    fn spawn_scout(&mut self) {
        let (scout, scout_tx) = scout::Scout::new(
            self.self_tx.clone(),
            self.peer_txs.clone(),
            self.ballot,
            self.count,
            self.backoff,
        );
        self.scout_tx = Some(scout_tx);
        tokio::spawn_async(async move {
            scout.run();
        })
    }
}

use std::time;

use hashbrown::HashMap as Map;
use tokio::prelude::*;

use crate::message;
use crate::thread::{Tx, Rx};
use crate::thread::{commander, replica, scout};
use crate::shared;
use crate::state;

#[derive(Clone, Debug)]
pub enum In<I> {
    Propose(message::Proposal<I>),
    Preempt(message::BallotID),
    Adopt(Vec<message::PValue<I>>),
}

pub struct Leader<I> {
    id: usize,
    count: usize,
    self_rx: Rx<In<I>>,
    self_tx: Tx<In<I>>,
    tx: shared::Shared<I>,
    active: bool,
    ballot: message::BallotID,
    backoff: time::Duration,
    proposals: Map<I, usize>,
}

impl<I: state::Identifier> Leader<I> {

    pub fn new(id: usize, count: usize, self_rx: Rx<In<I>>, self_tx: Tx<In<I>>, tx: shared::Shared<I>) -> Self {
        Leader {
            id,
            count,
            self_rx,
            self_tx,
            tx,
            active: false,
            ballot: message::BallotID {
                b_id: 1,
                l_id: id,
            },
            backoff: time::Duration::from_millis(50),
            proposals: Map::default(),
        }
    }

    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.self_rx.next()) {
                match message {
                | In::Propose(proposal) => self.respond_propose(proposal),
                | In::Preempt(ballot) => self.respond_preempt(ballot),
                | In::Adopt(pvalues) => self.respond_adopt(pvalues),
                }
            }
        }
    }

    fn respond_propose(&mut self, proposal: message::Proposal<I>) {
        if self.proposals.contains_key(&proposal.c_id) {
            return
        }
        self.proposals.insert(proposal.c_id.clone(), proposal.s_id);
        if self.active {
            self.spawn_commander(proposal);
        }
    }

    fn respond_preempt(&mut self, ballot: message::BallotID) {
        if ballot < self.ballot { return }
        self.active = false;
        self.ballot = message::BallotID {
            b_id: ballot.b_id + 1,
            l_id: self.id,
        };
        self.spawn_scout();
    }

    fn respond_adopt(&mut self, pvalues: Vec<message::PValue<I>>) {
        for (op, s_id) in Self::pmax(pvalues) {
            self.proposals.insert(op, s_id);
        }
        let proposals = std::mem::replace(
            &mut self.proposals,
            Map::with_capacity(0)
        );
        for (c_id, s_id) in &proposals {
            let proposal = message::Proposal {
                s_id: s_id.clone(),
                c_id: c_id.clone(),
            };
            self.spawn_commander(proposal);
        }
        self.proposals = proposals;
        self.active = true;
    }

    fn pmax<T>(pvalues: T) -> impl Iterator<Item = (I, usize)>
        where T: IntoIterator<Item = message::PValue<I>> {
        let mut pmax: Map<usize, (message::BallotID, I)> = Map::default();
        for pvalue in pvalues.into_iter() {
            pmax.entry(pvalue.s_id)
                .and_modify(|(b_id, c_id)| {
                    if pvalue.b_id > *b_id {
                        *b_id = pvalue.b_id;
                        *c_id = pvalue.c_id;
                    }
                });
        }
        pmax.into_iter().map(|(s_id, (_, op))| (op, s_id))
    }

    fn spawn_commander(&mut self, proposal: message::Proposal<I>) {
        let id = (self.ballot, proposal.s_id);
        let pvalue = message::PValue {
            s_id: proposal.s_id,
            b_id: self.ballot,
            c_id: proposal.c_id,
        };
        let commander = commander::Commander::new(
                self.self_tx.clone(),
                self.tx.clone(),
                pvalue,
                self.count
        );
        tokio::spawn_async(async move {
            commander.run();
        })
    }

    fn spawn_scout(&mut self) {
        let scout = scout::Scout::new(
            self.self_tx.clone(),
            self.tx.clone(),
            self.ballot,
            self.count,
            self.backoff,
        );
        tokio::spawn_async(async move {
            scout.run();
        })
    }
}

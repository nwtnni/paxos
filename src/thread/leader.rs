use std::time;

use hashbrown::HashMap as Map;
use tokio::prelude::*;

use crate::message;
use crate::thread::{Tx, Rx};
use crate::thread::{commander, scout};
use crate::shared;
use crate::state;

pub enum In<C: state::Command> {
    Propose(message::Proposal<C>),
    Preempt(message::BallotID),
    Adopt(Vec<message::PValue<C>>),
}

pub struct Leader<S: state::State> {
    id: usize,
    count: usize,
    self_rx: Rx<In<S::Command>>,
    self_tx: Tx<In<S::Command>>,
    shared_tx: shared::Shared<S>,
    active: bool,
    ballot: message::BallotID,
    backoff: time::Duration,
    proposals: Map<message::CommandID<S::Command>, usize>,
}

impl<S: state::State> Leader<S> {

    pub fn new(
        id: usize,
        count: usize,
        self_rx: Rx<In<S::Command>>,
        self_tx: Tx<In<S::Command>>,
        shared_tx: shared::Shared<S>,
    ) -> Self {
        Leader {
            id,
            count,
            self_rx,
            self_tx,
            shared_tx,
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

    fn respond_propose(&mut self, proposal: message::Proposal<S::Command>) {
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

    fn respond_adopt(&mut self, pvalues: Vec<message::PValue<S::Command>>) {
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

    fn pmax<I>(pvalues: I) -> impl Iterator<Item = (message::CommandID<S::Command>, usize)>
        where I: IntoIterator<Item = message::PValue<S::Command>> {
        let mut pmax: Map<usize, (message::BallotID, message::CommandID<S::Command>)> = Map::default();
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

    fn spawn_commander(&mut self, proposal: message::Proposal<S::Command>) {
        let id = (self.ballot, proposal.s_id);
        let pvalue = message::PValue {
            s_id: proposal.s_id,
            b_id: self.ballot,
            c_id: proposal.c_id,
        };
        let commander = commander::Commander::new(
                self.self_tx.clone(),
                self.shared_tx.clone(),
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
            self.shared_tx.clone(),
            self.ballot,
            self.count,
            self.backoff,
        );
        tokio::spawn_async(async move {
            scout.run();
        })
    }
}

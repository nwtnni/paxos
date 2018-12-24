use std::time;

use bimap::BiMap;
use hashbrown::HashMap as Map;
use tokio::prelude::*;

use crate::message;
use crate::thread::{Tx, Rx};
use crate::thread::{commander, scout};
use crate::shared;
use crate::state;

#[derive(Debug)]
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
    proposals: BiMap<usize, message::Command<S::Command>>,
    backoff: f32,
    timeout: time::Duration,
}

impl<S: state::State> Leader<S> {

    pub fn new(
        id: usize,
        count: usize,
        self_rx: Rx<In<S::Command>>,
        self_tx: Tx<In<S::Command>>,
        shared_tx: shared::Shared<S>,
        timeout: time::Duration,
    ) -> Self {
        let leader = Leader {
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
            proposals: BiMap::default(),
            backoff: 100.0 * rand::random::<f32>(),
            timeout,
        };
        leader.spawn_scout();
        leader
    }

    fn respond_propose(&mut self, proposal: message::Proposal<S::Command>) {
        info!("Responding to proposal {:?} with proposals {:?}", proposal, self.proposals);
        if self.proposals.contains_right(&proposal.command) {
            return
        }
        self.proposals.insert(proposal.s_id, proposal.command.clone());
        if self.active {
            self.spawn_commander(proposal);
        }
    }

    fn respond_preempt(&mut self, ballot: message::BallotID) {
        self.active = false;
        self.ballot = message::BallotID {
            b_id: ballot.b_id + 1,
            l_id: self.id,
        };
        self.backoff *= 1.0 + rand::random::<f32>() / 2.0;
        info!("backoff: {:?}", self.backoff);
        self.spawn_scout();
    }

    fn respond_adopt(&mut self, pvalues: Vec<message::PValue<S::Command>>) {
        let proposals = std::mem::replace(
            &mut self.proposals,
            Self::pmax(pvalues).collect(),
        );

        for (s_id, command) in proposals {
            if !self.proposals.contains_left(&s_id) {
                self.proposals.insert(s_id, command);
            }
        }
        info!("{:?}", self.proposals);

        for (s_id, command) in &self.proposals {
            let proposal = message::Proposal {
                s_id: s_id.clone(),
                command: command.clone(),
            };
            self.spawn_commander(proposal);
        }

        self.active = true;
    }

    fn pmax<I>(pvalues: I) -> impl Iterator<Item = (usize, message::Command<S::Command>)>
        where I: IntoIterator<Item = message::PValue<S::Command>> {
        let mut pmax: Map<usize, (message::BallotID, message::Command<S::Command>)> = Map::default();
        for pvalue in pvalues.into_iter() {
            if let Some((b_id, command)) = pmax.get_mut(&pvalue.s_id) {
                if pvalue.b_id > *b_id {
                    *b_id = pvalue.b_id;
                    *command = pvalue.command;
                }
            } else {
                pmax.insert(pvalue.s_id, (pvalue.b_id, pvalue.command));
            }
        }
        pmax.into_iter().map(|(s_id, (_, command))| (s_id, command))
    }

    fn spawn_commander(&self, proposal: message::Proposal<S::Command>) {
        let pvalue = message::PValue {
            s_id: proposal.s_id,
            b_id: self.ballot,
            command: proposal.command,
        };
        let commander = commander::Commander::new(
            self.self_tx.clone(),
            self.shared_tx.clone(),
            pvalue,
            self.count,
            self.timeout,
        );
        tokio::spawn(commander);
    }

    fn spawn_scout(&self) {
        let scout = scout::Scout::new(
            self.self_tx.clone(),
            self.shared_tx.clone(),
            self.ballot,
            self.count,
            std::time::Duration::from_millis(self.backoff.round() as u64),
            self.timeout,
        );
        tokio::spawn(scout);
    }
}

impl<S: state::State> Future for Leader<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Async::Ready(Some(message)) = self.self_rx.poll()? {
            info!("received {:?}", message);
            match message {
            | In::Propose(proposal) => self.respond_propose(proposal),
            | In::Preempt(ballot) => self.respond_preempt(ballot),
            | In::Adopt(pvalues) => self.respond_adopt(pvalues),
            }
        }
        Ok(Async::NotReady)
    }
}

use std::collections::HashMap as Map;
use std::time;

use serde_derive::{Serialize, Deserialize};
use tokio::prelude::*;

use crate::message;
use crate::thread::{Tx, Rx};
use crate::thread::{commander, scout};
use crate::shared;
use crate::state;
use crate::storage;

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
    backoff: f32,
    stable: Stable<S>,
    storage: storage::Storage<Stable<S>>,
    timeout: time::Duration,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
struct Stable<S: state::State> {
    ballot: message::BallotID,
    proposals: Map<usize, message::Command<S::Command>>,
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
        let storage = storage::Storage::new(format!("leader-{:>02}.paxos", id));
        let stable = storage.load()
            .unwrap_or(Stable {
                ballot: message::BallotID { b_id: 1, l_id: id }, 
                proposals: Map::default(), 
            });

        let leader = Leader {
            id,
            count,
            self_rx,
            self_tx,
            shared_tx,
            active: false,
            backoff: 100.0 * rand::random::<f32>(),
            storage,
            stable,
            timeout,
        };
        leader.spawn_scout();
        leader
    }

    fn respond_propose(&mut self, proposal: message::Proposal<S::Command>) {
        if self.stable.proposals.contains_key(&proposal.s_id) {
            return
        }
        debug!("{:?} proposed", proposal);
        self.stable.proposals.insert(proposal.s_id, proposal.command.clone());
        self.storage.save(&self.stable);
        if self.active {
            self.spawn_commander(proposal);
        }
    }

    fn respond_preempt(&mut self, ballot: message::BallotID) {
        if ballot <= self.stable.ballot { return }
        debug!("preempted by {:?}", ballot);
        self.active = false;
        self.stable.ballot = message::BallotID {
            b_id: ballot.b_id + 1,
            l_id: self.id,
        };
        self.storage.save(&self.stable);
        self.backoff *= 1.0 + rand::random::<f32>() / 2.0;
        self.spawn_scout();
    }

    fn respond_adopt(&mut self, pvalues: Vec<message::PValue<S::Command>>) {
        let proposals = std::mem::replace(
            &mut self.stable.proposals,
            Self::pmax(pvalues).collect(),
        );

        for (s_id, command) in proposals {
            if !self.stable.proposals.contains_key(&s_id) {
                self.stable.proposals.insert(s_id, command);
            }
        }

        self.storage.save(&self.stable);

        for (s_id, command) in &self.stable.proposals {
            let proposal = message::Proposal {
                s_id: s_id.clone(),
                command: command.clone(),
            };
            self.spawn_commander(proposal);
        }

        info!("adopted with ballot {:?}", self.stable.ballot);
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
            b_id: self.stable.ballot,
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
            self.stable.ballot,
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
            debug!("received {:?}", message);
            match message {
            | In::Propose(proposal) => self.respond_propose(proposal),
            | In::Preempt(ballot) => self.respond_preempt(ballot),
            | In::Adopt(pvalues) => self.respond_adopt(pvalues),
            }
        }
        Ok(Async::NotReady)
    }
}

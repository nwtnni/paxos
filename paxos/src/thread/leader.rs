//! # Summary
//!
//! This module defines the `Leader` struct, which is responsible
//! for vying for a majority of acceptors with a scout, and then 
//! proposing commands to them via commanders.

use std::collections::HashMap as Map;
use std::time;

use serde_derive::{Serialize, Deserialize};
use tokio::prelude::*;

use crate::internal;
use crate::message;
use crate::thread::{commander, scout};
use crate::shared;
use crate::state;
use crate::storage;

/// Leaders can only receive proposals from replicas,
/// preempts from scouts or commanders, and adopts
/// from scouts.
#[derive(Debug)]
pub enum In<C: state::Command> {
    Propose(message::Proposal<C>),
    Preempt(message::Ballot),
    Adopt(Vec<message::PValue<C>>),
    Decide(usize),
}

/// Functions as invariant-upholding command proposer.
pub struct Leader<S: state::State> {
    /// Unique ID of leader
    id: usize,

    /// Internal receiving channel
    rx: internal::Rx<In<S::Command>>,

    /// Internal transmitting channel
    tx: internal::Tx<In<S::Command>>,

    /// Internal shared transmitting channels
    shared_tx: shared::Shared<S>,

    /// Adopted by a majority of acceptors and ready to spawn commanders
    active: bool,

    /// Exponential backoff for spawning new scouts after preempts
    backoff: f32,

    /// Total number of acceptors
    count: usize,

    /// Time for scouts and commanders to wait before resending their messages
    timeout: time::Duration,

    /// Persistent leader state across failures
    stable: Stable<S>,

    /// Backing store for stable storage
    storage: storage::Storage<Stable<S>>,
}

/// Leaders need to keep track of their current ballot and the
/// proposals they plan to propose to acceptors.
#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
struct Stable<S: state::State> {
    /// Leader's current ballot
    ballot: message::Ballot,

    /// Planned proposals
    proposals: Map<usize, message::Command<S::Command>>,

    /// Latest known decision
    decided: Option<usize>,
}

impl<S: state::State> Leader<S> {

    pub fn new(
        id: usize,
        count: usize,
        rx: internal::Rx<In<S::Command>>,
        tx: internal::Tx<In<S::Command>>,
        shared_tx: shared::Shared<S>,
        timeout: time::Duration,
    ) -> Self {
        let storage_file = format!("leader-{:>02}.paxos", id);
        let storage = storage::Storage::new(storage_file);
        let stable = storage.load()
            .unwrap_or(Stable {
                ballot: message::Ballot { b_id: 1, l_id: id }, 
                proposals: Map::default(), 
                decided: None,
            });
        let leader = Leader {
            id,
            count,
            rx,
            tx,
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

    /// Add a new proposal to the map. Directly spawn commander for it if
    /// we're already active.
    fn respond_propose(&mut self, proposal: message::Proposal<S::Command>) {
        if self.stable.proposals.contains_key(&proposal.s_id)
        || self.stable.decided.is_some() && self.stable.decided.unwrap() >= proposal.s_id {
            return
        }
        debug!("{:?} proposed", proposal);
        self.stable.proposals.insert(proposal.s_id, proposal.command.clone());
        self.storage.save(&self.stable);
        if self.active { self.spawn_commander(proposal) }
    }

    /// Update current ballot to out-compete preempted ballot. Apply
    /// exponential backoff before spawning new scout.
    fn respond_preempt(&mut self, ballot: message::Ballot) {
        if ballot <= self.stable.ballot { return }
        debug!("preempted by {:?}", ballot);
        self.active = false;
        self.stable.ballot = message::Ballot {
            b_id: ballot.b_id + 1,
            l_id: self.id,
        };
        self.storage.save(&self.stable);
        self.backoff *= 1.0 + rand::random::<f32>() / 2.0;
        self.spawn_scout();
    }

    /// Update proposal map to uphold Paxos invariants before spawning commanders
    /// for each proposal.
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
            if self.stable.decided.is_some()
            && *s_id <= self.stable.decided.unwrap() {
                continue
            }
            let proposal = message::Proposal {
                s_id: s_id.clone(),
                command: command.clone(),
            };
            self.spawn_commander(proposal);
        }

        info!("adopted with ballot {:?}", self.stable.ballot);
        self.active = true;
    }

    /// Mark a decided slot to reduce P1B message size and avoid spawning
    /// redundant commanders for already known decisions.
    fn respond_decide(&mut self, s_id: usize) {
        self.stable.decided = match self.stable.decided {
        | None                             => Some(s_id),
        | Some(decided) if s_id >= decided => Some(s_id),
        | Some(decided)                    => Some(decided),
        };
        self.stable.proposals.remove(&s_id);
        self.storage.save(&self.stable);
    }

    /// Calculate the most recently accepted commands for each slot to
    /// uphold Paxos invariants.
    fn pmax<I>(pvalues: I) -> impl Iterator<Item = (usize, message::Command<S::Command>)>
        where I: IntoIterator<Item = message::PValue<S::Command>> {
        let mut pmax: Map<usize, (message::Ballot, message::Command<S::Command>)> = Map::default();
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

    /// Spawn a new commander thread for the given proposal.
    fn spawn_commander(&self, proposal: message::Proposal<S::Command>) {
        let pvalue = message::PValue {
            s_id: proposal.s_id,
            b_id: self.stable.ballot,
            command: proposal.command,
        };
        let commander = commander::Commander::new(
            self.tx.clone(),
            self.shared_tx.clone(),
            pvalue,
            self.count,
            self.timeout,
        );
        tokio::spawn(commander);
    }

    /// Spawn a new scout thread for the current ballot.
    fn spawn_scout(&self) {
        let scout = scout::Scout::new(
            self.tx.clone(),
            self.shared_tx.clone(),
            self.stable.ballot,
            self.count,
            self.stable.decided,
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
        while let Async::Ready(Some(message)) = self.rx.poll()? {
            debug!("received {:?}", message);
            match message {
            | In::Propose(proposal) => self.respond_propose(proposal),
            | In::Preempt(ballot) => self.respond_preempt(ballot),
            | In::Adopt(pvalues) => self.respond_adopt(pvalues),
            | In::Decide(s_id) => self.respond_decide(s_id),
            }
        }
        Ok(Async::NotReady)
    }
}

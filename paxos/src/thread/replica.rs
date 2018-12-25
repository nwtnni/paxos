use std::collections::HashMap as Map;

use serde_derive::{Serialize, Deserialize};
use tokio::prelude::*;

use crate::message;
use crate::shared;
use crate::state;
use crate::state::Command;
use crate::storage;
use crate::thread::*;

#[derive(Debug)]
pub enum In<C: state::Command> {
    Request(C),
    Decision(message::Proposal<C>),
}

pub struct Replica<S: state::State> {
    rx: Rx<In<S::Command>>,
    leader_tx: Tx<leader::In<S::Command>>,
    shared_tx: shared::Shared<S>,
    stable: Stable<S>,
    storage: storage::Storage<Stable<S>>,
    state: S,
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
#[derive(Derivative)]
#[derivative(Default(bound = ""))]
struct Stable<S: state::State> {
    proposal_slot: usize,
    decision_slot: usize,
    proposals: Map<usize, message::Command<S::Command>>,
    decisions: Map<usize, message::Command<S::Command>>,
}

impl<S: state::State> Replica<S> {
    pub fn new(
        id: usize,
        leader_tx: Tx<leader::In<S::Command>>,
        shared_tx: shared::Shared<S>,
        rx: Rx<In<S::Command>>,
    ) -> Self {
        let storage_file = format!("replica-{:>02}.paxos", id);
        let storage: storage::Storage<Stable<S>> = storage::Storage::new(storage_file);
        let stable = storage.load().unwrap_or_default();
        let mut state = S::default();

        // Replay decisions in order
        for slot in 0..stable.decision_slot {
            state.execute(slot, stable.decisions[&slot].clone().inner());
        }

        Replica {
            leader_tx,
            shared_tx,
            rx,
            stable,
            storage,
            state,
        }
    }

    fn respond_request(&mut self, command: S::Command) {
        self.propose(command.into());
    }

    fn respond_decision(&mut self, decision: message::Proposal<S::Command>) {
        self.stable.decisions.insert(decision.s_id, decision.command);
        self.storage.save(&self.stable);
        while let Some(c1) = self.stable.decisions.get(&self.stable.decision_slot).cloned() {
            if let Some(c2) = self.stable.proposals.get(&self.stable.decision_slot) {
                if c1 != *c2 {
                    self.propose(c2.clone());
                }
            }
            self.perform(c1);
        }
    }

    fn propose(&mut self, command: message::Command<S::Command>) {
        for previous in self.stable.decisions.values() {
            if *previous == command { return }
        }

        while self.stable.proposals.contains_key(&self.stable.proposal_slot)
           || self.stable.decisions.contains_key(&self.stable.proposal_slot) {
            self.stable.proposal_slot += 1;
        }

        info!("proposing {:?} for slot {:?}", command, self.stable.proposal_slot);
        self.stable.proposals.insert(self.stable.proposal_slot, command.clone());
        self.storage.save(&self.stable);

        let proposal = leader::In::Propose(message::Proposal {
            s_id: self.stable.proposal_slot,
            command: command,
        });

        self.leader_tx.unbounded_send(proposal)
            .expect("[INTERNAL ERROR]: failed to send proposal");
    }

    fn perform(&mut self, command: message::Command<S::Command>) {
        for (s, previous) in &self.stable.decisions {
            if *previous == command && *s < self.stable.decision_slot {
                self.stable.decision_slot += 1;
                self.storage.save(&self.stable);
                return
            }
        }
        info!("executing {:?} in slot {}", command, self.stable.decision_slot);
        let client_id = command.client_id();
        if let Some(result) = self.state.execute(self.stable.decision_slot, command.inner()) {
            self.shared_tx
                .read()
                .send_client(client_id, result);
        }
        self.stable.decision_slot += 1;
        self.storage.save(&self.stable);
    }
}

impl<S: state::State> Future for Replica<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Async::Ready(Some(message)) = self.rx.poll()? {
            debug!("received {:?}", message);
            match message {
            | In::Request(command) => self.respond_request(command),
            | In::Decision(proposal) => self.respond_decision(proposal),
            }
        }
        Ok(Async::NotReady)
    }
}

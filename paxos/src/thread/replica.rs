use std::collections::HashMap as Map;
use tokio::prelude::*;

use crate::message;
use crate::shared;
use crate::state;
use crate::state::Command;
use crate::thread::*;

#[derive(Debug)]
pub enum In<C: state::Command> {
    Request(C),
    Decision(message::Proposal<C>),
}

pub struct Replica<S: state::State> {
    leader_tx: Tx<leader::In<S::Command>>,
    shared_tx: shared::Shared<S>,
    rx: Rx<In<S::Command>>,
    state: S,
    decision_slot: usize,
    proposal_slot: usize,
    proposals: Map<usize, message::Command<S::Command>>,
    decisions: Map<usize, message::Command<S::Command>>,
}

impl<S: state::State> Replica<S> {
    pub fn new(
        leader_tx: Tx<leader::In<S::Command>>,
        shared_tx: shared::Shared<S>,
        rx: Rx<In<S::Command>>,
    ) -> Self {
        Replica {
            leader_tx,
            shared_tx,
            rx,
            state: S::default(),
            decision_slot: 0,
            proposal_slot: 0,
            proposals: Map::default(),
            decisions: Map::default(),
        }
    }

    fn respond_request(&mut self, command: S::Command) {
        self.propose(command.into());
    }

    fn respond_decision(&mut self, decision: message::Proposal<S::Command>) {
        self.decisions.insert(decision.s_id, decision.command);
        while let Some(c1) = self.decisions.get(&self.decision_slot).cloned() {
            if let Some(c2) = self.proposals.get(&self.decision_slot) {
                if c1 != *c2 {
                    self.propose(c2.clone());
                }
            }
            self.perform(c1);
        }
    }

    fn propose(&mut self, command: message::Command<S::Command>) {
        for previous in self.decisions.values() {
            if *previous == command { return }
        }

        while self.proposals.contains_key(&self.proposal_slot)
           || self.decisions.contains_key(&self.proposal_slot) {
            self.proposal_slot += 1;
        }

        info!("proposing {:?} for slot {:?}", command, self.proposal_slot);
        self.proposals.insert(self.proposal_slot, command.clone());

        let proposal = leader::In::Propose(message::Proposal {
            s_id: self.proposal_slot,
            command: command,
        });

        self.leader_tx.unbounded_send(proposal)
            .expect("[INTERNAL ERROR]: failed to send proposal");
    }

    fn perform(&mut self, command: message::Command<S::Command>) {
        for (s, previous) in &self.decisions {
            if *previous == command && *s < self.decision_slot {
                self.decision_slot += 1;
                return
            }
        }
        info!("executing {:?} in slot {}", command, self.decision_slot);
        let client_id = command.client_id();
        if let Some(result) = self.state.execute(self.decision_slot, command.inner()) {
            self.shared_tx
                .read()
                .send_client(client_id, result);
        }
        self.decision_slot += 1;
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

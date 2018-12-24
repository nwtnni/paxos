use bimap::BiMap;
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
    proposals: BiMap<message::Command<S::Command>, usize>,
    decisions: BiMap<message::Command<S::Command>, usize>,
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
            proposals: BiMap::default(),
            decisions: BiMap::default(),
        }
    }

    fn respond_request(&mut self, command: S::Command) {
        self.propose(command.into());
    }

    fn respond_decision(&mut self, decision: message::Proposal<S::Command>) {
        self.decisions.insert(decision.command.clone(), decision.s_id);
        while let Some(c1) = self.decisions.get_by_right(&self.decision_slot).cloned() {
            if let Some(c2) = self.proposals.get_by_right(&self.decision_slot).cloned() {
                if c1 != c2 {
                    self.propose(c2);
                }
            }
            self.perform(c1);
        }
    }

    fn propose(&mut self, command: message::Command<S::Command>) {
        if self.decisions.contains_left(&command) { return }

        while self.proposals.contains_right(&self.proposal_slot)
           || self.decisions.contains_right(&self.proposal_slot) {
            self.proposal_slot += 1;
        }

        self.proposals.insert(command.clone(), self.proposal_slot);

        let proposal = leader::In::Propose(message::Proposal {
            s_id: self.proposal_slot,
            command: command,
        });

        self.leader_tx.unbounded_send(proposal)
            .expect("[INTERNAL ERROR]: failed to send proposal");
    }

    fn perform(&mut self, command: message::Command<S::Command>) {
        if let Some(s) = self.decisions.get_by_left(&command) {
            if *s < self.decision_slot {
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
            info!("received {:?}", message);
            match message {
            | In::Request(command) => self.respond_request(command),
            | In::Decision(proposal) => self.respond_decision(proposal),
            }
        }
        Ok(Async::NotReady)
    }
}

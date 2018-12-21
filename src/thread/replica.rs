use std::collections::HashMap as Map;

use bimap::BiMap;
use tokio::prelude::*;

use crate::message;
use crate::shared;
use crate::state;
use crate::state::Command;
use crate::thread::*;

pub enum In<C: state::Command> {
    Request(C),
    Decision(message::Proposal<C>),
}

pub struct Replica<S: state::State> {
    leader_tx: Tx<leader::In<S::Command>>,
    shared_tx: shared::Shared<S>,
    rx: Rx<In<S::Command>>,
    state: S,
    slot: usize,
    proposals: BiMap<message::CommandID<S::Command>, usize>,
    decisions: BiMap<message::CommandID<S::Command>, usize>,
    commands: Map<message::CommandID<S::Command>, S::Command>,
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
            slot: 0,
            proposals: BiMap::default(),
            decisions: BiMap::default(),
            commands: Map::default(),
        }
    }

    pub async fn run(mut self) {
        loop {
            while let Some(Ok(message)) = await!(self.rx.next()) {
                match message {
                | In::Request(command) => self.respond_request(command),
                | In::Decision(proposal) => self.respond_decision(proposal),
                }
            }
        }
    }

    fn respond_request(&mut self, command: S::Command) {
        let c_id = message::CommandID {
            c_id: command.client_id(),
            l_id: command.local_id(),
        };
        self.commands.insert(c_id.clone(), command);
        self.propose(c_id);
    }

    fn respond_decision(&mut self, decision: message::Proposal<S::Command>) {
        self.decisions.insert(decision.c_id, decision.s_id);

        while let Some(c1) = self.decisions.get_by_right(&self.slot).cloned() {
            if let Some(c2) = self.proposals.get_by_right(&self.slot).cloned() {
                if c1 != c2 {
                    self.propose(c2);
                }
            }
            let command = self.commands.remove(&c1)
                .expect("[INTERNAL ERROR]: each command should be performed exactly once");
            self.perform(command);
        }
    }

    fn propose(&mut self, c_id: message::CommandID<S::Command>) {
        if self.decisions.contains_left(&c_id) { return }

        let next = 1 + std::cmp::max(
            self.proposals.right_values().max().unwrap_or(&0),
            self.decisions.right_values().max().unwrap_or(&0),
        );

        self.proposals.insert(c_id.clone(), next);

        let proposal = leader::In::Propose(message::Proposal {
            s_id: next,
            c_id: c_id,
        });

        self.leader_tx.unbounded_send(proposal)
            .expect("[INTERNAL ERROR]: failed to send proposal");
    }

    fn perform(&mut self, c: S::Command) {
        let client_id = c.client_id();
        let local_id = c.local_id();
        let command_id = message::CommandID {
            c_id: client_id.clone(),
            l_id: local_id,
        };
        if let Some(s) = self.decisions.get_by_left(&command_id) {
            if *s < self.slot {
                self.slot += 1;
                return
            }
        }
        let result = self.state.execute(c); 
        self.slot += 1;
        self.shared_tx
            .read()
            .send_client(client_id, result);
    }
}

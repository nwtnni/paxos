use std::marker;
use std::collections::HashMap as Map;

use bimap::BiMap;
use tokio::prelude::*;
use tokio_serde_bincode::WriteBincode;
use tokio::codec;

use crate::message;
use crate::state;
use crate::thread::*;

pub type In<C> = message::Proposal<C>;

pub struct Replica<C: state::Command, R, S> {
    client_rx: SocketRx<C>,
    client_tx: SocketTx,
    leader_tx: Tx<leader::In<C::ID>>,
    rx: Rx<In<C::ID>>,
    state: S,
    slot: usize,
    proposals: BiMap<C::ID, usize>,
    decisions: BiMap<C::ID, usize>,
    commands: Map<C::ID, C>,
    _marker: marker::PhantomData<R>,
}

impl<C: state::Command, R: state::Response, S: state::State<C, R>> Replica<C, R, S> {
    pub async fn run(mut self) {
        loop {
            while let Some(Ok(request)) = await!(self.client_rx.next()) {
                self.respond_request(request);
            }

            while let Some(Ok(decision)) = await!(self.rx.next()) {
                self.respond_decision(decision);
            }
        }
    }

    fn respond_request(&mut self, request: C) {
        let c_id = request.id().clone();
        self.commands.insert(c_id.clone(), request);
        self.propose(c_id);
    }

    fn respond_decision(&mut self, decision: message::Proposal<C::ID>) {
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

    fn propose(&mut self, c_id: C::ID) {
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

    fn perform(&mut self, c: C) {
        if let Some(s) = self.decisions.get_by_left(&c.id()) {
            if *s < self.slot {
                self.slot += 1;
                return
            }
        }

        let result = self.state.execute(c); 
        self.slot += 1;

        // TODO: what's the best way to do this asynchronously?
        WriteBincode::new(&mut self.client_tx)
            .send(result)
            .wait()
            .expect("[INTERNAL ERROR]: failed to send to client");
    }
}

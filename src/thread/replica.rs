use std::collections::HashMap as Map;

use bimap::BiMap;
use tokio::prelude::*;
use tokio_serde_bincode::{ReadBincode, WriteBincode};
use tokio::{codec, net};

use crate::message;
use crate::shared;
use crate::state;
use crate::state::Command;
use crate::thread::*;

pub type In<C> = message::Proposal<C>;

pub struct Replica<S: state::State> {
    client_rx: SocketRx<S::Command>,
    client_tx: SocketTx,
    leader_tx: Tx<leader::In<S::Command>>,
    rx: Rx<In<S::Command>>,
    state: S,
    slot: usize,
    proposals: BiMap<message::CommandID<S::Command>, usize>,
    decisions: BiMap<message::CommandID<S::Command>, usize>,
    commands: Map<message::CommandID<S::Command>, S::Command>,
}

impl<S: state::State> Replica<S> {
    pub fn new(
        client: net::tcp::TcpStream,   
        leader_tx: Tx<leader::In<S::Command>>,
        shared_tx: shared::Shared<S>,
        rx: Rx<In<S::Command>>,
        state: S,
    ) -> Self {
        let (client_rx, client_tx) = client.split();

        let client_rx = ReadBincode::new(
            codec::length_delimited::Builder::new()
                .new_read(client_rx)
                .from_err::<bincode::Error>()
        );

        let client_tx = codec::length_delimited::Builder::new()
            .new_write(client_tx)
            .sink_from_err::<bincode::Error>();

        Replica {
            client_rx,
            client_tx,
            leader_tx,
            rx,
            state,
            slot: 0,
            proposals: BiMap::default(),
            decisions: BiMap::default(),
            commands: Map::default(),
        }
    }

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

    fn respond_request(&mut self, request: S::Command) {
        let c_id = message::CommandID {
            c_id: request.client_id(),
            l_id: request.local_id(),
        };
        self.commands.insert(c_id.clone(), request);
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
        let c_id = message::CommandID {
            c_id: c.client_id(),
            l_id: c.local_id(),
        };
        if let Some(s) = self.decisions.get_by_left(&c_id) {
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

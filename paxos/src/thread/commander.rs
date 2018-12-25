use std::collections::HashSet as Set;

use futures::sync::mpsc;
use tokio::prelude::*;
use tokio::timer;

use crate::message;
use crate::shared;
use crate::state;
use crate::thread::{leader, peer, Tx, Rx};

pub type In = message::P2B;

pub struct Commander<S: state::State> {
    id: message::CommanderID,
    rx: Rx<In>,
    leader_tx: Tx<leader::In<S::Command>>,
    shared_tx: shared::Shared<S>,
    waiting: Set<usize>,
    minority: usize,
    pvalue: message::PValue<S::Command>,
    timeout: timer::Interval,
}

impl<S: state::State> Commander<S> {
    pub fn new(
        leader_tx: Tx<leader::In<S::Command>>,
        shared_tx: shared::Shared<S>,
        pvalue: message::PValue<S::Command>,
        count: usize,
        timeout: std::time::Duration,
    ) -> Self {
        let waiting = (0..count).collect();
        let minority = (count - 1) / 2;
        let (self_tx, self_rx) = mpsc::unbounded();
        let id = message::CommanderID {
            b_id: pvalue.b_id,
            s_id: pvalue.s_id,
        };
        let timeout = timer::Interval::new(
            std::time::Instant::now() + timeout,
            timeout,
        );
        debug!("starting for {:?}", id);
        shared_tx.write().connect_commander(id, self_tx);
        let commander = Commander {
            id,
            rx: self_rx,
            leader_tx,
            shared_tx,
            waiting,
            minority,
            pvalue,
            timeout,
        };
        commander.send_p2a();
        commander
    }

    fn send_p2a(&self) {
        let p2a = peer::In::P2A(
            self.id,
            self.pvalue.clone()
        );
        self.shared_tx
            .read()
            .narrowcast(&self.waiting, p2a);
    }

    fn send_decide(&self) {
        let decide = message::Proposal {
            s_id: self.pvalue.s_id,
            command: self.pvalue.command.clone(),
        };
        debug!("{:?} decided", self.pvalue);
        self.shared_tx
            .read()
            .broadcast(peer::In::Decision(decide));
    }

    fn send_preempt(&self, b_id: message::Ballot) {
        let preempt = leader::In::Preempt::<S::Command>(b_id);
        debug!("{:?} preempted", self.pvalue);
        self.leader_tx
            .unbounded_send(preempt)
            .expect("[INTERNAL ERROR]: failed to send preempted");
    }
}

impl<S: state::State> Future for Commander<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {

        // Narrowcast P2A to acceptors who haven't responded
        while let Async::Ready(Some(_)) = self.timeout
            .poll()
            .map_err(|_| ())?
        {
            self.send_p2a();
        }

        // Respond to incoming P2B messages
        while let Async::Ready(Some(p2b)) = self.rx
            .poll()
            .map_err(|_| ())?
        {
            debug!("received {:?}", p2b);

            // Commander has not been preempted
            if p2b.b_id == self.pvalue.b_id {

                self.waiting.remove(&p2b.a_id);

                // Notify leader that we've achieved a majority
                if self.waiting.len() <= self.minority {
                    self.send_decide();
                    return Ok(Async::Ready(()))
                }
            }

            // Notify leader that we've been preempted
            else if p2b.b_id > self.pvalue.b_id {
                self.send_preempt(p2b.b_id);
                return Ok(Async::Ready(()))
            }
        }

        Ok(Async::NotReady)
    }
}

impl<S: state::State> Drop for Commander<S> {
    fn drop(&mut self) {
        debug!("dropping {:?}", self.pvalue);
        self.shared_tx.write().disconnect_commander(self.id);
    }
}

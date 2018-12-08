use hashbrown::HashSet as Set;
use futures::sync::mpsc;
use tokio::prelude::*;
use tokio::timer;

use crate::constants::COMMANDER_TIMEOUT;
use crate::message;
use crate::shared;
use crate::state;
use crate::thread::{leader, peer, Tx, Rx};

pub type In = message::P2B;

pub type ID = (message::BallotID, usize);

pub struct Commander<O: Clone> {
    id: ID,
    rx: Rx<In>,
    leader_tx: Tx<leader::In<O>>,
    shared_tx: shared::Shared<O>,
    waiting: Set<usize>,
    minority: usize,
    pvalue: message::PValue<O>,
    timeout: timer::Interval,
}

impl<O: state::Operation> Commander<O> {
    pub fn new(
        leader_tx: Tx<leader::In<O>>,
        shared_tx: shared::Shared<O>,
        pvalue: message::PValue<O>,
        count: usize
    ) -> Self {
        let waiting = (0..count).collect();
        let minority = (count - 1) / 2;
        let timeout = timer::Interval::new_interval(COMMANDER_TIMEOUT);
        let (self_tx, self_rx) = mpsc::unbounded();
        let id = (pvalue.b_id, pvalue.s_id);
        shared_tx.write().connect_commander(id, self_tx);
        Commander {
            id,
            rx: self_rx,
            leader_tx,
            shared_tx,
            waiting,
            minority,
            pvalue,
            timeout,
        }
    }

    pub async fn run(mut self) {
        'outer: loop {

            // Narrowcast P2A to acceptors who haven't responded
            while let Some(_) = await!(self.timeout.next()) {
                self.send_p2a();    
            }

            // Respond to incoming P2B messages
            while let Some(Ok(p2b)) = await!(self.rx.next()) {
                
                // Commander has not been preempted
                if p2b.b_id == self.pvalue.b_id {

                    self.waiting.remove(&p2b.a_id);

                    // Notify leader that we've achieved a majority
                    if self.waiting.len() <= self.minority {
                        self.send_decide();
                        break 'outer
                    }
                }
                
                // Notify leader that we've been preempted
                else {
                    debug_assert!(p2b.b_id > self.pvalue.b_id);
                    self.send_preempt(p2b.b_id);
                    break 'outer
                }
            }
        }
    }

    fn send_p2a(&self) {
        let p2a = peer::In::P2A(self.pvalue.clone());
        self.shared_tx
            .read()
            .narrowcast(&self.waiting, p2a);
    }

    fn send_decide(self) {
        let id = (self.pvalue.b_id, self.pvalue.s_id);
        let decide = leader::In::Decide(id, message::Proposal {
            s_id: self.pvalue.s_id,
            op: self.pvalue.op.clone(),
        });
        self.leader_tx
            .unbounded_send(decide)
            .expect("[INTERNAL ERROR]: failed to send decision");
    }

    fn send_preempt(self, b_id: message::BallotID) {
        let preempt = leader::In::Preempt::<O>(b_id); 
        self.leader_tx
            .unbounded_send(preempt)
            .expect("[INTERNAL ERROR]: failed to send preempted");
    }
}

impl<O: Clone> Drop for Commander<O> {
    fn drop(&mut self) {
        self.shared_tx.write().disconnect_commander(self.id);
    }
}

use hashbrown::HashSet as Set;
use futures::sync::mpsc;
use tokio::prelude::*;
use tokio::timer;

use crate::constants::COMMANDER_TIMEOUT;
use crate::message;
use crate::shared;
use crate::state;
use crate::thread::{leader, peer, replica, Tx, Rx};

pub type In = message::P2B;

pub type ID = (message::BallotID, usize);

pub struct Commander<I: Clone> {
    id: ID,
    rx: Rx<In>,
    leader_tx: Tx<leader::In<I>>,
    shared_tx: shared::Shared<I>,
    waiting: Set<usize>,
    minority: usize,
    pvalue: message::PValue<I>,
    timeout: timer::Interval,
}

impl<I: state::Identifier> Commander<I> {
    pub fn new(
        leader_tx: Tx<leader::In<I>>,
        shared_tx: shared::Shared<I>,
        pvalue: message::PValue<I>,
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
        let p2a = peer::In::P2A(
            self.id,
            self.pvalue.clone()
        );
        self.shared_tx
            .read()
            .narrowcast(&self.waiting, p2a);
    }

    fn send_decide(self) {
        let decide = message::Proposal {
            s_id: self.pvalue.s_id,
            c_id: self.pvalue.c_id.clone(),
        };
        self.shared_tx
            .read()
            .send_replica(decide);
    }

    fn send_preempt(self, b_id: message::BallotID) {
        let preempt = leader::In::Preempt::<I>(b_id); 
        self.leader_tx
            .unbounded_send(preempt)
            .expect("[INTERNAL ERROR]: failed to send preempted");
    }
}

impl<I: Clone> Drop for Commander<I> {
    fn drop(&mut self) {
        self.shared_tx.write().disconnect_commander(self.id);
    }
}

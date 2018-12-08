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

pub struct Commander<O> {
    rx: Rx<In>,
    leader_tx: Tx<leader::In<O>>,
    peer_txs: shared::Shared<O>,
    waiting: Set<usize>,
    minority: usize,
    pvalue: message::PValue<O>,
    timeout: timer::Interval,
}

impl<O: state::Operation> Commander<O> {
    pub fn new(
        leader_tx: Tx<leader::In<O>>,
        peer_txs: shared::Shared<O>,
        pvalue: message::PValue<O>,
        count: usize
    ) -> (Self, Tx<In>) {
        let waiting = (0..count).collect();
        let minority = (count - 1) / 2;
        let timeout = timer::Interval::new_interval(COMMANDER_TIMEOUT);
        let (self_tx, self_rx) = mpsc::unbounded();
        let commander = Commander {
            rx: self_rx,
            leader_tx,
            peer_txs,
            waiting,
            minority,
            pvalue,
            timeout,
        };
        (commander, self_tx)
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
        self.peer_txs
            .read()
            .narrowcast(&self.waiting, p2a);
    }

    fn send_decide(self) {
        let decide = leader::In::Decide(message::Proposal {
            s_id: self.pvalue.s_id,
            op: self.pvalue.op,
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

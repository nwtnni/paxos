use std::time;

use hashbrown::HashSet as Set;
use futures::sync::mpsc;
use tokio::prelude::*;
use tokio::timer;

use crate::constants::SCOUT_TIMEOUT;
use crate::message;
use crate::shared;
use crate::state;
use crate::thread::{leader, peer, Tx, Rx};

pub type In<O> = message::P1B<O>;

pub struct Scout<O> {
    rx: Rx<In<O>>,
    leader_tx: Tx<leader::In<O>>,
    peer_txs: shared::Shared<O>,
    waiting: Set<usize>,
    minority: usize,
    ballot: message::BallotID,
    pvalues: Set<message::PValue<O>>,
    timeout: timer::Interval,
}

impl<O: state::Operation> Scout<O> {
    pub fn new(
        leader_tx: Tx<leader::In<O>>,
        peer_txs: shared::Shared<O>, 
        ballot: message::BallotID,
        count: usize,
        delay: time::Duration
    ) -> (Self, Tx<In<O>>) {
        let waiting = (0..count).collect();  
        let minority = (count - 1) / 2;
        let timeout = timer::Interval::new(
            time::Instant::now() + delay,
            SCOUT_TIMEOUT
        );
        let pvalues = Set::default();
        let (self_tx, self_rx) = mpsc::unbounded();
        let scout = Scout {
            rx: self_rx,
            leader_tx,
            peer_txs,
            waiting,
            minority,
            ballot,
            pvalues,
            timeout,
        };
        (scout, self_tx)
    }

    pub async fn run(mut self) {
        'outer: loop {

            // Narrowcast P1A to acceptors who haven't responded
            while let Some(Ok(_)) = await!(self.timeout.next()) {
                self.send_p1a();
            }

            // Respond to incoming P1B messages
            while let Some(Ok(p1b)) = await!(self.rx.next()) {

                // Scout has not been preempted
                if p1b.b_id == self.ballot {

                    // Union known pvalues with acceptor's set
                    self.pvalues.extend(p1b.pvalues.into_iter());
                    self.waiting.remove(&p1b.a_id);

                    // Notify leader that we've achieved a majority
                    if self.waiting.len() <= self.minority {
                        self.send_adopt(p1b.b_id);
                        break 'outer
                    }
                }
                
                // Notify leader that we've been preempted
                else {
                    debug_assert!(p1b.b_id > self.ballot);
                    self.send_preempt(p1b.b_id);
                    break 'outer
                }
            }
        }
    }

    fn send_p1a(&self) {
        let p1a = peer::In::P1A::<O>(self.ballot);
        self.peer_txs
            .read()
            .narrowcast(&self.waiting, p1a);
    }

    fn send_adopt(self, b_id: message::BallotID) {
        let pvalues = self.pvalues.into_iter().collect();
        let adopt = leader::In::Adopt(b_id, pvalues);
        self.leader_tx
            .unbounded_send(adopt)
            .expect("[INTERNAL ERROR]: failed to send adopt");
    }

    fn send_preempt(self, b_id: message::BallotID) {
        let preempt = leader::In::Preempt::<O>(b_id); 
        self.leader_tx
            .unbounded_send(preempt)
            .expect("[INTERNAL ERROR]: failed to send preempt");
    }
}

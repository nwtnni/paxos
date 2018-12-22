use std::time;

use hashbrown::HashSet as Set;
use futures::sync::mpsc;
use tokio::prelude::*;
use tokio::timer;

use crate::message;
use crate::shared;
use crate::state;
use crate::thread::{leader, peer, Tx, Rx};

pub type In<C> = message::P1B<C>;

pub struct Scout<S: state::State> {
    rx: Rx<In<S::Command>>,
    leader_tx: Tx<leader::In<S::Command>>,
    shared_tx: shared::Shared<S>,
    waiting: Set<usize>,
    minority: usize,
    ballot: message::BallotID,
    pvalues: Set<message::PValue<S::Command>>,
    timeout: timer::Interval,
}

impl<S: state::State> Scout<S> {
    pub fn new(
        leader_tx: Tx<leader::In<S::Command>>,
        shared_tx: shared::Shared<S>, 
        ballot: message::BallotID,
        count: usize,
        delay: time::Duration,
        timeout: time::Duration,
    ) -> Self {
        let waiting = (0..count).collect();  
        let minority = (count - 1) / 2;
        let timeout = timer::Interval::new(
            time::Instant::now() + delay,
            timeout,
        );
        let pvalues = Set::default();
        let (self_tx, self_rx) = mpsc::unbounded();
        shared_tx.write().replace_scout(self_tx);
        Scout {
            rx: self_rx,
            leader_tx,
            shared_tx,
            waiting,
            minority,
            ballot,
            pvalues,
            timeout,
        }
    }

    pub async fn run(mut self) {
        info!("starting for {:?}", self.ballot);
        'outer: loop {

            // Narrowcast P1A to acceptors who haven't responded
            if let Some(Ok(_)) = await!(self.timeout.next()) {
                self.send_p1a();
            }

            // Respond to incoming P1B messages
            if let Some(Ok(p1b)) = await!(self.rx.next()) {
                trace!("received message {:?}", p1b);

                // Scout has not been preempted
                if p1b.b_id == self.ballot {

                    // Union known pvalues with acceptor's set
                    self.pvalues.extend(p1b.pvalues.into_iter());
                    self.waiting.remove(&p1b.a_id);

                    // Notify leader that we've achieved a majority
                    if self.waiting.len() <= self.minority {
                        self.send_adopt();
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
        let p1a = peer::In::P1A::<S::Command>(self.ballot);
        self.shared_tx
            .read()
            .narrowcast(&self.waiting, p1a);
    }

    fn send_adopt(self) {
        let pvalues = self.pvalues.into_iter().collect();
        let adopt = leader::In::Adopt(pvalues);
        debug!("{:?} adopted", self.ballot);
        self.leader_tx
            .unbounded_send(adopt)
            .expect("[INTERNAL ERROR]: failed to send adopt");
    }

    fn send_preempt(self, b_id: message::BallotID) {
        let preempt = leader::In::Preempt::<S::Command>(b_id); 
        debug!("{:?} preempted by {:?}", self.ballot, b_id);
        self.leader_tx
            .unbounded_send(preempt)
            .expect("[INTERNAL ERROR]: failed to send preempt");
    }
}

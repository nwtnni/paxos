use std::collections::HashSet as Set;
use std::time;

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
    ballot: message::Ballot,
    minority: usize,
    pvalues: Set<message::PValue<S::Command>>,
    timeout: timer::Interval,
    waiting: Set<usize>,
}

impl<S: state::State> Scout<S> {
    pub fn new(
        leader_tx: Tx<leader::In<S::Command>>,
        shared_tx: shared::Shared<S>,
        ballot: message::Ballot,
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
        debug!("starting for {:?} with delay {:?}", ballot, delay);
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

    fn send_p1a(&self) {
        let p1a = peer::In::P1A::<S::Command>(self.ballot);
        self.shared_tx
            .read()
            .narrowcast(&self.waiting, p1a);
    }

    fn send_adopt(&mut self) {
        debug!("{:?} adopted", self.ballot);
        let pvalues = std::mem::replace(&mut self.pvalues, Set::with_capacity(0))
            .into_iter()
            .collect();
        let adopt = leader::In::Adopt(pvalues);
        self.leader_tx
            .unbounded_send(adopt)
            .expect("[INTERNAL ERROR]: failed to send adopt");
    }

    fn send_preempt(&self, b_id: message::Ballot) {
        debug!("{:?} preempted by {:?}", self.ballot, b_id);
        let preempt = leader::In::Preempt::<S::Command>(b_id);
        self.leader_tx
            .unbounded_send(preempt)
            .expect("[INTERNAL ERROR]: failed to send preempt");
    }
}

impl<S: state::State> Future for Scout<S> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        // Narrowcast P1A to acceptors who haven't responded
        while let Async::Ready(Some(_)) = self.timeout
            .poll()
            .map_err(|_| ())?
        {
            self.send_p1a();
        }

        // Respond to incoming P1B messages
        while let Async::Ready(Some(p1b)) = self.rx.poll()? {

            // Scout has not been preempted
            if p1b.b_id == self.ballot {

                // Union known pvalues with acceptor's set
                self.pvalues.extend(p1b.pvalues.into_iter());
                self.waiting.remove(&p1b.a_id);

                // Notify leader that we've achieved a majority
                if self.waiting.len() <= self.minority {
                    self.send_adopt();
                    return Ok(Async::Ready(()))
                }
            }

            // Notify leader that we've been preempted
            else if p1b.b_id > self.ballot {
                self.send_preempt(p1b.b_id);
                return Ok(Async::Ready(()))
            }
        }
        Ok(Async::NotReady)
    }
}

impl<S: state::State> Drop for Scout<S> {
    fn drop(&mut self) {
        debug!("dropping {:?}", self.ballot); 
    }
}

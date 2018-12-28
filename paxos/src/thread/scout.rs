//! # Summary
//!
//! This module implements the `Scout` struct, which is responsible
//! for proposing ballots to acceptors. Once its ballot has been
//! adopted by a majority of acceptors, its leader is free to begin
//! proposing according to the PValues the scout has collected.

use std::collections::HashSet as Set;
use std::time;

use tokio::prelude::*;
use tokio::timer;

use crate::internal;
use crate::message;
use crate::shared;
use crate::state;
use crate::thread::{leader, peer};

/// Scouts can only receive P1B from acceptors.
pub type In<C> = message::P1B<C>;

/// Competes with other scouts for adoption by
/// a majority of acceptors.
pub struct Scout<S: state::State> {
    /// Internal receiving channel
    rx: internal::Rx<In<S::Command>>,

    /// Internal leader transmitting channel
    leader_tx: internal::Tx<leader::In<S::Command>>,

    /// Internal shared transmitting channel
    shared_tx: shared::Shared<S>,

    /// Associated ballot
    ballot: message::Ballot,

    /// Latest known decision
    decided: Option<usize>,

    /// Number of acceptors constituting a minority of all acceptors
    minority: usize,

    /// Latest PValues accepted by contacted acceptors
    pvalues: Set<message::PValue<S::Command>>,

    /// Interval at which to re-send P1A messages to unresponsive acceptors
    timeout: timer::Interval,

    /// Acceptors that have yet to respond
    waiting: Set<usize>,
}

impl<S: state::State> Scout<S> {
    pub fn new(
        leader_tx: internal::Tx<leader::In<S::Command>>,
        shared_tx: shared::Shared<S>,
        ballot: message::Ballot,
        count: usize,
        decided: Option<usize>,
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
        let (rx, tx) = internal::new();
        shared_tx.write().replace_scout(tx);
        debug!("starting for {:?} with delay {:?}", ballot, delay);
        Scout {
            rx,
            leader_tx,
            shared_tx,
            ballot,
            decided,
            minority,
            pvalues,
            timeout,
            waiting,
        }
    }

    /// Narrowcast P1A messages to all acceptors who haven't responded
    fn send_p1a(&self) {
        let p1a = peer::In::P1A::<S::Command>(message::P1A {
            b_id: self.ballot,
            decided: self.decided,
        });
        self.shared_tx
            .read()
            .narrowcast(&self.waiting, p1a);
    }

    /// Inform leader that its ballot has been adopted by a majority of acceptors
    fn send_adopt(&mut self) {
        debug!("{:?} adopted", self.ballot);
        let pvalues = std::mem::replace(&mut self.pvalues, Set::with_capacity(0))
            .into_iter()
            .collect();
        let adopt = leader::In::Adopt(pvalues);
        self.leader_tx.send(adopt);
    }

    /// Notify leader that its ballot has been preempted
    fn send_preempt(&self, b_id: message::Ballot) {
        debug!("{:?} preempted by {:?}", self.ballot, b_id);
        let preempt = leader::In::Preempt::<S::Command>(b_id);
        self.leader_tx.send(preempt);
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

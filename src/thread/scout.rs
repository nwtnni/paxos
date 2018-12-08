use hashbrown::HashSet as Set;
use futures::sync::mpsc;
use tokio::prelude::*;
use tokio::timer;

use crate::constants::SCOUT_TIMEOUT;
use crate::message;
use crate::thread::{Tx, Rx};
use crate::thread::leader;

pub type In<O> = message::P1B<O>;

pub type SendResult<O> = Result<(), mpsc::SendError<In<O>>>;

pub struct Scout<O> {
    rx: Rx<In<O>>,
    tx: Tx<leader::In<O>>,
    waiting: Set<usize>,
    minority: usize,
    ballot: message::BallotID,
    pvalues: Set<message::PValue<O>>,
    timeout: timer::Interval,
}

impl<O: Eq + std::hash::Hash> Scout<O> {
    pub fn new(tx: Tx<leader::In<O>>, ballot: message::BallotID, count: usize) -> (Self, Tx<In<O>>) {
        let waiting = (0..count).collect();  
        let minority = (count - 1) / 2;
        let timeout = timer::Interval::new_interval(SCOUT_TIMEOUT);
        let pvalues = Set::default();
        let (self_tx, self_rx) = mpsc::unbounded();
        let scout = Scout {
            rx: self_rx,
            tx,
            waiting,
            minority,
            ballot,
            pvalues,
            timeout,
        };
        (scout, self_tx)
    }

    pub async fn run(mut self) -> leader::SendResult<O> {
        'outer: loop {

            // Narrowcast P1A to acceptors who haven't responded
            while let Some(Ok(_)) = await!(self.timeout.next()) {
                self.send_p1a()?;
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
                        self.send_adopt(p1b.b_id)?;
                        break 'outer
                    }
                }
                
                // Notify leader that we've been preempted
                else {
                    debug_assert!(p1b.b_id > self.ballot);
                    self.send_preempt(p1b.b_id)?;
                    break 'outer
                }
            }
        }

        Ok(())
    }

    fn send_p1a(&self) -> leader::SendResult<O> {
        let waiting = self.waiting.iter()
            .cloned()
            .collect();
        let p1a = leader::In::P1A::<O>(waiting, self.ballot);
        self.tx.unbounded_send(p1a)
    }

    fn send_adopt(self, b_id: message::BallotID) -> leader::SendResult<O> {
        let pvalues = self.pvalues.into_iter()
            .collect();
        let adopt = leader::In::Adopt(b_id, pvalues);
        self.tx.unbounded_send(adopt)
    }

    fn send_preempt(self, b_id: message::BallotID) -> leader::SendResult<O> {
        let preempt = leader::In::Preempt::<O>(b_id); 
        self.tx.unbounded_send(preempt)
    }
}

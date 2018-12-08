use hashbrown::HashSet as Set;
use futures::sync::mpsc;
use tokio::prelude::*;
use tokio::timer;

use crate::message;
use crate::thread::leader;

pub type In<O> = message::P1B<O>;

pub struct Scout<O> {
    rx: mpsc::UnboundedReceiver<In<O>>,
    tx: mpsc::UnboundedSender<leader::In<O>>,
    waiting: Set<usize>,
    minority: usize,
    ballot: message::BallotID,
    pvalues: Set<message::PValue<O>>,
    interval: timer::Interval,
}

type SendError<O> = mpsc::SendError<leader::In<O>>;

impl<O: Eq + std::hash::Hash> Scout<O> {
    pub async fn run(mut self) -> Result<(), mpsc::SendError<leader::In<O>>> {
        'outer: loop {

            // Narrowcast P1A to acceptors who haven't responded
            while let Some(Ok(_)) = await!(self.interval.next()) {
                self.send_p1a()?;
            }

            // Respond to incoming P1B messages
            while let Some(Ok(p1b)) = await!(self.rx.next()) {

                // Acceptor has not been preempted
                if p1b.b_id == self.ballot {

                    // Union known pvalues with acceptor's set
                    self.pvalues.extend(p1b.pvalues.into_iter());
                    self.waiting.remove(&p1b.a_id);

                    // Notify leader that we've achieved a majority
                    if self.waiting.len() <= self.minority {
                        self.send_adopt(p1b.b_id)?;
                        break 'outer;
                    }
                }
                
                // Notify leader that we've been preempted
                else if p1b.b_id > self.ballot {
                    self.send_preempt(p1b.b_id)?;
                    break 'outer;
                }
            }
        }

        Ok(())
    }

    fn send_p1a(&self) -> Result<(), SendError<O>> {
        let waiting = self.waiting.iter()
            .cloned()
            .collect();
        let p1a = leader::In::P1A::<O>(waiting, self.ballot);
        self.tx.unbounded_send(p1a)
    }

    fn send_adopt(&mut self, b_id: message::BallotID) -> Result<(), SendError<O>> {
        let pvalues = std::mem::replace(&mut self.pvalues, Set::with_capacity(0))
            .into_iter()
            .collect();
        let adopt = leader::In::Adopt(b_id, pvalues);
        self.tx.unbounded_send(adopt)
    }

    fn send_preempt(&self, b_id: message::BallotID) -> Result<(), SendError<O>> {
        let preempt = leader::In::Preempt::<O>(b_id); 
        self.tx.unbounded_send(preempt)
    }
}

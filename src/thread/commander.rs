use hashbrown::HashSet as Set;
use futures::sync::mpsc;
use tokio::prelude::*;
use tokio::timer;

use crate::message;
use crate::thread::leader;

pub type In = message::P2B;

pub struct Commander<O> {
    rx: mpsc::UnboundedReceiver<In>,
    tx: mpsc::UnboundedSender<leader::In<O>>,
    waiting: Set<usize>,
    minority: usize,
    pvalue: message::PValue<O>,
    interval: timer::Interval,
}

type SendError<O> = mpsc::SendError<leader::In<O>>;

impl<O: Clone> Commander<O> {
    pub async fn run(mut self) -> Result<(), SendError<O>> {
        'outer: loop {

            // Narrowcast P2A to acceptors who haven't responded
            while let Some(_) = await!(self.interval.next()) {
                self.send_p2a()?;    
            }

            // Respond to incoming P2B messages
            while let Some(Ok(p2b)) = await!(self.rx.next()) {
                
                // Commander has not been preempted
                if p2b.b_id == self.pvalue.b_id {

                    self.waiting.remove(&p2b.a_id);

                    // Notify leader that we've achieved a majority
                    if self.waiting.len() <= self.minority {
                        self.send_decide()?;
                        break 'outer
                    }
                }
                
                // Notify leader that we've been preempted
                else {
                    debug_assert!(p2b.b_id > self.pvalue.b_id);
                    self.send_preempt(p2b.b_id)?;
                    break 'outer
                }
            }

        }
        Ok(())
    }

    fn send_p2a(&self) -> Result<(), SendError<O>> {
        let waiting = self.waiting.iter()
            .cloned()
            .collect();
        let p2a = leader::In::P2A(waiting, self.pvalue.clone());
        self.tx.unbounded_send(p2a)
    }

    fn send_decide(self) -> Result<(), SendError<O>> {
        let decide = leader::In::Decide(self.pvalue);
        self.tx.unbounded_send(decide)
    }

    fn send_preempt(self, b_id: message::BallotID) -> Result<(), SendError<O>> {
        let preempt = leader::In::Preempt::<O>(b_id); 
        self.tx.unbounded_send(preempt)
    }

}

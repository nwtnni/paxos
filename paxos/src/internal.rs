//! # Summary
//!
//! This module abstracts over internal connections to other threads.
//!
//! Currently backed by the `futures::sync::mpsc`: multiple-producer
//! single-consumer channels. In most cases, since we're using unbounded
//! channels, the only way for a send to fail is if the receiving end
//! has been dropped, which should be impossible unless there's some logic
//! error in the implementation. This is why the `send` method on `Tx`
//! calls `expect` internally.

use tokio::prelude::*;
use futures::sync::mpsc;

/// Intra-server receiving channel.
#[derive(Debug)]
pub struct Rx<T>(mpsc::UnboundedReceiver<T>);

/// Intra-server transmission channel. All clones send to the same receiving end.
#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
#[derive(Debug)]
pub struct Tx<T>(mpsc::UnboundedSender<T>);

/// Create a new pair of linked receiving and transmitting channels.
pub fn new<T>() -> (Rx<T>, Tx<T>) {
    let (tx, rx) = mpsc::unbounded();
    (Rx(rx), Tx(tx))
}

impl<T> Tx<T> {
    /// Force a message through the channel.
    /// Panics if the receiving end has been dropped.
    pub fn send(&self, message: T) {
        self.0.unbounded_send(message).expect("[INTERNAL ERROR]: receiver dropped");
    }

    /// Attempt to send a message through the channel.
    /// Does nothing if the receiving end has been dropped.
    pub fn try_send(&self, message: T) {
        self.0.unbounded_send(message).ok();
    }
}

impl<T> Stream for Rx<T> {
    type Item = T;
    type Error = ();

    #[inline]
    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        self.0.poll()
    }
}

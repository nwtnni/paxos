pub(crate) mod acceptor;
pub(crate) mod commander;
pub(crate) mod leader;
pub(crate) mod peer;
pub(crate) mod replica;
pub(crate) mod scout;

use futures::sync::mpsc;

pub type Tx<T> = mpsc::UnboundedSender<T>;
pub type Rx<T> = mpsc::UnboundedReceiver<T>;

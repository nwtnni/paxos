use std::marker;

use futures::sync::mpsc;

use crate::shared;
use crate::state;
use crate::thread;

const DEFAULT_PORT: usize = 20000;

#[derive(Copy, Clone, Debug)]
pub struct Config<S> {
    /// Unique replica ID
    id: usize,

    /// Port for incoming client requests
    port: usize,

    /// Total number of replicas
    count: usize,

    /// Initial state
    initial: S,
}

impl<S: state::State> Config<S> {
    pub fn new(id: usize, port: usize, count: usize, initial: S) -> Self {
        Config {
            id,
            port,
            count,
            initial,
        }
    }

    pub async fn run(self) {

        let (acceptor_tx, acceptor_rx) = mpsc::unbounded();
        let (leader_tx, leader_rx) = mpsc::unbounded();
        let (scout_tx, scout_rx) = mpsc::unbounded();
        let (replica_tx, replica_rx) = mpsc::unbounded();

        let addr = format!("localhost:{}", self.port)
            .parse::<std::net::SocketAddr>()
            .unwrap();

        let shared_tx: shared::Shared<<S::Command as state::Command>::ID> = shared::Shared::new(
            scout_tx,
            replica_tx
        );

        let acceptor = thread::acceptor::Acceptor::new(
            self.id,
            acceptor_rx,
            shared_tx.clone(), 
        );

        let leader = thread::leader::Leader::new(
            self.id,
            self.count,
            leader_rx,
            leader_tx.clone(),
            shared_tx.clone(),
        );




    }
}

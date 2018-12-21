use futures::sync::mpsc;
use tokio::prelude::*;

use crate::shared;
use crate::state;
use crate::thread;

const INTERNAL_PORT: usize = 20000;

#[derive(Copy, Clone, Debug)]
pub struct Config<S> {
    /// Unique replica ID
    id: usize,

    /// Port for incoming client requests
    port: usize,

    /// Total number of replicas
    count: usize,

    /// Timeout for detecting unresponsive servers
    timeout: std::time::Duration,

    _marker: std::marker::PhantomData<S>,
}

impl<S: state::State> Config<S> {
    pub fn new(id: usize, port: usize, count: usize) -> Self {
        Config {
            id,
            port,
            count,
            timeout: std::time::Duration::from_secs(1),
            _marker: Default::default(),
        }
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn run(self) {
        let (acceptor_tx, acceptor_rx) = mpsc::unbounded();
        let (leader_tx, leader_rx) = mpsc::unbounded();
        let (scout_tx, _) = mpsc::unbounded();
        let (replica_tx, replica_rx) = mpsc::unbounded();

        let mut internal_port = format!("localhost:{}", self.id + INTERNAL_PORT)
            .parse::<std::net::SocketAddr>()
            .map(|addr| tokio::net::tcp::TcpListener::bind(&addr))
            .expect("[INTERNAL ERROR]: invalid socket address")
            .expect("[INTERNAL ERROR]: failed to bind to socket")
            .incoming();

        let mut external_port = format!("localhost:{}", self.port)
            .parse::<std::net::SocketAddr>()
            .map(|addr| tokio::net::tcp::TcpListener::bind(&addr))
            .expect("[INTERNAL ERROR]: invalid socket address")
            .expect("[INTERNAL ERROR]: failed to bind to socket")
            .incoming();

        let shared_tx: shared::Shared<S> = shared::Shared::new(
            scout_tx,
            replica_tx.clone(),
        );

        let acceptor = thread::acceptor::Acceptor::new(
            self.id,
            acceptor_rx,
            shared_tx.clone(),
        );

        let replica = thread::replica::Replica::new(
            leader_tx.clone(),
            shared_tx.clone(),
            replica_rx,
        );

        let leader = thread::leader::Leader::new(
            self.id,
            self.count,
            leader_rx,
            leader_tx.clone(),
            shared_tx.clone(),
            self.timeout,
        );

        let self_id = self.id;
        let shared = shared_tx.clone();
        tokio::spawn_async(async move {
            while let Some(Ok(stream)) = await!(internal_port.next()) {
                let connecting = thread::peer::Connecting::new(
                    self_id,
                    stream,
                    acceptor_tx.clone(),
                    shared.clone(),
                    self.timeout,
                );
                let peer = await!(connecting.run());
                tokio::spawn_async(peer.run());
            }
        });

        let shared = shared_tx.clone();
        tokio::spawn_async(async move {
            while let Some(Ok(stream)) = await!(external_port.next()) {
                let connecting = thread::client::Connecting::new(
                    self_id,
                    stream,
                    replica_tx.clone(),
                    shared.clone(),
                );

                if let Ok(client) = await!(connecting.run()) {
                    tokio::spawn_async(client.run());
                }
            }
        });

        tokio::spawn_async(acceptor.run());
        tokio::spawn_async(replica.run());
        tokio::spawn_async(leader.run());
    }
}

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

        let mut internal_port = format!("127.0.0.1:{}", self.id + INTERNAL_PORT)
            .parse::<std::net::SocketAddr>()
            .map(|addr| tokio::net::tcp::TcpListener::bind(&addr))
            .expect("[INTERNAL ERROR]: invalid socket address")
            .expect("[INTERNAL ERROR]: failed to bind to socket")
            .incoming();

        let mut external_port = format!("127.0.0.1:{}", self.port)
            .parse::<std::net::SocketAddr>()
            .map(|addr| tokio::net::tcp::TcpListener::bind(&addr))
            .expect("[INTERNAL ERROR]: invalid socket address")
            .expect("[INTERNAL ERROR]: failed to bind to socket")
            .incoming();

        let shared_tx: shared::Shared<S> = shared::Shared::new(
            self.id,
            scout_tx,
            replica_tx.clone(),
            acceptor_tx.clone(),
        );

        let acceptor_thread = thread::acceptor::Acceptor::new(
            self.id,
            acceptor_rx,
            shared_tx.clone(),
        );

        let replica_thread = thread::replica::Replica::new(
            leader_tx.clone(),
            shared_tx.clone(),
            replica_rx,
        );

        let leader_thread = thread::leader::Leader::new(
            self.id,
            self.count,
            leader_rx,
            leader_tx.clone(),
            shared_tx.clone(),
            self.timeout,
        );

        let acceptor = acceptor_tx.clone();
        let self_id = self.id;
        let timeout = self.timeout;
        let shared = shared_tx.clone();
        tokio::spawn_async(async move {
            while let Some(Ok(stream)) = await!(internal_port.next()) {
                let connecting = thread::peer::Connecting::new(
                    self_id,
                    stream,
                    acceptor.clone(),
                    shared.clone(),
                    timeout,
                );
                tokio::spawn(connecting.and_then(|peer| peer));
            }
        });

        let shared = shared_tx.clone();
        tokio::spawn_async(async move {
            while let Some(Ok(stream)) = await!(external_port.next()) {
                let connecting = thread::client::Connecting::new(
                    stream,
                    replica_tx.clone(),
                    shared.clone(),
                );
                tokio::spawn(connecting.and_then(|client| client));
            }
        });

        for peer_id in (0..self.count).filter(|id| *id != self_id) {
            let acceptor = acceptor_tx.clone();
            let shared = shared_tx.clone();
            let addr = format!("127.0.0.1:{}", peer_id + INTERNAL_PORT)
                .parse::<std::net::SocketAddr>()
                .unwrap();
            let connect = tokio::net::tcp::TcpStream::connect(&addr)
                .map_err(|_| ())
                .and_then(move |stream| {
                    thread::peer::Peer::new(
                        self_id,
                        peer_id,
                        stream,
                        acceptor.clone(),
                        shared.clone(),
                        timeout,
                    )
                });
            tokio::spawn(connect);
        }

        tokio::spawn(acceptor_thread);
        tokio::spawn(replica_thread);
        tokio::spawn(leader_thread);
    }
}

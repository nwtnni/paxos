//! # Summary
//!
//! This module defines a single replicated Paxos server. A library
//! user can create an instance of `Config` with a state implementation
//! of their choice, and then call `run` to launch the Paxos server.

use tokio::prelude::*;

use crate::internal;
use crate::shared;
use crate::state;
use crate::thread;

const INTERNAL_PORT: usize = 20000;

/// Defines a single Paxos server with state type `S`.
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

    /// Create a new server with unique ID `id`, out of a cluster
    /// of `count` servers, listening on TCP port `port`.
    pub fn new(id: usize, port: usize, count: usize) -> Self {
        Config {
            id,
            port,
            count,
            timeout: std::time::Duration::from_secs(1),
            _marker: Default::default(),
        }
    }

    /// Configure timeout duration for detecting disconnected peers.
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Launch server asynchronously.
    pub async fn run(self) {
        let (acceptor_rx, acceptor_tx) = internal::new();
        let (leader_rx, leader_tx) = internal::new();
        let (_, scout_tx) = internal::new();
        let (replica_rx, replica_tx) = internal::new();

        // Listen for connections to other peer servers
        let mut internal_port = format!("127.0.0.1:{}", self.id + INTERNAL_PORT)
            .parse::<std::net::SocketAddr>()
            .map(|addr| tokio::net::tcp::TcpListener::bind(&addr))
            .expect("[INTERNAL ERROR]: invalid socket address")
            .expect("[INTERNAL ERROR]: failed to bind to socket")
            .incoming();

        // Listen for connections to clients
        let mut external_port = format!("127.0.0.1:{}", self.port)
            .parse::<std::net::SocketAddr>()
            .map(|addr| tokio::net::tcp::TcpListener::bind(&addr))
            .expect("[INTERNAL ERROR]: invalid socket address")
            .expect("[INTERNAL ERROR]: failed to bind to socket")
            .incoming();

        // Initialize message forwarding hub
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
            self.id,
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

        // Asynchronously listen for and create new server-to-server connections
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

        // Asynchronously listen for and create new server-to-client connections
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

        // Attempt to connect to all other servers directly on startup
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

        // Spawn persistent acceptor, replica, and leader threads
        tokio::spawn(acceptor_thread);
        tokio::spawn(replica_thread);
        tokio::spawn(leader_thread);
    }
}

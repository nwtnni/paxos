//! # Summary
//!
//! This crate implements the Paxos distributed consensus protocol according
//! to the description in the paper [Paxos Made Moderately Complex][1]. It
//! probably needs more optimization and review before any production usage,
//! but the heart of the protocol is implemented, and directly follows the
//! pseudo-code from the paper.
//!
//! # Usage
//!
//! To use this library, you should implement the `State` trait, which defines
//! a state machine to be replicated using Paxos. The protocol guarantees that
//! all operations are applied to all replicas in the same order, so--assuming
//! deterministic `Command` implementations--all replicas will have the same
//! state.
//!
//! Then you want to create an instance of `Config`, which is used to configure
//! and launch an actual server. You can call `run` to start listening for client
//! connections on the provided port.
//!
//! ## **Important**
//!
//! Currently, servers listen for clients using TCP streams, and communicate using
//! length-delimited `bincode`-encoded Rust data. For convenience, `Sink` and `Stream`
//! implementations of the receiving and transmitting wrappers around `TcpStream`
//! are exposed as `Rx<T>` and `Tx<T>`, respectively.
//!
//! # Example
//!
//! Here's an example of a basic chatroom state machine, which only has `Get` and
//! `Put` commands. This is the same state machine used in the test harness, and
//! the runnable source code is in the GitHub repository.
//!
//! ```rust
//! use serde_derive::{Serialize, Deserialize};
//! use paxos;
//!
//! #[derive(Serialize, Deserialize)]
//! #[derive(Clone, Debug)]
//! pub struct Command {
//!     pub client_id: usize,
//!     pub local_id: usize,
//!     pub mode: Mode,
//! }
//! 
//! #[derive(Serialize, Deserialize)]
//! #[derive(Clone, Debug)]
//! pub enum Mode {
//!     Get,
//!     Put(String),
//! }
//! 
//! #[derive(Serialize, Deserialize)]
//! #[derive(Clone, Debug)]
//! pub enum Response {
//!     Messages(Vec<String>),
//! }
//! 
//! #[derive(Serialize, Deserialize)]
//! #[derive(Clone, Debug, Default)]
//! pub struct State {
//!     pub messages: Vec<String>,
//! }
//! 
//! impl paxos::Command for Command {
//!     type ClientID = usize;
//!     type LocalID = usize;
//!     fn client_id(&self) -> Self::ClientID {
//!         self.client_id
//!     }
//!     fn local_id(&self) -> Self::LocalID {
//!         self.local_id
//!     }
//! }
//! 
//! impl paxos::State for State {
//!     type Command = Command;
//!     type Response = Response;
//!     fn execute(&mut self, _: usize, command: Self::Command) -> Option<Self::Response> {
//!         match command.mode {
//!         | Mode::Get => {
//!             Some(Response::Messages(self.messages.clone()))
//!         }
//!         | Mode::Put(message) => {
//!             self.messages.push(message);
//!             None
//!         },
//!         }
//!     }
//! }
//! ```
//!
//! # Implementation Details
//!
//! Everything should be documented for the curious. As for specific extensions
//! described by the paper:
//!
//! - Acceptors, replicas, and leaders are all co-located
//! - Acceptors only keep track of the most recently accepted `PValue` per slot
//! - Leaders use exponential backoff for new scouts when preempted
//!
//! [1]: http://paxos.systems/index.html

#![feature(await_macro, async_await, futures_api, pin)]

#[macro_use] extern crate derivative;
#[macro_use] extern crate log;
#[macro_use] extern crate tokio;

mod config;
mod message;
mod state;
mod shared;
mod socket;
mod storage;
mod thread;

pub use crate::config::Config;
pub use crate::state::{Identifier, Command, Response, State};
pub use crate::socket::{Rx, Tx};

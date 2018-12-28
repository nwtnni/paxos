# paxos

[Paxos][1] is a fault-tolerant distributed consensus algorithm. It guarantees safety and liveness
in asynchronous settings with less than `n / 2` crash failures, where `n` is the number of processes.

This library uses Paxos to implement a generic replicated state machine (also known as Multi-Paxos).
Assuming all commands are executed deterministically, replicas that execute 
them in the same order will arrive at the same final state. Command logs and 
other data are serialized to disk as `*.paxos` files for failure recovery, and 
need to be deleted between fresh runs.

## Overview

This repository contains the following:

- A library implementation of Paxos (based on the paper ["Paxos Made Moderately Complex"][2]) in `paxos`.
- An example replicated state machine in `chatroom`.
- A test harness for `chatroom` in `harness`.
- A test suite in `tests`

**Note:** this project was partially written to test out Rust's new async/await features,
and therefore requires Rust Nightly for now. You can override your default compiler for this
repository using the following command:

```
> rustup override set nightly
```

## Running Tests

For more information about the test harness binary, you can run:

```
> cargo run --bin harness -- --help
```

Example usage for running the `basic.json` test:

```
> cargo build
> cargo run --bin harness -- --server target/debug/chatroom-server --file tests/basic.json
```

The test suite covers some basic failure modes and higher throughput concurrent
writes--it's definitely not comprehensive though.

## Launching Chatroom

For help launching a chatroom server, you can run:

```
> cargo run --bin chatroom-server -- --help
```

For example, you can start a cluster of three servers running in the background locally with:

```
> cargo run --bin chatroom-server -- --count 3 --id 0 --port 10000
> cargo run --bin chatroom-server -- --count 3 --id 1 --port 10001
> cargo run --bin chatroom-server -- --count 3 --id 2 --port 10002
```

For help launching a chatroom client, you can run:

```
> cargo run --bin chatroom-client -- --help
```

For example, you can start a single client with:

```
> cargo run --bin chatroom-client -- --id 0
```

Logging is configurable on a per-server basis by passing `-v` flags to the
`chatroom-server` binary, if you want to see the messages being passed around
during the execution of the protocol.

## Using Library

To set up a replicated state machine, you have to implement the following traits:

```rust
/// Unique identifier
pub trait Identifier: std::hash::Hash
    + std::fmt::Debug
    + Clone
    + Eq
    + Send
    + Sync
{
}

/// Operation that can be applied to a state machine
pub trait Command: Send
    + Clone
    + std::fmt::Debug
    + serde::Serialize
    + serde::de::DeserializeOwned
{
    type ClientID: Identifier;
    type LocalID: Identifier;
    fn client_id(&self) -> Self::ClientID;
    fn local_id(&self) -> Self::LocalID;
}

/// Result of applying an operation to a state machine
pub trait Response: Send
    + std::fmt::Debug
    + serde::Serialize
{
}

/// Replicated state machine
pub trait State: Default + Send + 'static {
    type Command: Command;
    type Response: Response;
    fn execute(&mut self, slot: usize, command: Self::Command) -> Option<Self::Response>;
}
```

From there, you can create an instance of `Config` with the appropriate
parameters and types:

```rust
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
```

Finally, you can launch your server using the `tokio::run_async` or `tokio::spawn_async`.
For example:

```rust
mod state;

fn main() {
  let config = Config::<state::State>::new(0, 10000, 1);
  tokio::run_async(config.run())
}
```

Take a look at the `chatroom` sub-crate for an example of how to launch and communicate
with servers.

## References

1. Lamport, Leslie (1998). ["The Part-Time Parliament"][3]
2. Lamport, Leslie (2001). ["Paxos Made Simple"][4]
3. Renesse, Robbert Van and Altinbuken, Deniz (2015). ["Paxos Made Moderately Complex"][2].

[1]: https://en.wikipedia.org/wiki/Paxos_(computer_science)
[2]: http://paxos.systems/index.html
[3]: https://lamport.azurewebsites.net/pubs/lamport-paxos.pdf
[4]: https://lamport.azurewebsites.net/pubs/paxos-simple.pdf

#![feature(await_macro, async_await, futures_api, pin)]

#[macro_use]
extern crate tokio;

use std::collections::HashMap as Map;

use structopt::StructOpt;
use tokio::prelude::*;
use tokio_serde_bincode::{ReadBincode, WriteBincode};
use tokio::codec;

mod command;

use crate::command::{Command, Execution};

#[derive(StructOpt)]
#[structopt(name = "harness")]
struct Opt {
    /// Paxos server binary
    #[structopt(short = "s", long = "server")]
    server: std::path::PathBuf,

    /// Test file
    #[structopt(short = "f", long = "file")]
    file: std::path::PathBuf,

    /// Logging output verbosity
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,
}

struct Server(std::process::Child);

impl Server {
    fn new(
        path: &std::path::PathBuf,
        id: usize,
        port: usize,
        count: usize,
        verbose: u8,
    ) -> Self {
        let id = id.to_string();
        let port = port.to_string();
        let count = count.to_string();
        let mut command = std::process::Command::new(path);

        if verbose > 0 {
            let verbosity = "-".to_string() + &"v".repeat(verbose as usize);
            command.arg(&verbosity);
        }

        command.args(&["-i", &id])
            .args(&["-p", &port])
            .args(&["-c", &count])
            .spawn()
            .map(Server)
            .expect("[INTERNAL ERROR]: could not spawn server")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.0.kill().ok();
    }
}

async fn run() {

    let opt = Opt::from_args();

    // Test execution
    let execution: Execution = std::fs::File::open(opt.file)
        .map(serde_json::from_reader)
        .expect("[INTERNAL ERROR]: could not find file")
        .expect("[INTERNAL ERROR]: could not parse test");

    // TCP connections
    let mut connections: Map<usize, tokio::net::tcp::TcpStream> = Map::default();

    // Running servers
    let mut servers: Map<usize, Server> = Map::default();

    // Server ports
    let mut ports: Map<usize, usize> = Map::default();

    // Local command identifier
    let mut counter = 0;

    for command in execution.0 {
        println!("Executing command {:?}", command);
        match command {
        | Command::Start { id, port, count } => {
            servers.insert(id, Server::new(&opt.server, id, port, count, opt.verbose));
            ports.insert(id, port);
        }
        | Command::Connect { id } => {
            let connection = format!("127.0.0.1:{}", ports[&id])
                .parse::<std::net::SocketAddr>()
                .map(|address| std::net::TcpStream::connect(&address).unwrap())
                .map(|stream| tokio::net::tcp::TcpStream::from_std(stream, &tokio::reactor::Handle::default()))
                .unwrap()
                .expect("[INTERNAL ERROR]: could not connect to server");
            connections.insert(id, connection);
        }
        | Command::Disconnect { id } => {
            connections.remove(&id);
        }
        | Command::Get { id } => {
            let writer = connections[&id].try_clone().unwrap();
            let client_id = id;
            let command = chatroom::Command {
                client_id,
                local_id: counter,
                mode: chatroom::Mode::Get,
            };

            counter += 1;
            tokio::spawn_async(async {
                let writer = WriteBincode::new(
                    codec::length_delimited::Builder::new()
                        .new_write(writer)
                        .sink_from_err::<bincode::Error>()
                );
                let _ = await!(writer.send(command));
            });

            let mut reader = ReadBincode::new(
                codec::length_delimited::Builder::new()
                    .new_read(&connections[&id])
                    .from_err::<bincode::Error>()
            );

            if let Some(Ok(chatroom::Response::Messages(messages))) = await!(reader.next()) {
                println!("Client {} received message log {:?}", id, messages);
            }
        }
        | Command::Put { id, message } => {
            let writer = connections[&id].try_clone().unwrap();
            let client_id = id;
            let command = chatroom::Command {
                client_id,
                local_id: counter,
                mode: chatroom::Mode::Put(message),
            };

            counter += 1;
            tokio::spawn_async(async move {
                let writer = WriteBincode::new(
                    codec::length_delimited::Builder::new()
                        .new_write(writer)
                        .sink_from_err::<bincode::Error>()
                );
                let _ = await!(writer.send(command));
            });
        }
        | Command::Crash { id } => {
            servers.remove(&id);
        }
        | Command::Sleep { ms } => {
            std::thread::sleep(std::time::Duration::from_millis(ms))
        }
        }
    }
}

fn main() {
    tokio::run_async(run());
}

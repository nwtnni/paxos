#![feature(await_macro, async_await, futures_api, pin)]

#[macro_use]
extern crate tokio;

use std::collections::HashMap as Map;
use std::sync::{Arc, atomic};

use structopt::StructOpt;
use tokio::prelude::*;
use tokio_serde_bincode::{ReadBincode, WriteBincode};
use tokio::codec;

mod command;
mod socket;

use crate::command::{Command, Execution};

#[derive(StructOpt)]
#[structopt(name = "harness")]
struct Opt {
    #[structopt(short = "s", long = "server")]
    server: std::path::PathBuf,

    #[structopt(short = "f", long = "file")]
    file: std::path::PathBuf,
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
    let mut servers: Map<usize, std::process::Child> = Map::default();

    // Server ports
    let mut ports: Map<usize, usize> = Map::default();

    // Operation identifier
    let operations: Arc<atomic::AtomicUsize> = Arc::new(atomic::AtomicUsize::new(0));

    for command in execution.0 {
        println!("Executing command {:?}", command);
        match command {
        | Command::Start { id, port, count } => {
            let child = std::process::Command::new(&opt.server)
                .args(&["-i", &id.to_string()])
                .args(&["-p", &port.to_string()])
                .args(&["-c", &count.to_string()])
                .spawn()
                .expect("[INTERNAL ERROR]: could not spawn server");
            println!("child {:?}", child);
            servers.insert(id, child);
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
            let counter = operations.clone();
            let client_id = id;
            let local_id = counter.fetch_add(1, atomic::Ordering::SeqCst);
            let command = chatroom::Command {
                client_id,
                local_id,
                mode: chatroom::Mode::Get,
            };

            tokio::spawn_async(async {
                let writer = WriteBincode::new(
                    codec::length_delimited::Builder::new()
                        .new_write(writer)
                        .sink_from_err::<bincode::Error>()
                );
                let _ = await!(writer.send(command));
            });

            let reader = ReadBincode::new(
                codec::length_delimited::Builder::new()
                    .new_read(&connections[&id])
                    .from_err::<bincode::Error>()
            );

            let (response, _) = await!(reader.into_future())
                .map_err(|_| ())
                .unwrap();

            if let Some(chatroom::Response::Messages(messages)) = response {
                println!("Client {} received message log {:?}", id, messages);
            }
        }
        | Command::Put { id, message } => {
            let writer = connections[&id].try_clone().unwrap();
            let counter = operations.clone();
            let client_id = id;
            let local_id = counter.fetch_add(1, atomic::Ordering::SeqCst);
            let command = chatroom::Command {
                client_id,
                local_id,
                mode: chatroom::Mode::Put(message),
            };

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
            if let Some(mut server) = servers.remove(&id) {
                server.kill().ok();
            }
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

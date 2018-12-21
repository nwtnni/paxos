#![feature(await_macro, async_await, futures_api, pin)]

use std::collections::HashMap as Map;
use std::sync::{Arc, Mutex, atomic};

use structopt::StructOpt;
use tokio::prelude::*;
use tokio_serde_bincode::{ReadBincode, WriteBincode};

mod command;
mod socket;

use crate::command::{Command, Execution};

#[derive(StructOpt)]
struct Opt {
    #[structopt(short = "s", long = "server")]
    server: std::path::PathBuf,

    #[structopt(short = "f", long = "file")]
    file: std::path::PathBuf,
}

fn main() -> Result<(), Box<std::error::Error>> {

    let opt = Opt::from_args();

    // Test execution
    let execution: Execution = std::fs::File::open(opt.file)
        .map(serde_json::from_reader)??;

    let mut readers: Map<usize, socket::Rx> = Map::default();

    let mut writers: Map<usize, Arc<Mutex<socket::Tx>>> = Map::default();

    // Running servers
    let mut servers: Map<usize, std::process::Child> = Map::default();

    // Server ports
    let mut ports: Map<usize, usize> = Map::default();

    // Operation identifier
    let operations: Arc<atomic::AtomicUsize> = Arc::new(atomic::AtomicUsize::new(0));

    for command in execution.0 {
        match command {
        | Command::Start { id, port, count } => {
            let child = std::process::Command::new(&opt.server)
                .args(&["-i", &id.to_string()])
                .args(&["-p", &port.to_string()])
                .args(&["-c", &count.to_string()])
                .spawn()?;
            servers.insert(id, child);
            ports.insert(id, port);
        }
        | Command::Connect { id } => {
            let connection = format!("127.0.0.1:{}", ports[&id])
                .parse::<std::net::SocketAddr>()
                .map(|address| tokio::net::tcp::TcpStream::connect(&address))?
                .wait()?;
            let (rx, tx) = socket::split(connection);
            readers.insert(id, rx);
            writers.insert(id, Arc::new(Mutex::new(tx)));
        }
        | Command::Disconnect { id } => {
            readers.remove(&id);
            writers.remove(&id);
        }
        | Command::Get { id } => {
            let writer = writers[&id].clone();
            let counter = operations.clone();
            tokio::spawn_async(async move {
                let client_id = id;
                let local_id = counter.fetch_add(1, atomic::Ordering::SeqCst);
                let command = chatroom::Command {
                    client_id,
                    local_id,
                    mode: chatroom::Mode::Get,
                };
                WriteBincode::new(&mut *writer.lock().unwrap())
                    .send(command)
                    .wait()
                    .unwrap();
            });

            let mut reader = readers.get_mut(&id).unwrap();
            let (response, _) = ReadBincode::new(&mut reader)
                .into_future()
                .wait()
                .map_err(|_| ())
                .unwrap();

            if let Some(chatroom::Response::Messages(messages)) = response {
                println!("Client {} received message log {:?}", id, messages);
            }
        }
        | Command::Put { id, message } => {
            let writer = writers[&id].clone();
            let counter = operations.clone();
            tokio::spawn_async(async move {
                let client_id = id;
                let local_id = counter.fetch_add(1, atomic::Ordering::SeqCst);
                let command = chatroom::Command {
                    client_id,
                    local_id,
                    mode: chatroom::Mode::Put(message),
                };
                WriteBincode::new(&mut *writer.lock().unwrap())
                    .send(command)
                    .wait()
                    .unwrap();
            });
        }
        | Command::Crash { id } => {
            if let Some(mut server) = servers.remove(&id) {
                server.kill().ok();
            }
        }
        }
    }

    Ok(())
}

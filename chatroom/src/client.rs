#![feature(await_macro, async_await, futures_api, pin)]

#[macro_use]
extern crate tokio;

use std::io::{BufReader, BufRead};

use structopt::StructOpt;
use tokio::prelude::*;

enum Command {
    /// Connect to the given replica
    Connect {
        port: usize,
    },

    /// Disconnect from the current replica
    Disconnect,

    /// Send a GET request to the current replica
    Get,

    /// Send a PUT request to the current replica
    Put {
        message: String,
    },

    Help,
}

fn usage() {
    println!(
        "{}{}{}{}{}{}{}",
        "--------------------------------------------------------------\n",
        "Possible commands:\n",
        "connect <PORT> | c <PORT>  -- Connect to server at <PORT>\n",
        "disconnect     | d         -- Disconnect from current server\n",
        "get            | g         -- Get chat log from current server\n",
        "put <MSG>      | p <MSG>   -- Write <MSG> to current server\n",
        "--------------------------------------------------------------",
    );
}

impl std::str::FromStr for Command {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.trim().splitn(2, " ");
        match iter.next() {
        | Some("help") | Some("h") => Ok(Command::Help),
        | Some("get") | Some("g") => Ok(Command::Get),
        | Some("disconnect") | Some("d") => Ok(Command::Disconnect),
        | Some("connect") | Some("c") => {
            iter.next()
                .ok_or(())
                .and_then(|port| port.parse().map_err(|_| ()))
                .map(|port| Command::Connect { port })
        }
        | Some("put") | Some("p") => {
            iter.next()
                .map(|message| Command::Put { message: message.to_string() })
                .ok_or(())
        }
        | _ => Err(()),
        }
    }
}

async fn run(id: usize) {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut reader: Option<paxos::socket::Rx<chatroom::Response>> = None;
    let mut writer: Option<paxos::socket::Tx<chatroom::Command>> = None;
    let mut counter = 0;
    let mut lines = BufReader::new(stdin)
        .lines()
        .filter_map(|line| line.ok());
    loop {
        print!("> ");
        stdout.flush().unwrap();
        if let Ok(command) = lines.next().unwrap().parse::<Command>() {
            match command {
            | Command::Connect { port } => {
                let stream = format!("127.0.0.1:{}", port)
                    .parse::<std::net::SocketAddr>()
                    .map(|addr| std::net::TcpStream::connect(&addr).unwrap())
                    .map(|stream| tokio::net::TcpStream::from_std(stream, &tokio::reactor::Handle::default()))
                    .unwrap()
                    .expect("[INTERNAL ERROR]: could not connect to server");
                let (rx, tx) = paxos::socket::split(stream);
                reader = Some(rx);
                writer = Some(tx);
            }
            | Command::Disconnect => {
                reader = None;
                writer = None;
            }
            | Command::Get => {
                if reader.is_none() || writer.is_none() {
                    println!("[ERROR]: not connected to a server");
                    continue
                }
                counter += 1;
                let writer = writer.as_mut().unwrap();
                let reader = reader.as_mut().unwrap();
                let _ = await!(
                    writer.send(chatroom::Command {
                        client_id: id,    
                        local_id: counter,
                        mode: chatroom::Mode::Get,
                    })
                );
                if let Some(Ok(chatroom::Response::Messages(messages))) = await!(reader.next())  {
                    println!("[RESPONSE]: {:?}", messages);
                }
            }
            | Command::Put { message } => {
                if writer.is_none() {
                    println!("[ERROR]: not connected to a server");
                    continue
                }
                counter += 1;
                let writer = writer.as_mut().unwrap();
                let _ = await!(
                    writer.send(chatroom::Command {
                        client_id: id,    
                        local_id: counter,
                        mode: chatroom::Mode::Put(message),
                    })
                );
            }
            | Command::Help => usage(),
            }
        } else {
            println!("[ERROR]: could not parse command");
        }
    }
}

#[derive(StructOpt)]
#[structopt(name = "chatroom-client")]
struct Opt {
    /// Unique client ID
    #[structopt(short = "i", long = "id")]
    id: usize
}

fn main() {
    let opt = Opt::from_args();
    tokio::run_async(run(opt.id))
}

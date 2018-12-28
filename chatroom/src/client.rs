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
                .ok_or(())
                .map(|message| Command::Put { message: message.to_string() })
        }
        | _ => Err(()),
        }
    }
}

async fn run(id: usize) {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut reader: Option<paxos::external::Rx<chatroom::Response>> = None;
    let mut writer: Option<paxos::external::Tx<chatroom::Command>> = None;
    let mut counter = 0;
    let mut lines = BufReader::new(stdin)
        .lines()
        .filter_map(|line| line.ok());

    // Main interaction loop
    loop {
        print!("> ");
        stdout.flush().unwrap();

        // Attempt to parse command from user input
        let command = match lines.next().unwrap().parse::<Command>() {
        | Ok(command) => command,
        | Err(())     => {
            println!("[ERROR]: could not parse command");
            continue
        }
        };

        match command {
        | Command::Connect { port } => {
            let addr = format!("127.0.0.1:{}", port);

            // Attempt to parse port
            let addr = match addr.parse::<std::net::SocketAddr>() {
            | Ok(addr) => addr,
            | _ => {
                println!("[ERROR]: invalid port {}", port);
                continue
            }
            };

            // Attempt to connect to server
            let stream = match await!(tokio::net::TcpStream::connect(&addr)) {
            | Ok(stream) => {
                println!("[RESPONSE]: connected to server at port {}", port);
                stream
            }
            |_ => {
                println!("[ERROR]: failed to connect to server at port {}", port);
                continue
            }
            };

            let (rx, tx) = paxos::external::new(stream);
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
            let command = chatroom::Command {
                client_id: id,
                local_id: counter,
                mode: chatroom::Mode::Get,
            };

            // Write GET command
            match await!(writer.send(command)) {
            | Ok(_) => println!("[RESPONSE]: successfully sent GET request"),
            | Err(_) => println!("[ERROR]: failed to send GET request"),
            };

            // Listen for response 
            if let Some(Ok(chatroom::Response::Messages(messages))) = await!(reader.next())  {
                println!("[RESPONSE]: {:?}", messages);
            }
        }
        | Command::Put { message } => {
            if reader.is_none() || writer.is_none() {
                println!("[ERROR]: not connected to a server");
                continue
            }

            counter += 1;
            let writer = writer.as_mut().unwrap();
            let command = chatroom::Command {
                client_id: id,
                local_id: counter,
                mode: chatroom::Mode::Put(message),
            };

            // Write PUT command
            match await!(writer.send(command)) {
            | Ok(_) => println!("[RESPONSE]: successfully sent message"),
            | Err(_) => println!("[ERROR]: failed to send message"),
            };
        }
        | Command::Help => usage(),
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

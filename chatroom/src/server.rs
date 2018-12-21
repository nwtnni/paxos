use serde_derive::{Serialize, Deserialize};
use paxos;

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct Command {
    client_id: usize,
    local_id: usize,
    mode: Mode,
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub enum Mode {
    Get,
    Put(String),
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub enum Response {
    Connected(usize),
    Received(usize),
    Messages(Vec<String>),
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Default)]
pub struct State {
    messages: Vec<String>,
}

impl paxos::Command for Command {
    type ClientID = usize;
    type LocalID = usize;
    fn client_id(&self) -> Self::ClientID {
        self.client_id
    }
    fn local_id(&self) -> Self::LocalID {
        self.local_id
    }
}

impl paxos::State for State {
    type Command = Command;
    type Response = Response;
    fn execute(&mut self, slot: usize, command: Self::Command) -> Self::Response {
        match command.mode {
        | Mode::Get => Response::Messages(self.messages.clone()),
        | Mode::Put(message) => {
            self.messages.push(message);
            Response::Received(slot)
        }
        }
    }
}

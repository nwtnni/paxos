use serde_derive::{Serialize, Deserialize};
use paxos;

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct Command {
    pub client_id: usize,
    pub local_id: usize,
    pub mode: Mode,
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
    Messages(Vec<String>),
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Default)]
pub struct State {
    pub messages: Vec<String>,
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
    fn execute(&mut self, slot: usize, command: Self::Command) -> Option<Self::Response> {
        match command.mode {
        | Mode::Get => {
            Some(Response::Messages(self.messages.clone()))
        }
        | Mode::Put(message) => {
            self.messages.push(message);
            None
        },
        }
    }
}

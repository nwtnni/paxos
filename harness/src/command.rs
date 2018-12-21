use serde_derive::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Execution(pub Vec<Command>);

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Command {
    /// Start a replica with the given parameters
    Start {
        id: usize,
        port: usize,
        count: usize,
    },

    /// Connect to the given replica
    Connect {
        id: usize,
    },

    /// Disconnect from the given replica
    Disconnect {
        id: usize,
    },

    /// Send a GET request to the specified replica
    Get {
        id: usize,
    },

    /// Send a PUT request to the specified replica
    Put {
        id: usize,
        message: String,
    },

    /// Crash the specified replica
    Crash {
        id: usize,
    },
}

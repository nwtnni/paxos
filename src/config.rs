use std::marker;

use futures::sync::mpsc;

use crate::{state, thread};

const DEFAULT_PORT: usize = 20000;

#[derive(Copy, Clone, Debug)]
pub struct Config<C, R, S> {
    /// Unique replica ID
    id: usize,

    /// Port for incoming client requests
    port: usize,

    /// Total number of replicas
    count: usize,

    _marker: marker::PhantomData<(C, R, S)>,     
}

impl<C: state::Command, R: state::Response, S: state::State<C, R>> Config<C, R, S> {
    pub fn new(id: usize, port: usize, count: usize) -> Self {
        Config {
            id,
            port,
            count,
            _marker: Default::default(),
        }
    }

    pub async fn run(self) {





    }
}

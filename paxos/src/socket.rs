//! # Summary
//!
//! This module abstracts over external connections to clients and peer servers.
//! 
//! Currently uses `tokio-serde-bincode` to wrap around `tokio`'s length-delimited
//! codec, which in turn wraps around `tokio`'s asynchronous TCP stream. This allows
//! us to serialize and deserialize Rust structs through a TCP connection with
//! minimal boilerplate on the sending and receiving ends.

use futures::{sink, stream};
use tokio::{io, net};
use tokio::prelude::*;
use tokio::codec::{FramedRead, FramedWrite, LengthDelimitedCodec, length_delimited};
use tokio_serde_bincode::{ReadBincode, WriteBincode};

type ReadTcp = io::ReadHalf<net::TcpStream>;
type WriteTcp = io::WriteHalf<net::TcpStream>;

pub struct Rx<T>(ReadBincode<stream::FromErr<FramedRead<ReadTcp, LengthDelimitedCodec>, bincode::Error>, T>);
pub struct Tx<T>(WriteBincode<sink::SinkFromErr<FramedWrite<WriteTcp, LengthDelimitedCodec>, bincode::Error>, T>);

pub fn split<R, T>(stream: net::TcpStream) -> (Rx<R>, Tx<T>)
where R: serde::de::DeserializeOwned,
      T: serde::Serialize,
{
    let (rx, tx) = stream.split();
    let rx = length_delimited::Builder::new()
        .new_read(rx)
        .from_err::<bincode::Error>();
    let tx = length_delimited::Builder::new()
        .new_write(tx)
        .sink_from_err::<bincode::Error>();
    (Rx(ReadBincode::new(rx)), Tx(WriteBincode::new(tx)))
}

impl<R: serde::de::DeserializeOwned> Stream for Rx<R> {
    type Item = R;
    type Error = ();

    #[inline]
    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        self.0.poll().map_err(|_| ())
    }
}

impl<T: serde::Serialize> Sink for Tx<T> {
    type SinkItem = T; 
    type SinkError = ();

    #[inline]
    fn start_send(&mut self, item: Self::SinkItem) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        self.0.start_send(item).map_err(|_| ())
    }

    #[inline]
    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        self.0.poll_complete().map_err(|_| ())
    }
}

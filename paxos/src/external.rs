//! # Summary
//!
//! This module abstracts over external connections to clients and peer servers.
//! 
//! Currently uses `tokio-serde-bincode` to wrap around `tokio`'s length-delimited
//! codec, which in turn wraps around `tokio`'s asynchronous TCP stream. This allows
//! us to serialize and deserialize Rust structs through a TCP connection with
//! minimal boilerplate on the sending and receiving ends.

use futures::{sink, stream};
use tokio::prelude::*;
use tokio::codec::{FramedRead, FramedWrite, LengthDelimitedCodec, length_delimited};
use tokio_serde_bincode::{ReadBincode, WriteBincode};

/// External receiving channel. Expects length-delimited, bincode-encoded
/// Rust data of type `T` sent via TCP.
pub struct Rx<T>(
    ReadBincode<
        stream::FromErr<
            FramedRead<
                tokio::io::ReadHalf<tokio::net::TcpStream>,
                LengthDelimitedCodec,
            >,
            bincode::Error,
        >,
        T
    >
);

/// External transmission channel. Sends length-delimited, bincode-encoded
/// Rust data of type `T` over TCP.
pub struct Tx<T>(
    WriteBincode<
        sink::SinkFromErr<
            FramedWrite<
                tokio::io::WriteHalf<tokio::net::TcpStream>,
                LengthDelimitedCodec,
            >,
            bincode::Error,
        >,
        T,
    >
);

/// Split a `tokio::net::TcpStream` into a pair of receiving and transmitting
/// channels capable of reading and writing bincode-encoded data.
pub fn new<R, T>(stream: tokio::net::TcpStream) -> (Rx<R>, Tx<T>)
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

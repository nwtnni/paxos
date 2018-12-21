use futures::{sink, stream};
use tokio::{io, net};
use tokio::prelude::*;
use tokio::codec::{FramedRead, FramedWrite, LengthDelimitedCodec, length_delimited};

type ReadTcp = io::ReadHalf<net::TcpStream>;
type WriteTcp = io::WriteHalf<net::TcpStream>;

pub type Rx = stream::FromErr<FramedRead<ReadTcp, LengthDelimitedCodec>, bincode::Error>;
pub type Tx = sink::SinkFromErr<FramedWrite<WriteTcp, LengthDelimitedCodec>, bincode::Error>;

pub fn split(stream: net::tcp::TcpStream) -> (Rx, Tx) {
    let (rx, tx) = stream.split();
    let rx = length_delimited::Builder::new()
        .new_read(rx)
        .from_err::<bincode::Error>();
    let tx = length_delimited::Builder::new()
        .new_write(tx)
        .sink_from_err::<bincode::Error>();
    (rx, tx)
}

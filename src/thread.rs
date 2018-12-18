pub(crate) mod acceptor;
pub(crate) mod commander;
pub(crate) mod leader;
pub(crate) mod peer;
pub(crate) mod replica;
pub(crate) mod scout;

use futures::{sink, stream};
use futures::sync::mpsc;
use tokio::{io, net};
use tokio::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_serde_bincode::{
    ReadBincode as RB,
    WriteBincode as WB,
};

pub type Tx<T> = mpsc::UnboundedSender<T>;
pub type Rx<T> = mpsc::UnboundedReceiver<T>;

type ReadTcp = io::ReadHalf<net::TcpStream>;
type WriteTcp = io::WriteHalf<net::TcpStream>;

pub type SocketRx<T> = RB<stream::FromErr<FramedRead<ReadTcp, LengthDelimitedCodec>, bincode::Error>, T>;
pub type SocketTx = sink::SinkFromErr<FramedWrite<WriteTcp, LengthDelimitedCodec>, bincode::Error>;

#![feature(await_macro, async_await, futures_api, pin)]

use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "chatroom")]
struct Opt {
    /// Unique server ID
    #[structopt(short = "i", long = "id")]
    id: usize,

    /// Port to listen on for client requests
    #[structopt(short = "p", long = "port")]
    port: usize,

    /// Total number of servers
    #[structopt(short = "c", long = "count")]
    count: usize,

    /// Timeout between servers (in milliseconds)
    #[structopt(short = "t", long = "timeout", default_value = "1000")]
    timeout: u64,
}

fn main() {
    let opt = Opt::from_args();

    let config = paxos::Config::<chatroom::State>::new(
            opt.id,
            opt.port,
            opt.count
        ).with_timeout(
            std::time::Duration::from_millis(opt.timeout)
        );

    tokio::run_async(config.run());
}

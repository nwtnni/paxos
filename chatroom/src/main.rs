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

    /// Logging level
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbosity: u8,
}

fn main() {
    let opt = Opt::from_args();
    let id = opt.id;
    let level = match opt.verbosity {
    | 0 => log::LevelFilter::Off,
    | 1 => log::LevelFilter::Info,
    | 2 => log::LevelFilter::Debug,
    | _ => log::LevelFilter::Trace,
    };

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}]: {}",
                id,
                record.level(),
                record.target(),
                message
            ))
        })
        .level_for("paxos", level)
        .level_for("tokio_io", log::LevelFilter::Off)
        .level_for("tokio_threadpool", log::LevelFilter::Off)
        .level_for("tokio_reactor", log::LevelFilter::Off)
        .level_for("mio", log::LevelFilter::Off)
        .chain(std::io::stdout())
        .apply()
        .unwrap();

    let config = paxos::Config::<chatroom::State>::new(
            opt.id,
            opt.port,
            opt.count
        ).with_timeout(
            std::time::Duration::from_millis(opt.timeout)
        );

    tokio::run_async(config.run());
}

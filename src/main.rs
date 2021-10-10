mod err;
mod opt;
mod routes;
mod server;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), err::DisplayError> {
    let opt::Options {
        verbose,
        listen_addr,
        secret_key,
    } = structopt::StructOpt::from_args();

    env_logger::Builder::new()
        .filter_level(match verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        })
        .init();

    server::run(listen_addr, secret_key).await?;

    Ok(())
}

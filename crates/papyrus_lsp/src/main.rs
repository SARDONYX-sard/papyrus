mod convert;
mod handlers;
mod server;
mod state;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (connection, io_threads) = lsp_server::Connection::stdio();
    server::run(connection)?;
    io_threads.join()?;

    Ok(())
}

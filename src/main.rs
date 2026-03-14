use clap::Parser;
use rmcp::ServiceExt;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use mcp_server_ableton::config::{Cli, Command};
use mcp_server_ableton::server::AbletonMcpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(Command::Install { force }) = &cli.command {
        mcp_server_ableton::installer::install(*force)?;
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let ct = CancellationToken::new();
    let _guard = ct.clone().drop_guard();
    let server = AbletonMcpServer::new(ct);

    tracing::info!("starting stdio transport");
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;

    Ok(())
}

use std::sync::Arc;

use clap::Parser;
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use tracing_subscriber::EnvFilter;

use mcp_server_ableton::config::{Cli, Config, Transport};
use mcp_server_ableton::server::AbletonMcpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let config = Config::from_cli(cli)?;
    let config = Arc::new(config);

    let server = AbletonMcpServer::new(config.clone());

    match &config.transport {
        Transport::Stdio => {
            tracing::info!("starting stdio transport");
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
        }
        Transport::Http { host, port } => {
            let addr = std::net::SocketAddr::new(*host, *port);
            tracing::info!("starting HTTP transport on {addr}");

            let ct = tokio_util::sync::CancellationToken::new();

            let session_manager: Arc<LocalSessionManager> = Arc::default();
            let service = rmcp::transport::StreamableHttpService::new(
                move || Ok(server.clone()),
                session_manager,
                rmcp::transport::StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let router = axum::Router::new().nest_service("/mcp", service);
            let listener = tokio::net::TcpListener::bind(addr).await?;

            tracing::info!("listening on {addr}");
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown_signal(ct))
                .await?;
        }
    }

    Ok(())
}

async fn shutdown_signal(ct: tokio_util::sync::CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    tokio::select! {
        () = ctrl_c => {},
        () = ct.cancelled() => {},
    }

    tracing::info!("shutting down");
    ct.cancel();
}

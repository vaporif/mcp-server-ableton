use std::net::IpAddr;

use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "mcp-server-ableton", about = "MCP server for Ableton Live")]
pub struct Cli {
    /// Transport protocol
    #[arg(long, default_value = "stdio", env = "MCP_TRANSPORT")]
    pub transport: TransportArg,

    /// Host to bind for HTTP transport
    #[arg(long, default_value = "127.0.0.1", env = "HOST")]
    pub host: IpAddr,

    /// Port for HTTP transport
    #[arg(long, default_value = "3000", env = "PORT")]
    pub port: u16,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum TransportArg {
    Stdio,
    StreamableHttp,
}

pub enum Transport {
    Stdio,
    Http { host: IpAddr, port: u16 },
}

pub struct Config {
    pub transport: Transport,
}

impl Config {
    /// # Errors
    /// Returns an error if configuration is invalid.
    pub fn from_cli(cli: Cli) -> Result<Self, crate::errors::Error> {
        let transport = match cli.transport {
            TransportArg::Stdio => Transport::Stdio,
            TransportArg::StreamableHttp => Transport::Http {
                host: cli.host,
                port: cli.port,
            },
        };

        Ok(Self { transport })
    }
}

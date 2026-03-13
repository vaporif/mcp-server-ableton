use std::net::IpAddr;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "mcp-server-ableton", about = "MCP server for Ableton Live")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(long, default_value = "stdio", env = "MCP_TRANSPORT")]
    pub transport: TransportArg,

    #[arg(long, default_value = "127.0.0.1", env = "HOST")]
    pub host: IpAddr,

    #[arg(long, default_value = "3000", env = "PORT")]
    pub port: u16,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Install {
        #[arg(long)]
        force: bool,
    },
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
    pub const fn from_cli(cli: &Cli) -> Result<Self, crate::errors::Error> {
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

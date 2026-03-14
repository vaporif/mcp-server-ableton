use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "mcp-server-ableton", about = "MCP server for Ableton Live")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Install {
        #[arg(long)]
        force: bool,
    },
}

# Ableton MCP Server Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust MCP server that bridges Claude to Ableton Live via OSC, supporting transport control, track/clip/device manipulation, MIDI note generation, and batch operations.

**Architecture:** Stateless MCP server (stdio/HTTP) communicates with AbletonOSC (Max for Live device) over UDP. Tools call an `OscClient` directly (flat dispatch). OscClient is lazily initialized via `OnceCell` on first tool call (MCP handshake succeeds without Ableton). Ephemeral recv port (`127.0.0.1:0`) allows multiple instances. Every mutation returns updated entity state plus a session summary. Queries are serialized through a mutex. `FromOsc` trait provides type-safe response parsing.

**Tech Stack:** Rust 2024, rmcp 1.1 (MCP SDK), rosc (OSC encoding), tokio (async runtime), clap (CLI), thiserror/anyhow (errors), include_dir (asset embedding), dirs (platform paths), serde/schemars (serialization/schema).

**Spec:** `docs/specs/2026-03-12-ableton-mcp-server-design.md`

---

## Chunk 1: Foundation (Installer + OSC Transport + Error Types)

### Task 1: Add dependencies to Cargo.toml

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add new dependencies**

Add to `[dependencies]`:

```toml
rosc = "0.11"
include_dir = "0.7"
dirs = "6"
```

Add to `tokio` features: `"net"` (for UDP sockets).

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "Add rosc, include_dir, dirs dependencies"
```

---

### Task 2: Add AbletonOSC git submodule

**Files:**
- Create: `.gitmodules`
- Create: `AbletonOSC/` (submodule)

- [ ] **Step 1: Add the submodule**

```bash
git submodule add https://github.com/ideoforms/AbletonOSC.git AbletonOSC
```

- [ ] **Step 2: Pin to a specific commit**

Check the latest stable commit on the AbletonOSC repo and pin:

```bash
cd AbletonOSC && git checkout <latest-stable-commit> && cd ..
git add AbletonOSC
```

- [ ] **Step 3: Verify submodule is tracked**

Run: `git submodule status`
Expected: shows pinned commit hash for AbletonOSC

- [ ] **Step 4: Commit**

```bash
git add .gitmodules AbletonOSC
git commit -m "Add AbletonOSC as git submodule"
```

---

### Task 3: Extend error types

**Files:**
- Modify: `src/errors.rs`

- [ ] **Step 1: Add OSC and installer error variants**

```rust
use rmcp::ErrorData as McpError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("config: {0}")]
    Config(String),

    #[error("OSC timeout: AbletonOSC not responding — is the Max for Live device loaded in your Ableton session?")]
    OscTimeout,

    #[error("OSC decode error: {0}")]
    OscDecode(String),

    #[error("unexpected OSC response: {0}")]
    UnexpectedResponse(String),

    #[error("installer error: {0}")]
    Installer(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<Error> for McpError {
    fn from(err: Error) -> Self {
        let message = err.to_string();
        tracing::error!("{message}");
        Self::internal_error(message, None)
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add src/errors.rs
git commit -m "Add OSC and installer error variants"
```

---

### Task 4: Implement AbletonOSC installer

**Files:**
- Create: `src/installer.rs`
- Modify: `src/lib.rs`
- Modify: `src/config.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/installer.rs`**

```rust
use std::path::PathBuf;

use include_dir::{Dir, include_dir};

use crate::errors::Error;

// Embed the AbletonOSC directory at compile time.
// This path is relative to the crate root (where Cargo.toml lives).
static ABLETON_OSC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/AbletonOSC");

/// Returns the platform-specific Ableton User Library path for Max MIDI Effects.
fn ableton_midi_effects_path() -> Result<PathBuf, Error> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().ok_or_else(|| Error::Installer("could not determine home directory".into()))?;
        Ok(home.join("Music/Ableton/User Library/Presets/MIDI Effects/Max MIDI Effect"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = dirs::data_dir().ok_or_else(|| Error::Installer("could not determine AppData directory".into()))?;
        Ok(appdata.join("Ableton/User Library/Presets/MIDI Effects/Max MIDI Effect"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(Error::Installer("AbletonOSC installer only supports macOS and Windows".into()))
    }
}

/// Install AbletonOSC into the Ableton User Library.
pub fn install(force: bool) -> Result<(), Error> {
    let target_base = ableton_midi_effects_path()?;
    let target_dir = target_base.join("AbletonOSC");

    if !target_base.exists() {
        return Err(Error::Installer(format!(
            "Ableton User Library not found at {}. Is Ableton Live installed?",
            target_base.display()
        )));
    }

    if target_dir.exists() && !force {
        println!("AbletonOSC is already installed at {}", target_dir.display());
        println!("Use --force to overwrite.");
        return Ok(());
    }

    // Extract embedded files to target directory
    ABLETON_OSC_DIR.extract(&target_base).map_err(|e| {
        Error::Installer(format!("failed to extract AbletonOSC: {e}"))
    })?;

    println!("AbletonOSC installed to {}", target_dir.display());
    println!();
    println!("Next steps:");
    println!("  1. Open Ableton Live");
    println!("  2. Drag AbletonOSC from your User Library into any track");
    println!("  3. You're ready to go!");

    Ok(())
}
```

- [ ] **Step 2: Add `installer` module to `src/lib.rs`**

Add `pub mod installer;` to `src/lib.rs`.

- [ ] **Step 3: Add `Install` subcommand to `src/config.rs`**

Update the `Cli` struct to use subcommands:

```rust
use std::net::IpAddr;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "mcp-server-ableton", about = "MCP server for Ableton Live")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

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

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install AbletonOSC into Ableton's User Library
    Install {
        /// Overwrite existing installation
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
    /// # Errors
    /// Returns an error if configuration is invalid.
    pub fn from_cli(cli: &Cli) -> Result<Self, crate::errors::Error> {
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
```

- [ ] **Step 4: Update `src/main.rs` to dispatch install subcommand**

Add early dispatch before server startup:

```rust
use clap::Parser;

use mcp_server_ableton::config::{Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle subcommands before starting the server
    if let Some(Command::Install { force }) = &cli.command {
        mcp_server_ableton::installer::install(*force)?;
        return Ok(());
    }

    // ... rest of existing server startup code, using Config::from_cli(&cli)
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build`
Expected: compiles. The `include_dir!` macro embeds the AbletonOSC submodule files.

- [ ] **Step 6: Test the install subcommand help**

Run: `cargo run -- install --help`
Expected: shows install subcommand with `--force` flag

- [ ] **Step 7: Commit**

```bash
git add src/installer.rs src/lib.rs src/config.rs src/main.rs
git commit -m "Add AbletonOSC installer with install subcommand"
```

---

### Task 5: Implement OscClient with dispatcher

**Files:**
- Create: `src/osc.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create `src/osc.rs`**

Key design points:
- Bind recv socket to `127.0.0.1:0` (ephemeral port, OS-assigned). AbletonOSC replies to sender address.
- Send to `127.0.0.1:11000` (AbletonOSC's fixed listen port).
- Background recv task with `CancellationToken` for shutdown.
- `query_mutex: tokio::sync::Mutex<()>` serializes all queries.
- `FromOsc` trait for type-safe response parsing.

```rust
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rosc::{OscMessage, OscPacket, OscType, encoder, decoder};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, oneshot};
use tokio_util::sync::CancellationToken;

use crate::errors::Error;

const SEND_ADDR: &str = "127.0.0.1:11000";
const RECV_ADDR: &str = "127.0.0.1:0"; // Ephemeral port — OS assigns
const QUERY_TIMEOUT: Duration = Duration::from_millis(1000);
const RECV_BUF_SIZE: usize = 65535;

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<OscMessage>>>>;

/// Type-safe extraction from AbletonOSC reply args.
/// AbletonOSC prepends index args to responses, so implementations
/// scan for the last value of the expected type.
pub trait FromOsc: Sized {
    fn from_osc(args: &[OscType]) -> Result<Self, Error>;
}

impl FromOsc for f64 {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            match arg {
                OscType::Float(f) => return Ok(f64::from(*f)),
                OscType::Double(d) => return Ok(*d),
                _ => continue,
            }
        }
        Err(Error::UnexpectedResponse("expected float in OSC args".into()))
    }
}

impl FromOsc for i32 {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            if let OscType::Int(i) = arg {
                return Ok(*i);
            }
        }
        Err(Error::UnexpectedResponse("expected int in OSC args".into()))
    }
}

impl FromOsc for String {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            if let OscType::String(s) = arg {
                return Ok(s.clone());
            }
        }
        Err(Error::UnexpectedResponse("expected string in OSC args".into()))
    }
}

impl FromOsc for bool {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            match arg {
                OscType::Bool(b) => return Ok(*b),
                OscType::Int(i) => return Ok(*i != 0),
                _ => continue,
            }
        }
        Err(Error::UnexpectedResponse("expected bool in OSC args".into()))
    }
}

pub struct OscClient {
    socket: UdpSocket,
    send_addr: SocketAddr,
    pending: PendingMap,
    query_mutex: Mutex<()>,
}

impl OscClient {
    pub async fn new(cancel: CancellationToken) -> Result<Arc<Self>, Error> {
        let socket = UdpSocket::bind(RECV_ADDR).await?;
        let send_addr: SocketAddr = SEND_ADDR.parse().expect("valid send address");
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

        let client = Arc::new(Self {
            socket,
            send_addr,
            pending: Arc::clone(&pending),
            query_mutex: Mutex::new(()),
        });

        let client_clone = Arc::clone(&client);
        let recv_pending = Arc::clone(&pending);
        tokio::spawn(async move {
            let mut buf = vec![0u8; RECV_BUF_SIZE];
            loop {
                tokio::select! {
                    () = cancel.cancelled() => {
                        tracing::debug!("OSC recv task shutting down");
                        break;
                    }
                    result = client_clone.socket.recv_from(&mut buf) => {
                        match result {
                            Ok((size, _addr)) => {
                                Self::handle_packet(&buf[..size], &recv_pending).await;
                            }
                            Err(e) => {
                                tracing::warn!("OSC recv error: {e}");
                            }
                        }
                    }
                }
            }
        });

        Ok(client)
    }

    async fn handle_packet(data: &[u8], pending: &PendingMap) {
        let packet = match decoder::decode_udp(data) {
            Ok((_, packet)) => packet,
            Err(e) => {
                tracing::warn!("failed to decode OSC packet: {e}");
                return;
            }
        };

        match packet {
            OscPacket::Message(msg) => {
                let mut map = pending.lock().await;
                if let Some(sender) = map.remove(&msg.addr) {
                    let _ = sender.send(msg);
                } else {
                    tracing::trace!("unsolicited OSC message: {}", msg.addr);
                }
            }
            OscPacket::Bundle(_) => {
                tracing::trace!("ignoring OSC bundle");
            }
        }
    }

    pub async fn send(&self, address: &str, args: Vec<OscType>) -> Result<(), Error> {
        let msg = OscMessage {
            addr: address.to_string(),
            args,
        };
        let packet = OscPacket::Message(msg);
        let buf = encoder::encode(&packet)
            .map_err(|e| Error::OscDecode(format!("failed to encode OSC message: {e}")))?;
        self.socket.send_to(&buf, self.send_addr).await?;
        Ok(())
    }

    pub async fn query(&self, address: &str, args: Vec<OscType>) -> Result<OscMessage, Error> {
        let _guard = self.query_mutex.lock().await;

        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(address.to_string(), tx);
        }

        self.send(address, args).await?;

        match tokio::time::timeout(QUERY_TIMEOUT, rx).await {
            Ok(Ok(msg)) => Ok(msg),
            Ok(Err(_)) | Err(_) => {
                let mut map = self.pending.lock().await;
                map.remove(address);
                Err(Error::OscTimeout)
            }
        }
    }

    /// Query and extract a typed value from the response using FromOsc.
    pub async fn query_val<T: FromOsc>(&self, address: &str, args: Vec<OscType>) -> Result<T, Error> {
        let msg = self.query(address, args).await?;
        T::from_osc(&msg.args)
    }
}
```

- [ ] **Step 2: Add `osc` module to `src/lib.rs`**

Add `pub mod osc;` to `src/lib.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add src/osc.rs src/lib.rs
git commit -m "Add OscClient with dispatcher and serialized queries"
```

---

### Task 6: Wire OscClient into AbletonMcpServer with OnceCell

**Files:**
- Modify: `src/server.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Update `src/server.rs` to use `OnceCell<Arc<OscClient>>`**

The OscClient is lazily initialized on first tool call. MCP handshake succeeds without Ableton.

```rust
use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool_handler, tool_router, ServerHandler};
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::errors::Error;
use crate::osc::OscClient;

#[derive(Clone)]
pub struct AbletonMcpServer {
    #[allow(dead_code)]
    config: Arc<Config>,
    osc_cell: Arc<OnceCell<Arc<OscClient>>>,
    cancel: CancellationToken,
    tool_router: ToolRouter<Self>,
}

impl AbletonMcpServer {
    #[must_use]
    pub fn new(config: Arc<Config>, cancel: CancellationToken) -> Self {
        let tool_router = Self::tool_router();
        Self {
            config,
            osc_cell: Arc::new(OnceCell::new()),
            cancel,
            tool_router,
        }
    }

    /// Get or lazily initialize the OscClient.
    pub async fn osc(&self) -> Result<&Arc<OscClient>, Error> {
        self.osc_cell
            .get_or_try_init(|| OscClient::new(self.cancel.child_token()))
            .await
    }
}

#[tool_router]
impl AbletonMcpServer {
    // Tools will be added in subsequent tasks
}

#[tool_handler]
impl ServerHandler for AbletonMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("mcp-server-ableton", env!("CARGO_PKG_VERSION")),
        )
    }
}
```

- [ ] **Step 2: Update `src/main.rs`**

Remove OscClient creation from main. Pass only `CancellationToken` to the server.

```rust
use std::sync::Arc;

use clap::Parser;
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use mcp_server_ableton::config::{Cli, Command, Config, Transport};
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

    let config = Config::from_cli(&cli)?;
    let config = Arc::new(config);

    let ct = CancellationToken::new();
    let server = AbletonMcpServer::new(config.clone(), ct.clone());

    match &config.transport {
        Transport::Stdio => {
            tracing::info!("starting stdio transport");
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
            ct.cancel();
        }
        Transport::Http { host, port } => {
            let addr = std::net::SocketAddr::new(*host, *port);
            tracing::info!("starting HTTP transport on {addr}");

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

async fn shutdown_signal(ct: CancellationToken) {
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
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add src/server.rs src/main.rs
git commit -m "Wire OscClient into AbletonMcpServer with OnceCell lazy init"
```

---

### Task 7: Add session summary helper and common types

**Files:**
- Create: `src/tools.rs`
- Create: `src/tools/common.rs`

Every tool response includes a session summary (`{tempo, is_playing, selected_track}`). This task creates the shared helper that all tools will use.

- [ ] **Step 1: Create `src/tools.rs` (module root)**

```rust
pub mod common;
```

- [ ] **Step 2: Create `src/tools/common.rs`**

Uses `OscClient::query_val` with `FromOsc` trait instead of manual extraction.

```rust
use std::sync::Arc;

use serde::Serialize;

use crate::errors::Error;
use crate::osc::OscClient;

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub tempo: f64,
    pub is_playing: bool,
    pub selected_track: i32,
}

/// Query the session summary from Ableton: tempo, is_playing, selected_track.
pub async fn query_session_summary(osc: &Arc<OscClient>) -> Result<SessionSummary, Error> {
    let tempo: f64 = osc.query_val("/live/song/get/tempo", vec![]).await?;
    let is_playing: bool = osc.query_val("/live/song/get/is_playing", vec![]).await?;
    let selected_track: i32 = osc.query_val("/live/view/get/selected_track", vec![]).await?;

    Ok(SessionSummary {
        tempo,
        is_playing,
        selected_track,
    })
}

/// Build a JSON tool response with data + session summary.
pub fn tool_response<T: Serialize>(data: &T, summary: &SessionSummary) -> Result<String, Error> {
    let mut value = serde_json::to_value(data)?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("session_summary".to_string(), serde_json::to_value(summary)?);
    }
    Ok(serde_json::to_string_pretty(&value)?)
}

/// Build a JSON tool response where data is the top-level object.
pub fn tool_response_raw(data: serde_json::Value, summary: &SessionSummary) -> Result<String, Error> {
    let mut value = data;
    if let Some(obj) = value.as_object_mut() {
        obj.insert("session_summary".to_string(), serde_json::to_value(summary)?);
    }
    Ok(serde_json::to_string_pretty(&value)?)
}
```

- [ ] **Step 3: Add `tools` module to `src/lib.rs`**

Add `pub mod tools;` to `src/lib.rs`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/tools.rs src/tools/common.rs src/lib.rs
git commit -m "Add session summary helper and common OSC extraction utils"
```

---

## Chunk 2: Transport + Track + Scene Tools

### Task 8: Transport tools

**Files:**
- Create: `src/tools/transport.rs`
- Modify: `src/tools.rs`
- Modify: `src/server.rs`

- [ ] **Step 1: Create `src/tools/transport.rs`**

```rust
use rmcp::model::{CallToolResult, Content};
use rmcp::{ErrorData as McpError, tool};
use rmcp::handler::server::wrapper::Parameters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::server::AbletonMcpServer;
use crate::tools::common::{self, SessionSummary};

#[derive(Debug, Serialize)]
struct TransportState {
    is_playing: bool,
    tempo: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTempoParams {
    /// Tempo in BPM (e.g., 120.0)
    pub bpm: f32,
}

impl AbletonMcpServer {
    /// Start playback in Ableton Live.
    #[tool(description = "Start playback in Ableton Live")]
    pub async fn ableton_play(&self) -> Result<CallToolResult, McpError> {
        self.osc().await.map_err(McpError::from)?.send("/live/play", vec![]).await.map_err(McpError::from)?;
        let summary = common::query_session_summary(self.osc().await.map_err(McpError::from)?).await.map_err(McpError::from)?;
        let state = TransportState { is_playing: summary.is_playing, tempo: summary.tempo };
        let json = common::tool_response(&state, &summary).map_err(McpError::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Stop playback in Ableton Live.
    #[tool(description = "Stop playback in Ableton Live")]
    pub async fn ableton_stop(&self) -> Result<CallToolResult, McpError> {
        self.osc().await.map_err(McpError::from)?.send("/live/stop", vec![]).await.map_err(McpError::from)?;
        let summary = common::query_session_summary(self.osc().await.map_err(McpError::from)?).await.map_err(McpError::from)?;
        let state = TransportState { is_playing: summary.is_playing, tempo: summary.tempo };
        let json = common::tool_response(&state, &summary).map_err(McpError::from)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get the current tempo (BPM) of the Ableton session.
    #[tool(description = "Get the current tempo (BPM) of the Ableton session")]
    pub async fn ableton_get_tempo(&self) -> Result<CallToolResult, McpError> {
        let summary = common::query_session_summary(self.osc().await.map_err(McpError::from)?).await.map_err(McpError::from)?;
        let json = serde_json::to_string_pretty(&summary).map_err(|e| McpError::from(crate::errors::Error::from(e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Set the tempo (BPM) of the Ableton session. Value should be in BPM (e.g., 120.0).
    #[tool(description = "Set the tempo (BPM) of the Ableton session. Value should be in BPM (e.g., 120.0).")]
    pub async fn ableton_set_tempo(&self, Parameters(params): Parameters<SetTempoParams>) -> Result<CallToolResult, McpError> {
        self.osc().await.map_err(McpError::from)?.send("/live/song/set/tempo", vec![rosc::OscType::Float(params.bpm)])
            .await.map_err(McpError::from)?;
        let summary = common::query_session_summary(self.osc().await.map_err(McpError::from)?).await.map_err(McpError::from)?;
        let json = serde_json::to_string_pretty(&summary).map_err(|e| McpError::from(crate::errors::Error::from(e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
```

- [ ] **Step 2: Add transport module to `src/tools.rs`**

Add `pub mod transport;` to `src/tools.rs`.

- [ ] **Step 3: Update `src/server.rs` to combine tool routers**

The rmcp `#[tool_router]` macro approach requires all tools to be in the same impl block, OR you use the multi-router pattern with `tool_router_a() + tool_router_b()`. Since tools are in separate files, use the multi-router approach:

In `src/server.rs`, change the `#[tool_router]` impl to be empty and manually combine routers. The transport tools are defined in a separate impl block in `transport.rs` with their own `#[tool_router]` attribute.

Actually, looking at the rmcp API more carefully: the `#[tool_router]` macro generates a `Self::tool_router()` method. For multiple files, each file defines a `#[tool_router(router = tool_router_<name>, vis = "pub")]` impl block, and the server combines them.

Update `src/tools/transport.rs` to add `#[tool_router(router = tool_router_transport, vis = "pub(crate)")]` on the impl block.

Update `src/server.rs`:
```rust
impl AbletonMcpServer {
    #[must_use]
    pub fn new(config: Arc<Config>, osc: Arc<OscClient>) -> Self {
        let tool_router = crate::tools::transport::tool_router_transport();
        Self {
            config,
            osc,
            tool_router,
        }
    }
}
```

Remove the empty `#[tool_router] impl AbletonMcpServer {}` block from server.rs.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/tools/transport.rs src/tools.rs src/server.rs
git commit -m "Add transport tools (play, stop, get/set tempo)"
```

---

### Task 9: Track tools (atomic)

**Files:**
- Create: `src/tools/tracks.rs`
- Modify: `src/tools.rs`
- Modify: `src/server.rs`

- [ ] **Step 1: Create `src/tools/tracks.rs`**

Implement tools: `ableton_list_tracks`, `ableton_set_track_volume`, `ableton_set_track_name`, `ableton_mute_track`, `ableton_unmute_track`.

Key patterns:
- `list_tracks`: query `/live/song/get/num_tracks`, then loop `/live/track/get/name [i]` for each
- `set_track_volume`: send `/live/track/set/volume [track, volume]`, then read-after-write by querying volume/mute/solo/pan for mixer state
- Use `#[tool_router(router = tool_router_tracks, vis = "pub(crate)")]`

Define param structs with `Deserialize + JsonSchema`:
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrackIndexParams { pub track: i32 }

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTrackVolumeParams {
    pub track: i32,
    /// Volume 0.0 to 1.0
    pub volume: f32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTrackNameParams {
    pub track: i32,
    pub name: String,
}
```

For mixer state read-after-write, query: volume, panning, mute, solo for the track.

- [ ] **Step 2: Add `pub mod tracks;` to `src/tools.rs`**

- [ ] **Step 3: Add `tool_router_tracks()` to the combined router in `src/server.rs`**

```rust
let tool_router = crate::tools::transport::tool_router_transport()
    + crate::tools::tracks::tool_router_tracks();
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`

- [ ] **Step 5: Commit**

```bash
git add src/tools/tracks.rs src/tools.rs src/server.rs
git commit -m "Add track tools (list, volume, name, mute/unmute)"
```

---

### Task 10: Scene tools

**Files:**
- Create: `src/tools/scenes.rs`
- Modify: `src/tools.rs`
- Modify: `src/server.rs`

- [ ] **Step 1: Create `src/tools/scenes.rs`**

Tools: `ableton_fire_scene`, `ableton_list_scenes`.

- `list_scenes`: query `/live/song/get/num_scenes`, then `/live/scene/get/name [i]` per scene
- `fire_scene`: send `/live/scene/fire [index]`, read-after-write with transport state
- Use `#[tool_router(router = tool_router_scenes, vis = "pub(crate)")]`

- [ ] **Step 2: Add `pub mod scenes;` to `src/tools.rs`**

- [ ] **Step 3: Add `tool_router_scenes()` to combined router in `src/server.rs`**

- [ ] **Step 4: Verify and commit**

```bash
cargo build
git add src/tools/scenes.rs src/tools.rs src/server.rs
git commit -m "Add scene tools (fire, list)"
```

---

## Chunk 3: Clip + Device Tools

### Task 11: Clip tools (atomic)

**Files:**
- Create: `src/tools/clips.rs`
- Modify: `src/tools.rs`
- Modify: `src/server.rs`

- [ ] **Step 1: Create `src/tools/clips.rs`**

Tools: `ableton_fire_clip`, `ableton_stop_clip`, `ableton_get_clip_name`, `ableton_create_midi_clip`, `ableton_add_notes`, `ableton_get_notes`, `ableton_remove_notes`.

Key types:
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipParams { pub track: i32, pub slot: i32 }

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Note {
    pub pitch: i32,
    pub start: f32,
    pub duration: f32,
    pub velocity: i32,
    #[serde(default)]
    pub mute: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddNotesParams {
    pub track: i32,
    pub slot: i32,
    /// Array of notes. Max 1000 notes per call.
    pub notes: Vec<Note>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMidiClipParams {
    pub track: i32,
    pub slot: i32,
    /// Length in beats (e.g., 4.0 for one bar in 4/4)
    pub length: f32,
}
```

For `add_notes`: validate `notes.len() <= 1000`, flatten to `[pitch, start, duration, velocity, mute, pitch, start, ...]` as `Vec<OscType>`.

For `remove_notes`: send `/live/clip/remove/notes [track, slot, 0, 128, 0.0, 100000.0]`.

For `get_notes`: query `/live/clip/get/notes [track, slot]`, parse response args back into `Vec<Note>`. AbletonOSC returns flattened note data.

- [ ] **Step 2: Add module and router**

Add `pub mod clips;` to `src/tools.rs`. Add `tool_router_clips()` to combined router.

- [ ] **Step 3: Verify and commit**

```bash
cargo build
git add src/tools/clips.rs src/tools.rs src/server.rs
git commit -m "Add clip tools (fire, stop, name, create, add/get/remove notes)"
```

---

### Task 12: Device tools (atomic)

**Files:**
- Create: `src/tools/devices.rs`
- Modify: `src/tools.rs`
- Modify: `src/server.rs`

- [ ] **Step 1: Create `src/tools/devices.rs`**

Tools: `ableton_list_devices`, `ableton_list_device_parameters`, `ableton_set_device_parameter`.

Key patterns:
- `list_devices`: query `/live/track/get/devices/name [track]` and `/live/track/get/devices/class_name [track]`. Both return arrays in the args. Zip them together with indices.
- `list_device_parameters`: query `/live/device/get/parameters/name [track, device]`, `/live/device/get/parameters/value [track, device]`, `/live/device/get/parameters/min [track, device]`, `/live/device/get/parameters/max [track, device]`. Combine into `[{index, name, value, min, max}]`.
- `set_device_parameter`: send `/live/device/set/parameter/value [track, device, param, value]`, then read-after-write by querying full device state (re-use list_device_parameters logic).

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeviceParams { pub track: i32, pub device: i32 }

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetDeviceParameterParams {
    pub track: i32,
    pub device: i32,
    pub param: i32,
    pub value: f32,
}
```

- [ ] **Step 2: Add module and router**

- [ ] **Step 3: Verify and commit**

```bash
cargo build
git add src/tools/devices.rs src/tools.rs src/server.rs
git commit -m "Add device tools (list devices, list/set parameters)"
```

---

## Chunk 4: Compound Tools

### Task 13: Compound read tools (session state, track detail, device full)

**Files:**
- Create: `src/tools/session.rs`
- Modify: `src/tools.rs`
- Modify: `src/server.rs`
- Modify: `src/tools/tracks.rs` (add `get_track_detail`)
- Modify: `src/tools/devices.rs` (add `get_device_full`)

- [ ] **Step 1: Create `src/tools/session.rs` with `ableton_get_session_state`**

Queries: tempo, is_playing, num_tracks, track names/mute/volume/device names, num_scenes, scene names. Returns a large JSON object. This tool IS the session summary — no need to append a separate summary.

- [ ] **Step 2: Add `ableton_get_track_detail` to `src/tools/tracks.rs`**

Takes `track: i32`. Returns: track name, mute, volume, solo, pan, device list (name + class_name), clip slot info (which slots have clips + clip names).

- [ ] **Step 3: Add `ableton_get_device_full` to `src/tools/devices.rs`**

Takes `track: i32, device: i32`. Returns: device name, class_name, all parameters (name, value, min, max).

- [ ] **Step 4: Add modules and routers**

- [ ] **Step 5: Verify and commit**

```bash
cargo build
git add src/tools/session.rs src/tools/tracks.rs src/tools/devices.rs src/tools.rs src/server.rs
git commit -m "Add compound read tools (session state, track detail, device full)"
```

---

### Task 14: Compound write tools

**Files:**
- Modify: `src/tools/clips.rs` (add `create_midi_clip_with_notes`, `clear_and_write_notes`)
- Modify: `src/tools/devices.rs` (add `set_device_parameters`)
- Modify: `src/tools/tracks.rs` (add `set_mixer`)

- [ ] **Step 1: Add `ableton_create_midi_clip_with_notes` to clips.rs**

Takes: track, slot, length, notes. Internally: create_clip → add_notes. Returns clip state + note count.

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMidiClipWithNotesParams {
    pub track: i32,
    pub slot: i32,
    /// Length in beats
    pub length: f32,
    /// Notes to add. Max 1000.
    pub notes: Vec<Note>,
}
```

- [ ] **Step 2: Add `ableton_clear_and_write_notes` to clips.rs**

Takes: track, slot, notes. Internally: remove_notes → add_notes. Returns new note list.

- [ ] **Step 3: Add `ableton_set_device_parameters` to devices.rs**

Takes: track, device, parameters (array of {index, value}). Loops through and sets each. Returns full device state after.

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetDeviceParametersParams {
    pub track: i32,
    pub device: i32,
    pub parameters: Vec<ParameterValue>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParameterValue {
    pub index: i32,
    pub value: f32,
}
```

- [ ] **Step 4: Add `ableton_set_mixer` to tracks.rs**

Takes: track, optional volume/pan/mute/solo. Sets only the provided fields. Returns full mixer state.

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetMixerParams {
    pub track: i32,
    /// Volume 0.0-1.0. Omit to leave unchanged.
    pub volume: Option<f32>,
    /// Pan -1.0 to 1.0. Omit to leave unchanged.
    pub pan: Option<f32>,
    /// Mute state. Omit to leave unchanged.
    pub mute: Option<bool>,
    /// Solo state. Omit to leave unchanged.
    pub solo: Option<bool>,
}
```

- [ ] **Step 5: Verify and commit**

```bash
cargo build
git add src/tools/clips.rs src/tools/devices.rs src/tools/tracks.rs
git commit -m "Add compound write tools (create clip with notes, set mixer, batch params)"
```

---

## Chunk 5: Template Tracks + Batch + Phase 2

### Task 15: Template tracks

**Files:**
- Modify: `src/tools/tracks.rs`

- [ ] **Step 1: Add `ableton_list_templates` and `ableton_create_from_template`**

`list_templates`: call `list_tracks` internally, filter names starting with `[TPL] `.

`create_from_template`: find template track by name, send `/live/song/duplicate_track [track_index]`, then rename the new track (strip `[TPL] ` prefix) via `/live/track/set/name`. The new track appears at `track_index + 1`. Return new track state with devices.

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateFromTemplateParams {
    /// Name of the template (without [TPL] prefix), e.g., "Pad", "Drums"
    pub template_name: String,
}
```

- [ ] **Step 2: Verify and commit**

```bash
cargo build
git add src/tools/tracks.rs
git commit -m "Add template track tools (list, create from template)"
```

---

### Task 16: Batch tool

**Files:**
- Create: `src/tools/batch.rs`
- Modify: `src/tools.rs`
- Modify: `src/server.rs`

- [ ] **Step 1: Create `src/tools/batch.rs`**

This is the most complex tool. It takes an array of actions and dispatches them to the appropriate tool logic.

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchParams {
    /// Array of actions to execute sequentially
    pub actions: Vec<serde_json::Value>,
    /// Error handling: "continue" (default) or "abort"
    #[serde(default = "default_on_error")]
    pub on_error: String,
}

fn default_on_error() -> String { "continue".to_string() }
```

Each action is a JSON object with an `"action"` field (short name like `"set_track_volume"`, `"mute_track"`, etc.) plus the action-specific fields.

Implementation approach: match on the `action` string, deserialize the rest into the appropriate params struct, call the same OSC logic that the atomic tools use. Extract the shared logic from atomic tools into internal helper functions so both the tool and the batch dispatcher can call them.

Return: array of per-action results, plus a final session summary.

For `on_error: "abort"`: stop on first error, return partial results. For `on_error: "continue"`: execute all, report per-action success/failure.

- [ ] **Step 2: Add module and router**

- [ ] **Step 3: Verify and commit**

```bash
cargo build
git add src/tools/batch.rs src/tools.rs src/server.rs
git commit -m "Add batch tool for multi-action dispatch"
```

---

### Task 17: Phase 2 compound tools

**Files:**
- Modify: `src/tools/clips.rs`

- [ ] **Step 1: Add `ableton_create_musical_phrase`**

Takes: track, slot, length, notes, optional `device_params`. Internally: create_clip → add_notes → optionally set_device_parameters. Returns clip state + device state.

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateMusicalPhraseParams {
    pub track: i32,
    pub slot: i32,
    pub length: f32,
    pub notes: Vec<Note>,
    /// Optional device parameter tweaks to apply after creating the clip
    pub device_params: Option<Vec<DeviceParamGroup>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeviceParamGroup {
    pub device: i32,
    pub parameters: Vec<ParameterValue>,
}
```

- [ ] **Step 2: Add `ableton_adjust_clip_sound`**

Takes: track, slot, optional notes, optional clear_existing_notes (bool), optional device_params. Internally: optionally remove_notes → add_notes → set_device_parameters.

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AdjustClipSoundParams {
    pub track: i32,
    pub slot: i32,
    /// New notes to add. Omit to leave notes unchanged.
    pub notes: Option<Vec<Note>>,
    /// Clear existing notes before adding new ones. Default false.
    #[serde(default)]
    pub clear_existing_notes: bool,
    /// Device parameter tweaks. Omit to leave devices unchanged.
    pub device_params: Option<Vec<DeviceParamGroup>>,
}
```

- [ ] **Step 3: Verify and commit**

```bash
cargo build
git add src/tools/clips.rs
git commit -m "Add Phase 2 compound tools (create musical phrase, adjust clip sound)"
```

---

## Chunk 6: Final Verification

### Task 18: Full build, clippy, format check

**Files:** none (verification only)

- [ ] **Step 1: Format**

Run: `cargo fmt`

- [ ] **Step 2: Clippy**

Run: `cargo clippy -- -D warnings`
Fix any warnings.

- [ ] **Step 3: Build all targets**

Run: `cargo build`
Expected: clean build, zero warnings

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "Fix clippy warnings and formatting"
```

---

### Task 19: Manual end-to-end test with Ableton

**Files:** none (testing only)

- [ ] **Step 1: Install AbletonOSC**

Run: `cargo run -- install`
Expected: AbletonOSC copied to Ableton User Library

- [ ] **Step 2: Open Ableton, load AbletonOSC device**

Drag AbletonOSC from User Library into any track.

- [ ] **Step 3: Test via stdio**

Run: `cargo run` (stdio mode)
Send a JSON-RPC `tools/list` request to verify all tools are registered.
Send `tools/call` with `ableton_get_tempo` to verify OSC communication works.
Send `tools/call` with `ableton_play` and `ableton_stop` to verify transport.

- [ ] **Step 4: Test compound tools**

Test `ableton_get_session_state` to get full session dump.
Test `ableton_create_midi_clip_with_notes` with a simple chord.
Test `ableton_batch` with multiple operations.

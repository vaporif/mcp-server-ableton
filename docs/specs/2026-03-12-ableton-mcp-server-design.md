# Ableton MCP Server — Design Spec

## Overview

A Rust MCP server that bridges Claude (or any MCP client) to Ableton Live via OSC, using the AbletonOSC Max for Live device as the transport layer.

```
Claude/Client <-> MCP Server (Rust, stdio/HTTP) <-> OSC over UDP <-> AbletonOSC (M4L) <-> Ableton Live
```

## Prerequisites

- AbletonOSC Max for Live device installed and loaded in the user's Ableton session (auto-installed via `mcp-server-ableton install`)
- AbletonOSC listens on UDP 11000 (receive) and replies on UDP 11001 (send)
- Max for Live requires Ableton Suite or the Max for Live add-on

## Design Decisions

- **Stateless**: Every tool call queries OSC live. No caching or session mirroring.
- **Lazy connection**: No startup probe. Timeout errors surface per-tool with a clear message.
- **Flat dispatch**: Tools call `OscClient` directly — no intermediate domain layer.
- **Raw MIDI values**: Notes expressed as `{pitch: i32, start: f32, duration: f32, velocity: i32, mute: i32}`. The `mute` field defaults to 0 if omitted. No musical abstraction layer.
- **Read-after-write**: Every mutation returns the updated state of the affected entity.
- **Session summary in every response**: `{tempo, is_playing, selected_track}` appended to all tool responses for baseline LLM context. Selected track is queried via `/live/view/get/selected_track`.
- **Serialized queries**: OSC queries are serialized through a mutex — only one query in-flight at a time. AbletonOSC replies have no request ID, so concurrent queries on the same path would collide. Sequential queries over loopback UDP are ~1ms each, so serialization has negligible performance impact.

## OSC Transport (`osc.rs`)

### OscClient

Struct holding a UDP send socket and a query mutex. A background tokio task owns the recv socket and dispatches replies.

### Dispatcher

- On `OscClient::new()`, spawn a background task that loops on `socket.recv()` with a recv buffer of 65535 bytes
- Maintains `Arc<Mutex<HashMap<String, oneshot::Sender<OscMessage>>>>` for pending queries
- On each received packet: decode with `rosc`, look up the OSC path in the map, send through the oneshot, remove the entry
- Unsolicited messages (start_listen callbacks, `/live/error`) are logged via `tracing` and dropped — they must not clog the map
- The background task is shut down via `tokio_util::sync::CancellationToken` (`tokio-util` is already in Cargo.toml) when the server exits

### Query Serialization

All calls to `query()` are serialized through a `tokio::sync::Mutex`. This ensures only one OSC request is in-flight at a time, avoiding reply correlation issues since AbletonOSC replies have no request ID and use the same path as the request.

This means compound tools like `get_session_state` issue sequential queries. For a session with 20 tracks, this is ~20-40 queries at ~1ms each over loopback = 20-40ms total. Acceptable for LLM interactions where latency tolerance is seconds.

### Methods

- **`send(address: &str, args: Vec<OscType>) -> Result<()>`** — Fire-and-forget. Encode with `rosc`, send UDP packet. Used for commands (play, stop, fire_clip). Does NOT acquire the query mutex.
- **`query(address: &str, args: Vec<OscType>) -> Result<OscMessage>`** — Acquires the query mutex, inserts a oneshot into the dispatcher map, sends the UDP packet, awaits the receiver with `tokio::time::timeout` (1000ms). On timeout, cleans up the map entry and returns an error.

### Error Messages

Timeout error text: "AbletonOSC not responding — is the Max for Live device loaded in your Ableton session?"

### Hardcoded Config

- Send: `127.0.0.1:11000`
- Recv: `127.0.0.1:11001`
- Timeout: 1000ms
- Recv buffer: 65535 bytes

No configurability in MVP. AbletonOSC defaults are fixed.

## Error Handling (`errors.rs`)

Extend the existing `Error` enum with:

- `OscTimeout` — query timed out
- `OscDecode` — malformed reply from AbletonOSC
- `UnexpectedResponse` — reply had wrong type or arity

All convert to MCP errors via the existing `Into<McpError>` impl with human-readable messages.

## Tool Design Principles

- **Detailed descriptions with schema hints** so the LLM picks compound/batch tools over chaining atomics
- **Read-after-write**: mutations return updated entity state, not `{"status": "ok"}`
- **Session summary**: every response includes `{tempo, is_playing, selected_track}` (queried via `/live/song/get/tempo`, `/live/song/get/is_playing`, `/live/view/get/selected_track`)
- **Note limit**: `add_notes` capped at 1000 notes per call — error if exceeded, not silent truncation
- **Note format**: `[pitch, start_time, duration, velocity, mute]` — 5 fields per note matching AbletonOSC's expected format. `mute` defaults to 0 if not provided by the LLM.
- **Parameter ranges**: volume/pan are 0.0-1.0, tempo is actual BPM. Documented per-tool.
- **Sequential queries for bulk reads**: `list_tracks` and `list_scenes` issue N+1 queries (get count, then get each name). Over loopback this is fast (~1ms each). If AbletonOSC adds bulk endpoints in the future, these can be optimized without changing the tool interface.

## MVP Tools

### Atomic — Transport (`transport.rs`)

| Tool | OSC Address | Args | Returns |
|------|-------------|------|---------|
| `ableton_play` | `/live/play` | none | transport state + session summary |
| `ableton_stop` | `/live/stop` | none | transport state + session summary |
| `ableton_get_tempo` | `/live/song/get/tempo` | none | tempo float + session summary |
| `ableton_set_tempo` | `/live/song/set/tempo` | `[float]` | session summary (includes new tempo) |

### Atomic — Tracks (`tracks.rs`)

| Tool | OSC Address | Args | Returns |
|------|-------------|------|---------|
| `ableton_list_tracks` | `/live/song/get/num_tracks` + `/live/track/get/name` per track | none | `[{index, name}]` + session summary |
| `ableton_set_track_volume` | `/live/track/set/volume` | `[track, float 0.0-1.0]` | full mixer state + session summary |
| `ableton_set_track_name` | `/live/track/set/name` | `[track, string]` | track state + session summary |
| `ableton_mute_track` | `/live/track/set/mute` | `[track, 1]` | full mixer state + session summary |
| `ableton_unmute_track` | `/live/track/set/mute` | `[track, 0]` | full mixer state + session summary |

### Atomic — Clips (`clips.rs`)

| Tool | OSC Address | Args | Returns |
|------|-------------|------|---------|
| `ableton_fire_clip` | `/live/clip/fire` | `[track, slot]` | track playing state + session summary |
| `ableton_stop_clip` | `/live/clip/stop` | `[track, slot]` | track playing state + session summary |
| `ableton_get_clip_name` | `/live/clip/get/name` | `[track, slot]` | clip name + session summary |
| `ableton_create_midi_clip` | `/live/clip_slot/create_clip` | `[track, slot, length]` | clip metadata + session summary |
| `ableton_add_notes` | `/live/clip/add/notes` | `[track, slot, ...notes]` | note count + session summary |
| `ableton_get_notes` | `/live/clip/get/notes` | `[track, slot]` | `[{pitch, start, duration, velocity, mute}]` + session summary |
| `ableton_remove_notes` | `/live/clip/remove/notes` | `[track, slot, 0, 128, 0.0, 100000.0]` | clip state + session summary |

`add_notes` flattening: notes are encoded as `[pitch, start_time, duration, velocity, mute]` repeated per note, matching AbletonOSC's expected format. Max 1000 notes per call.

`remove_notes` removes all notes by passing pitch_start=0, pitch_span=128 (covers all 128 MIDI pitches), time_start=0.0, time_span=100000.0 (practical upper bound — avoids potential f32::MAX encoding issues with OSC). Selective removal is not exposed in MVP.

### Atomic — Devices (`devices.rs`)

| Tool | OSC Address | Args | Returns |
|------|-------------|------|---------|
| `ableton_list_devices` | `/live/track/get/devices/name` + `/live/track/get/devices/class_name` | `[track]` | `[{index, name, class_name}]` + session summary |
| `ableton_list_device_parameters` | `/live/device/get/parameters/name` + `value` + `min` + `max` | `[track, device]` | `[{index, name, value, min, max}]` + session summary |
| `ableton_set_device_parameter` | `/live/device/set/parameter/value` | `[track, device, param, float]` | full device state + session summary |

### Atomic — Scenes (`scenes.rs`)

| Tool | OSC Address | Args | Returns |
|------|-------------|------|---------|
| `ableton_fire_scene` | `/live/scene/fire` | `[index]` | scene + transport state + session summary |
| `ableton_list_scenes` | `/live/song/get/num_scenes` + `/live/scene/get/name` per scene | none | `[{index, name}]` + session summary |

### Compound — Reads (`session.rs`, `tracks.rs`, `devices.rs`)

| Tool | Description | Returns |
|------|-------------|---------|
| `ableton_get_session_state` | Queries tempo, is_playing, all tracks (name, mute, volume, device names), all scenes | Full session summary — this IS the session summary |
| `ableton_get_track_detail` | Track info + device list + clip names for one track | Complete track snapshot + session summary |
| `ableton_get_device_full` | Device name, class, all parameters (name, value, min, max) | Complete device state + session summary |

### Compound — Writes (`clips.rs`, `devices.rs`, `tracks.rs`)

| Tool | Description | Returns |
|------|-------------|---------|
| `ableton_create_midi_clip_with_notes` | Create clip + add notes in one call | clip state + note count + session summary |
| `ableton_clear_and_write_notes` | Remove all notes + add new notes | new note list + session summary |
| `ableton_set_device_parameters` | Batch set N parameters on one device | full device state + session summary |
| `ableton_set_mixer` | Batch set volume/pan/mute/solo (all optional) for one track | full mixer state + session summary |

### Template Tracks (`tracks.rs`)

| Tool | Description | Returns |
|------|-------------|---------|
| `ableton_list_templates` | List tracks with `[TPL]` prefix | `[{index, name}]` + session summary |
| `ableton_create_from_template` | Duplicate template track via `/live/song/duplicate_track`, strip `[TPL]` prefix via `set_track_name` | new track state with devices + session summary |

Convention: user creates tracks named `[TPL] Pad`, `[TPL] Drums`, etc. in their Ableton session. No MCP server config needed. Superseded by Phase 3 M4L companion device.

### Batch (`batch.rs`)

| Tool | Description | Returns |
|------|-------------|---------|
| `ableton_batch` | Execute array of operations in sequence | per-action results + all affected state + session summary |

**Input format:**
```json
{
  "actions": [
    {"action": "set_track_volume", "track": 0, "value": 0.8},
    {"action": "mute_track", "track": 2},
    {"action": "set_device_parameters", "track": 0, "device": 1, "parameters": [{"index": 3, "value": 0.5}]},
    {"action": "fire_scene", "index": 2}
  ]
}
```

**Batch action names** use the short form (without `ableton_` prefix): `set_track_volume`, `mute_track`, `fire_scene`, etc.

**Batchable actions**: all atomic write tools. Compound tools and reads are not batchable — use them directly.

**Error handling:** Configurable via `on_error` field — `"continue"` (default, execute all, report per-action success/failure) or `"abort"` (stop on first error, return partial results).

**Total MVP: ~30 tools**

## Phase 2 Tools

| Tool | Description |
|------|-------------|
| `ableton_create_musical_phrase` | Create clip + notes + optional device param tweaks in one call. Input includes `notes[]` and optional `device_params[{device, parameters[{index, value}]}]`. Internally: create_clip → add_notes → set_device_parameters. Returns clip state + device state + session summary. |
| `ableton_adjust_clip_sound` | Modify notes + device params on existing clip. Supports `clear_existing_notes` boolean flag (default false). Optional `device_params[]`. Internally: optionally remove_notes → add_notes → set_device_parameters. Returns clip state + device state + session summary. |

These are specialized compound tools for the generative music workflow — "create a bass line with this sound" or "make that part more staccato and darken the filter" as single tool calls.

## Phase 3 — Custom M4L Companion Device (Future)

A custom Max for Live device that extends AbletonOSC with operations the stock device doesn't support. This is a separate project (JavaScript/Max patching, not Rust).

### Capabilities

- **Load device by name**: `Track.create_device()` via the internal Live API — enables `add_device(track, "Reverb")` or `load_vst(track, "Serum")`
- **Preset management**: Load/save device presets by name
- **Plugin discovery**: List available devices/VSTs/AUs on the system

### Impact

Supersedes template tracks for device creation. The LLM could say "add a Wavetable synth to track 2" directly instead of duplicating a template.

### Status

Not in scope for implementation. Documented here for future planning.

## AbletonOSC Installer (`installer.rs`)

### Bundling

AbletonOSC is included as a git submodule pinned to a specific commit of `ideoforms/AbletonOSC` (MIT licensed — no conflict with GPL-3.0). This ensures the MCP server ships with a known-good version and users don't need to manually download anything.

### CLI Subcommand

`mcp-server-ableton install` — copies the bundled AbletonOSC files to the correct Ableton User Library path.

**Installation paths:**
- macOS: `~/Music/Ableton/User Library/Presets/MIDI Effects/Max MIDI Effect/`
- Windows: `%APPDATA%\Ableton\User Library\Presets\MIDI Effects\Max MIDI Effect\`

**Behavior:**
- Detects OS and resolves the correct target path
- Checks if AbletonOSC is already installed at the target (compare file contents or version marker)
- If not installed: copies the bundled AbletonOSC device files to the target directory
- If already installed: reports "already installed" and skips (or `--force` to overwrite)
- Prints clear instructions after install: "Open Ableton Live, drag AbletonOSC from your User Library into any track, and you're ready to go"
- Errors clearly if the Ableton User Library path doesn't exist (Ableton may not be installed)

**Implementation:**
- New `installer.rs` module with `install()` function
- `config.rs` gains an `Install` variant in the CLI subcommand enum (via clap)
- `main.rs` dispatches to `installer::install()` before starting the MCP server if the `install` subcommand is used
- Uses `include_dir` or `rust-embed` to bundle the submodule files into the binary at compile time, OR reads from a known path relative to the binary. Compile-time embedding is preferred — single binary, no external files needed at runtime.

**No runtime dependency:** The installer is a one-time setup step. The MCP server itself only needs AbletonOSC to be loaded in Ableton — it doesn't care how it got there.

## Project Structure

```
├── AbletonOSC/              # git submodule (ideoforms/AbletonOSC, MIT)
src/
├── main.rs              # MCP stdio/HTTP server startup + install dispatch (existing, extend)
├── lib.rs               # Module exports (existing)
├── server.rs            # AbletonMcpServer + Arc<OscClient> + tool router (existing, extend)
├── config.rs            # CLI args + Install subcommand (existing, extend)
├── errors.rs            # Error types (existing, add OSC + installer variants)
├── installer.rs         # AbletonOSC auto-installer (new)
├── osc.rs               # OscClient with dispatcher (new)
├── tools.rs             # Module root for tools/ (new)
└── tools/
    ├── transport.rs     # play, stop, tempo
    ├── tracks.rs        # list, volume, mute, name, templates, mixer
    ├── clips.rs         # fire, stop, notes, create_with_notes, clear_and_write
    ├── devices.rs       # list, parameters, set_parameter, set_parameters, get_device_full
    ├── scenes.rs        # fire, list
    ├── session.rs       # get_session_state, get_track_detail
    └── batch.rs         # generic batch dispatch
```

Module structure uses modern Rust syntax: `src/tools.rs` as the module root with tool files in `src/tools/`. No `mod.rs`.

## Dependencies

**New:**
- `rosc` — OSC encoding/decoding
- `include_dir` (or `rust-embed`) — embed AbletonOSC files into the binary at compile time
- `dirs` — resolve platform-specific user library paths (macOS/Windows)

**Existing (already in Cargo.toml):** `rmcp`, `tokio`, `serde_json`, `thiserror`, `anyhow`, `tracing`, `axum`, `clap`, `schemars`.

## Build Order

1. AbletonOSC git submodule + `installer.rs` + `install` CLI subcommand — get AbletonOSC auto-install working first
2. `osc.rs` — OscClient with dispatcher, send/query methods, query mutex, CancellationToken shutdown
3. `errors.rs` — add OSC error variants
4. Transport tools (play, stop, tempo) — first end-to-end test with Ableton
5. Track + scene tools (including `set_track_name`)
6. Clip tools (including MIDI note operations with 5-field format)
7. Device tools
8. Compound read tools (session state, track detail, device full)
9. Compound write tools (create_midi_clip_with_notes, set_device_parameters, set_mixer)
10. Template tracks
11. Batch tool
12. Phase 2: create_musical_phrase, adjust_clip_sound

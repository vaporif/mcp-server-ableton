# Ableton Copilot — Product Specification

## Overview

A self-contained AI copilot for Ableton Live that runs entirely on the producer's machine. Provides natural language control over session setup, MIDI generation, device management, routing, and mix analysis. No cloud dependency required — local inference on Apple Silicon.

The product ships as three components:

1. **MCP Server** (Rust binary) — orchestration layer between LLM and Ableton
2. **AbletonOSC fork** (Python Remote Script) — control plane inside Ableton
3. **Spectral Analyzer** (Max for Live device, v2) — audio data plane

---

## Architecture

```
┌─────────────┐     stdio/JSON-RPC      ┌──────────────┐
│   LLM       │◄────────────────────────►│  MCP Server  │
│  (local or  │                          │   (Rust)     │
│   Claude)   │                          └──────┬───────┘
└─────────────┘                                 │
                                          UDP/OSC│
                                                │
                              ┌─────────────────┼────────────────┐
                              │                 │                │
                              ▼                 ▼                ▼
                     ┌──────────────┐  ┌──────────────┐  ┌────────────┐
                     │ AbletonOSC   │  │ AbletonOSC   │  │ M4L        │
                     │ (stock       │  │ (browser     │  │ Spectral   │
                     │  endpoints)  │  │  extensions) │  │ Analyzer   │
                     └──────────────┘  └──────────────┘  └────────────┘
                              │                 │                │
                              └─────────────────┼────────────────┘
                                                │
                                          ┌─────▼─────┐
                                          │  Ableton  │
                                          │   Live    │
                                          └───────────┘
```

---

## Component 1: MCP Server (Rust)

### Crate Dependencies

| Crate | Purpose |
|---|---|
| `rmcp` | MCP protocol (JSON-RPC over stdio) |
| `rosc` | OSC encoding/decoding |
| `tokio` | Async runtime, UDP, channels |
| `llama-cpp-2` | Local inference via llama.cpp (Metal) |
| `rustfft` | Spectral analysis (v2) |

### OSC Transport Layer

- Single `UdpSocket` bound to ephemeral port (`127.0.0.1:0`)
- OS assigns port; AbletonOSC replies to sender's address
- Enables multiple MCP server instances (multi-session support)
- Socket initialized lazily via `Arc<OnceCell<OscClient>>` — first tool call triggers connection
- MCP handshake succeeds without Ableton running

### Dispatcher

Background tokio task routes incoming OSC replies:

```
HashMap<String, oneshot::Sender<OscMessage>>
```

Keyed by OSC address path. Tool sends OSC query, registers a oneshot receiver for the expected reply path, awaits response with timeout. Eliminates the `clear_recv_buffer()` hack used by remix-mcp.

### Tool Design Principles

**Read-after-write:** Every mutation tool returns the updated state of the affected entity, not just `{"status": "ok"}`. Reduces follow-up queries by the LLM.

**Session summary:** Every tool response includes:
```json
{
  "session": {
    "tempo": 120.0,
    "is_playing": false,
    "selected_track": 2
  }
}
```

Keeps the LLM oriented without extra round-trips.

**GBNF grammar constraints:** When using local inference, output is constrained to valid JSON matching tool schemas. Eliminates JSON parse failures.

### Tool Categories

#### Transport & Song

| Tool | OSC Path(s) | Description |
|---|---|---|
| `get_song_info` | `/live/song/get/tempo`, etc. | Tempo, time sig, is_playing, loop state |
| `set_tempo` | `/live/song/set/tempo` | Set BPM |
| `start_playback` | `/live/song/start_playing` | Play |
| `stop_playback` | `/live/song/stop_playing` | Stop |
| `set_loop` | `/live/song/set/loop_start`, `loop_length` | Set loop region |
| `undo` | `/live/song/undo` | Undo last action |
| `redo` | `/live/song/redo` | Redo |

#### Track Management

| Tool | OSC Path(s) | Description |
|---|---|---|
| `get_track_info` | `/live/track/get/*` | Name, volume, pan, mute, solo, arm, sends |
| `create_track` | `/live/song/create_audio_track`, `create_midi_track` | Create new track |
| `set_mixer` | `/live/track/set/volume`, `panning`, `mute`, `solo` | Batch mixer params in one call |
| `set_track_name` | `/live/track/set/name` | Rename track |
| `get_all_tracks` | `/live/song/get/num_tracks` + per-track queries | Full session track list |

#### Clip & MIDI

| Tool | OSC Path(s) | Description |
|---|---|---|
| `create_midi_clip` | `/live/clip_slot/create_clip` | Empty clip |
| `create_midi_clip_with_notes` | `create_clip` + `/live/clip/add/notes` | Clip with notes in one call |
| `get_clip_notes` | `/live/clip/get/notes` | Read MIDI notes |
| `add_notes` | `/live/clip/add/notes` | Append notes to existing clip |
| `remove_notes` | `/live/clip/remove/notes` | Remove notes by range |
| `set_clip_loop` | `/live/clip/set/loop_start`, `loop_end` | Set clip loop |
| `fire_clip` | `/live/clip_slot/fire` | Launch clip |

MIDI note format: `[pitch, start_time, duration, velocity, mute]`
- pitch: 0-127 (MIDI note number, no musical abstraction)
- start_time: beats (float)
- duration: beats (float)
- velocity: 1-127
- mute: 0 or 1

#### Device & Parameters

| Tool | OSC Path(s) | Description |
|---|---|---|
| `get_device_info` | `/live/device/get/*` | Name, class, type, all parameters |
| `set_device_parameter` | `/live/device/set/parameter/value` | Single param |
| `set_device_parameters` | Multiple `/live/device/set/parameter/value` | Batch params in one call |
| `get_device_parameters` | `/live/device/get/parameters/value` | All param values |

#### Browser (from AbletonOSC fork)

| Tool | OSC Path(s) | Description |
|---|---|---|
| `load_instrument` | `/live/browser/load_instrument` | Load instrument by name |
| `load_plugin` | `/live/browser/load_plugin` | Load VST/AU plugin |
| `load_audio_effect` | `/live/browser/load_audio_effect` | Load audio effect |
| `load_sample` | `/live/browser/load_sample` | Load sample to track |
| `list_plugins` | `/live/browser/list_plugins` | Available plugins |
| `search_browser` | `/live/browser/search` | Search Ableton browser |

#### Scene

| Tool | OSC Path(s) | Description |
|---|---|---|
| `fire_scene` | `/live/scene/fire` | Launch scene |
| `get_scene_info` | `/live/scene/get/*` | Scene name, tempo |

### Compound Tools

These reduce LLM round-trips by combining multiple OSC operations into single tool calls:

| Tool | Operations | Use Case |
|---|---|---|
| `create_midi_clip_with_notes` | create_clip + add_notes | Write melody/chords in one call |
| `set_mixer` | volume + pan + mute + solo | Adjust mix in one call |
| `set_device_parameters` | N × set_parameter | Configure plugin preset in one call |
| `scaffold_session` | N × create_track + load_instrument + load_audio_effect + set_mixer | Full session setup from description |
| `duplicate_with_variation` | get_clip_notes + transform + create_clip + add_notes | Generate variations |
| `setup_sidechain` | create send + load compressor + route sidechain input | Routing automation |
| `batch` | Arbitrary list of tool calls | Generic compound operation |

### Error Handling

- OSC timeout (no reply within 2s): return error with last known session state
- Ableton not running: OSC send succeeds (UDP is fire-and-forget), timeout on reply, return "Ableton not connected"
- Invalid track/clip/device index: AbletonOSC returns error message, forward to LLM
- Partial compound failure: execute sequentially, stop on first error, return completed operations + error

---

## Component 2: AbletonOSC Fork (Remote Script)

Based on `ideoforms/AbletonOSC` with extensions from `christopherwxyz`'s fork:

### Stock Endpoints (upstream)

Full control over Ableton's Live Object Model:
- Song: transport, tempo, time signature, loop, recording, undo/redo
- Track: mixer, clips, devices, routing, monitoring, meters
- Clip: notes, transport, loop, audio properties, warp markers
- Clip Slot: create/delete/duplicate/fire
- Device: parameters (get/set/listen), name, class, type
- Scene: fire, properties
- View: selection

### Browser Extensions (from christopherwxyz fork)

Custom Python handlers added to the Remote Script:
- `/live/browser/load_instrument` — load instrument by name to selected track
- `/live/browser/load_plugin` — load VST/AU by name
- `/live/browser/load_audio_effect` — load effect to selected track
- `/live/browser/load_sample` — load audio sample
- `/live/browser/list_plugins` — enumerate available plugins
- `/live/browser/search` — search Ableton's browser
- `/live/browser/hotswap_start` / `hotswap_load` — hot-swap devices

### Reply Port Fix (from christopherwxyz fork)

Replies sent to sender's address instead of hardcoded port 11001. Required for ephemeral port binding and multi-instance support.

### Installation

Remote Script installed to:
```
~/Music/Ableton/User Library/Remote Scripts/AbletonOSC/
```
Activated in Ableton Preferences → Link/Tempo/MIDI → Control Surface.

---

## Component 3: Spectral Analyzer M4L Device (v2)

Max for Live device that streams audio analysis data to the MCP server.

### Signal Chain

```
Track Audio → pfft~ (256 bins) → downsample (10fps) → OSC out → MCP Server
```

### Data Format

Per-frame OSC message to MCP server on dedicated port:

```
/spectral/track/{id}/frame [bin0, bin1, ..., bin255, rms_db, peak_db, timestamp]
```

~10KB/s per tracked track. Negligible CPU/bandwidth overhead.

### Analysis Capabilities (computed in Rust MCP server)

| Metric | Description | Use Case |
|---|---|---|
| Frequency balance | Energy per band (sub/low/mid/high/air) | "Your low end is muddy below 200Hz" |
| Spectral centroid | Brightness measure | "This track is dull, consider a high shelf" |
| RMS / LUFS | Loudness | "Mix is peaking at -6 LUFS, too loud for streaming" |
| Frequency masking | Overlap between tracks | "Kick and bass are fighting at 80-120Hz" |
| Crest factor | Dynamic range | "Drums are over-compressed" |
| Estimated key | Pitch class profile | "This sample is in Eb minor" |

### LLM Integration

Analysis results exposed as MCP tools:

| Tool | Description |
|---|---|
| `analyze_track` | Full spectral analysis of a single track |
| `analyze_mix` | Cross-track frequency analysis, masking detection |
| `suggest_eq` | Frequency-based EQ recommendations |
| `compare_tracks` | A/B spectral comparison |

---

## Local Inference

### Model Selection

Primary: **Qwen 2.5 Coder 14B** (Q4_K_M, ~9GB)

| Property | Value |
|---|---|
| Quantization | Q4_K_M (GGUF) |
| RAM footprint | ~9GB |
| Throughput (M2 Max) | 35-45 tok/s |
| Tool-call latency | 1-4s typical |

Fallback (fast mode): **Qwen 2.5 7B** (Q5_K_M, ~5.5GB, 60-80 tok/s)

### llama.cpp Integration

Via `llama-cpp-2` Rust crate. Metal acceleration automatic on macOS.

Key features used:
- **GBNF grammars** — constrain output to valid JSON matching tool schemas
- **LoRA hot-loading** — swap adapters at runtime without reloading base model
- **KV cache quantization** — reduce memory pressure alongside Ableton
- **Speculative decoding** — optional draft model (Qwen 2.5 0.5B) for ~1.5x speedup

### LoRA Fine-Tuning

**Training data sources:**
1. Synthetic tool-call pairs generated via Claude API (primary, 1000+ examples)
2. MIDI file mining — Lakh MIDI Dataset, extract patterns → natural language descriptions
3. Music theory mappings — scales, chords, progressions, voicings → note arrays

**Training format (ChatML):**
```json
{
  "messages": [
    {"role": "system", "content": "You are an Ableton Live assistant..."},
    {"role": "user", "content": "Create a Cm7 chord at C3, whole notes"},
    {"role": "assistant", "content": "{\"tool\": \"create_midi_clip_with_notes\", ...}"}
  ]
}
```

**Training (MLX on M2 Max):**
```
mlx_lm.lora --model ./qwen-14b --data ./training_data \
  --batch-size 1 --lora-layers 16 --epochs 3 --learning-rate 1e-5
```

~2-4 hours for 2000 examples. Output: ~100-200MB LoRA adapter.

**Eval criteria (200 held-out test prompts):**

| Metric | Target |
|---|---|
| JSON parse rate | >98% |
| Schema compliance | >95% |
| Musical correctness | >90% |
| Compound tool accuracy | >80% |

Stop training when metrics plateau for 2 consecutive checkpoints.

**Distribution:** Ship base GGUF once (9GB), LoRA adapters separately (~100MB). Update LoRA without full re-download.

---

## Distribution

### MCP Server

Built with `maturin` (`bindings = "bin"`). Rust binary distributed via PyPI:

```bash
pip install ableton-copilot
# or
uvx ableton-copilot
```

No Python runtime dependency — it's a compiled Rust binary.

### AbletonOSC Remote Script

Bundled with the MCP server package. Installer copies to Ableton's Remote Scripts directory.

### Local Model

First-run setup downloads:
1. Base GGUF model (~9GB) from HuggingFace
2. Ableton LoRA adapter (~100MB) from product CDN

Cached in `~/.ableton-copilot/models/`.

### M4L Device (v2)

Distributed as `.amxd` file. User drops into Ableton or installs via Ableton Packs.

---

## Roadmap

### v0 — Demo (2 weeks)

- Fork remix-mcp, clean up architecture
- Add compound tools: `scaffold_session`, `create_midi_clip_with_notes`, `set_mixer`
- Implement dispatcher (replace `clear_recv_buffer`)
- Add read-after-write + session summary
- Connect to Claude API (not local inference)
- Record demo video: empty session → full session in one prompt
- Post to r/ableton, r/WeAreTheMusicMakers, Twitter/X

### v1 — Local Inference

- Integrate llama.cpp via `llama-cpp-2`
- Train and ship initial music LoRA
- GBNF grammar constraints for all tool schemas
- First-run model download + caching
- Package as standalone installer (no CLI required for end users)

### v2 — Mix Analysis

- M4L Spectral Analyzer device
- Spectral analysis pipeline in Rust
- `analyze_track`, `analyze_mix`, `suggest_eq` tools
- Cross-track frequency masking detection

### v3 — Polish

- UI layer (standalone app or Ableton control surface)
- Multiple LoRA profiles (MIDI generation, mixing, sound design)
- Speculative decoding for faster responses
- Session templates marketplace

---

## Competitive Landscape

| Product | Status | Differentiator vs Us |
|---|---|---|
| remix-mcp | Open source (MIT) | 266 tools but no compound tools, no read-after-write, no local inference, no audio analysis |
| Feater | Private beta | Claude-dependent, no local inference, unknown tool depth |
| MIDI Agent | Active | MIDI-only (no device control, no routing, no mix), supports Ollama offline |
| iZotope Neutron 5 | Active product | Mix-only, no session control, no MIDI, no natural language |
| MixAnalytic | Web app | Upload-based analysis, no DAW integration, no real-time |

**Our position:** Only product combining full DAW control + local inference + audio analysis in a self-contained offline package.

---

## Open Questions

1. **Licensing model** — One-time purchase vs subscription? Plugin market expects one-time ($50-200 range).
2. **Ableton version support** — AbletonOSC targets Live 11+. Live 12 compatibility needs testing.
3. **Windows support** — Local inference via llama.cpp works on Windows (CUDA/Vulkan). AbletonOSC is cross-platform. Main question is whether to support Windows at launch or Mac-only.
4. **DAW expansion** — Architecture is OSC-based. Other DAWs (Bitwig, Reaper) have OSC support. How portable is the tool layer?
5. **Audio recording from M4L** — Spectral analysis is lightweight. Should v2 also support full audio buffer capture for more advanced analysis (waveform display, transient detection)?

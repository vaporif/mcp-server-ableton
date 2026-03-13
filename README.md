# mcp-server-ableton

[Model Context Protocol](https://modelcontextprotocol.io/) server for [Ableton Live](https://www.ableton.com/), enabling AI agents to control your Ableton session over OSC.

## Requirements

- Ableton Live with [AbletonOSC](https://github.com/ideoforms/AbletonOSC) loaded as a Max for Live device

## Install

```bash
cargo install --git https://github.com/vaporif/mcp-server-ableton
```

Install the bundled AbletonOSC device into your Ableton User Library:

```bash
mcp-server-ableton install
```

Then drag AbletonOSC from your User Library onto any track in Ableton.

## Usage

### Stdio (default)

```bash
mcp-server-ableton
```

### Streamable HTTP

```bash
mcp-server-ableton --transport streamable-http --host 127.0.0.1 --port 3000
```

## Tools

| Category | Tools |
|---|---|
| Transport | play, stop, get/set tempo |
| Tracks | list, rename, volume, mute/unmute, mixer, detail |
| Scenes | list, fire |
| Clips | fire, stop, name, create MIDI clip, add/get/remove notes |
| Devices | list, parameters, set parameter(s) |
| Templates | list template tracks, create track from template |
| Compound | create clip with notes, musical phrase, adjust clip sound, batch |
| Session | full session state |

## Credits

- [AbletonOSC](https://github.com/ideoforms/AbletonOSC) by Daniel Jones - Max for Live device providing OSC control of Ableton Live
- [rmcp](https://github.com/anthropics/rmcp) - Rust MCP SDK

## License

GPL-3.0-or-later

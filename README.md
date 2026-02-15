<h1 align="center">presence-bridge</h1>

<p align="center">
  Native cross-platform bridge from <b>Now Playing</b> to <b>Discord Rich Presence</b>.
  <br/>
  <sub>Built in Rust. Minimal overhead. Production-focused.</sub>
</p>

<p align="center">
  <a href="https://github.com/vincenzomaritato/presence-bridge/actions/workflows/ci.yml">
    <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/vincenzomaritato/presence-bridge/ci.yml?branch=main&label=CI" />
  </a>
  <a href="https://github.com/vincenzomaritato/presence-bridge/releases">
    <img alt="Release" src="https://img.shields.io/github/v/release/vincenzomaritato/presence-bridge" />
  </a>
  <a href="/LICENSE">
    <img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue" />
  </a>
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-success" />
</p>

---

## What Is It

`presence-bridge` is a native daemon that publishes your active media session to Discord Rich Presence.

Pipeline:

```text
Now Playing Provider -> Event Engine -> Discord RPC
```

Supported providers:

- macOS: Apple Music via JXA (`osascript -l JavaScript`)
- Windows: GSMTC (`Windows.Media.Control`)
- Linux: MPRIS (`org.mpris.MediaPlayer2.*` via DBus)

---

## Quick Start (For Normal Users)

1. Create a Discord app at [Discord Developer Portal](https://discord.com/developers/applications).
2. Copy your **Application ID**.
3. Initialize config:

```bash
cargo run -- config init
```

4. Set this value in your config:

```toml
discord_app_id = "YOUR_DISCORD_APPLICATION_ID"
```

5. Run:

```bash
cargo run -- run
```

Quick checks:

```bash
cargo run -- doctor
cargo run -- status
```

Default config path:

- macOS: `~/Library/Application Support/presence-bridge/config.toml`
- Linux: `~/.config/presence-bridge/config.toml`
- Windows: `%APPDATA%\\presence-bridge\\config.toml`

---

## Download (Direct)

<p align="center">
  <a href="https://github.com/vincenzomaritato/presence-bridge/releases/latest/download/presence-bridge-macos-x86_64.tar.gz">
    <img alt="Download macOS" src="https://img.shields.io/badge/Download-macOS-black?style=for-the-badge&logo=apple" />
  </a>
  <a href="https://github.com/vincenzomaritato/presence-bridge/releases/latest/download/presence-bridge-windows-x86_64.zip">
    <img alt="Download Windows" src="https://img.shields.io/badge/Download-Windows-0078D6?style=for-the-badge&logo=windows" />
  </a>
  <a href="https://github.com/vincenzomaritato/presence-bridge/releases/latest/download/presence-bridge-linux-x86_64.tar.gz">
    <img alt="Download Linux Tar" src="https://img.shields.io/badge/Download-Linux_TAR-FCC624?style=for-the-badge&logo=linux&logoColor=black" />
  </a>
</p>

<p align="center">
  <a href="https://github.com/vincenzomaritato/presence-bridge/releases/latest/download/presence-bridge-linux-x86_64.deb">
    <img alt="Download Debian" src="https://img.shields.io/badge/Linux-.deb-A81D33?style=for-the-badge&logo=debian" />
  </a>
  <a href="https://github.com/vincenzomaritato/presence-bridge/releases/latest/download/presence-bridge-linux-x86_64.rpm">
    <img alt="Download RPM" src="https://img.shields.io/badge/Linux-.rpm-1793D1?style=for-the-badge&logo=redhat" />
  </a>
  <a href="https://github.com/vincenzomaritato/presence-bridge/releases/latest">
    <img alt="All releases" src="https://img.shields.io/badge/All-Release_Assets-2ea44f?style=for-the-badge&logo=github" />
  </a>
</p>

---

## Why It Is Better

- No Electron dependency
- Smart updates (no 1Hz Discord spam)
- Stable track timestamps
- Debounce for fast play/pause jitter
- Reliable reconnect strategy
- Structured logs + diagnostics

---

## Configuration

```toml
schema_version = 1
discord_app_id = "YOUR_DISCORD_APP_ID"
provider_priority = ["apple_music", "windows", "mpris"]
enable_buttons = true
log_level = "info"

[intervals]
playing_poll_ms = 1000
paused_poll_ms = 7000
stopped_poll_ms = 30000
presence_min_update_ms = 15000
debounce_ms = 500
file_watch_poll_ms = 10000

[assets]
large_image = "app_icon"
large_text = "presence-bridge"
small_play_image = "play"
small_pause_image = "pause"
```

Environment overrides:

- `PRESENCE_BRIDGE_DISCORD_APP_ID`
- `PRESENCE_BRIDGE_LOG_LEVEL`
- `PRESENCE_BRIDGE_ENABLE_BUTTONS` (`true` / `false`)

---

## CLI

```bash
presence-bridge run
presence-bridge doctor
presence-bridge status
presence-bridge config init
```

With Cargo:

```bash
cargo run -- run
cargo run -- doctor
cargo run -- status
cargo run -- config init
```

---

## Build From Source

```bash
git clone https://github.com/vincenzomaritato/presence-bridge.git
cd presence-bridge
cargo build --release
```

Binary output:

- macOS/Linux: `target/release/presence-bridge`
- Windows: `target/release/presence-bridge.exe`

---

## Troubleshooting

- Discord closed: daemon stays alive and retries with backoff.
- No media detected:
  - macOS: verify Music.app is running and playing
  - Windows: verify your player exposes media session
  - Linux: verify MPRIS player exists (`playerctl -l`)
- Invalid config: run `cargo run -- config init` again.

macOS Apple Music permission:

1. Open **System Settings**
2. Go to **Privacy & Security -> Automation**
3. Allow Terminal/iTerm to control **Music**

---

## Open Source Quality

- Unit tests (diff engine, URL builder)
- CI matrix (macOS, Windows, Linux)
- Release artifacts + SHA256 checksums
- Linux packaging (`.deb`, `.rpm`)

Workflows:

- `/Users/vmaritato/Desktop/Open Source/presence-bridge/.github/workflows/ci.yml`
- `/Users/vmaritato/Desktop/Open Source/presence-bridge/.github/workflows/release.yml`

---

## Project Links

- Issues: [github.com/vincenzomaritato/presence-bridge/issues](https://github.com/vincenzomaritato/presence-bridge/issues)
- Releases: [github.com/vincenzomaritato/presence-bridge/releases](https://github.com/vincenzomaritato/presence-bridge/releases)
- Security: `/Users/vmaritato/Desktop/Open Source/presence-bridge/SECURITY.md`
- Contributing: `/Users/vmaritato/Desktop/Open Source/presence-bridge/CONTRIBUTING.md`
- Code of Conduct: `/Users/vmaritato/Desktop/Open Source/presence-bridge/CODE_OF_CONDUCT.md`

---

## License

Apache-2.0. See `/Users/vmaritato/Desktop/Open Source/presence-bridge/LICENSE`.

---

<p align="center">
  <sub>
    Crafted with focus on performance, reliability, and clean developer experience.
  </sub>
  <br/>
  <sub>
    Copyright (c) presence-bridge contributors.
  </sub>
</p>

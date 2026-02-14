# presence-bridge

Cross-platform Rust daemon that bridges **Now Playing** metadata to **Discord Rich Presence**.

Pipeline: `Now Playing -> Event Engine -> Discord RPC`

## Features

- Native, non-Electron architecture.
- Smart scheduling (no fixed 1Hz Discord updates).
- Providers:
  - macOS: Apple Music (Music.app) via JXA + `osascript -l JavaScript`.
  - Windows: GSMTC (`Windows.Media.Control`).
  - Linux: MPRIS via DBus (`org.mpris.MediaPlayer2.*`).
- Discord RPC over local IPC (Unix socket / named pipe) with websocket fallback.
- Activity type set to **Listening** (instead of generic game activity).
- Diff engine with debounce + throttle.
- Stable `startTimestamp` per track.
- Configurable TOML + hot reload (`SIGHUP` on Unix, file polling fallback).
- Structured logging (`tracing`).
- CLI: `run`, `doctor`, `status`, `config init`.
- Unit tests for diff engine + URL builders.

## Install

```bash
git clone https://github.com/your-org/presence-bridge.git
cd presence-bridge
cargo build --release
```

Binary path:
- macOS/Linux: `target/release/presence-bridge`
- Windows: `target/release/presence-bridge.exe`

## Configuration

Initialize default config:

```bash
cargo run -- config init
```

Default path:
- macOS: `~/Library/Application Support/presence-bridge/config.toml`
- Linux: `~/.config/presence-bridge/config.toml`
- Windows: `%APPDATA%\presence-bridge\config.toml`

Example:

```toml
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

`discord_app_id` is required. Create an app in the Discord Developer Portal and copy its **Application ID**.

Assets are optional. If not uploaded, text + buttons still work.

## Usage

Run daemon:

```bash
cargo run -- run
```

Doctor diagnostics:

```bash
cargo run -- doctor
```

Status (single snapshot):

```bash
cargo run -- status
```

Reload config without restart:
- Unix: `kill -HUP <pid>`
- Windows: edit config file, watcher picks it up.

Environment overrides:
- `PRESENCE_BRIDGE_DISCORD_APP_ID`
- `PRESENCE_BRIDGE_LOG_LEVEL`
- `PRESENCE_BRIDGE_ENABLE_BUTTONS` (`true` / `false`)

## Scheduler policy

- `Playing`: poll provider every ~1s, send Discord updates at most every `presence_min_update_ms` (default 15s), unless track/state changed.
- `Paused`: poll every ~7s.
- `Stopped/no session`: poll every ~30s.

This keeps CPU usage low while preserving responsive transitions.

## macOS permissions (Apple Music)

If provider fails with automation/script errors:

1. Open **System Settings**.
2. Go to **Privacy & Security -> Automation**.
3. Allow your terminal app (Terminal/iTerm) to control **Music**.
4. Re-run `cargo run -- doctor`.

## Troubleshooting

- Discord closed: process keeps running and retries with exponential backoff.
- No media detected:
  - macOS: verify Music.app is running and playing.
  - Windows: check active GSMTC session (media app supports system media integration).
  - Linux: check MPRIS player is active (`playerctl -l`).
- Bad config: run `cargo run -- config init` and edit generated file.

## Development

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Releases

- CI builds and tests on macOS, Windows, Linux (`.github/workflows/ci.yml`).
- Tagging `v*` triggers packaged release artifacts + SHA256 checksums (`.github/workflows/release.yml`).
- Linux release also includes `.deb` and `.rpm` artifacts.

## Package Channels

- Homebrew template formula: `packaging/homebrew/presence-bridge.rb.tmpl`
- Scoop template manifest: `packaging/scoop/presence-bridge.json.tmpl`
- Linux systemd unit: `packaging/linux/presence-bridge.service`
- Automatic package index publish workflow: `.github/workflows/publish-package-indexes.yml`

Generate Homebrew formula from release checksum:

```bash
scripts/update-homebrew-formula.sh 0.1.0 <sha256-of-presence-bridge-macos-x86_64.tar.gz>
```

Generate Scoop manifest from release checksum:

```bash
scripts/update-scoop-manifest.sh 0.1.0 <sha256-of-presence-bridge-windows-x86_64.zip>
```

Automate Homebrew/Scoop updates after each GitHub Release:

1. Create a Homebrew tap repo (for example `vincenzomaritato/homebrew-tap`) and/or a Scoop bucket repo (for example `vincenzomaritato/scoop-bucket`).
2. Add repository secrets in this project:
   - `PACKAGE_REPOS_TOKEN`: PAT with write access to the tap/bucket repos.
   - `HOMEBREW_TAP_REPO`: `owner/repo` of tap repository.
   - `SCOOP_BUCKET_REPO`: `owner/repo` of Scoop bucket repository.
3. Publish a release (`v*` tag). The workflow will:
   - Read checksums from release assets.
   - Render formula/manifest from templates.
   - Commit and push updates to tap/bucket repos.

## License

Apache-2.0. See `LICENSE`.

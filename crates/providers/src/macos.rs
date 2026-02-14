use crate::{NowPlayingProvider, ProviderSnapshot};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use presence_bridge_core::{urls, PlaybackState, SourceApp, Track, TrackLinks};
use serde::Deserialize;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::process::Command;

#[derive(Default)]
pub struct AppleMusicProvider;

#[derive(Debug, Deserialize)]
struct JxaResult {
    state: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<u64>,
    position: Option<u64>,
    #[serde(rename = "persistentId")]
    persistent_id: Option<String>,
    error: Option<String>,
}

impl AppleMusicProvider {
    pub fn new() -> Self {
        Self
    }

    fn script_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("macos")
            .join("jxa_now_playing.js")
    }
}

#[async_trait]
impl NowPlayingProvider for AppleMusicProvider {
    fn name(&self) -> &'static str {
        "apple_music"
    }

    fn source(&self) -> SourceApp {
        SourceApp::AppleMusicMac
    }

    async fn poll(&mut self) -> Result<ProviderSnapshot> {
        let output = Command::new("osascript")
            .arg("-l")
            .arg("JavaScript")
            .arg(Self::script_path())
            .output()
            .await
            .context("failed to run osascript for Apple Music")?;

        if !output.status.success() {
            return Err(anyhow!(
                "osascript failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8(output.stdout).context("invalid UTF-8 from osascript")?;
        let parsed: JxaResult =
            serde_json::from_str(stdout.trim()).context("invalid JSON from jxa script")?;

        if let Some(err) = parsed.error {
            return Ok(ProviderSnapshot::with_error(self.name(), err));
        }

        match parsed.state.as_str() {
            "playing" | "paused" => {
                let title = parsed.title.unwrap_or_else(|| "Unknown Title".to_string());
                let artist = parsed
                    .artist
                    .unwrap_or_else(|| "Unknown Artist".to_string());
                let links = TrackLinks {
                    apple_music: Some(urls::apple_music_search_url(&artist, &title)),
                    spotify_search: Some(urls::spotify_search_url(&artist, &title)),
                };

                let track = Track {
                    id: parsed
                        .persistent_id
                        .clone()
                        .unwrap_or_else(|| format!("{}:{}", artist, title)),
                    title,
                    artist,
                    album: parsed.album,
                    duration_ms: parsed.duration,
                    position_ms: parsed.position,
                    is_playing: parsed.state == "playing",
                    source: SourceApp::AppleMusicMac,
                    links,
                    updated_at: SystemTime::now(),
                };

                Ok(ProviderSnapshot {
                    provider_name: self.name(),
                    state: if track.is_playing {
                        PlaybackState::Playing
                    } else {
                        PlaybackState::Paused
                    },
                    track: Some(track),
                    raw_state: Some(parsed.state),
                    last_error: None,
                })
            }
            _ => Ok(ProviderSnapshot::stopped(self.name())),
        }
    }
}

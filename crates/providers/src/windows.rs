use crate::{NowPlayingProvider, ProviderSnapshot};
use anyhow::Result;
use async_trait::async_trait;
use presence_bridge_core::{urls, PlaybackState, SourceApp, Track, TrackLinks};
use std::time::SystemTime;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};

#[derive(Default)]
pub struct WindowsGsmtcProvider;

impl WindowsGsmtcProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NowPlayingProvider for WindowsGsmtcProvider {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn source(&self) -> SourceApp {
        SourceApp::WindowsMediaSession
    }

    async fn poll(&mut self) -> Result<ProviderSnapshot> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
        let session = match manager.GetCurrentSession() {
            Ok(s) => s,
            Err(_) => return Ok(ProviderSnapshot::stopped(self.name())),
        };

        let props = session.TryGetMediaPropertiesAsync()?.get()?;
        let playback = session.GetPlaybackInfo()?;
        let timeline = session.GetTimelineProperties()?;

        let title = props.Title()?.to_string_lossy();
        let artist = props.Artist()?.to_string_lossy();
        let album = props.AlbumTitle()?.to_string_lossy();
        let status = playback.PlaybackStatus()?;

        let is_playing = status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
        let state = if is_playing {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        };

        let duration_100ns = timeline.EndTime()?.Duration - timeline.StartTime()?.Duration;
        let duration_ms = if duration_100ns > 0 {
            Some((duration_100ns as u64) / 10_000)
        } else {
            None
        };
        let position_100ns = timeline.Position()?.Duration;
        let position_ms = if position_100ns > 0 {
            Some((position_100ns as u64) / 10_000)
        } else {
            None
        };

        if title.is_empty() && artist.is_empty() {
            return Ok(ProviderSnapshot::stopped(self.name()));
        }

        let links = TrackLinks {
            apple_music: Some(urls::apple_music_search_url(&artist, &title)),
            spotify_search: Some(urls::spotify_search_url(&artist, &title)),
        };

        let track = Track {
            id: format!("{}:{}:{}", artist, title, album),
            title,
            artist,
            album: if album.is_empty() { None } else { Some(album) },
            duration_ms,
            position_ms,
            is_playing,
            source: SourceApp::WindowsMediaSession,
            links,
            updated_at: SystemTime::now(),
        };

        Ok(ProviderSnapshot {
            provider_name: self.name(),
            state,
            track: Some(track),
            raw_state: Some(format!("{status:?}")),
            last_error: None,
        })
    }
}

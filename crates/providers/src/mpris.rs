use crate::{NowPlayingProvider, ProviderSnapshot};
use anyhow::{Context, Result};
use async_trait::async_trait;
use presence_bridge_core::{urls, PlaybackState, SourceApp, Track, TrackLinks};
use std::time::SystemTime;
use zbus::zvariant::{OwnedValue, Str};
use zbus::{Connection, Proxy};

#[derive(Default)]
pub struct MprisProvider;

impl MprisProvider {
    pub fn new() -> Self {
        Self
    }

    async fn find_player(conn: &Connection) -> Result<Option<String>> {
        let proxy = Proxy::new(
            conn,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )
        .await?;

        let names: Vec<String> = proxy.call("ListNames", &()).await?;
        let mut players: Vec<String> = names
            .into_iter()
            .filter(|n| n.starts_with("org.mpris.MediaPlayer2."))
            .collect();
        players.sort();
        Ok(players.into_iter().next())
    }

    fn ov_to_string(v: &OwnedValue) -> Option<String> {
        let owned = v.try_clone().ok()?;
        if let Ok(s) = String::try_from(owned.try_clone().ok()?) {
            return Some(s.to_string());
        }
        if let Ok(s) = Str::try_from(owned) {
            return Some(s.to_string());
        }
        None
    }

    fn ov_to_i64(v: &OwnedValue) -> Option<i64> {
        if let Ok(i) = <i64>::try_from(v) {
            return Some(i);
        }
        if let Ok(u) = <u64>::try_from(v) {
            return Some(u as i64);
        }
        None
    }

    fn artist_from_value(v: &OwnedValue) -> Option<String> {
        if let Ok(arr) = Vec::<String>::try_from(v.try_clone().ok()?) {
            return arr.into_iter().next();
        }
        None
    }
}

#[async_trait]
impl NowPlayingProvider for MprisProvider {
    fn name(&self) -> &'static str {
        "mpris"
    }

    fn source(&self) -> SourceApp {
        SourceApp::Mpris
    }

    async fn poll(&mut self) -> Result<ProviderSnapshot> {
        let conn = Connection::session()
            .await
            .context("failed to connect DBus session")?;
        let player = match Self::find_player(&conn).await? {
            Some(p) => p,
            None => return Ok(ProviderSnapshot::stopped(self.name())),
        };

        let proxy = Proxy::new_owned(
            conn.clone(),
            player.clone(),
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2.Player",
        )
        .await?;

        let status: String = proxy.get_property("PlaybackStatus").await?;
        if status == "Stopped" {
            return Ok(ProviderSnapshot::stopped(self.name()));
        }

        let metadata: std::collections::HashMap<String, OwnedValue> =
            proxy.get_property("Metadata").await?;

        let title = metadata
            .get("xesam:title")
            .and_then(Self::ov_to_string)
            .unwrap_or_else(|| "Unknown Title".to_string());
        let artist = metadata
            .get("xesam:artist")
            .and_then(Self::artist_from_value)
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let album = metadata.get("xesam:album").and_then(Self::ov_to_string);
        let duration_ms = metadata
            .get("mpris:length")
            .and_then(Self::ov_to_i64)
            .map(|v| (v as u64) / 1_000);

        let position_raw: i64 = proxy.get_property("Position").await.unwrap_or(0);

        let position_ms = if position_raw > 0 {
            Some((position_raw as u64) / 1_000)
        } else {
            None
        };

        let is_playing = status == "Playing";
        let links = TrackLinks {
            apple_music: Some(urls::apple_music_search_url(&artist, &title)),
            spotify_search: Some(urls::spotify_search_url(&artist, &title)),
        };

        let track = Track {
            id: format!("{}:{}", artist, title),
            title,
            artist,
            album,
            duration_ms,
            position_ms,
            is_playing,
            source: SourceApp::Mpris,
            links,
            updated_at: SystemTime::now(),
        };

        Ok(ProviderSnapshot {
            provider_name: self.name(),
            state: if is_playing {
                PlaybackState::Playing
            } else {
                PlaybackState::Paused
            },
            track: Some(track),
            raw_state: Some(status),
            last_error: None,
        })
    }
}

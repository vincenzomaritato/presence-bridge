use presence_bridge_core::{AppConfig, PlaybackState, Track};
use presence_bridge_providers::ProviderSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    TrackChanged,
    StateChanged,
    Nothing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceButton {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceState {
    pub activity_type: u8,
    pub name: String,
    pub details: String,
    pub state: String,
    pub start_timestamp: Option<i64>,
    pub is_playing: bool,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
    pub buttons: Vec<PresenceButton>,
}

#[derive(Debug, Clone)]
pub enum EngineAction {
    Send(PresenceState),
    Clear,
    None,
}

#[derive(Debug, Clone)]
pub struct EngineOutput {
    pub action: EngineAction,
    pub next_poll_in: Duration,
    pub diff: DiffKind,
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub playing_poll: Duration,
    pub paused_poll: Duration,
    pub stopped_poll: Duration,
    pub min_presence_update_interval: Duration,
    pub debounce: Duration,
    pub enable_buttons: bool,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_play_image: Option<String>,
    pub small_pause_image: Option<String>,
}

impl EngineConfig {
    pub fn from_app_config(cfg: &AppConfig) -> Self {
        Self {
            playing_poll: Duration::from_millis(cfg.intervals.playing_poll_ms),
            paused_poll: Duration::from_millis(cfg.intervals.paused_poll_ms),
            stopped_poll: Duration::from_millis(cfg.intervals.stopped_poll_ms),
            min_presence_update_interval: Duration::from_millis(
                cfg.intervals.presence_min_update_ms,
            ),
            debounce: Duration::from_millis(cfg.intervals.debounce_ms),
            enable_buttons: cfg.enable_buttons,
            large_image: cfg.assets.large_image.clone(),
            large_text: cfg.assets.large_text.clone(),
            small_play_image: cfg.assets.small_play_image.clone(),
            small_pause_image: cfg.assets.small_pause_image.clone(),
        }
    }
}

pub struct EventEngine {
    cfg: EngineConfig,
    last_track: Option<Track>,
    last_sent_hash: Option<u64>,
    last_sent_at: Option<Instant>,
    last_state_flip_at: Option<Instant>,
    stable_start_timestamp: Option<i64>,
}

impl EventEngine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            cfg,
            last_track: None,
            last_sent_hash: None,
            last_sent_at: None,
            last_state_flip_at: None,
            stable_start_timestamp: None,
        }
    }

    pub fn update_config(&mut self, cfg: EngineConfig) {
        self.cfg = cfg;
    }

    pub fn tick(
        &mut self,
        snapshot: ProviderSnapshot,
        now_instant: Instant,
        now_system: SystemTime,
    ) -> EngineOutput {
        let next_poll_in = self.next_poll(snapshot.state);

        let current_track = snapshot.track;
        let diff = self.compute_diff(current_track.as_ref());

        let jitter_state_flip = match (&self.last_track, &current_track) {
            (Some(prev), Some(curr))
                if prev.id == curr.id && prev.is_playing != curr.is_playing =>
            {
                let by_flip = self
                    .last_state_flip_at
                    .map(|at| now_instant.duration_since(at) < self.cfg.debounce)
                    .unwrap_or(false);
                let by_recent_send = self
                    .last_sent_at
                    .map(|at| now_instant.duration_since(at) < self.cfg.debounce)
                    .unwrap_or(false);
                by_flip || by_recent_send
            }
            _ => false,
        };

        if matches!(diff, DiffKind::StateChanged)
            && jitter_state_flip
            && !matches!(diff, DiffKind::TrackChanged)
        {
            return EngineOutput {
                action: EngineAction::None,
                next_poll_in,
                diff: DiffKind::Nothing,
            };
        }

        if let Some(ref track) = current_track {
            if track.is_playing {
                match (&self.last_track, self.stable_start_timestamp) {
                    (Some(prev), Some(stable)) if prev.id == track.id => {
                        self.stable_start_timestamp = Some(stable);
                    }
                    _ => {
                        self.stable_start_timestamp = compute_start_timestamp(track, now_system);
                    }
                }
            } else {
                self.stable_start_timestamp = None;
            }
        } else {
            self.stable_start_timestamp = None;
        }

        let action = match current_track.as_ref() {
            Some(track) => {
                let presence = self.to_presence(track);
                let hash = hash_presence(&presence);
                let immediate_change =
                    diff == DiffKind::TrackChanged || diff == DiffKind::StateChanged;
                let due_keepalive = self
                    .last_sent_at
                    .map(|at| {
                        now_instant.duration_since(at) >= self.cfg.min_presence_update_interval
                    })
                    .unwrap_or(true);

                if immediate_change || (track.is_playing && due_keepalive) {
                    if self.last_sent_hash != Some(hash) || due_keepalive || immediate_change {
                        self.last_sent_hash = Some(hash);
                        self.last_sent_at = Some(now_instant);
                        EngineAction::Send(presence)
                    } else {
                        EngineAction::None
                    }
                } else {
                    EngineAction::None
                }
            }
            None => {
                if self.last_track.is_some() {
                    self.last_sent_hash = None;
                    self.last_sent_at = Some(now_instant);
                    EngineAction::Clear
                } else {
                    EngineAction::None
                }
            }
        };

        if let (Some(prev), Some(curr)) = (&self.last_track, &current_track) {
            if prev.id == curr.id && prev.is_playing != curr.is_playing {
                self.last_state_flip_at = Some(now_instant);
            }
        }

        self.last_track = current_track;

        EngineOutput {
            action,
            next_poll_in,
            diff,
        }
    }

    fn compute_diff(&self, current: Option<&Track>) -> DiffKind {
        match (&self.last_track, current) {
            (None, None) => DiffKind::Nothing,
            (None, Some(_)) | (Some(_), None) => DiffKind::TrackChanged,
            (Some(prev), Some(curr)) => {
                if prev.id != curr.id {
                    DiffKind::TrackChanged
                } else if prev.is_playing != curr.is_playing {
                    DiffKind::StateChanged
                } else {
                    DiffKind::Nothing
                }
            }
        }
    }

    fn to_presence(&self, track: &Track) -> PresenceState {
        let details = format!("{} — {}", track.artist, track.title);
        let state = if track.is_playing {
            track
                .album
                .as_ref()
                .map(|a| format!("on {a}"))
                .unwrap_or_else(|| "Playing".to_string())
        } else {
            "Paused".to_string()
        };

        let mut buttons = Vec::new();
        if self.cfg.enable_buttons {
            if let Some(url) = &track.links.apple_music {
                buttons.push(PresenceButton {
                    label: "Open/Search Apple Music".to_string(),
                    url: url.clone(),
                });
            }
            if let Some(url) = &track.links.spotify_search {
                buttons.push(PresenceButton {
                    label: "Search Spotify".to_string(),
                    url: url.clone(),
                });
            }
        }
        buttons.truncate(2);

        PresenceState {
            activity_type: 2,
            name: "Listening".to_string(),
            details,
            state,
            start_timestamp: if track.is_playing {
                self.stable_start_timestamp
            } else {
                None
            },
            is_playing: track.is_playing,
            large_image: self.cfg.large_image.clone(),
            large_text: self.cfg.large_text.clone(),
            small_image: if track.is_playing {
                self.cfg.small_play_image.clone()
            } else {
                self.cfg.small_pause_image.clone()
            },
            small_text: Some(if track.is_playing {
                "Playing".to_string()
            } else {
                "Paused".to_string()
            }),
            buttons,
        }
    }

    fn next_poll(&self, state: PlaybackState) -> Duration {
        match state {
            PlaybackState::Playing => self.cfg.playing_poll,
            PlaybackState::Paused => self.cfg.paused_poll,
            PlaybackState::Stopped => self.cfg.stopped_poll,
        }
    }
}

fn compute_start_timestamp(track: &Track, now_system: SystemTime) -> Option<i64> {
    if !track.is_playing {
        return None;
    }
    let now_epoch = now_system.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let pos_sec = (track.position_ms.unwrap_or(0) / 1_000) as i64;
    Some(now_epoch - pos_sec)
}

fn hash_presence(state: &PresenceState) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.details.hash(&mut hasher);
    state.activity_type.hash(&mut hasher);
    state.name.hash(&mut hasher);
    state.state.hash(&mut hasher);
    state.start_timestamp.hash(&mut hasher);
    state.is_playing.hash(&mut hasher);
    state.large_image.hash(&mut hasher);
    state.large_text.hash(&mut hasher);
    state.small_image.hash(&mut hasher);
    state.small_text.hash(&mut hasher);
    for b in &state.buttons {
        b.label.hash(&mut hasher);
        b.url.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{DiffKind, EngineAction, EngineConfig, EventEngine};
    use presence_bridge_core::{PlaybackState, SourceApp, Track, TrackLinks};
    use presence_bridge_providers::ProviderSnapshot;
    use std::time::{Duration, Instant, SystemTime};

    fn cfg() -> EngineConfig {
        EngineConfig {
            playing_poll: Duration::from_secs(1),
            paused_poll: Duration::from_secs(7),
            stopped_poll: Duration::from_secs(30),
            min_presence_update_interval: Duration::from_secs(15),
            debounce: Duration::from_millis(500),
            enable_buttons: true,
            large_image: Some("app_icon".to_string()),
            large_text: Some("presence-bridge".to_string()),
            small_play_image: Some("play".to_string()),
            small_pause_image: Some("pause".to_string()),
        }
    }

    fn snapshot(id: &str, playing: bool) -> ProviderSnapshot {
        ProviderSnapshot {
            provider_name: "test",
            state: if playing {
                PlaybackState::Playing
            } else {
                PlaybackState::Paused
            },
            track: Some(Track {
                id: id.to_string(),
                title: "Title".to_string(),
                artist: "Artist".to_string(),
                album: Some("Album".to_string()),
                duration_ms: Some(120_000),
                position_ms: Some(20_000),
                is_playing: playing,
                source: SourceApp::Unknown,
                links: TrackLinks {
                    apple_music: Some("https://example.com/apple".to_string()),
                    spotify_search: Some("https://example.com/spotify".to_string()),
                },
                updated_at: SystemTime::now(),
            }),
            raw_state: None,
            last_error: None,
        }
    }

    #[test]
    fn detects_track_change() {
        let mut engine = EventEngine::new(cfg());
        let now = Instant::now();

        let first = engine.tick(snapshot("1", true), now, SystemTime::now());
        assert!(matches!(first.action, EngineAction::Send(_)));

        let second = engine.tick(
            snapshot("2", true),
            now + Duration::from_secs(1),
            SystemTime::now(),
        );
        assert_eq!(second.diff, DiffKind::TrackChanged);
        assert!(matches!(second.action, EngineAction::Send(_)));
    }

    #[test]
    fn debounces_rapid_play_pause_jitter() {
        let mut engine = EventEngine::new(cfg());
        let now = Instant::now();

        let _ = engine.tick(snapshot("1", true), now, SystemTime::now());
        let paused = engine.tick(
            snapshot("1", false),
            now + Duration::from_millis(100),
            SystemTime::now(),
        );

        assert_eq!(paused.diff, DiffKind::Nothing);
        assert!(matches!(paused.action, EngineAction::None));
    }

    #[test]
    fn keeps_stable_timestamp_during_same_track() {
        let mut engine = EventEngine::new(cfg());
        let now = Instant::now();

        let first = engine.tick(
            snapshot("1", true),
            now,
            SystemTime::UNIX_EPOCH + Duration::from_secs(100),
        );
        let first_ts = match first.action {
            EngineAction::Send(p) => p.start_timestamp,
            _ => None,
        };

        let second = engine.tick(
            snapshot("1", true),
            now + Duration::from_secs(16),
            SystemTime::UNIX_EPOCH + Duration::from_secs(116),
        );

        let second_ts = match second.action {
            EngineAction::Send(p) => p.start_timestamp,
            _ => None,
        };

        assert_eq!(first_ts, second_ts);
    }
}

use serde::{Deserialize, Serialize};

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigIntervals {
    pub playing_poll_ms: u64,
    pub paused_poll_ms: u64,
    pub stopped_poll_ms: u64,
    pub presence_min_update_ms: u64,
    pub debounce_ms: u64,
    pub file_watch_poll_ms: u64,
}

impl Default for ConfigIntervals {
    fn default() -> Self {
        Self {
            playing_poll_ms: 1_000,
            paused_poll_ms: 7_000,
            stopped_poll_ms: 30_000,
            presence_min_update_ms: 15_000,
            debounce_ms: 500,
            file_watch_poll_ms: 10_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetsConfig {
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_play_image: Option<String>,
    pub small_pause_image: Option<String>,
}

impl Default for AssetsConfig {
    fn default() -> Self {
        Self {
            large_image: Some("app_icon".to_string()),
            large_text: Some("presence-bridge".to_string()),
            small_play_image: Some("play".to_string()),
            small_pause_image: Some("pause".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub discord_app_id: String,
    pub provider_priority: Vec<String>,
    pub intervals: ConfigIntervals,
    pub enable_buttons: bool,
    pub log_level: String,
    pub assets: AssetsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            discord_app_id: "YOUR_DISCORD_APP_ID".to_string(),
            provider_priority: vec![
                "apple_music".to_string(),
                "windows".to_string(),
                "mpris".to_string(),
            ],
            intervals: ConfigIntervals::default(),
            enable_buttons: true,
            log_level: "info".to_string(),
            assets: AssetsConfig::default(),
        }
    }
}

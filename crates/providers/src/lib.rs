use anyhow::Result;
use async_trait::async_trait;
use presence_bridge_core::{PlaybackState, SourceApp, Track};

#[derive(Debug, Clone)]
pub struct ProviderSnapshot {
    pub provider_name: &'static str,
    pub state: PlaybackState,
    pub track: Option<Track>,
    pub raw_state: Option<String>,
    pub last_error: Option<String>,
}

impl ProviderSnapshot {
    pub fn stopped(provider_name: &'static str) -> Self {
        Self {
            provider_name,
            state: PlaybackState::Stopped,
            track: None,
            raw_state: Some("stopped".to_string()),
            last_error: None,
        }
    }

    pub fn with_error(provider_name: &'static str, err: impl ToString) -> Self {
        Self {
            provider_name,
            state: PlaybackState::Stopped,
            track: None,
            raw_state: Some("error".to_string()),
            last_error: Some(err.to_string()),
        }
    }
}

#[async_trait]
pub trait NowPlayingProvider: Send {
    fn name(&self) -> &'static str;
    fn source(&self) -> SourceApp;
    async fn poll(&mut self) -> Result<ProviderSnapshot>;
}

pub struct ProviderChain {
    providers: Vec<Box<dyn NowPlayingProvider>>,
}

impl ProviderChain {
    pub fn new(providers: Vec<Box<dyn NowPlayingProvider>>) -> Self {
        Self { providers }
    }

    pub async fn poll_best(&mut self) -> ProviderSnapshot {
        let mut fallback: Option<ProviderSnapshot> = None;
        for provider in self.providers.iter_mut() {
            match provider.poll().await {
                Ok(snapshot) => {
                    if snapshot.state != PlaybackState::Stopped || snapshot.track.is_some() {
                        return snapshot;
                    }
                    if fallback.is_none() {
                        fallback = Some(snapshot);
                    }
                }
                Err(err) => {
                    if fallback.is_none() {
                        fallback = Some(ProviderSnapshot::with_error(provider.name(), err));
                    }
                }
            }
        }

        fallback.unwrap_or_else(|| ProviderSnapshot::stopped("none"))
    }

    pub fn provider_names(&self) -> Vec<&'static str> {
        self.providers.iter().map(|p| p.name()).collect()
    }
}

pub fn build_provider_chain(priority: &[String]) -> ProviderChain {
    let mut providers: Vec<Box<dyn NowPlayingProvider>> = Vec::new();

    for item in priority {
        match item.as_str() {
            "apple_music" => {
                if let Some(p) = platform::apple_music_provider() {
                    providers.push(p);
                }
            }
            "windows" => {
                if let Some(p) = platform::windows_provider() {
                    providers.push(p);
                }
            }
            "mpris" => {
                if let Some(p) = platform::mpris_provider() {
                    providers.push(p);
                }
            }
            _ => {}
        }
    }

    if providers.is_empty() {
        providers.push(Box::new(NullProvider));
    }

    ProviderChain::new(providers)
}

struct NullProvider;

#[async_trait]
impl NowPlayingProvider for NullProvider {
    fn name(&self) -> &'static str {
        "null"
    }

    fn source(&self) -> SourceApp {
        SourceApp::Unknown
    }

    async fn poll(&mut self) -> Result<ProviderSnapshot> {
        Ok(ProviderSnapshot::stopped(self.name()))
    }
}

mod platform {
    use super::NowPlayingProvider;

    #[cfg(target_os = "linux")]
    pub fn mpris_provider() -> Option<Box<dyn NowPlayingProvider>> {
        Some(Box::new(crate::mpris::MprisProvider::new()))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn mpris_provider() -> Option<Box<dyn NowPlayingProvider>> {
        None
    }

    #[cfg(target_os = "macos")]
    pub fn apple_music_provider() -> Option<Box<dyn NowPlayingProvider>> {
        Some(Box::new(crate::macos::AppleMusicProvider::new()))
    }

    #[cfg(not(target_os = "macos"))]
    pub fn apple_music_provider() -> Option<Box<dyn NowPlayingProvider>> {
        None
    }

    #[cfg(target_os = "windows")]
    pub fn windows_provider() -> Option<Box<dyn NowPlayingProvider>> {
        Some(Box::new(crate::windows::WindowsGsmtcProvider::new()))
    }

    #[cfg(not(target_os = "windows"))]
    pub fn windows_provider() -> Option<Box<dyn NowPlayingProvider>> {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod mpris;
#[cfg(target_os = "windows")]
mod windows;

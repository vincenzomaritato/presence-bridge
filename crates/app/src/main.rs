use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use presence_bridge_core::AppConfig;
use presence_bridge_discord_rpc::DiscordRpcClient;
use presence_bridge_engine::{EngineAction, EngineConfig, EventEngine};
use presence_bridge_providers::build_provider_chain;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(
    name = "presence-bridge",
    about = "Now Playing -> Event Engine -> Discord RPC"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Run,
    Doctor,
    Status,
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cmd = cli.command.unwrap_or(Commands::Run);
    let cfg_path = cli.config.unwrap_or_else(default_config_path);

    match cmd {
        Commands::Config {
            action: ConfigAction::Init,
        } => {
            init_config(&cfg_path)?;
            println!("Initialized config at {}", cfg_path.display());
            Ok(())
        }
        Commands::Doctor => {
            let cfg = load_or_default(&cfg_path)?;
            init_logging(&cfg.log_level);
            doctor(&cfg).await
        }
        Commands::Status => {
            let cfg = load_or_default(&cfg_path)?;
            init_logging(&cfg.log_level);
            status(&cfg).await
        }
        Commands::Run => {
            let cfg = load_or_default(&cfg_path)?;
            init_logging(&cfg.log_level);
            run(cfg, cfg_path).await
        }
    }
}

async fn run(mut cfg: AppConfig, cfg_path: PathBuf) -> Result<()> {
    let mut chain = build_provider_chain(&cfg.provider_priority);
    let mut engine = EventEngine::new(EngineConfig::from_app_config(&cfg));
    let mut discord = DiscordRpcClient::new(cfg.discord_app_id.clone());

    info!(providers = ?chain.provider_names(), "presence-bridge started");

    let (reload_tx, mut reload_rx) = mpsc::channel::<()>(4);
    spawn_reload_watchers(
        cfg_path.clone(),
        cfg.intervals.file_watch_poll_ms,
        reload_tx,
    )
    .await?;

    let mut next_poll_in = Duration::from_secs(0);

    loop {
        tokio::select! {
            _ = tokio::time::sleep(next_poll_in) => {
                let snapshot = chain.poll_best().await;
                let out = engine.tick(snapshot, Instant::now(), SystemTime::now());
                next_poll_in = out.next_poll_in;

                match out.action {
                    EngineAction::Send(state) => {
                        if let Err(err) = discord.set_activity(&state).await {
                            warn!(error=%err, "discord rpc set_activity failed; will retry with backoff");
                        }
                    }
                    EngineAction::Clear => {
                        if let Err(err) = discord.clear_activity().await {
                            warn!(error=%err, "discord rpc clear_activity failed; will retry with backoff");
                        }
                    }
                    EngineAction::None => {}
                }
            }
            msg = reload_rx.recv() => {
                if msg.is_some() {
                    match load_or_default(&cfg_path) {
                        Ok(new_cfg) => {
                            cfg = new_cfg;
                            engine.update_config(EngineConfig::from_app_config(&cfg));
                            discord.update_client_id(cfg.discord_app_id.clone());
                            chain = build_provider_chain(&cfg.provider_priority);
                            info!("configuration reloaded");
                            next_poll_in = Duration::from_secs(0);
                        }
                        Err(err) => {
                            error!(error=%err, "failed to reload config");
                        }
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("received ctrl-c; shutting down");
                break;
            }
        }
    }

    Ok(())
}

async fn doctor(cfg: &AppConfig) -> Result<()> {
    println!("== presence-bridge doctor ==");

    let discord_ok = discord_running().await;
    println!(
        "Discord RPC local endpoint: {}",
        if discord_ok {
            "reachable"
        } else {
            "not reachable"
        }
    );

    let mut chain = build_provider_chain(&cfg.provider_priority);
    let snapshot = chain.poll_best().await;
    println!("Provider checked: {}", snapshot.provider_name);
    println!("Provider state: {:?}", snapshot.state);

    if let Some(track) = snapshot.track {
        println!("Now playing: {} - {}", track.artist, track.title);
    } else {
        println!("No active media session");
    }

    if let Some(err) = snapshot.last_error {
        println!("Provider error: {err}");
    }

    #[cfg(target_os = "macos")]
    {
        println!(
            "macOS automation: verify System Settings > Privacy & Security > Automation allows Terminal (or your shell) to control Music"
        );
    }

    Ok(())
}

async fn status(cfg: &AppConfig) -> Result<()> {
    let mut chain = build_provider_chain(&cfg.provider_priority);
    let snapshot = chain.poll_best().await;

    println!("provider: {}", snapshot.provider_name);
    println!("state: {:?}", snapshot.state);
    if let Some(track) = snapshot.track {
        println!("track: {} - {}", track.artist, track.title);
        if let Some(album) = track.album {
            println!("album: {}", album);
        }
    } else {
        println!("track: <none>");
    }

    if let Some(err) = snapshot.last_error {
        println!("error: {err}");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn default_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("presence-bridge").join("config.toml")
}

#[cfg(not(target_os = "macos"))]
fn default_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("presence-bridge").join("config.toml")
}

fn init_config(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }
    let cfg = AppConfig::default();
    let toml = toml::to_string_pretty(&cfg)?;
    std::fs::write(path, toml)
        .with_context(|| format!("failed to write config file {}", path.display()))?;
    Ok(())
}

fn load_or_default(path: &Path) -> Result<AppConfig> {
    let mut cfg = if !path.exists() {
        AppConfig::default()
    } else {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))?
    };
    apply_env_overrides(&mut cfg);
    Ok(cfg)
}

fn init_logging(log_level: &str) {
    let filter = tracing_subscriber::EnvFilter::try_new(log_level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .try_init();
}

async fn spawn_reload_watchers(path: PathBuf, poll_ms: u64, tx: mpsc::Sender<()>) -> Result<()> {
    let tx_poll = tx.clone();
    tokio::spawn(async move {
        let mut known_mtime = file_mtime(&path);
        let sleep = Duration::from_millis(poll_ms.max(2_000));
        loop {
            tokio::time::sleep(sleep).await;
            let current = file_mtime(&path);
            if current.is_some() && current != known_mtime {
                known_mtime = current;
                let _ = tx_poll.send(()).await;
            }
        }
    });

    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let tx_hup = tx.clone();
        tokio::spawn(async move {
            if let Ok(mut sig) = signal(SignalKind::hangup()) {
                while sig.recv().await.is_some() {
                    let _ = tx_hup.send(()).await;
                }
            }
        });
    }

    Ok(())
}

fn file_mtime(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

async fn discord_running() -> bool {
    #[cfg(unix)]
    {
        for slot in 0..=9 {
            if discord_ipc_exists(slot) {
                return true;
            }
        }
    }

    let ports = [6463, 6464, 6465, 6466, 6467, 6468, 6469, 6470, 6471, 6472];
    for port in ports {
        let addr = format!("127.0.0.1:{port}");
        if tokio::time::timeout(
            Duration::from_millis(200),
            tokio::net::TcpStream::connect(addr),
        )
        .await
        .ok()
        .and_then(Result::ok)
        .is_some()
        {
            return true;
        }
    }
    false
}

#[cfg(unix)]
fn discord_ipc_exists(slot: u8) -> bool {
    let mut candidates = Vec::new();
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        candidates.push(PathBuf::from(tmpdir).join(format!("discord-ipc-{slot}")));
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        candidates.push(PathBuf::from(runtime).join(format!("discord-ipc-{slot}")));
    }
    candidates.push(PathBuf::from(format!("/tmp/discord-ipc-{slot}")));
    candidates.push(PathBuf::from(format!("/private/tmp/discord-ipc-{slot}")));

    candidates.into_iter().any(|p| p.exists())
}

fn apply_env_overrides(cfg: &mut AppConfig) {
    if let Ok(v) = std::env::var("PRESENCE_BRIDGE_DISCORD_APP_ID") {
        if !v.trim().is_empty() {
            cfg.discord_app_id = v;
        }
    }
    if let Ok(v) = std::env::var("PRESENCE_BRIDGE_LOG_LEVEL") {
        if !v.trim().is_empty() {
            cfg.log_level = v;
        }
    }
    if let Ok(v) = std::env::var("PRESENCE_BRIDGE_ENABLE_BUTTONS") {
        if let Ok(parsed) = v.parse::<bool>() {
            cfg.enable_buttons = parsed;
        }
    }
}

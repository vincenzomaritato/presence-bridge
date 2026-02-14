use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use presence_bridge_engine::PresenceState;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, warn};
use url::Url;

const PORTS: [u16; 10] = [6463, 6464, 6465, 6466, 6467, 6468, 6469, 6470, 6471, 6472];
const IPC_SLOTS: [u8; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
const BACKOFF_STEPS: [Duration; 4] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(30),
];

const OPCODE_HANDSHAKE: i32 = 0;
const OPCODE_FRAME: i32 = 1;

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

enum Transport {
    Ipc(IpcTransport),
    Ws(Ws),
}

#[cfg(unix)]
enum IpcTransport {
    Unix(tokio::net::UnixStream),
}

#[cfg(windows)]
enum IpcTransport {
    Pipe(tokio::net::windows::named_pipe::NamedPipeClient),
}

pub struct DiscordRpcClient {
    client_id: String,
    transport: Option<Transport>,
    backoff_idx: usize,
    next_retry_at: Instant,
}

impl DiscordRpcClient {
    pub fn new(client_id: String) -> Self {
        Self {
            client_id,
            transport: None,
            backoff_idx: 0,
            next_retry_at: Instant::now(),
        }
    }

    pub fn update_client_id(&mut self, client_id: String) {
        if self.client_id != client_id {
            self.client_id = client_id;
            self.transport = None;
            self.backoff_idx = 0;
            self.next_retry_at = Instant::now();
        }
    }

    pub async fn set_activity(&mut self, state: &PresenceState) -> Result<()> {
        self.ensure_connected().await?;
        let mut activity = json!({
            "type": state.activity_type,
            "name": state.name,
            "details": state.details,
            "state": state.state,
            "timestamps": state.start_timestamp.map(|ts| json!({"start": ts})).unwrap_or(json!({})),
            "buttons": state.buttons.iter().map(|b| json!({"label": b.label, "url": b.url})).collect::<Vec<_>>()
        });

        if let Some(obj) = activity.as_object_mut() {
            if let Some(assets) = build_assets(state) {
                obj.insert("assets".to_string(), assets);
            }
        }

        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": activity
            },
            "nonce": format!("{}", uuid_like())
        });

        if let Err(err) = self.send_payload(payload).await {
            self.transport = None;
            self.schedule_backoff();
            return Err(err);
        }
        Ok(())
    }

    pub async fn clear_activity(&mut self) -> Result<()> {
        self.ensure_connected().await?;
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": serde_json::Value::Null
            },
            "nonce": format!("{}", uuid_like())
        });

        if let Err(err) = self.send_payload(payload).await {
            self.transport = None;
            self.schedule_backoff();
            return Err(err);
        }
        Ok(())
    }

    async fn ensure_connected(&mut self) -> Result<()> {
        if self.transport.is_some() {
            return Ok(());
        }
        let now = Instant::now();
        if now < self.next_retry_at {
            return Err(anyhow!("discord reconnect backoff active"));
        }

        if let Some(ipc) = try_connect_ipc(&self.client_id).await {
            self.transport = Some(Transport::Ipc(ipc));
            self.backoff_idx = 0;
            self.next_retry_at = Instant::now();
            return Ok(());
        }

        if let Some(ws) = try_connect_ws(&self.client_id).await {
            self.transport = Some(Transport::Ws(ws));
            self.backoff_idx = 0;
            self.next_retry_at = Instant::now();
            return Ok(());
        }

        self.schedule_backoff();
        Err(anyhow!("unable to connect to local Discord RPC"))
    }

    async fn send_payload(&mut self, payload: serde_json::Value) -> Result<()> {
        match self.transport.as_mut() {
            Some(Transport::Ipc(ipc)) => {
                send_ipc_frame(ipc, OPCODE_FRAME, payload.to_string().as_bytes()).await?;
                let (_, raw) = recv_ipc_frame(ipc).await?;
                validate_rpc_response(&raw)
            }
            Some(Transport::Ws(ws)) => {
                ws.send(Message::Text(payload.to_string()))
                    .await
                    .context("failed sending discord ws message")?;
                if let Some(msg) = ws.next().await {
                    match msg {
                        Ok(Message::Text(text)) => validate_rpc_response(text.as_bytes()),
                        Ok(Message::Binary(bin)) => validate_rpc_response(&bin),
                        Err(err) => Err(anyhow!("discord ws receive failed: {err}")),
                        _ => Ok(()),
                    }
                } else {
                    Err(anyhow!("discord ws closed"))
                }
            }
            None => Err(anyhow!("discord transport not connected")),
        }
    }

    fn schedule_backoff(&mut self) {
        let idx = self.backoff_idx.min(BACKOFF_STEPS.len() - 1);
        self.next_retry_at = Instant::now() + BACKOFF_STEPS[idx];
        self.backoff_idx = (self.backoff_idx + 1).min(BACKOFF_STEPS.len() - 1);
    }
}

async fn try_connect_ws(client_id: &str) -> Option<Ws> {
    for port in PORTS {
        let url = Url::parse(&format!("ws://127.0.0.1:{port}/?v=1&client_id={client_id}")).ok()?;
        match connect_async(url.as_str()).await {
            Ok((mut ws, _)) => {
                let handshake = json!({ "v": 1, "client_id": client_id });
                if ws.send(Message::Text(handshake.to_string())).await.is_err() {
                    continue;
                }
                if ws.next().await.is_some() {
                    debug!("connected to discord rpc websocket on port {}", port);
                    return Some(ws);
                }
            }
            Err(err) => {
                debug!("discord ws connect failed on port {}: {}", port, err);
            }
        }
    }
    None
}

async fn try_connect_ipc(client_id: &str) -> Option<IpcTransport> {
    for slot in IPC_SLOTS {
        match connect_ipc_slot(slot).await {
            Ok(mut ipc) => {
                let hs = json!({"v": 1, "client_id": client_id}).to_string();
                if send_ipc_frame(&mut ipc, OPCODE_HANDSHAKE, hs.as_bytes())
                    .await
                    .is_err()
                {
                    continue;
                }
                if recv_ipc_frame(&mut ipc).await.is_ok() {
                    debug!("connected to discord ipc slot {}", slot);
                    return Some(ipc);
                }
            }
            Err(err) => {
                debug!("discord ipc slot {} unavailable: {}", slot, err);
            }
        }
    }
    None
}

#[cfg(unix)]
async fn connect_ipc_slot(slot: u8) -> Result<IpcTransport> {
    use std::path::PathBuf;

    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        paths.push(PathBuf::from(tmpdir).join(format!("discord-ipc-{slot}")));
    }
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        paths.push(PathBuf::from(runtime_dir).join(format!("discord-ipc-{slot}")));
    }
    paths.push(PathBuf::from(format!("/tmp/discord-ipc-{slot}")));
    paths.push(PathBuf::from(format!("/private/tmp/discord-ipc-{slot}")));

    for p in paths {
        if let Ok(stream) = tokio::net::UnixStream::connect(&p).await {
            return Ok(IpcTransport::Unix(stream));
        }
    }

    Err(anyhow!("no unix discord ipc socket found"))
}

#[cfg(windows)]
async fn connect_ipc_slot(slot: u8) -> Result<IpcTransport> {
    use tokio::net::windows::named_pipe::ClientOptions;
    let path = format!(r"\\?\pipe\discord-ipc-{}", slot);
    let pipe = ClientOptions::new().open(&path)?;
    Ok(IpcTransport::Pipe(pipe))
}

async fn send_ipc_frame(ipc: &mut IpcTransport, opcode: i32, payload: &[u8]) -> Result<()> {
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    frame.extend_from_slice(payload);

    match ipc {
        #[cfg(unix)]
        IpcTransport::Unix(stream) => {
            stream.write_all(&frame).await?;
            stream.flush().await?;
        }
        #[cfg(windows)]
        IpcTransport::Pipe(pipe) => {
            pipe.write_all(&frame).await?;
            pipe.flush().await?;
        }
    }
    Ok(())
}

async fn recv_ipc_frame(ipc: &mut IpcTransport) -> Result<(i32, Vec<u8>)> {
    let mut hdr = [0u8; 8];
    match ipc {
        #[cfg(unix)]
        IpcTransport::Unix(stream) => stream.read_exact(&mut hdr).await?,
        #[cfg(windows)]
        IpcTransport::Pipe(pipe) => pipe.read_exact(&mut hdr).await?,
    };

    let opcode = i32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
    let len = i32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
    if len < 0 {
        return Err(anyhow!("invalid discord ipc frame length"));
    }

    let mut payload = vec![0u8; len as usize];
    match ipc {
        #[cfg(unix)]
        IpcTransport::Unix(stream) => stream.read_exact(&mut payload).await?,
        #[cfg(windows)]
        IpcTransport::Pipe(pipe) => pipe.read_exact(&mut payload).await?,
    };

    if opcode != OPCODE_FRAME && opcode != OPCODE_HANDSHAKE {
        warn!("discord ipc unexpected opcode {}", opcode);
    }

    Ok((opcode, payload))
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{n:x}")
}

fn build_assets(state: &PresenceState) -> Option<serde_json::Value> {
    let mut assets = serde_json::Map::new();
    if let Some(v) = &state.large_image {
        assets.insert("large_image".to_string(), json!(v));
    }
    if let Some(v) = &state.large_text {
        assets.insert("large_text".to_string(), json!(v));
    }
    if let Some(v) = &state.small_image {
        assets.insert("small_image".to_string(), json!(v));
    }
    if let Some(v) = &state.small_text {
        assets.insert("small_text".to_string(), json!(v));
    }
    if assets.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(assets))
    }
}

fn validate_rpc_response(raw: &[u8]) -> Result<()> {
    let value: serde_json::Value = match serde_json::from_slice(raw) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    if value
        .get("evt")
        .and_then(|v| v.as_str())
        .map(|evt| evt.eq_ignore_ascii_case("ERROR"))
        .unwrap_or(false)
    {
        let data = value.get("data");
        let code = data
            .and_then(|d| d.get("code"))
            .and_then(|c| c.as_i64())
            .unwrap_or_default();
        let msg = data
            .and_then(|d| d.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown discord rpc error");
        return Err(anyhow!("discord rpc error {code}: {msg}"));
    }

    Ok(())
}

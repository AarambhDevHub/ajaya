//! WebSocket multi-room chat server example.
//!
//! Run: cargo run --manifest-path examples/websocket_chat/Cargo.toml
//!
//! Connect via browser: http://localhost:8080
//! Connect via wscat:   wscat -c ws://localhost:8080/ws
//! Connect (room):      wscat -c "ws://localhost:8080/ws/room/general"
//!
//! # Architecture
//!
//! - Each room has a `tokio::sync::broadcast::Sender<String>`.
//! - Rooms are created on first join and cleaned up when empty.
//! - Messages are JSON: `{ "from": "...", "msg": "..." }`.
//! - System events: `{ "system": true, "msg": "... joined/left" }`.

use std::collections::HashMap;
use std::sync::Arc;

use arvik::ws::{Message, WebSocket, WebSocketUpgrade};
use arvik::{Html, IntoResponse, Path, Query, Router, State, get};
use tokio::sync::{Mutex, broadcast};
use tracing::info;
use tracing_subscriber::EnvFilter;

// ── Types ─────────────────────────────────────────────────────────────────────

const CHANNEL_CAPACITY: usize = 64;
const DEFAULT_ROOM: &str = "general";

/// Shared state — one broadcast channel per room.
#[derive(Clone, Default)]
struct Rooms(Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>);

impl Rooms {
    /// Join a room, creating it if needed. Returns a (Sender, Receiver) pair.
    async fn join(&self, room: &str) -> (broadcast::Sender<String>, broadcast::Receiver<String>) {
        let mut map = self.0.lock().await;
        let tx = map
            .entry(room.to_owned())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .clone();
        let rx = tx.subscribe();
        (tx, rx)
    }

    /// Remove a room if it has no active receivers.
    async fn cleanup(&self, room: &str) {
        let mut map = self.0.lock().await;
        if let Some(tx) = map.get(room)
            && tx.receiver_count() == 0
        {
            map.remove(room);
            info!("room '{}' closed (empty)", room);
        }
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info,arvik=debug"))
        .init();

    let rooms = Rooms::default();

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/rooms", get(rooms_handler))
        .route("/ws", get(ws_default_handler))
        .route("/ws/room/{room}", get(ws_room_handler))
        .with_state(rooms);

    let addr = "0.0.0.0:8080";
    info!("Chat server listening on http://{addr}");
    arvik::serve_app(addr, app).await.unwrap();
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// Return the current list of active rooms as JSON.
async fn rooms_handler(State(rooms): State<Rooms>) -> impl IntoResponse {
    use arvik::Json;
    let map = rooms.0.lock().await;
    let mut list: Vec<String> = map.keys().cloned().collect();
    // Always include the default rooms even if empty
    for r in ["general", "rust", "random"] {
        if !list.contains(&r.to_string()) {
            list.push(r.to_string());
        }
    }
    list.sort();
    Json(list)
}

/// Serve the embedded HTML chat UI.
async fn index_handler() -> Html<&'static str> {
    Html(CHAT_HTML)
}

/// `/ws` — join the default "general" room.
async fn ws_default_handler(
    State(rooms): State<Rooms>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let username = sanitize_username(params.get("name").map(String::as_str).unwrap_or(""));
    ws.protocols(["chat"])
        .on_upgrade(move |socket| handle_socket(socket, rooms, DEFAULT_ROOM.to_owned(), username))
}

/// `/ws/room/{room}` — join a named room.
async fn ws_room_handler(
    Path(room): Path<String>,
    State(rooms): State<Rooms>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let room = sanitize_room(&room);
    let username = sanitize_username(params.get("name").map(String::as_str).unwrap_or(""));
    ws.protocols(["chat"])
        .on_upgrade(move |socket| handle_socket(socket, rooms, room, username))
}

// ── WebSocket session ─────────────────────────────────────────────────────────

async fn handle_socket(mut socket: WebSocket, rooms: Rooms, room: String, username: String) {
    let (tx, mut rx) = rooms.join(&room).await;

    // Use provided name or fall back to Guest-{ms}
    let user_id = if username.is_empty() {
        format!(
            "Guest-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_millis())
                .unwrap_or(0)
        )
    } else {
        username
    };

    info!("{} joined room '{}'", user_id, room);

    // Announce join
    let join_msg = serde_json::json!({
        "system": true,
        "room": room,
        "msg": format!("{user_id} joined the room"),
    })
    .to_string();
    let _ = tx.send(join_msg);

    // Send a welcome message directly to this socket
    let welcome = serde_json::json!({
        "system": true,
        "room": room,
        "msg": format!("Welcome {user_id}! You are in room '{room}'."),
    })
    .to_string();
    let _ = socket.send(Message::Text(welcome)).await;

    loop {
        tokio::select! {
            // ── Incoming from this client ──────────────────────────────────
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text = text.trim().to_owned();
                        if text.is_empty() { continue; }

                        let broadcast = serde_json::json!({
                            "from": user_id,
                            "room": room,
                            "msg": text,
                        })
                        .to_string();

                        info!("[{}] {}: {}", room, user_id, text);
                        let _ = tx.send(broadcast);
                    }
                    Some(Ok(Message::Binary(_))) => {
                        // Binary not supported in this example
                        let _ = socket.send(Message::Text(
                            r#"{"system":true,"msg":"Binary messages not supported"}"#.into()
                        )).await;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        // auto-pong: recv() handles this transparently,
                        // but in case a raw Ping slips through — pong it.
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        info!("{} error: {e}", user_id);
                        break;
                    }
                }
            }

            // ── Broadcast from other clients in this room ──────────────────
            broadcast = rx.recv() => {
                match broadcast {
                    Ok(msg) => {
                        if socket.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        info!("{} lagged by {} messages", user_id, n);
                    }
                    Err(_) => break, // channel closed
                }
            }
        }
    }

    // Announce leave
    let leave_msg = serde_json::json!({
        "system": true,
        "room": room,
        "msg": format!("{user_id} left the room"),
    })
    .to_string();
    let _ = tx.send(leave_msg);
    info!("{} left room '{}'", user_id, room);

    rooms.cleanup(&room).await;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Sanitize room name: lowercase alphanumeric + hyphens, max 32 chars.
fn sanitize_room(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .map(|c| c.to_ascii_lowercase())
        .take(32)
        .collect()
}

/// Sanitize username: alphanumeric + hyphens + underscores, max 20 chars.
fn sanitize_username(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(20)
        .collect()
}

// ── Embedded HTML ─────────────────────────────────────────────────────────────

const CHAT_HTML: &str = include_str!("../chat.html");

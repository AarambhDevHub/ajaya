//! # arvik SSE Demo
//!
//! Three SSE endpoints in a single binary:
//!
//! | Endpoint             | Description                                      |
//! |----------------------|--------------------------------------------------|
//! | `GET /counter`       | Simple integer counter, one tick per second      |
//! | `GET /json-stream`   | Structured JSON metrics at 500 ms intervals      |
//! | `GET /notifications` | Broadcast channel — fan-out to all subscribers  |
//! | `POST /notify`       | Push a message to all `/notifications` clients  |
//! | `GET /`             | HTML page that connects to all three streams     |
//!
//! Run with:
//! ```text
//! cargo run --manifest-path examples/sse_demo/Cargo.toml
//! ```
//! Then open http://127.0.0.1:3000 in your browser.

use std::convert::Infallible;
use std::time::Duration;

use arvik::sse::{Event, KeepAlive, Sse};
use arvik::{Html, Json, Router, State, get, post, serve_app};
use futures_util::{Stream, StreamExt as _};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::wrappers::{BroadcastStream, IntervalStream};

// ── Shared application state ──────────────────────────────────────────────────

/// Broadcast channel sender shared across all `/notifications` clients.
#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<String>,
}

// ── Payload types ─────────────────────────────────────────────────────────────

/// JSON payload emitted by `/json-stream`.
#[derive(Serialize)]
struct Metric {
    /// Monotonic sequence number.
    seq: u64,
    /// Simulated CPU usage (0–100 %).
    cpu: f64,
    /// Simulated memory usage (0–100 %).
    mem: f64,
}

/// Body accepted by `POST /notify`.
#[derive(Deserialize)]
struct NotifyPayload {
    message: String,
}

// ── Handler: simple counter ───────────────────────────────────────────────────

/// `GET /counter` — emits an integer every second.
///
/// Stream type: `Result<Event, Infallible>` (never errors).
async fn counter() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = IntervalStream::new(tokio::time::interval(Duration::from_secs(1)))
        .enumerate()
        .map(|(i, _): (usize, _)| {
            Ok(Event::default()
                .event("tick")
                .id(i.to_string())
                .data(i.to_string()))
        });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

// ── Handler: JSON metric stream ───────────────────────────────────────────────

/// `GET /json-stream` — emits a structured `Metric` JSON payload every 500 ms.
///
/// Uses `Event::json_data()` so there is no `.unwrap()` needed in the handler.
/// Stream type: `Result<Event, serde_json::Error>`.
async fn json_stream() -> Sse<impl Stream<Item = Result<Event, serde_json::Error>>> {
    let stream = IntervalStream::new(tokio::time::interval(Duration::from_millis(500)))
        .enumerate()
        .map(|(i, _): (usize, _)| {
            let metric = Metric {
                seq: i as u64,
                // Simulate smooth oscillating values without external deps.
                cpu: 30.0 + ((i as f64) * 0.25).sin().abs() * 55.0,
                mem: 40.0 + ((i as f64) * 0.17).cos().abs() * 35.0,
            };
            Event::default()
                .event("metric")
                .id(i.to_string())
                .json_data(&metric) // Returns Result<Event, serde_json::Error>
        });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)))
}

// ── Handler: broadcast channel (chat / notifications) ────────────────────────

/// `GET /notifications` — each subscriber receives every message sent to
/// `POST /notify`. Uses `tokio::sync::broadcast` for fan-out.
///
/// Stream type: `Result<Event, Infallible>` (lagged messages are silently skipped).
async fn notifications(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();

    // BroadcastStream yields `Result<T, BroadcastStreamRecvError>`.
    // We filter out `Lagged` errors (subscriber was too slow) with `filter_map`.
    let stream = BroadcastStream::new(rx)
        .filter_map(|r: Result<String, _>| async move { r.ok() })
        .map(|msg| Ok(Event::default().event("notification").data(msg)));

    Sse::new(stream).keep_alive(KeepAlive::new())
}

/// `POST /notify` — push `{"message": "..."}` to all `/notifications` clients.
async fn notify(State(state): State<AppState>, Json(payload): Json<NotifyPayload>) -> &'static str {
    // send() only errors when there are 0 receivers, which is fine.
    let _ = state.tx.send(payload.message);
    "ok"
}

// ── Handler: demo HTML page ───────────────────────────────────────────────────

async fn index() -> Html<&'static str> {
    Html(HTML)
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    // Capacity 32: if a subscriber can't keep up it will receive a Lagged error
    // (which we silently drop in the stream filter_map above).
    let (tx, _) = broadcast::channel(32);
    let state = AppState { tx };

    let app = Router::new()
        .route("/", get(index))
        .route("/counter", get(counter))
        .route("/json-stream", get(json_stream))
        .route("/notifications", get(notifications))
        .route("/notify", post(notify))
        .with_state(state);

    println!("SSE demo → http://127.0.0.1:3000");
    serve_app("0.0.0.0:3000", app).await.unwrap();
}

// ── Embedded HTML demo page ───────────────────────────────────────────────────

static HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Arvik SSE Demo</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

    :root {
      --bg:      #0d0f14;
      --surface: #151820;
      --border:  #252a35;
      --accent:  #6c63ff;
      --green:   #3ecf8e;
      --amber:   #f5a623;
      --red:     #ff5c5c;
      --text:    #e2e8f0;
      --muted:   #7a8499;
      --radius:  14px;
    }

    body {
      background: var(--bg);
      color: var(--text);
      font-family: 'Inter', 'Segoe UI', system-ui, sans-serif;
      min-height: 100vh;
      padding: 2rem 1rem;
    }

    header {
      text-align: center;
      margin-bottom: 2.5rem;
    }
    header h1 {
      font-size: 2rem;
      font-weight: 700;
      background: linear-gradient(135deg, var(--accent), var(--green));
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
    }
    header p { color: var(--muted); margin-top: .4rem; font-size: .95rem; }

    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
      gap: 1.5rem;
      max-width: 1100px;
      margin: 0 auto;
    }

    .card {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: var(--radius);
      padding: 1.5rem;
      display: flex;
      flex-direction: column;
      gap: 1rem;
    }

    .card-header {
      display: flex;
      align-items: center;
      gap: .75rem;
    }
    .dot {
      width: 10px; height: 10px;
      border-radius: 50%;
      background: var(--muted);
      flex-shrink: 0;
      transition: background .3s;
    }
    .dot.live { background: var(--green); box-shadow: 0 0 8px var(--green); animation: pulse 1.5s infinite; }
    @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:.4} }

    .card-header h2 { font-size: 1rem; font-weight: 600; }
    .card-header span { margin-left: auto; font-size: .75rem; color: var(--muted); font-family: monospace; }

    /* Counter */
    .counter-display {
      font-size: 4rem;
      font-weight: 800;
      text-align: center;
      font-variant-numeric: tabular-nums;
      background: linear-gradient(135deg, var(--accent), #a78bfa);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      min-height: 5rem;
      display: flex;
      align-items: center;
      justify-content: center;
    }

    /* Metric bars */
    .metric { display: flex; flex-direction: column; gap: .4rem; }
    .metric label { font-size: .8rem; color: var(--muted); display: flex; justify-content: space-between; }
    .bar-track { background: var(--border); border-radius: 999px; height: 8px; overflow: hidden; }
    .bar-fill  { height: 100%; border-radius: 999px; transition: width .4s ease; }
    #cpu-bar   { background: linear-gradient(90deg, var(--accent), #a78bfa); }
    #mem-bar   { background: linear-gradient(90deg, var(--green), #34d399); }
    .metric-seq { font-size: .75rem; color: var(--muted); text-align: right; }

    /* Notifications */
    .notif-log {
      background: #0a0c10;
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: .75rem;
      height: 160px;
      overflow-y: auto;
      font-family: monospace;
      font-size: .82rem;
      display: flex;
      flex-direction: column;
      gap: .3rem;
    }
    .notif-log .entry { color: var(--green); }
    .notif-log .entry span { color: var(--muted); margin-right: .5rem; }

    .send-row { display: flex; gap: .5rem; }
    .send-row input {
      flex: 1;
      background: #0a0c10;
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: .5rem .75rem;
      color: var(--text);
      font-size: .9rem;
      outline: none;
      transition: border-color .2s;
    }
    .send-row input:focus { border-color: var(--accent); }
    .send-row button {
      background: var(--accent);
      color: #fff;
      border: none;
      border-radius: 8px;
      padding: .5rem 1rem;
      font-size: .9rem;
      font-weight: 600;
      cursor: pointer;
      transition: opacity .2s, transform .1s;
    }
    .send-row button:hover { opacity: .85; }
    .send-row button:active { transform: scale(.96); }

    footer {
      text-align: center;
      margin-top: 3rem;
      color: var(--muted);
      font-size: .8rem;
    }
    footer a { color: var(--accent); text-decoration: none; }
  </style>
</head>
<body>
  <header>
    <h1>Arvik SSE Demo</h1>
    <p>Server-Sent Events — counter · JSON stream · broadcast notifications</p>
  </header>

  <div class="grid">

    <!-- ── Counter ─────────────────────────────────── -->
    <div class="card">
      <div class="card-header">
        <div class="dot" id="counter-dot"></div>
        <h2>Simple Counter</h2>
        <span>GET /counter</span>
      </div>
      <div class="counter-display" id="counter-val">—</div>
    </div>

    <!-- ── JSON Metric Stream ──────────────────────── -->
    <div class="card">
      <div class="card-header">
        <div class="dot" id="json-dot"></div>
        <h2>JSON Metric Stream</h2>
        <span>GET /json-stream</span>
      </div>
      <div class="metric">
        <label>CPU <span id="cpu-pct">—</span></label>
        <div class="bar-track"><div class="bar-fill" id="cpu-bar" style="width:0%"></div></div>
      </div>
      <div class="metric">
        <label>Memory <span id="mem-pct">—</span></label>
        <div class="bar-track"><div class="bar-fill" id="mem-bar" style="width:0%"></div></div>
      </div>
      <div class="metric-seq" id="metric-seq">seq —</div>
    </div>

    <!-- ── Broadcast Notifications ────────────────── -->
    <div class="card">
      <div class="card-header">
        <div class="dot" id="notif-dot"></div>
        <h2>Broadcast Notifications</h2>
        <span>GET /notifications</span>
      </div>
      <div class="notif-log" id="notif-log">
        <div class="entry" data-placeholder style="color:var(--muted)">Waiting for messages…</div>
      </div>
      <div class="send-row">
        <input id="notif-input" type="text" placeholder="Type a message and press Send" maxlength="200" />
        <button id="notif-send">Send</button>
      </div>
    </div>

  </div>

  <footer>
    Built with <a href="https://github.com/AarambhDevHub/arvik">Arvik</a> · SSE v0.5.1
  </footer>

  <script>
    // ── Counter stream ────────────────────────────────────────────────────────
    (function () {
      const dot = document.getElementById('counter-dot');
      const val = document.getElementById('counter-val');
      const es  = new EventSource('/counter');

      es.addEventListener('tick', e => {
        dot.classList.add('live');
        val.textContent = e.data;
      });
      es.onerror = () => dot.classList.remove('live');
    })();

    // ── JSON metric stream ────────────────────────────────────────────────────
    (function () {
      const dot    = document.getElementById('json-dot');
      const cpuPct = document.getElementById('cpu-pct');
      const cpuBar = document.getElementById('cpu-bar');
      const memPct = document.getElementById('mem-pct');
      const memBar = document.getElementById('mem-bar');
      const seq    = document.getElementById('metric-seq');
      const es     = new EventSource('/json-stream');

      es.addEventListener('metric', e => {
        dot.classList.add('live');
        const d = JSON.parse(e.data);
        cpuPct.textContent = d.cpu.toFixed(1) + '%';
        cpuBar.style.width = d.cpu.toFixed(1) + '%';
        memPct.textContent = d.mem.toFixed(1) + '%';
        memBar.style.width = d.mem.toFixed(1) + '%';
        seq.textContent    = 'seq ' + d.seq;
      });
      es.onerror = () => dot.classList.remove('live');
    })();

    // ── Broadcast notifications ───────────────────────────────────────────────
    (function () {
      const dot   = document.getElementById('notif-dot');
      const log   = document.getElementById('notif-log');
      const input = document.getElementById('notif-input');
      const btn   = document.getElementById('notif-send');
      const es    = new EventSource('/notifications');

      function appendMsg(text) {
        // Remove the placeholder the first time a real message arrives.
        // Using querySelector avoids the firstChild/text-node TypeError.
        const ph = log.querySelector('[data-placeholder]');
        if (ph) ph.remove();

        const now = new Date().toLocaleTimeString();
        const div = document.createElement('div');
        div.className = 'entry';
        div.innerHTML = `<span>${now}</span>${text}`;
        log.appendChild(div);
        log.scrollTop = log.scrollHeight;
      }

      // Light up the dot as soon as the SSE connection opens.
      es.onopen = () => dot.classList.add('live');

      es.addEventListener('notification', e => {
        appendMsg(e.data);
      });
      es.onerror = () => dot.classList.remove('live');

      async function sendMsg() {
        const msg = input.value.trim();
        if (!msg) return;
        input.value = '';
        await fetch('/notify', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ message: msg }),
        });
      }

      btn.addEventListener('click', sendMsg);
      input.addEventListener('keydown', e => { if (e.key === 'Enter') sendMsg(); });
    })();
  </script>
</body>
</html>
"#;

//! In-process testing utilities for the Arvik web framework.
//!
//! HTTP requests are dispatched directly against Arvik's Tower service without
//! binding a port. WebSocket tests use a short-lived loopback listener because
//! Arvik WebSockets rely on Hyper's upgrade IO.

use std::sync::{Arc, Mutex};

use arvik_core::{Body, Request, Response};
use arvik_router::Router;
use arvik_router::layer::{BoxCloneService, oneshot};
use bytes::Bytes;
use cookie::{Cookie, CookieJar};
use http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use serde::Serialize;

/// In-process HTTP test client for Arvik routers and services.
#[derive(Clone, Debug)]
pub struct TestClient {
    service: BoxCloneService,
    cookies: Arc<Mutex<CookieJar>>,
}

impl TestClient {
    /// Create a test client from a router.
    pub fn new(router: Router) -> Self {
        Self::from_service(router.into_service())
    }

    /// Create a test client from a pre-built Arvik service.
    pub fn from_service(service: BoxCloneService) -> Self {
        Self {
            service,
            cookies: Arc::new(Mutex::new(CookieJar::new())),
        }
    }

    /// Start a GET request.
    pub fn get(&self, path: impl Into<String>) -> TestRequestBuilder {
        self.request(Method::GET, path)
    }

    /// Start a POST request.
    pub fn post(&self, path: impl Into<String>) -> TestRequestBuilder {
        self.request(Method::POST, path)
    }

    /// Start a PUT request.
    pub fn put(&self, path: impl Into<String>) -> TestRequestBuilder {
        self.request(Method::PUT, path)
    }

    /// Start a DELETE request.
    pub fn delete(&self, path: impl Into<String>) -> TestRequestBuilder {
        self.request(Method::DELETE, path)
    }

    /// Start a PATCH request.
    pub fn patch(&self, path: impl Into<String>) -> TestRequestBuilder {
        self.request(Method::PATCH, path)
    }

    /// Start a request with an arbitrary method.
    pub fn request(&self, method: Method, path: impl Into<String>) -> TestRequestBuilder {
        TestRequestBuilder {
            client: self.clone(),
            method,
            uri: path.into(),
            headers: HeaderMap::new(),
            body: Body::empty(),
            extra_cookies: Vec::new(),
        }
    }

    /// Open a WebSocket connection to this app through a loopback listener.
    #[cfg(feature = "ws")]
    pub async fn ws(&self, path: impl AsRef<str>) -> Result<TestWebSocket, TestWebSocketError> {
        TestWebSocket::connect(self.service.clone(), path.as_ref()).await
    }

    fn cookie_header(&self) -> Option<HeaderValue> {
        let jar = self.cookies.lock().expect("test cookie jar poisoned");
        let header = jar
            .iter()
            .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
            .collect::<Vec<_>>()
            .join("; ");

        if header.is_empty() {
            None
        } else {
            HeaderValue::from_str(&header).ok()
        }
    }

    fn store_response_cookies(&self, headers: &HeaderMap) {
        let mut jar = self.cookies.lock().expect("test cookie jar poisoned");
        for value in headers.get_all(SET_COOKIE) {
            let Some(value) = value.to_str().ok() else {
                continue;
            };
            let Ok(cookie) = Cookie::parse(value.to_owned()) else {
                continue;
            };

            // Honor deletion so logout/expiry assertions behave like real
            // browsers: `Max-Age<=0` or a past `Expires` removes the cookie.
            if set_cookie_is_expired(value) {
                jar.remove(cookie.into_owned());
            } else {
                jar.add(cookie.into_owned());
            }
        }
    }
}

/// RFC 6265 §5.3: `Max-Age` takes precedence over `Expires`; both zero/past
/// values delete the cookie.
fn set_cookie_is_expired(raw: &str) -> bool {
    let attributes = raw.split(';').skip(1).map(str::trim);

    for attribute in attributes.clone() {
        if let Some(v) = attribute
            .strip_prefix("Max-Age=")
            .or_else(|| attribute.strip_prefix("max-age="))
        {
            return v
                .trim()
                .parse::<i64>()
                .map(|secs| secs <= 0)
                .unwrap_or(false);
        }
    }

    for attribute in attributes {
        if let Some(v) = attribute
            .strip_prefix("Expires=")
            .or_else(|| attribute.strip_prefix("expires="))
        {
            return httpdate::parse_http_date(v.trim())
                .map(|time| time <= std::time::SystemTime::now())
                .unwrap_or(false);
        }
    }

    false
}

/// Builder for test requests.
#[derive(Debug)]
pub struct TestRequestBuilder {
    client: TestClient,
    method: Method,
    uri: String,
    headers: HeaderMap,
    body: Body,
    extra_cookies: Vec<Cookie<'static>>,
}

impl TestRequestBuilder {
    /// Add a request header.
    #[must_use]
    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        K::Error: std::fmt::Debug,
        V: TryInto<HeaderValue>,
        V::Error: std::fmt::Debug,
    {
        self.headers.insert(
            key.try_into().expect("valid header name"),
            value.try_into().expect("valid header value"),
        );
        self
    }

    /// Append serialized query parameters to the request URI.
    #[must_use]
    pub fn query<T: Serialize + ?Sized>(mut self, query: &T) -> Self {
        let query = serde_urlencoded::to_string(query).expect("valid query serialization");
        if query.is_empty() {
            return self;
        }

        let separator = if self.uri.contains('?') { '&' } else { '?' };
        self.uri.push(separator);
        self.uri.push_str(&query);
        self
    }

    /// Set a JSON request body.
    #[must_use]
    pub fn json<T: Serialize + ?Sized>(mut self, value: &T) -> Self {
        let bytes = serde_json::to_vec(value).expect("valid JSON serialization");
        self.headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        self.body = Body::from(Bytes::from(bytes));
        self
    }

    /// Set an `application/x-www-form-urlencoded` request body.
    #[must_use]
    pub fn form<T: Serialize + ?Sized>(mut self, value: &T) -> Self {
        let body = serde_urlencoded::to_string(value).expect("valid form serialization");
        self.headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
        self.body = Body::from(body);
        self
    }

    /// Set a raw request body.
    #[must_use]
    pub fn body(mut self, body: impl Into<Body>) -> Self {
        self.body = body.into();
        self
    }

    /// Add a one-off cookie to this request.
    #[must_use]
    pub fn cookie(mut self, cookie: Cookie<'static>) -> Self {
        self.extra_cookies.push(cookie);
        self
    }

    /// Dispatch the request and return the response.
    pub async fn send(mut self) -> TestResponse {
        let jar_header = self
            .client
            .cookie_header()
            .and_then(|value| value.to_str().ok().map(str::to_owned));
        if let Some(jar_header) = jar_header {
            // Merge with any explicitly set Cookie header instead of
            // clobbering it.
            let merged = match self.headers.get(COOKIE).and_then(|v| v.to_str().ok()) {
                Some(existing) => format!("{existing}; {jar_header}"),
                None => jar_header,
            };
            if let Ok(value) = HeaderValue::from_str(&merged) {
                self.headers.insert(COOKIE, value);
            }
        }

        if !self.extra_cookies.is_empty() {
            let mut cookie_header = self
                .headers
                .get(COOKIE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
                .unwrap_or_default();

            for cookie in self.extra_cookies {
                if !cookie_header.is_empty() {
                    cookie_header.push_str("; ");
                }
                cookie_header.push_str(cookie.name());
                cookie_header.push('=');
                cookie_header.push_str(cookie.value());
            }

            self.headers.insert(
                COOKIE,
                HeaderValue::from_str(&cookie_header).expect("valid cookie header"),
            );
        }

        let uri: Uri = self.uri.parse().expect("valid request URI");
        let mut builder = http::Request::builder().method(self.method).uri(uri);

        for (name, value) in self.headers {
            if let Some(name) = name {
                builder = builder.header(name, value);
            }
        }

        let request = Request::new(builder.body(self.body).expect("valid request"));
        let response = oneshot(self.client.service.clone(), request).await;
        self.client.store_response_cookies(response.headers());
        TestResponse { inner: response }
    }
}

/// Response returned by [`TestClient`].
#[derive(Debug)]
pub struct TestResponse {
    inner: Response,
}

impl TestResponse {
    /// Return the HTTP status.
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// Return the response headers.
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Consume the response and return its raw bytes.
    pub async fn bytes(self) -> Result<Bytes, arvik_core::body::BoxError> {
        self.inner.into_body().to_bytes().await
    }

    /// Consume the response and return UTF-8 text.
    pub async fn text(self) -> Result<String, arvik_core::body::BoxError> {
        self.inner.into_body().to_string().await
    }

    /// Consume the response and deserialize JSON.
    pub async fn json<T: serde::de::DeserializeOwned>(self) -> serde_json::Result<T> {
        let bytes = self
            .inner
            .into_body()
            .to_bytes()
            .await
            .map_err(|err| serde_json::Error::io(std::io::Error::other(err.to_string())))?;
        serde_json::from_slice(&bytes)
    }

    /// Consume the wrapper and return the inner response.
    pub fn into_inner(self) -> Response {
        self.inner
    }
}

/// WebSocket test error.
#[cfg(feature = "ws")]
pub type TestWebSocketError = Box<dyn std::error::Error + Send + Sync>;

/// WebSocket handle returned by [`TestClient::ws`].
#[cfg(feature = "ws")]
pub struct TestWebSocket {
    inner: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: tokio::task::JoinHandle<()>,
}

#[cfg(feature = "ws")]
impl TestWebSocket {
    async fn connect(service: BoxCloneService, path: &str) -> Result<Self, TestWebSocketError> {
        let server = arvik_hyper::Server::bind("127.0.0.1:0").await?;
        let addr = server.local_addr();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server_task = tokio::spawn(async move {
            let signal = async {
                let _ = shutdown_rx.await;
            };
            let shutdown = arvik_hyper::ShutdownConfig::default()
                .drain_timeout(std::time::Duration::from_secs(1));
            if let Err(err) = server
                .serve_service_with_graceful_shutdown(service, signal, shutdown)
                .await
            {
                eprintln!("test websocket server failed: {err}");
            }
        });

        let path = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        };
        let url = format!("ws://{addr}{path}");
        let (inner, _) = tokio_tungstenite::connect_async(url).await?;

        Ok(Self {
            inner,
            shutdown: Some(shutdown_tx),
            server_task,
        })
    }

    /// Send a WebSocket message.
    pub async fn send(
        &mut self,
        message: impl Into<arvik_ws::Message>,
    ) -> Result<(), TestWebSocketError> {
        use futures_util::SinkExt;

        self.inner.send(message.into().into()).await?;
        Ok(())
    }

    /// Receive the next WebSocket message.
    pub async fn recv(&mut self) -> Option<Result<arvik_ws::Message, TestWebSocketError>> {
        use futures_util::StreamExt;

        match self.inner.next().await {
            Some(Ok(message)) => Some(Ok(arvik_ws::Message::from(message))),
            Some(Err(err)) => Some(Err(Box::new(err))),
            None => None,
        }
    }

    /// Send a text message.
    pub async fn send_text(&mut self, text: impl Into<String>) -> Result<(), TestWebSocketError> {
        self.send(arvik_ws::Message::Text(text.into())).await
    }

    /// Close the WebSocket and signal the test server to stop accepting.
    pub async fn close(mut self) -> Result<(), TestWebSocketError> {
        match self.inner.close(None).await {
            Ok(()) => {
                self.stop_server();
                Ok(())
            }
            Err(err) => {
                self.stop_server();
                Err(Box::new(err))
            }
        }
    }

    fn stop_server(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.server_task.abort();
    }
}

#[cfg(feature = "ws")]
impl Drop for TestWebSocket {
    fn drop(&mut self) {
        self.stop_server();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arvik_core::ResponseBuilder;
    use arvik_router::{PathParams, get, post};
    use serde::{Deserialize, Serialize};

    #[tokio::test]
    async fn get_request_returns_text() {
        let app = Router::new().route("/", get(|| async { "Hello" }));
        let client = TestClient::new(app);

        let response = client.get("/").send().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "Hello");
    }

    #[tokio::test]
    async fn path_params_are_available() {
        async fn user(req: Request) -> String {
            let id = req
                .extension::<PathParams>()
                .and_then(|params| params.get("id"))
                .unwrap();
            format!("user:{id}")
        }

        let app = Router::new().route("/users/{id}", get(user));
        let client = TestClient::new(app);

        let response = client.get("/users/42").send().await;

        assert_eq!(response.text().await.unwrap(), "user:42");
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Payload {
        name: String,
    }

    #[tokio::test]
    async fn json_body_and_response_work() {
        async fn echo(req: Request) -> Response {
            let payload: Payload =
                serde_json::from_slice(&req.into_body().to_bytes().await.unwrap()).unwrap();
            ResponseBuilder::new().json(&payload)
        }

        let app = Router::new().route("/echo", post(echo));
        let client = TestClient::new(app);

        let response = client
            .post("/echo")
            .json(&Payload {
                name: "Alice".into(),
            })
            .send()
            .await;
        let payload: Payload = response.json().await.unwrap();

        assert_eq!(
            payload,
            Payload {
                name: "Alice".into()
            }
        );
    }

    #[tokio::test]
    async fn query_headers_form_and_body_work() {
        async fn inspect(req: Request) -> String {
            let header = req
                .headers()
                .get("x-test")
                .and_then(|value| value.to_str().ok())
                .unwrap()
                .to_owned();
            let body = req.into_body().to_string().await.unwrap();
            format!("{header}:{body}")
        }

        let app = Router::new().route("/inspect", post(inspect));
        let client = TestClient::new(app);

        let response = client
            .post("/inspect")
            .query(&[("page", "1")])
            .header("x-test", "ok")
            .form(&[("name", "Alice")])
            .send()
            .await;

        assert_eq!(response.text().await.unwrap(), "ok:name=Alice");
    }

    #[tokio::test]
    async fn cookie_jar_persists_across_requests() {
        async fn set_cookie() -> Response {
            ResponseBuilder::new()
                .header(SET_COOKIE, "session=abc; Path=/")
                .text("set")
        }

        async fn read_cookie(req: Request) -> String {
            req.headers()
                .get(COOKIE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_owned()
        }

        let app = Router::new()
            .route("/set", get(set_cookie))
            .route("/read", get(read_cookie));
        let client = TestClient::new(app);

        let _ = client.get("/set").send().await;
        let response = client.get("/read").send().await;

        assert_eq!(response.text().await.unwrap(), "session=abc");
    }

    #[tokio::test]
    async fn missing_route_is_not_found() {
        let app = Router::new().route("/", get(|| async { "Hello" }));
        let client = TestClient::new(app);

        let response = client.delete("/missing").send().await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "ws")]
    #[tokio::test]
    async fn websocket_echo_works_over_loopback() {
        async fn ws_handler(ws: arvik_ws::WebSocketUpgrade) -> Response {
            ws.on_upgrade(|mut socket| async move {
                if let Some(Ok(message)) = socket.recv().await {
                    let _ = socket.send(message).await;
                }
            })
        }

        let app = Router::new().route("/ws", get(ws_handler));
        let client = TestClient::new(app);
        let mut ws = match client.ws("/ws").await {
            Ok(ws) => ws,
            Err(err) if is_permission_denied(err.as_ref()) => {
                eprintln!("skipping websocket test: local sockets are not permitted");
                return;
            }
            Err(err) => panic!("failed to connect test websocket: {err}"),
        };

        ws.send_text("hello").await.unwrap();
        let message = ws.recv().await.unwrap().unwrap();

        assert_eq!(message.as_text(), Some("hello"));
    }

    #[cfg(feature = "ws")]
    fn is_permission_denied(err: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
        if err
            .downcast_ref::<std::io::Error>()
            .is_some_and(|err| err.kind() == std::io::ErrorKind::PermissionDenied)
        {
            return true;
        }

        let mut source = err.source();
        while let Some(err) = source {
            if err
                .downcast_ref::<std::io::Error>()
                .is_some_and(|err| err.kind() == std::io::ErrorKind::PermissionDenied)
            {
                return true;
            }
            source = err.source();
        }

        false
    }
}

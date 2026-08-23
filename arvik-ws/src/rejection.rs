//! Rejection types for WebSocket upgrade failures.

use arvik_core::into_response::IntoResponse;
use arvik_core::response::{Response, ResponseBuilder};
use http::StatusCode;

/// Rejection for [`WebSocketUpgrade`](crate::WebSocketUpgrade) extraction failures.
#[derive(Debug)]
pub enum WebSocketUpgradeRejection {
    /// The request method must be GET.
    MethodNotGet,
    /// The `Connection` header must include `upgrade`.
    ConnectionNotUpgrade,
    /// The `Upgrade` header must be `websocket`.
    UpgradeNotWebSocket,
    /// The `Sec-WebSocket-Key` header is missing.
    MissingSecWebSocketKey,
    /// The `Sec-WebSocket-Version` header must be `13`.
    InvalidWebSocketVersionHeader,
    /// The server connection does not support upgrades.
    /// Ensure you are using a hyper-based server.
    ConnectionNotUpgradable,
}

impl std::fmt::Display for WebSocketUpgradeRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MethodNotGet => write!(f, "WebSocket upgrade requires GET method"),
            Self::ConnectionNotUpgrade => {
                write!(f, "Connection header must contain 'upgrade'")
            }
            Self::UpgradeNotWebSocket => {
                write!(f, "Upgrade header must be 'websocket'")
            }
            Self::MissingSecWebSocketKey => {
                write!(f, "Sec-WebSocket-Key header is missing")
            }
            Self::InvalidWebSocketVersionHeader => {
                write!(f, "Sec-WebSocket-Version must be '13'")
            }
            Self::ConnectionNotUpgradable => {
                write!(
                    f,
                    "Connection is not upgradable — ensure you're using serve_app()"
                )
            }
        }
    }
}

impl std::error::Error for WebSocketUpgradeRejection {}

impl IntoResponse for WebSocketUpgradeRejection {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::MethodNotGet => StatusCode::METHOD_NOT_ALLOWED,
            // RFC 6455 §4.4/§4.2.2: an unsupported version gets 426 Upgrade
            // Required plus the version(s) this server speaks, so capable
            // clients can retry instead of failing unrecoverably.
            Self::InvalidWebSocketVersionHeader => StatusCode::UPGRADE_REQUIRED,
            Self::ConnectionNotUpgradable => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };

        let body = format!(
            r#"{{"error":"WebSocket upgrade failed","reason":"{}","code":{}}}"#,
            self,
            status.as_u16()
        );

        let mut builder = ResponseBuilder::new()
            .status(status)
            .header(http::header::CONTENT_TYPE, "application/json");

        if matches!(self, Self::InvalidWebSocketVersionHeader) {
            builder = builder.header("sec-websocket-version", "13");
        }

        builder.body(arvik_core::Body::from(body))
    }
}

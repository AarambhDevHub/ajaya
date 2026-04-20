//! Cookie extractors and response parts.
//!
//! Provides three cookie jar types:
//!
//! - [`CookieJar`] — plain cookies (no signing or encryption)
//! - [`SignedCookieJar`] — HMAC-signed cookies (tamper-proof)
//! - [`PrivateCookieJar`] — AES-GCM-encrypted cookies (tamper-proof + private)
//!
//! All three implement both [`FromRequestParts`] (to read cookies) and
//! [`IntoResponseParts`] (to write `Set-Cookie` headers).
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use ajaya::{Router, get, State, CookieJar};
//! use cookie::Cookie;
//!
//! async fn set_cookie(jar: CookieJar) -> (CookieJar, &'static str) {
//!     let jar = jar.add(Cookie::new("session", "abc123"));
//!     (jar, "Cookie set!")
//! }
//!
//! async fn get_cookie(jar: CookieJar) -> String {
//!     jar.get("session")
//!         .map(|c| format!("session={}", c.value()))
//!         .unwrap_or_else(|| "no session".into())
//! }
//! ```
//!
//! # Signed Cookies
//!
//! For signed or private cookies, the [`cookie::Key`] must be stored in
//! your application state and exposed via [`crate::state::FromRef`]:
//!
//! ```rust,ignore
//! use ajaya::{State, SignedCookieJar};
//! use ajaya_extract::state::FromRef;
//! use cookie::{Cookie, Key};
//!
//! #[derive(Clone)]
//! struct AppState {
//!     cookie_key: Key,
//! }
//!
//! impl FromRef<AppState> for Key {
//!     fn from_ref(state: &AppState) -> Self { state.cookie_key.clone() }
//! }
//!
//! async fn handler(jar: SignedCookieJar) -> (SignedCookieJar, &'static str) {
//!     let jar = jar.add(Cookie::new("user_id", "42"));
//!     (jar, "Signed cookie set!")
//! }
//! ```

use std::convert::Infallible;

use ajaya_core::extract::FromRequestParts;
use ajaya_core::into_response_parts::{IntoResponseParts, ResponseParts};
use ajaya_core::request_parts::RequestParts;
use cookie::{Cookie, CookieJar as InnerJar, Key};
use http::HeaderValue;
use http::header::{COOKIE, SET_COOKIE};

use crate::state::FromRef;

// ---------------------------------------------------------------------------
// Shared cookie parsing helper
// ---------------------------------------------------------------------------

/// Parse the `Cookie` header of a request into an inner `cookie::CookieJar`.
fn parse_cookie_header(parts: &RequestParts) -> InnerJar {
    let mut jar = InnerJar::new();

    for header in parts.headers().get_all(COOKIE) {
        let Ok(value_str) = header.to_str() else {
            continue;
        };
        // A single Cookie header can contain multiple key=value pairs
        for pair in value_str.split(';') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            // Parse as owned string so we get Cookie<'static>
            if let Ok(cookie) = Cookie::parse_encoded(pair.to_owned()) {
                jar.add_original(cookie);
            }
        }
    }

    jar
}

/// Drain the delta of a `cookie::CookieJar` and append `Set-Cookie` headers.
fn apply_jar_delta(jar: InnerJar, mut parts: ResponseParts) -> ResponseParts {
    for cookie in jar.delta() {
        // `cookie.to_string()` encodes the cookie in the correct
        // `Set-Cookie` wire format including attributes.
        if let Ok(header_value) = HeaderValue::from_str(&cookie.to_string()) {
            parts.headers_mut().append(SET_COOKIE, header_value);
        }
    }
    parts
}

// ---------------------------------------------------------------------------
// CookieJar
// ---------------------------------------------------------------------------

/// A plain (unsigned, unencrypted) cookie jar.
///
/// Reads cookies from the incoming `Cookie` header and writes
/// `Set-Cookie` headers in the response via [`IntoResponseParts`].
///
/// Only cookies added or removed since the jar was extracted appear in
/// the response (delta tracking is automatic).
#[derive(Debug, Clone)]
pub struct CookieJar {
    inner: InnerJar,
}

impl CookieJar {
    /// Create an empty jar.
    pub fn new() -> Self {
        Self {
            inner: InnerJar::new(),
        }
    }

    /// Get a cookie by name.
    pub fn get(&self, name: &str) -> Option<&Cookie<'static>> {
        self.inner.get(name)
    }

    /// Add or replace a cookie. It will appear as a `Set-Cookie` header.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, cookie: Cookie<'static>) -> Self {
        self.inner.add(cookie);
        self
    }

    /// Remove a cookie (generates an expired `Set-Cookie` header).
    #[must_use]
    pub fn remove(mut self, cookie: Cookie<'static>) -> Self {
        self.inner.remove(cookie);
        self
    }

    /// Iterate over all cookies currently in the jar (including original ones).
    pub fn iter(&self) -> impl Iterator<Item = &Cookie<'static>> {
        self.inner.iter()
    }
}

impl Default for CookieJar {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Send + Sync> FromRequestParts<S> for CookieJar {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(CookieJar {
            inner: parse_cookie_header(parts),
        })
    }
}

impl IntoResponseParts for CookieJar {
    type Error = Infallible;

    fn into_response_parts(self, parts: ResponseParts) -> Result<ResponseParts, Self::Error> {
        Ok(apply_jar_delta(self.inner, parts))
    }
}

// ---------------------------------------------------------------------------
// SignedCookieJar
// ---------------------------------------------------------------------------

/// A cookie jar that signs cookies with HMAC-SHA256.
///
/// Cookie values are signed on `add` and verified on `get`. An invalid
/// or tampered signature causes `get` to return `None`.
///
/// Requires [`cookie::Key`] in the application state:
///
/// ```rust,ignore
/// impl FromRef<AppState> for cookie::Key {
///     fn from_ref(state: &AppState) -> Self { state.cookie_key.clone() }
/// }
/// ```
#[derive(Clone)]
pub struct SignedCookieJar {
    inner: InnerJar,
    key: Key,
}

impl std::fmt::Debug for SignedCookieJar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedCookieJar")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl SignedCookieJar {
    /// Get a cookie by name, verifying its HMAC signature.
    ///
    /// Returns `None` if the cookie is absent or has an invalid signature.
    pub fn get(&self, name: &str) -> Option<Cookie<'static>> {
        self.inner.signed(&self.key).get(name)
    }

    /// Add a cookie, signing it with the jar's key.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, cookie: Cookie<'static>) -> Self {
        self.inner.signed_mut(&self.key).add(cookie);
        self
    }

    /// Remove a cookie by name (generates an expired `Set-Cookie`).
    #[must_use]
    pub fn remove(mut self, cookie: Cookie<'static>) -> Self {
        self.inner.remove(cookie);
        self
    }
}

impl<S> FromRequestParts<S> for SignedCookieJar
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let key = Key::from_ref(state);
        let inner = parse_cookie_header(parts);
        Ok(SignedCookieJar { inner, key })
    }
}

impl IntoResponseParts for SignedCookieJar {
    type Error = Infallible;

    fn into_response_parts(self, parts: ResponseParts) -> Result<ResponseParts, Self::Error> {
        Ok(apply_jar_delta(self.inner, parts))
    }
}

// ---------------------------------------------------------------------------
// PrivateCookieJar
// ---------------------------------------------------------------------------

/// A cookie jar that encrypts cookies with AES-256-GCM.
///
/// Cookie values are encrypted on `add` and decrypted on `get`. An absent
/// or tampered cookie causes `get` to return `None`.
///
/// Same key requirements as [`SignedCookieJar`].
#[derive(Clone)]
pub struct PrivateCookieJar {
    inner: InnerJar,
    key: Key,
}

impl std::fmt::Debug for PrivateCookieJar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateCookieJar")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl PrivateCookieJar {
    /// Get and decrypt a cookie by name.
    ///
    /// Returns `None` if absent or decryption fails.
    pub fn get(&self, name: &str) -> Option<Cookie<'static>> {
        self.inner.private(&self.key).get(name)
    }

    /// Encrypt and add a cookie.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, cookie: Cookie<'static>) -> Self {
        self.inner.private_mut(&self.key).add(cookie);
        self
    }

    /// Remove a cookie by name.
    #[must_use]
    pub fn remove(mut self, cookie: Cookie<'static>) -> Self {
        self.inner.remove(cookie);
        self
    }
}

impl<S> FromRequestParts<S> for PrivateCookieJar
where
    S: Send + Sync,
    Key: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut RequestParts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let key = Key::from_ref(state);
        let inner = parse_cookie_header(parts);
        Ok(PrivateCookieJar { inner, key })
    }
}

impl IntoResponseParts for PrivateCookieJar {
    type Error = Infallible;

    fn into_response_parts(self, parts: ResponseParts) -> Result<ResponseParts, Self::Error> {
        Ok(apply_jar_delta(self.inner, parts))
    }
}

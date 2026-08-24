//! Tower service adapter for the router.
//!
//! Provides [`ServiceHandler`] which wraps any Tower `Service`
//! into an Arvik `Handler`, enabling services to be mounted
//! inside the router via `Router::route_service` and
//! `Router::nest_service`.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

use arvik_core::OriginalUri;
use arvik_core::request::Request;
use arvik_core::response::Response;
use http::Uri;
use tower_service::Service;

/// Wraps a Tower [`Service`] to implement Arvik's `Handler` trait.
///
/// This adapter allows any compatible Tower service to be used
/// as a route handler within the router.
pub struct ServiceHandler<T> {
    service: T,
}

impl<T: Clone> Clone for ServiceHandler<T> {
    fn clone(&self) -> Self {
        Self {
            service: self.service.clone(),
        }
    }
}

impl<T> ServiceHandler<T> {
    /// Create a new `ServiceHandler` wrapping the given service.
    pub fn new(service: T) -> Self {
        Self { service }
    }
}

impl<T, S> arvik_core::handler::Handler<((),), S> for ServiceHandler<T>
where
    T: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    T::Future: Send + 'static,
    S: Send + 'static,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;

    fn call(self, req: Request, _state: S) -> Self::Future {
        let mut service = self.service;
        Box::pin(async move {
            // Service is always ready for our use case
            match service.call(req).await {
                Ok(response) => response,
                Err(infallible) => match infallible {},
            }
        })
    }
}

/// Service wrapper used by `Router::nest_service`.
///
/// It preserves the original URI in request extensions, then strips the mount
/// prefix from the URI seen by the nested service.
pub struct StripPrefixService<T> {
    prefix: String,
    service: T,
}

impl<T: Clone> Clone for StripPrefixService<T> {
    fn clone(&self) -> Self {
        Self {
            prefix: self.prefix.clone(),
            service: self.service.clone(),
        }
    }
}

impl<T> StripPrefixService<T> {
    /// Create a prefix-stripping service wrapper.
    pub fn new(prefix: impl Into<String>, service: T) -> Self {
        Self {
            prefix: prefix.into(),
            service,
        }
    }
}

impl<T> Service<Request> for StripPrefixService<T>
where
    T: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    T::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = T::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        // Single clone: serves both the OriginalUri extension and the strip.
        let original_uri = req.uri().clone();
        if req.extensions().get::<OriginalUri>().is_none() {
            req.extensions_mut()
                .insert(OriginalUri(original_uri.clone()));
        }

        if let Some(stripped) = strip_uri_prefix(&original_uri, &self.prefix) {
            *req.uri_mut() = stripped;
        }

        self.service.call(req)
    }
}

fn strip_uri_prefix(uri: &Uri, prefix: &str) -> Option<Uri> {
    let path = uri.path();
    let stripped_path = if prefix == "/" || !path.starts_with(prefix) {
        // Nothing to strip — callers keep the original Uri without a
        // to_string + re-parse round-trip (audit O6).
        return None;
    } else if path == prefix
        || (path.len() == prefix.len() + 1 && path.starts_with(prefix) && path.ends_with('/'))
    {
        "/"
    } else {
        path.strip_prefix(prefix)
            .filter(|rest| rest.starts_with('/'))?
    };

    let mut path_and_query = stripped_path.to_string();
    if let Some(query) = uri.query() {
        path_and_query.push('?');
        path_and_query.push_str(query);
    }

    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(path_and_query.parse().ok()?);
    Uri::from_parts(parts).ok()
}

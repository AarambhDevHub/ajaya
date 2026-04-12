//! Tower service adapter for the router.
//!
//! Provides [`ServiceHandler`] which wraps any Tower `Service`
//! into an Ajaya [`Handler`], enabling services to be mounted
//! inside the router via [`Router::route_service`] and
//! [`Router::nest_service`].

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

use ajaya_core::request::Request;
use ajaya_core::response::Response;
use tower_service::Service;

/// Wraps a Tower [`Service`] to implement Ajaya's [`Handler`] trait.
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

impl<T, S> ajaya_core::handler::Handler<((),), S> for ServiceHandler<T>
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

//! Shared application state extractor.
//!
//! Extracts a clone of the application state (or a sub-part of it)
//! from the router configuration.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ajaya::{Router, State, get};
//!
//! #[derive(Clone)]
//! struct AppState {
//!     db_url: String,
//! }
//!
//! async fn handler(State(state): State<AppState>) -> String {
//!     format!("DB: {}", state.db_url)
//! }
//!
//! let state = AppState { db_url: "postgres://...".into() };
//! let app = Router::new()
//!     .route("/", get(handler))
//!     .with_state(state);
//! ```
//!
//! # Sub-State via `FromRef`
//!
//! ```rust,ignore
//! use ajaya::{State, FromRef};
//!
//! #[derive(Clone)]
//! struct AppState {
//!     db_url: String,
//!     api_key: String,
//! }
//!
//! impl FromRef<AppState> for String {
//!     fn from_ref(state: &AppState) -> Self {
//!         state.db_url.clone()
//!     }
//! }
//!
//! // Now you can extract just the db_url:
//! async fn handler(State(db): State<String>) -> String { db }
//! ```

use ajaya_core::extract::FromRequestParts;
use ajaya_core::request_parts::RequestParts;

/// Shared application state extractor.
///
/// Extracts a value of type `T` from the application state `S` using
/// the [`FromRef`] trait. When `T == S`, the entire state is cloned.
#[derive(Debug, Clone)]
pub struct State<T>(pub T);

/// Trait for extracting a sub-type from the application state.
///
/// This enables handlers to extract specific parts of the application
/// state without receiving the entire state struct.
///
/// # Identity Implementation
///
/// A blanket implementation is provided for `T: Clone`, which simply
/// clones the input. This means `State<AppState>` always works when
/// the router is configured with `AppState`.
pub trait FromRef<T> {
    /// Extract a value from a reference to the source type.
    fn from_ref(input: &T) -> Self;
}

/// Identity impl: extracting `T` from `T` just clones it.
impl<T: Clone> FromRef<T> for T {
    fn from_ref(input: &T) -> Self {
        input.clone()
    }
}

impl<OuterState, T> FromRequestParts<OuterState> for State<T>
where
    T: FromRef<OuterState> + Send + Sync,
    OuterState: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        _parts: &mut RequestParts,
        state: &OuterState,
    ) -> Result<Self, Self::Rejection> {
        Ok(State(T::from_ref(state)))
    }
}

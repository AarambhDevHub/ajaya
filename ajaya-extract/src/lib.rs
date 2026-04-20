//! # ajaya-extract
//!
//! Request extractors for the Ajaya web framework.
//!
//! This crate provides all built-in extractors that implement
//! [`FromRequestParts`] and [`FromRequest`] from `ajaya-core`:
//!
//! ## Parts-Only Extractors (`FromRequestParts`)
//!
//! | Extractor | Description |
//! |---|---|
//! | [`Path<T>`] | URL path parameters, deserialized via serde |
//! | [`Query<T>`] | Query string parameters |
//! | [`RawPathParams`] | Untyped path parameter pairs |
//! | [`TypedHeader<T>`] | Single typed header via `headers` crate |
//! | [`HeaderMap`](http::HeaderMap) | Raw access to all headers |
//! | [`Method`](http::Method) | HTTP method |
//! | [`Uri`](http::Uri) | Full request URI |
//! | [`Version`](http::Version) | HTTP version |
//! | [`OriginalUri`] | URI before path rewrites |
//! | [`MatchedPath`] | The route pattern that matched |
//! | [`ConnectInfo<T>`] | Client connection info |
//! | [`Extension<T>`] | Typed request extension |
//! | [`State<S>`] | Shared application state |
//!
//! ## Body Extractors (`FromRequest`)
//!
//! | Extractor | Description |
//! |---|---|
//! | [`Json<T>`] | JSON body (validates Content-Type) |
//! | [`Form<T>`] | URL-encoded form body |
//! | [`Bytes`](bytes::Bytes) | Raw body bytes |
//! | [`String`] | Raw body as UTF-8 string |
//! | [`Multipart`] | Multipart form data / file uploads |

// Modules
pub mod body;
pub mod bytes_extract;
pub mod connect_info;
pub mod cookies;
pub mod extension;
pub mod form;
pub mod header_map;
pub mod json;
pub mod matched_path;
pub mod method;
pub mod multipart;
pub mod original_uri;
pub mod path;
mod path_de;
pub mod query;
pub mod raw_path_params;
pub mod rejection;
pub mod state;
pub mod typed_header;
pub mod uri;

// Re-export extractor types
pub use self::body::BodyExtractor;
pub use self::connect_info::ConnectInfo;
pub use self::cookies::{CookieJar, PrivateCookieJar, SignedCookieJar}; // 0.3.3
pub use self::extension::Extension;
pub use self::form::Form;
pub use self::json::Json;
pub use self::matched_path::MatchedPath;
pub use self::method::MethodExtractor;
pub use self::multipart::{Field, Multipart, MultipartConstraints};
pub use self::original_uri::OriginalUri;
pub use self::path::Path;
pub use self::query::Query;
pub use self::raw_path_params::RawPathParams;
pub use self::state::{FromRef, State};
pub use self::typed_header::TypedHeader;

// Re-export rejection types
pub use self::rejection::*;

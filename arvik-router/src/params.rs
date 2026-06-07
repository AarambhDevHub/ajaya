//! Path parameters extracted during routing.
//!
//! When a route pattern contains `:param` or `*wildcard` segments,
//! the matched values are captured and stored as [`PathParams`]
//! in the request extensions.
//!
//! # Examples
//!
//! ```rust,ignore
//! use arvik_router::PathParams;
//!
//! async fn get_user(req: Request) -> String {
//!     let params = req.extension::<PathParams>().unwrap();
//!     let id = params.get("id").unwrap();
//!     format!("User: {id}")
//! }
//! ```

use std::sync::Arc;

use smallvec::SmallVec;

/// Captured path parameters from route matching.
///
/// Stores key-value pairs from `{param}` and `{*wildcard}` segments.
#[derive(Debug, Clone, Default)]
pub struct PathParams {
    pairs: SmallVec<[(Arc<str>, String); 8]>,
}

impl PathParams {
    /// Create empty path params.
    pub fn new() -> Self {
        Self {
            pairs: SmallVec::new(),
        }
    }

    /// Get a parameter value by name.
    ///
    /// Returns `None` if the parameter is not present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k.as_ref() == key)
            .map(|(_, v)| v.as_str())
    }

    /// Iterate over all parameter key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pairs.iter().map(|(k, v)| (k.as_ref(), v.as_str()))
    }

    /// Return a parameter by insertion index.
    ///
    /// This preserves router capture order for tuple-style extraction while
    /// avoiding repeated iterator scans in deserializers.
    pub fn get_index(&self, index: usize) -> Option<(&str, &str)> {
        self.pairs
            .get(index)
            .map(|(key, value)| (key.as_ref(), value.as_str()))
    }

    /// Returns the number of parameters.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Returns true if there are no parameters.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Insert a key-value pair.
    pub(crate) fn push(&mut self, key: Arc<str>, value: String) {
        self.pairs.push((key, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_params_get() {
        let mut params = PathParams::new();
        params.push(Arc::from("id"), "42".to_string());
        params.push(Arc::from("name"), "alice".to_string());

        assert_eq!(params.get("id"), Some("42"));
        assert_eq!(params.get("name"), Some("alice"));
        assert_eq!(params.get("missing"), None);
        assert_eq!(params.len(), 2);
        assert!(!params.is_empty());
    }

    #[test]
    fn test_path_params_iter() {
        let mut params = PathParams::new();
        params.push(Arc::from("a"), "1".to_string());
        params.push(Arc::from("b"), "2".to_string());

        let collected: Vec<_> = params.iter().collect();
        assert_eq!(collected, vec![("a", "1"), ("b", "2")]);
    }

    #[test]
    fn test_path_params_get_index() {
        let mut params = PathParams::new();
        params.push(Arc::from("a"), "1".to_string());
        params.push(Arc::from("b"), "2".to_string());

        assert_eq!(params.get_index(0), Some(("a", "1")));
        assert_eq!(params.get_index(1), Some(("b", "2")));
        assert_eq!(params.get_index(2), None);
    }
}

//! HTTP method filter for routing.
//!
//! [`MethodFilter`] is a bitflag enum representing one or more
//! HTTP methods. Used by [`MethodRouter`](crate::handler) to
//! dispatch handlers based on the request method.

/// Bitflag filter matching one or more HTTP methods.
///
/// # Examples
///
/// ```rust
/// use ajaya_core::method_filter::MethodFilter;
///
/// let filter = MethodFilter::GET | MethodFilter::HEAD;
/// assert!(filter.contains(MethodFilter::GET));
/// assert!(!filter.contains(MethodFilter::POST));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MethodFilter(u16);

impl MethodFilter {
    /// Matches HTTP GET requests.
    pub const GET: Self = Self(1 << 0);
    /// Matches HTTP POST requests.
    pub const POST: Self = Self(1 << 1);
    /// Matches HTTP PUT requests.
    pub const PUT: Self = Self(1 << 2);
    /// Matches HTTP DELETE requests.
    pub const DELETE: Self = Self(1 << 3);
    /// Matches HTTP PATCH requests.
    pub const PATCH: Self = Self(1 << 4);
    /// Matches HTTP HEAD requests.
    pub const HEAD: Self = Self(1 << 5);
    /// Matches HTTP OPTIONS requests.
    pub const OPTIONS: Self = Self(1 << 6);
    /// Matches HTTP TRACE requests.
    pub const TRACE: Self = Self(1 << 7);
    /// Matches any HTTP method.
    pub const ANY: Self = Self(0xFF);
    /// Matches no HTTP method (empty filter).
    pub const NONE: Self = Self(0);

    /// Check if this filter contains the given filter.
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check if this filter matches the given HTTP method.
    pub fn matches(self, method: &http::Method) -> bool {
        self.contains(Self::from_method(method))
    }

    /// Convert an HTTP method to a `MethodFilter`.
    pub fn from_method(method: &http::Method) -> Self {
        match *method {
            http::Method::GET => Self::GET,
            http::Method::POST => Self::POST,
            http::Method::PUT => Self::PUT,
            http::Method::DELETE => Self::DELETE,
            http::Method::PATCH => Self::PATCH,
            http::Method::HEAD => Self::HEAD,
            http::Method::OPTIONS => Self::OPTIONS,
            http::Method::TRACE => Self::TRACE,
            _ => Self(0), // Unknown methods match nothing
        }
    }
}

impl std::ops::BitOr for MethodFilter {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for MethodFilter {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl From<http::Method> for MethodFilter {
    fn from(method: http::Method) -> Self {
        Self::from_method(&method)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_filter_contains() {
        let filter = MethodFilter::GET | MethodFilter::POST;
        assert!(filter.contains(MethodFilter::GET));
        assert!(filter.contains(MethodFilter::POST));
        assert!(!filter.contains(MethodFilter::DELETE));
    }

    #[test]
    fn test_method_filter_matches() {
        assert!(MethodFilter::GET.matches(&http::Method::GET));
        assert!(!MethodFilter::GET.matches(&http::Method::POST));
        assert!(MethodFilter::ANY.matches(&http::Method::DELETE));
    }

    #[test]
    fn test_method_filter_from_method() {
        assert_eq!(
            MethodFilter::from_method(&http::Method::GET),
            MethodFilter::GET
        );
        assert_eq!(
            MethodFilter::from_method(&http::Method::POST),
            MethodFilter::POST
        );
    }
}

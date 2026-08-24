//! Shared `Accept-Encoding` negotiation used by the compression middleware
//! and static precompressed asset selection.
//!
//! Parses full RFC 9110 §12.5.3 semantics the naive `contains("br")` check
//! missed: per-entry `q`-values, explicit `;q=0` refusals overriding the
//! `*` wildcard, and multi-line header values.

/// Parse one header value into `(coding, quality)` pairs, lowercased.
///
/// `q=0` entries are preserved so callers can distinguish an explicit
/// refusal from mere absence.
pub fn parse_accept_encodings(value: &str) -> Vec<(String, f32)> {
    value
        .split(',')
        .filter_map(|entry| {
            let mut parts = entry.trim().split(';');
            let coding = parts.next()?.trim().to_ascii_lowercase();
            if coding.is_empty() {
                return None;
            }
            let mut quality = 1.0_f32;
            for parameter in parts {
                let parameter = parameter.trim();
                if let Some(q) = parameter
                    .strip_prefix("q=")
                    .or_else(|| parameter.strip_prefix("Q="))
                {
                    quality = q.trim().parse().unwrap_or(1.0);
                }
            }
            Some((coding, quality.clamp(0.0, 1.0)))
        })
        .collect()
}

/// Pick the first entry of `supported` (in priority order) the client accepts.
///
/// An exact `;q=0` entry refuses that coding even when `*` is present; codings
/// not mentioned at all fall back to the `*` wildcard's weight.
///
/// Allocation-free: compares case-insensitively over the raw header slice.
pub fn negotiate<'a>(supported: &[&'a str], accept_encoding: &str) -> Option<&'a str> {
    for candidate in supported {
        let mut exact_seen = false;
        let mut exact_allowed = false;
        let mut wildcard_allowed = false;

        for entry in accept_encoding.split(',') {
            let mut segments = entry.trim().split(';');
            let coding = segments.next().map(str::trim).unwrap_or("");
            if coding.eq_ignore_ascii_case(candidate) {
                exact_seen = true;
                exact_allowed = parse_q(segments) > 0.0;
            } else if coding == "*" {
                wildcard_allowed = parse_q(segments) > 0.0;
            }
        }

        let allowed = if exact_seen {
            exact_allowed
        } else {
            wildcard_allowed
        };
        if allowed {
            return Some(candidate);
        }
    }
    None
}

/// Parse the `;q=` parameter from the remaining attribute segments
/// (defaulting to 1.0).
fn parse_q<'a, I: Iterator<Item = &'a str>>(segments: I) -> f32 {
    let mut quality = 1.0_f32;
    for parameter in segments {
        let parameter = parameter.trim();
        if let Some(q) = parameter
            .strip_prefix("q=")
            .or_else(|| parameter.strip_prefix("Q="))
        {
            quality = q.trim().parse().unwrap_or(1.0);
        }
    }
    quality.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quality_values() {
        let parsed = parse_accept_encodings("gzip;q=0.5, BR, zstd ; Q=0 , *;q=0.1");
        assert_eq!(
            parsed,
            vec![
                ("gzip".to_string(), 0.5),
                ("br".to_string(), 1.0),
                ("zstd".to_string(), 0.0),
                ("*".to_string(), 0.1),
            ]
        );
    }

    #[test]
    fn explicit_refusal_beats_wildcard() {
        // The client refused brotli even though * is allowed.
        assert_eq!(negotiate(&["br", "gzip"], "gzip, br;q=0, *"), Some("gzip"));
        assert_eq!(negotiate(&["br"], "*, br;q=0"), None);
    }

    #[test]
    fn unlisted_codings_follow_the_wildcard() {
        assert_eq!(negotiate(&["zstd"], "*"), Some("zstd"));
        assert_eq!(negotiate(&["zstd"], "gzip"), None);
    }

    #[test]
    fn priority_order_is_preserved() {
        // Server preference wins among acceptable codings (tower-http-style);
        // only explicit refusal or absence redirects the choice.
        assert_eq!(negotiate(&["br", "gzip"], "gzip, br"), Some("br"));
        assert_eq!(
            negotiate(&["br", "gzip"], "br;q=0.2, gzip"),
            Some("br"),
            "any q > 0 keeps the server-preferred coding"
        );
    }
}

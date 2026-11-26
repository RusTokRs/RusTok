//! Expected-origin normalization and outbound target selection for the iframe bridge.
//!
//! # Why this is not in `browser_runtime`
//!
//! This is pure string logic with no `web_sys` involvement, but it used to live inside
//! `browser_runtime`, which is gated behind `cfg(all(target_arch = "wasm32", feature =
//! "wasm-client"))`. Code behind that gate is only ever *compiled* by CI, never executed, so the
//! rule that decides whether a cross-origin message is accepted had no test covering it. Keeping
//! it here means the decision is exercised by `cargo test` on the host like anything else.
//!
//! # Two directions, two rules
//!
//! Inbound and outbound are not symmetric, and treating them as one rule breaks the sandboxed
//! canvas:
//!
//! * **Inbound** — [`normalize_expected_origin`] — accepts the literal `"null"`, because that is
//!   exactly what a sandboxed `srcdoc` iframe reports on `MessageEvent::origin()`.
//! * **Outbound** — [`outbound_target_origin`] — cannot use `"null"` at all: the platform
//!   requires `postMessage`'s `targetOrigin` to be `*`, `/`, or an absolute URL, and an opaque
//!   origin has no absolute form. Such targets are addressed with `*`, and the receiving side
//!   performs the actual origin check.

use url::Url;

/// The origin a sandboxed `srcdoc` iframe reports: opaque, and not addressable as a URL.
pub const OPAQUE_ORIGIN: &str = "null";

/// Normalize a configured expected origin, or reject it.
///
/// Returns `None` for an origin that must never be used:
///
/// * empty or whitespace-only — nothing meaningful to compare against;
/// * `*` — the wildcard, which would accept messages from any origin and is the single most
///   common way an iframe bridge becomes a cross-origin hole.
///
/// [`OPAQUE_ORIGIN`] is deliberately accepted: a sandboxed `srcdoc` iframe genuinely reports that
/// origin, and refusing it would break the editor preview rather than secure it.
///
/// Surrounding whitespace is removed. The previous implementation tested `origin.trim()` but
/// returned the *untrimmed* string, so a configuration value with a stray space passed validation
/// and then never equalled `MessageEvent::origin()`, which is always canonical — the bridge
/// silently discarded every inbound message and looked simply broken.
pub fn normalize_expected_origin(origin: &str) -> Option<String> {
    let origin = origin.trim();
    if origin.is_empty() || origin == "*" {
        return None;
    }
    if origin == OPAQUE_ORIGIN {
        return Some(OPAQUE_ORIGIN.to_string());
    }

    // WHATWG URL parsing can repair missing slashes, strip embedded controls,
    // and collapse dot segments. Reject non-origin syntax before normalization.
    let (scheme, rest) = origin.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    if authority.is_empty()
        || authority.contains(['/', '\\', '?', '#', '@'])
        || authority.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return None;
    }

    let parsed = Url::parse(origin).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return None;
    }

    Some(parsed.origin().ascii_serialization())
}

/// Opaque origins are shared by unrelated sandboxed documents. Inbound
/// subscriptions must bind an exact source window before admitting one.
#[cfg(any(test, all(target_arch = "wasm32", feature = "wasm-client")))]
pub(crate) fn normalize_subscription_origin(origin: &str, has_source: bool) -> Option<String> {
    let normalized = normalize_expected_origin(origin)?;
    (normalized != OPAQUE_ORIGIN || has_source).then_some(normalized)
}

/// The `targetOrigin` to hand to `postMessage` for a configured destination origin.
///
/// An opaque destination ([`OPAQUE_ORIGIN`]) is addressed with `*`, because the platform rejects
/// any other value there. The HTML specification requires `targetOrigin` to be `*`, `/`, or a
/// valid absolute URL; the URL parser fails on `"null"`, so passing it either raises
/// `SyntaxError: Invalid target origin` or, when a valid-but-different origin is supplied
/// instead, the browser drops the message with a console error because it can never equal the
/// recipient's reported origin. Both outcomes mean "the message is not delivered", and the
/// sandboxed authoring canvas is precisely the opaque case.
///
/// `*` is not a widening of trust here: `postMessage`'s `targetOrigin` selects the recipient the
/// caller already holds a reference to; it is not an access-control decision. Sender validation
/// on the receiving side, and [`normalize_expected_origin`] plus the source identity check on the
/// subscribing side, are where trust is decided.
pub fn outbound_target_origin(origin: &str) -> &str {
    if origin.trim() == OPAQUE_ORIGIN {
        "*"
    } else {
        origin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_origins_that_would_defeat_the_check() {
        for origin in ["", "   ", "\t\n", "*", " * "] {
            assert_eq!(
                normalize_expected_origin(origin),
                None,
                "accepted {origin:?}"
            );
        }
    }

    #[test]
    fn accepts_the_sandboxed_iframe_origin() {
        // A sandboxed `srcdoc` iframe reports exactly this; rejecting it breaks the preview.
        assert_eq!(
            normalize_expected_origin(OPAQUE_ORIGIN).as_deref(),
            Some("null")
        );
    }

    #[test]
    fn opaque_inbound_subscription_requires_a_bound_source_window() {
        for origin in ["null", " null ", "\tnull\n"] {
            assert!(normalize_subscription_origin(origin, false).is_none());
            assert_eq!(normalize_subscription_origin(origin, true).as_deref(), Some("null"));
        }
        for has_source in [false, true] {
            assert_eq!(
                normalize_subscription_origin("HTTPS://ADMIN.example:443/", has_source).as_deref(),
                Some("https://admin.example")
            );
            assert!(normalize_subscription_origin("*", has_source).is_none());
        }
    }

    #[test]
    fn opaque_targets_are_addressed_with_the_wildcard() {
        // `postMessage` cannot take `"null"`: it is not `*`, `/`, or an absolute URL, so the
        // platform either throws `SyntaxError: Invalid target origin` or drops the message.
        assert_eq!(outbound_target_origin(OPAQUE_ORIGIN), "*");
        assert_eq!(outbound_target_origin(" null "), "*");
    }

    #[test]
    fn concrete_targets_are_passed_through_unchanged() {
        for origin in [
            "https://admin.example",
            "http://localhost:3000",
            "https://admin.example:8443",
        ] {
            assert_eq!(outbound_target_origin(origin), origin);
        }
    }

    #[test]
    fn surrounding_whitespace_is_removed_rather_than_merely_tolerated() {
        // The bug this pins: validation trimmed before testing but returned the untrimmed value,
        // so the stored origin could never equal a canonical `MessageEvent::origin()` and the
        // bridge silently dropped everything.
        for (input, expected) in [
            (" https://admin.example ", "https://admin.example"),
            ("\thttps://admin.example\n", "https://admin.example"),
            ("https://admin.example", "https://admin.example"),
        ] {
            assert_eq!(
                normalize_expected_origin(input).as_deref(),
                Some(expected),
                "input {input:?}"
            );
        }
    }

    #[test]
    fn normalization_is_idempotent() {
        let once = normalize_expected_origin(" https://admin.example ").expect("valid");
        let twice = normalize_expected_origin(&once).expect("valid");
        assert_eq!(once, twice);
    }


    #[test]
    fn concrete_origins_are_canonicalized_and_non_origins_are_rejected() {
        assert_eq!(
            normalize_expected_origin("https://ADMIN.example:443/").as_deref(),
            Some("https://admin.example")
        );
        assert_eq!(
            normalize_expected_origin("http://localhost:3000/").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            normalize_expected_origin("HTTPS://[::1]:443/").as_deref(),
            Some("https://[::1]")
        );
        for origin in [
            "https://admin.example/path",
            "https://admin.example?x=1",
            "https://admin.example#fragment",
            "https://user@admin.example",
            "https://@admin.example",
            "https:///admin.example",
            "https:admin.example",
            "https://admin.example/a/..",
            "https://admin.example/%2e",
            "https://admin.example\\",
            "https://ad\nmin.example",
            "https://admin.example?",
            "https://admin.example#",
            "ftp://admin.example",
        ] {
            assert_eq!(normalize_expected_origin(origin), None, "accepted {origin}");
        }
    }


}

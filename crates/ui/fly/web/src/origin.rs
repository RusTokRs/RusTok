//! Expected-origin normalization for the iframe bridge.
//!
//! # Why this is not in `browser_runtime`
//!
//! This is pure string logic with no `web_sys` involvement, but it used to live inside
//! `browser_runtime`, which is gated behind `cfg(all(target_arch = "wasm32", feature =
//! "wasm-client"))`. Code behind that gate is only ever *compiled* by CI, never executed, so the
//! rule that decides whether a cross-origin message is accepted had no test covering it. Keeping
//! it here means the decision is exercised by `cargo test` on the host like anything else.

/// Normalize a configured expected origin, or reject it.
///
/// Returns `None` for an origin that must never be used:
///
/// * empty or whitespace-only — nothing meaningful to compare against;
/// * `*` — the wildcard, which would accept messages from any origin and is the single most
///   common way an iframe bridge becomes a cross-origin hole.
///
/// `"null"` is deliberately accepted: a sandboxed `srcdoc` iframe genuinely reports that origin,
/// and refusing it would break the editor preview rather than secure it.
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
    Some(origin.to_string())
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
        assert_eq!(normalize_expected_origin("null").as_deref(), Some("null"));
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
}

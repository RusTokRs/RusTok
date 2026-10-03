//! The single source of truth for URL policy in Fly.
//!
//! Three near-identical copies of this logic previously lived in `render.rs`, `validation.rs` and
//! here, which meant a hardening fix applied to one of them silently left the others exploitable:
//! the renderer could emit a URL that validation had already rejected, and vice versa. Everything
//! that decides "may this URL be emitted or stored?" must now route through this module.
//!
//! Two policies are intentionally distinct, and the difference is a deliberate decision rather
//! than drift:
//!
//! * [`normalize_safe_url`] — the strict policy for *authored actions* (navigation targets, form
//!   submission URLs). It is allow-list only and rejects `data:` outright, because these values
//!   are stored and later acted upon.
//! * [`url_allowed`] — the attribute policy for *rendered markup* and validation, which may permit
//!   inline `data:image/...;base64,` resources under [`UrlPolicy::allow_data_images`].

pub(crate) fn validate_safe_url(value: &str, label: &str) -> Result<(), String> {
    normalize_safe_url(value, label)?;
    Ok(())
}

pub(crate) fn normalize_safe_url(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    if value.is_empty()
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
        || value.contains('\\')
        || value.starts_with("//")
        || lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("vbscript:")
    {
        return Err(format!("{label} `{value}` is unsafe"));
    }

    if value.starts_with('/') || value.starts_with('#') || value.starts_with('?') {
        return Ok(value.to_string());
    }
    if lower.starts_with("https://") {
        validate_authority(value, "https://".len(), label)?;
        return Ok(value.to_string());
    }
    if lower.starts_with("http://") {
        validate_authority(value, "http://".len(), label)?;
        return Ok(value.to_string());
    }
    if lower.starts_with("mailto:") {
        validate_non_empty_target(value, "mailto:".len(), label)?;
        return Ok(value.to_string());
    }
    if lower.starts_with("tel:") {
        validate_non_empty_target(value, "tel:".len(), label)?;
        return Ok(value.to_string());
    }

    Err(format!("{label} `{value}` uses an unsupported URL form"))
}

fn validate_authority(value: &str, prefix_len: usize, label: &str) -> Result<(), String> {
    let authority = value[prefix_len..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() || authority.starts_with(':') {
        Err(format!("{label} `{value}` has no valid authority"))
    } else {
        Ok(())
    }
}

fn validate_non_empty_target(value: &str, prefix_len: usize, label: &str) -> Result<(), String> {
    if value[prefix_len..].is_empty() {
        Err(format!("{label} `{value}` has no target"))
    } else {
        Ok(())
    }
}


/// Which attribute slot a URL is destined for. Each slot allows a different set of schemes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UrlAttributeKind {
    Navigation,
    Resource,
    FormAction,
    /// Must point inside the current document (`#name`).
    ///
    /// `usemap` is the only such attribute today. It is not a navigable URL, but it was being
    /// emitted unchecked while the static publish layer classified it and refused anything that
    /// is not a fragment.
    Fragment,
}

impl UrlAttributeKind {
    pub(crate) fn for_attribute(name: &str) -> Option<Self> {
        match name {
            // Classification mirrors `rustok-page-builder`'s static publish policy. Where the
            // two layers disagreed, an attribute was validated at publish and emitted unchecked
            // by the renderer — the same incoherence as judging `src` and `url()` differently.
            "href" | "cite" => Some(Self::Navigation),
            "src" | "poster" => Some(Self::Resource),
            "action" | "formaction" => Some(Self::FormAction),
            "usemap" => Some(Self::Fragment),
            _ => None,
        }
    }
}

/// Which URL forms are permitted. `RenderPolicy` projects onto this; validation uses
/// [`UrlPolicy::permissive`], which accepts anything the renderer could ever be configured to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UrlPolicy {
    pub(crate) allow_http: bool,
    pub(crate) allow_https: bool,
    pub(crate) allow_relative_urls: bool,
    pub(crate) allow_hash_urls: bool,
    pub(crate) allow_mailto: bool,
    pub(crate) allow_tel: bool,
    pub(crate) allow_data_images: bool,
}

impl UrlPolicy {
    /// Accepts every form the renderer can be configured to emit.
    ///
    /// Validation deliberately uses this: it must not reject a document that some other, more
    /// permissive render policy would legitimately render.
    pub(crate) const fn permissive() -> Self {
        Self {
            allow_http: true,
            allow_https: true,
            allow_relative_urls: true,
            allow_hash_urls: true,
            allow_mailto: true,
            allow_tel: true,
            allow_data_images: true,
        }
    }
}

/// Reject URLs that are unsafe regardless of scheme, and trim the rest.
///
/// Protocol-relative (`//host`) is refused because it inherits the page scheme; backslashes
/// because browsers normalise them to `/` and they are a classic authority-confusion vector;
/// control characters and whitespace because they let a payload smuggle a newline past a filter.
pub(crate) fn normalized_url_candidate(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAXIMUM_URL_LENGTH
        || value.starts_with("//")
        || value.contains('\\')
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some(value)
}

/// Upper bound on a URL we are willing to emit or store.
pub(crate) const MAXIMUM_URL_LENGTH: usize = 2048;

pub(crate) fn relative_url_allowed(value: &str) -> bool {
    if value.starts_with('#') {
        return false;
    }
    let scheme_boundary = value.find(['/', '?', '#']).unwrap_or(value.len());
    !value[..scheme_boundary].contains(':')
}

pub(crate) fn absolute_url_has_authority(value: &str, scheme: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if !lower.starts_with(scheme) {
        return false;
    }
    let authority = value[scheme.len()..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    !authority.is_empty() && !authority.starts_with(':')
}

pub(crate) fn scheme_target_is_not_empty(value: &str, scheme: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with(scheme) && !value[scheme.len()..].is_empty()
}

pub(crate) fn safe_data_image(normalized: &str) -> bool {
    [
        "data:image/png;base64,",
        "data:image/jpeg;base64,",
        "data:image/gif;base64,",
        "data:image/webp;base64,",
        "data:image/avif;base64,",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix))
}

/// Decide whether `value` may appear in the given attribute slot under `policy`.
pub(crate) fn url_allowed(value: &str, kind: UrlAttributeKind, policy: &UrlPolicy) -> bool {
    let Some(value) = normalized_url_candidate(value) else {
        return false;
    };
    let normalized = value.to_ascii_lowercase();

    match kind {
        UrlAttributeKind::Navigation => {
            (policy.allow_hash_urls && normalized.starts_with('#'))
                || (policy.allow_relative_urls && relative_url_allowed(value))
                || (policy.allow_http && absolute_url_has_authority(value, "http://"))
                || (policy.allow_https && absolute_url_has_authority(value, "https://"))
                || (policy.allow_mailto && scheme_target_is_not_empty(value, "mailto:"))
                || (policy.allow_tel && scheme_target_is_not_empty(value, "tel:"))
        }
        UrlAttributeKind::Resource => {
            (policy.allow_relative_urls && relative_url_allowed(value))
                || (policy.allow_http && absolute_url_has_authority(value, "http://"))
                || (policy.allow_https && absolute_url_has_authority(value, "https://"))
                || (policy.allow_data_images && safe_data_image(&normalized))
        }
        UrlAttributeKind::FormAction => {
            (policy.allow_relative_urls && relative_url_allowed(value))
                || (policy.allow_http && absolute_url_has_authority(value, "http://"))
                || (policy.allow_https && absolute_url_has_authority(value, "https://"))
        }
        UrlAttributeKind::Fragment => {
            policy.allow_hash_urls && normalized.starts_with('#') && normalized.len() > 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_local_and_absolute_urls() {
        for value in [
            "/pricing",
            "#contact",
            "?source=hero",
            "https://example.com/path?q=1#section",
            "http://localhost:3000/form",
            "mailto:sales@example.com",
            "tel:+12025550123",
        ] {
            assert_eq!(normalize_safe_url(value, "URL").unwrap(), value);
        }
    }

    #[test]
    fn rejects_network_paths_backslashes_controls_and_unsafe_schemes() {
        for value in [
            "//attacker.example/path",
            "/\\attacker.example/path",
            "javascript:alert(1)",
            "data:text/html,unsafe",
            "vbscript:msgbox(1)",
            "https://example.com/has space",
            "https://example.com/line\nbreak",
        ] {
            assert!(normalize_safe_url(value, "URL").is_err(), "{value}");
        }
    }

    #[test]
    fn rejects_absolute_urls_without_authority_or_scheme_targets() {
        for value in ["https://", "http:///path", "mailto:", "tel:"] {
            assert!(normalize_safe_url(value, "URL").is_err(), "{value}");
        }
    }

    #[test]
    fn the_strict_action_policy_is_never_looser_than_the_permissive_attribute_policy() {
        // The whole point of consolidating is that these two cannot drift into contradiction:
        // anything the strict action policy accepts as a navigation target must also be
        // acceptable to the attribute policy, otherwise a stored action would be unrenderable.
        for value in [
            "/pricing",
            "#contact",
            "https://example.com/path?q=1#section",
            "http://localhost:3000/form",
            "mailto:sales@example.com",
            "tel:+12025550123",
        ] {
            assert!(normalize_safe_url(value, "URL").is_ok(), "strict rejected {value}");
            assert!(
                url_allowed(value, UrlAttributeKind::Navigation, &UrlPolicy::permissive()),
                "attribute policy rejected {value}"
            );
        }
    }

    #[test]
    fn shared_rejections_hold_for_both_policies() {
        for value in [
            "//attacker.example/path",
            "/\\attacker.example/path",
            "javascript:alert(1)",
            "vbscript:msgbox(1)",
            "https://example.com/has space",
            "https://example.com/line\nbreak",
            "https://",
        ] {
            assert!(normalize_safe_url(value, "URL").is_err(), "strict accepted {value}");
            assert!(
                !url_allowed(value, UrlAttributeKind::Navigation, &UrlPolicy::permissive()),
                "attribute policy accepted {value}"
            );
        }
    }

    #[test]
    fn data_images_are_resources_only_and_never_navigation_or_actions() {
        let value = "data:image/png;base64,iVBORw0KGgo=";
        let policy = UrlPolicy::permissive();
        assert!(url_allowed(value, UrlAttributeKind::Resource, &policy));
        assert!(!url_allowed(value, UrlAttributeKind::Navigation, &policy));
        assert!(!url_allowed(value, UrlAttributeKind::FormAction, &policy));
        // The strict action policy refuses `data:` entirely.
        assert!(normalize_safe_url(value, "URL").is_err());
    }

    #[test]
    fn non_image_data_urls_are_rejected_even_as_resources() {
        for value in [
            "data:text/html;base64,PHNjcmlwdD4=",
            "data:image/svg+xml;base64,PHN2Zz4=",
        ] {
            assert!(
                !url_allowed(value, UrlAttributeKind::Resource, &UrlPolicy::permissive()),
                "accepted {value}"
            );
        }
    }

    #[test]
    fn overlong_urls_are_rejected() {
        let value = format!("https://example.com/{}", "a".repeat(MAXIMUM_URL_LENGTH));
        assert!(!url_allowed(&value, UrlAttributeKind::Resource, &UrlPolicy::permissive()));
    }
}

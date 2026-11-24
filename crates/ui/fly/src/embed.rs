//! Allow-listed third-party embeds (video, maps) carried by components.
//!
//! Fly has no raw-HTML sink by design, so embeds arrive as a typed `flyEmbed` extension:
//! a provider id plus the provider embed URL. Rendering produces a sandboxed `<iframe>`
//! only when the URL is HTTPS, its host exactly matches the provider registration, and its
//! path stays inside the provider's embed surface. Anything else degrades to a plain link
//! (when the URL itself is safe) or to the embed title as text. Providers are data, not
//! code: a host registers its own catalog through [`EmbedProviderRegistry`], and the
//! built-in catalog covers the providers a landing page legitimately needs.

use crate::safe_url::{UrlAttributeKind, UrlPolicy, url_allowed};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

pub const FLY_EMBED_FIELD: &str = "flyEmbed";

/// Iframe sandbox applied to every rendered embed. Scripts and same-origin access are the
/// minimum interactive embeds (video players, maps) require; top navigation, popups, and
/// form submission from inside the frame stay denied.
pub const EMBED_IFRAME_SANDBOX: &str = "allow-scripts allow-same-origin allow-presentation";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbedSpec {
    pub provider: String,
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedProvider {
    pub id: &'static str,
    pub label: &'static str,
    /// Exact host names (no wildcards, no suffix matching).
    pub hosts: &'static [&'static str],
    /// The URL path must start with one of these prefixes.
    pub path_prefixes: &'static [&'static str],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmbedProviderRegistry {
    providers: Vec<EmbedProvider>,
}

impl EmbedProviderRegistry {
    pub fn with_builtins() -> Self {
        Self {
            providers: builtin_embed_providers(),
        }
    }

    pub fn register(&mut self, provider: EmbedProvider) {
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|candidate| candidate.id == provider.id)
        {
            *existing = provider;
        } else {
            self.providers.push(provider);
        }
    }

    pub fn provider(&self, id: &str) -> Option<&EmbedProvider> {
        self.providers.iter().find(|provider| provider.id == id)
    }

    pub fn providers(&self) -> &[EmbedProvider] {
        &self.providers
    }

    /// Full gate used by rendering and validation: registered provider, HTTPS, exact host,
    /// embed path prefix.
    pub fn validate(&self, spec: &EmbedSpec) -> Result<(), String> {
        let Some(provider) = self.provider(spec.provider.trim()) else {
            return Err(format!(
                "embed provider `{}` is not registered",
                spec.provider
            ));
        };
        validate_embed_url(provider, &spec.url)?;
        if spec.title.trim().is_empty() {
            return Err("embed requires a non-empty title for accessibility".to_string());
        }
        if let Some(ratio) = spec.aspect_ratio.as_deref()
            && !valid_aspect_ratio(ratio)
        {
            return Err(format!(
                "embed aspect ratio `{ratio}` must look like `16 / 9`"
            ));
        }
        Ok(())
    }
}

pub fn builtin_embed_providers() -> Vec<EmbedProvider> {
    vec![
        EmbedProvider {
            id: "youtube",
            label: "YouTube",
            hosts: &["www.youtube-nocookie.com", "www.youtube.com"],
            path_prefixes: &["/embed/"],
        },
        EmbedProvider {
            id: "vimeo",
            label: "Vimeo",
            hosts: &["player.vimeo.com"],
            path_prefixes: &["/video/"],
        },
        EmbedProvider {
            id: "google_maps",
            label: "Google Maps",
            hosts: &["www.google.com", "maps.google.com"],
            path_prefixes: &["/maps/embed"],
        },
    ]
}

/// Validate one embed URL against a provider registration.
pub fn validate_embed_url(provider: &EmbedProvider, raw_url: &str) -> Result<(), String> {
    let raw_url = raw_url.trim();
    if !url_allowed(raw_url, UrlAttributeKind::Resource, &UrlPolicy::permissive()) {
        return Err("embed URL is not a safe URL".to_string());
    }

    let parsed = Url::parse(raw_url)
        .map_err(|_| "embed URL must be a valid absolute HTTPS URL".to_string())?;
    if parsed.scheme() != "https" {
        return Err("embed URL must use HTTPS".to_string());
    }

    let authority = raw_url
        .split_once("://")
        .map(|(_, rest)| {
            rest.split(|character| matches!(character, '/' | '?' | '#'))
                .next()
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if authority.contains('@')
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("embed URL must not contain credentials".to_string());
    }
    if parsed.port().is_some() {
        return Err("embed URL must not use a non-default port".to_string());
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "embed URL must include a host".to_string())?
        .to_ascii_lowercase();
    if !provider
        .hosts
        .iter()
        .any(|allowed| host == allowed.to_ascii_lowercase())
    {
        return Err(format!(
            "embed host `{host}` is not registered for provider `{}`",
            provider.id
        ));
    }

    let path = parsed.path();
    let lower_path = path.to_ascii_lowercase();
    if ["%2e", "%2f", "%5c"].iter().any(|encoded| lower_path.contains(encoded))
        || path.split('/').any(|segment| segment == "." || segment == "..")
    {
        return Err("embed URL path must not contain encoded separators or dot segments".to_string());
    }
    if !provider
        .path_prefixes
        .iter()
        .any(|prefix| path_matches_prefix(path, prefix))
    {
        return Err(format!(
            "embed URL path must stay inside the provider embed surface ({})",
            provider.path_prefixes.join(", ")
        ));
    }
    Ok(())
}

fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    if prefix.ends_with('/') {
        path.strip_prefix(prefix)
            .is_some_and(|suffix| !suffix.is_empty() && !suffix.starts_with('/'))
    } else {
        path == prefix
            || path
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.len() > 1 && suffix.starts_with('/'))
    }
}

fn valid_aspect_ratio(value: &str) -> bool {
    let Some((width, height)) = value.split_once('/') else {
        return false;
    };
    let parse_positive = |part: &str| {
        part.trim()
            .parse::<f32>()
            .is_ok_and(|number| number > 0.0 && number.is_finite())
    };
    parse_positive(width) && parse_positive(height)
}

/// Decode and validate the `flyEmbed` extension against a host-supplied provider registry.
pub fn decode_embed_spec_with_registry(
    raw: &Value,
    registry: &EmbedProviderRegistry,
) -> Result<EmbedSpec, String> {
    crate::codec::ensure_depth_within_limit(raw, crate::MAXIMUM_DECODE_DEPTH)
        .map_err(|error| format!("embed specification exceeds safe JSON depth: {error}"))?;
    let mut spec: EmbedSpec = serde_json::from_value(raw.clone())
        .map_err(|error| format!("embed specification cannot be decoded: {error}"))?;
    spec.provider = spec.provider.trim().to_string();
    registry.validate(&spec)?;
    Ok(spec)
}

/// Decode and fully validate the `flyEmbed` extension against the built-in registry.
pub fn decode_embed_spec(raw: &Value) -> Result<EmbedSpec, String> {
    decode_embed_spec_with_registry(raw, &EmbedProviderRegistry::with_builtins())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec(value: Value) -> Result<EmbedSpec, String> {
        decode_embed_spec(&value)
    }

    #[test]
    fn builtin_providers_accept_their_embed_surfaces() {
        for (provider, url) in [
            ("youtube", "https://www.youtube-nocookie.com/embed/abc123"),
            ("youtube", "https://www.youtube.com/embed/abc123?rel=0"),
            ("vimeo", "https://player.vimeo.com/video/12345"),
            ("google_maps", "https://www.google.com/maps/embed?pb=xyz"),
        ] {
            let decoded = spec(json!({ "provider": provider, "url": url, "title": "Demo" }))
                .unwrap_or_else(|error| panic!("{provider} {url} must validate: {error}"));
            assert_eq!(decoded.provider, provider);
        }
    }

    #[test]
    fn embeds_reject_lookalike_hosts_http_and_off_surface_paths() {
        for url in [
            "https://www.youtube.com.evil.example/embed/abc",
            "https://evil-youtube.com/embed/abc",
            "http://www.youtube-nocookie.com/embed/abc",
            "https://www.youtube.com/watch?v=abc",
            "javascript:alert(1)",
            "//www.youtube.com/embed/abc",
        ] {
            assert!(
                spec(json!({ "provider": "youtube", "url": url, "title": "Demo" })).is_err(),
                "{url} must be rejected"
            );
        }
        assert!(
            spec(
                json!({ "provider": "unknown", "url": "https://example.com/embed", "title": "x" })
            )
            .is_err()
        );
        assert!(
            spec(json!({ "provider": "youtube", "url": "https://www.youtube.com/embed/a", "title": " " }))
                .is_err(),
            "an embed without an accessible title must be rejected"
        );
    }


    #[test]
    fn embeds_reject_credentials_non_default_ports_and_path_escape() {
        let registry = EmbedProviderRegistry::with_builtins();
        let youtube = registry.provider("youtube").expect("youtube provider");
        for url in [
            "https://attacker@www.youtube.com/embed/abc123",
            "https://www.youtube.com:8443/embed/abc123",
            "https://www.youtube.com/embed/../watch?v=abc123",
            "https://www.youtube.com/embed/%2e%2e/watch?v=abc123",
            "https://www.youtube.com/embed/%2f..%2fwatch",
        ] {
            assert!(
                validate_embed_url(youtube, url).is_err(),
                "unsafe embed URL was accepted: {url}"
            );
        }

        let custom = EmbedProvider {
            id: "custom",
            label: "Custom",
            hosts: &["player.example.test"],
            path_prefixes: &["/frame"],
        };
        assert!(validate_embed_url(&custom, "https://player.example.test/frame/123").is_ok());
        assert!(validate_embed_url(&custom, "https://player.example.test/frame.evil/123").is_err());
    }

    #[test]
    fn custom_registry_can_decode_a_provider_spec() {
        let mut registry = EmbedProviderRegistry::default();
        registry.register(EmbedProvider {
            id: "custom",
            label: "Custom",
            hosts: &["player.example.test"],
            path_prefixes: &["/frame/"],
        });
        let decoded = decode_embed_spec_with_registry(
            &json!({
                "provider": " custom ",
                "url": "https://player.example.test/frame/123",
                "title": "Player"
            }),
            &registry,
        )
        .expect("custom embed");
        assert_eq!(decoded.provider, "custom");
    }




    #[test]
    fn deeply_nested_embed_specs_are_rejected_before_deserialization() {
        let registry = EmbedProviderRegistry::with_builtins();
        let mut raw = json!({
            "provider": "youtube",
            "url": "https://www.youtube.com/embed/abc123",
            "title": "Video"
        });
        for _ in 0..=crate::MAXIMUM_DECODE_DEPTH {
            raw = json!({ "nested": raw });
        }
        assert!(decode_embed_spec_with_registry(&raw, &registry).is_err());
    }


}

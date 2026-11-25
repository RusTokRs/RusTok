use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use axum::extract::connect_info::ConnectInfo;
use axum::http::Extensions;
use axum::http::{
    HeaderMap,
    header::{FORWARDED, HOST},
};
use ipnet::IpNet;

use super::settings::{ForwardedHeadersMode, RequestTrustSettings};

fn trusted_proxy_nets(settings: &RequestTrustSettings) -> Vec<IpNet> {
    let mut nets = Vec::with_capacity(settings.trusted_proxy_cidrs.len());
    for value in &settings.trusted_proxy_cidrs {
        match IpNet::from_str(value.trim()) {
            Ok(net) => nets.push(net),
            Err(error) => {
                tracing::warn!(
                    cidr = %value,
                    %error,
                    "Ignoring invalid trusted proxy CIDR configuration"
                );
            }
        }
    }
    nets
}

pub fn peer_ip_from_extensions(extensions: &Extensions) -> Option<IpAddr> {
    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
        .or_else(|| extensions.get::<SocketAddr>().map(SocketAddr::ip))
}

pub fn peer_ip_from_headers(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| IpAddr::from_str(value.trim()).ok())
}

pub fn forwarded_headers_trusted(peer_ip: Option<IpAddr>, settings: &RequestTrustSettings) -> bool {
    match settings.forwarded_headers_mode {
        ForwardedHeadersMode::Ignore => false,
        ForwardedHeadersMode::TrustedOnly => {
            let Some(peer_ip) = peer_ip else {
                return false;
            };

            trusted_proxy_nets(settings)
                .into_iter()
                .any(|network| network.contains(&peer_ip))
        }
    }
}

fn has_forwarded_headers(headers: &HeaderMap) -> bool {
    headers.contains_key("x-forwarded-host")
        || headers.contains_key("x-forwarded-for")
        || headers.contains_key("x-forwarded-proto")
        || headers.contains_key(FORWARDED)
}

fn warn_ignored_forwarded_headers(headers: &HeaderMap, peer_ip: Option<IpAddr>, reason: &str) {
    if has_forwarded_headers(headers) {
        tracing::warn!(
            ?peer_ip,
            reason,
            "Ignoring forwarded headers because request trust policy rejected them"
        );
    }
}

/// Use the value appended by the nearest trusted proxy, not a client-supplied
/// value earlier in a comma-separated forwarding chain.
fn last_forwarded_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let values = headers.get_all(name);
    values
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_forwarded_pair<'a>(forwarded: &'a str, key: &str) -> Option<&'a str> {
    forwarded
        .split(',')
        .next()
        .and_then(|entry| {
            entry.split(';').find_map(|part| {
                let (candidate_key, candidate_value) = part.trim().split_once('=')?;
                (candidate_key.eq_ignore_ascii_case(key))
                    .then(|| candidate_value.trim_matches('"').trim())
            })
        })
        .filter(|value| !value.is_empty())
}

fn last_forwarded_pair(headers: &HeaderMap, key: &str) -> Option<String> {
    let values = headers.get_all(FORWARDED);
    values
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .and_then(|entry| parse_forwarded_pair(entry, key))
        .map(ToOwned::to_owned)
}

fn parse_forwarded_for_ip(value: &str) -> Option<IpAddr> {
    let candidate = value.trim();
    IpAddr::from_str(candidate)
        .ok()
        .or_else(|| SocketAddr::from_str(candidate).ok().map(|addr| addr.ip()))
        .or_else(|| candidate.strip_prefix('[')?.strip_suffix(']')?.parse().ok())
}

/// Resolve client identity from the nearest hop towards the caller. A trusted
/// reverse proxy may append to a header supplied by an untrusted client; the
/// leftmost value is therefore not necessarily the client's real address.
/// Stop at the first untrusted hop instead of trusting earlier claims.
///
/// Malformed hops on the trusted side of the boundary invalidate the chain.
fn client_ip_from_forwarded_chain(
    headers: &HeaderMap,
    header_name: &str,
    peer_ip: IpAddr,
    settings: &RequestTrustSettings,
    parse_hop: fn(&str) -> Option<IpAddr>,
) -> Option<IpAddr> {
    let values = headers.get_all(header_name);
    let fields: Vec<_> = values.iter().collect();
    if fields.is_empty() {
        return None;
    }

    let trusted_nets = trusted_proxy_nets(settings);
    let mut current = peer_ip;
    for field in fields.into_iter().rev() {
        // Decode each hop only after establishing that it belongs to the
        // trusted side of the chain. An opaque byte in a caller-controlled
        // earlier hop must not invalidate a proxy-appended client address.
        for entry in field.as_bytes().rsplit(|byte| *byte == b',') {
            if !trusted_nets.iter().any(|net| net.contains(&current)) {
                return Some(current);
            }
            if !entry.is_ascii() {
                return None;
            }
            current = parse_hop(std::str::from_utf8(entry).ok()?.trim())?;
        }
    }
    Some(current)
}

fn parse_forwarded_element_ip(element: &str) -> Option<IpAddr> {
    parse_forwarded_pair(element, "for").and_then(parse_forwarded_for_ip)
}

pub fn extract_effective_host(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &RequestTrustSettings,
) -> Option<String> {
    if forwarded_headers_trusted(peer_ip, settings) {
        // X-Forwarded-Host takes precedence when it is present. An invalid
        // last hop must fail closed instead of switching to a potentially
        // conflicting Forwarded or Host value from another authority.
        if headers.contains_key("x-forwarded-host") {
            return last_forwarded_header_value(headers, "x-forwarded-host");
        }
        return last_forwarded_pair(headers, "host").or_else(|| {
            headers
                .get(HOST)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        });
    }

    warn_ignored_forwarded_headers(headers, peer_ip, "untrusted_or_ignored_forwarded_host");
    headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn extract_effective_client_ip(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &RequestTrustSettings,
) -> Option<IpAddr> {
    if forwarded_headers_trusted(peer_ip, settings) {
        let peer = peer_ip?;
        // A malformed X-Forwarded-For must not fall through to a potentially
        // conflicting Forwarded header supplied by the same caller.
        if headers.contains_key("x-forwarded-for") {
            return client_ip_from_forwarded_chain(
                headers,
                "x-forwarded-for",
                peer,
                settings,
                |entry| IpAddr::from_str(entry).ok(),
            )
            .or(Some(peer));
        }
        if headers.contains_key(FORWARDED) {
            return client_ip_from_forwarded_chain(
                headers,
                "forwarded",
                peer,
                settings,
                parse_forwarded_element_ip,
            )
            .or(Some(peer));
        }
    } else {
        warn_ignored_forwarded_headers(headers, peer_ip, "untrusted_or_ignored_forwarded_for");
    }

    peer_ip
}

/// Whether a credential-bearing cookie must carry the Secure attribute.
///
/// Release builds share the host startup's production security posture even if
/// an operator selected a non-production environment name. In debug builds,
/// trusted forwarded HTTPS remains sufficient for local reverse-proxy setups.
pub fn secure_cookie_required(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &RequestTrustSettings,
) -> bool {
    secure_cookie_required_for_environment(
        headers,
        peer_ip,
        settings,
        super::settings::is_production_security_mode(),
    )
}

/// Testable policy boundary used by secure_cookie_required.
pub fn secure_cookie_required_for_environment(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &RequestTrustSettings,
    production_mode: bool,
) -> bool {
    production_mode
        || extract_effective_proto(headers, peer_ip, settings)
            .is_some_and(|value| value.eq_ignore_ascii_case("https"))
}

pub fn extract_effective_proto(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &RequestTrustSettings,
) -> Option<String> {
    if forwarded_headers_trusted(peer_ip, settings) {
        // Do not reinterpret a malformed X-Forwarded-Proto using a different
        // header family. Credential-cookie security must use one authority.
        let value = if headers.contains_key("x-forwarded-proto") {
            last_forwarded_header_value(headers, "x-forwarded-proto")
        } else {
            last_forwarded_pair(headers, "proto")
        };
        return value
            .map(|value| value.to_ascii_lowercase())
            .filter(|value| value == "http" || value == "https");
    }

    warn_ignored_forwarded_headers(headers, peer_ip, "untrusted_or_ignored_forwarded_proto");
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn trusted_settings() -> RequestTrustSettings {
        RequestTrustSettings {
            forwarded_headers_mode: ForwardedHeadersMode::TrustedOnly,
            trusted_proxy_cidrs: vec!["10.0.0.0/8".to_string()],
        }
    }

    #[test]
    fn production_cookie_policy_forces_secure_without_forwarded_headers() {
        let headers = HeaderMap::new();
        let peer = Some(IpAddr::from([192, 0, 2, 4]));
        let settings = RequestTrustSettings::default();

        assert!(secure_cookie_required_for_environment(
            &headers, peer, &settings, true,
        ));
        assert!(!secure_cookie_required_for_environment(
            &headers, peer, &settings, false,
        ));
    }

    #[test]
    fn release_cookie_policy_is_aligned_with_production_startup_mode() {
        let headers = HeaderMap::new();
        let peer = Some(IpAddr::from([192, 0, 2, 4]));
        let settings = RequestTrustSettings::default();
        assert_eq!(
            secure_cookie_required(&headers, peer, &settings),
            crate::common::settings::is_production_security_mode(),
        );
    }

    #[test]
    fn forwarded_headers_are_ignored_by_default() {
        let settings = RequestTrustSettings::default();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("tenant.example.test"),
        );
        headers.insert(HOST, HeaderValue::from_static("backend.internal"));

        let host = extract_effective_host(&headers, Some(IpAddr::from([10, 0, 0, 5])), &settings);

        assert_eq!(host.as_deref(), Some("backend.internal"));
    }

    #[test]
    fn trusted_proxy_allows_forwarded_host_and_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("tenant.example.test"),
        );
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.8"));
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        headers.insert(HOST, HeaderValue::from_static("backend.internal"));

        let settings = trusted_settings();
        let peer_ip = Some(IpAddr::from([10, 1, 2, 3]));

        assert_eq!(
            extract_effective_host(&headers, peer_ip, &settings).as_deref(),
            Some("tenant.example.test")
        );
        assert_eq!(
            extract_effective_client_ip(&headers, peer_ip, &settings),
            Some(IpAddr::from([203, 0, 113, 8]))
        );
        assert_eq!(
            extract_effective_proto(&headers, peer_ip, &settings).as_deref(),
            Some("https")
        );
    }

    #[test]
    fn trusted_proxy_uses_last_forwarded_host_and_proto_not_spoofed_first_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("evil.example.test, tenant.example.test"),
        );
        headers.insert("x-forwarded-proto", HeaderValue::from_static("http, https"));
        let peer = Some(IpAddr::from([10, 1, 2, 3]));
        let settings = trusted_settings();

        assert_eq!(
            extract_effective_host(&headers, peer, &settings).as_deref(),
            Some("tenant.example.test"),
        );
        assert_eq!(
            extract_effective_proto(&headers, peer, &settings).as_deref(),
            Some("https"),
        );
    }

    #[test]
    fn trusted_proxy_uses_last_rfc_forwarded_host_and_proto() {
        let mut headers = HeaderMap::new();
        headers.insert(
            FORWARDED,
            HeaderValue::from_static(
                "for=198.51.100.66;host=evil.example.test;proto=http, for=203.0.113.8;host=tenant.example.test;proto=https",
            ),
        );
        let peer = Some(IpAddr::from([10, 1, 2, 3]));
        let settings = trusted_settings();

        assert_eq!(
            extract_effective_host(&headers, peer, &settings).as_deref(),
            Some("tenant.example.test"),
        );
        assert_eq!(
            extract_effective_proto(&headers, peer, &settings).as_deref(),
            Some("https"),
        );
    }

    #[test]
    fn forwarded_proto_rejects_non_http_schemes() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("javascript"));
        assert_eq!(
            extract_effective_proto(
                &headers,
                Some(IpAddr::from([10, 1, 2, 3])),
                &trusted_settings(),
            ),
            None,
        );
    }

    #[test]
    fn invalid_x_forwarded_host_cannot_switch_to_other_host_authority() {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("backend.internal"));
        headers.insert(FORWARDED, HeaderValue::from_static("host=other.example.test"));
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("valid.example.test, "),
        );
        let peer = Some(IpAddr::from([10, 1, 2, 3]));

        assert_eq!(
            extract_effective_host(&headers, peer, &trusted_settings()),
            None,
            "empty nearest X-Forwarded-Host must not select an unrelated host"
        );
        assert_eq!(
            extract_effective_host(&headers, peer, &RequestTrustSettings::default()).as_deref(),
            Some("backend.internal"),
            "untrusted forwarding never overrides the direct Host header"
        );

        headers.remove("x-forwarded-host");
        assert_eq!(
            extract_effective_host(&headers, peer, &trusted_settings()).as_deref(),
            Some("other.example.test"),
            "Forwarded remains the supported fallback when X-Forwarded-Host is absent"
        );
    }

    #[test]
    fn invalid_x_forwarded_proto_cannot_switch_to_other_scheme_authority() {
        let mut headers = HeaderMap::new();
        headers.insert(FORWARDED, HeaderValue::from_static("proto=https"));
        headers.insert("x-forwarded-proto", HeaderValue::from_static("http, "));
        let peer = Some(IpAddr::from([10, 1, 2, 3]));

        assert_eq!(
            extract_effective_proto(&headers, peer, &trusted_settings()),
            None,
            "empty nearest X-Forwarded-Proto must not inherit another header's HTTPS"
        );
        headers.insert("x-forwarded-proto", HeaderValue::from_static("javascript"));
        assert_eq!(
            extract_effective_proto(&headers, peer, &trusted_settings()),
            None,
            "invalid explicit forwarding protocol must not select another header"
        );

        headers.remove("x-forwarded-proto");
        assert_eq!(
            extract_effective_proto(&headers, peer, &trusted_settings()).as_deref(),
            Some("https"),
            "RFC Forwarded stays supported when X-Forwarded-Proto is absent"
        );
    }

    #[test]
    fn trusted_proxy_ignores_spoofed_leftmost_forwarded_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.66, 203.0.113.8"),
        );
        let peer = Some(IpAddr::from([10, 1, 2, 3]));

        assert_eq!(
            extract_effective_client_ip(&headers, peer, &trusted_settings()),
            Some(IpAddr::from([203, 0, 113, 8])),
        );
    }

    #[test]
    fn trusted_proxy_chain_skips_only_verified_intermediaries() {
        let mut headers = HeaderMap::new();
        headers.append(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.66, 203.0.113.8"),
        );
        headers.append("x-forwarded-for", HeaderValue::from_static("10.0.0.4"));

        assert_eq!(
            extract_effective_client_ip(
                &headers,
                Some(IpAddr::from([10, 1, 2, 3])),
                &trusted_settings(),
            ),
            Some(IpAddr::from([203, 0, 113, 8])),
        );
    }

    #[test]
    fn untrusted_leftmost_garbage_does_not_poison_client_identity() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("not-an-ip, 203.0.113.8"),
        );

        assert_eq!(
            extract_effective_client_ip(
                &headers,
                Some(IpAddr::from([10, 1, 2, 3])),
                &trusted_settings(),
            ),
            Some(IpAddr::from([203, 0, 113, 8])),
        );
    }

    #[test]
    fn opaque_untrusted_hops_do_not_poison_client_identity() {
        let peer = Some(IpAddr::from([10, 1, 2, 3]));
        let client = Some(IpAddr::from([203, 0, 113, 8]));
        let settings = trusted_settings();

        for (name, combined, appended) in [
            (
                "x-forwarded-for",
                &b"\xff, 203.0.113.8"[..],
                "203.0.113.8",
            ),
            (
                "forwarded",
                &b"for=\xff, for=203.0.113.8"[..],
                "for=203.0.113.8",
            ),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(name, HeaderValue::from_bytes(combined).unwrap());
            assert_eq!(extract_effective_client_ip(&headers, peer, &settings), client);

            // The same boundary applies across multiple header field lines.
            headers.insert(name, HeaderValue::from_bytes(b"\xff").unwrap());
            headers.append(name, HeaderValue::from_static(appended));
            assert_eq!(extract_effective_client_ip(&headers, peer, &settings), client);

            // Opaque bytes on the trusted side still fail closed.
            headers.append(name, HeaderValue::from_bytes(b"\xff").unwrap());
            assert_eq!(extract_effective_client_ip(&headers, peer, &settings), peer);
        }
    }

    #[test]
    fn malformed_nearest_hop_falls_back_to_peer_not_other_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.5, invalid"),
        );
        headers.insert(FORWARDED, HeaderValue::from_static("for=198.51.100.6"));
        let peer = IpAddr::from([10, 1, 2, 3]);

        assert_eq!(
            extract_effective_client_ip(&headers, Some(peer), &trusted_settings()),
            Some(peer),
        );
    }

    #[test]
    fn forwarded_header_respects_nearest_untrusted_hop_and_ipv6_port() {
        let mut headers = HeaderMap::new();
        headers.insert(
            FORWARDED,
            HeaderValue::from_static("for=198.51.100.66;proto=http, for=\"[2001:db8::1234]:1234\""),
        );

        assert_eq!(
            extract_effective_client_ip(
                &headers,
                Some(IpAddr::from([10, 1, 2, 3])),
                &trusted_settings(),
            ),
            Some(IpAddr::from_str("2001:db8::1234").unwrap()),
        );
        assert_eq!(parse_forwarded_for_ip("[2001:db8::1234]garbage"), None);
        assert_eq!(parse_forwarded_for_ip("198.51.100.1:not-a-port"), None);
    }

    #[test]
    fn untrusted_proxy_cannot_spoof_forwarded_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("tenant.example.test"),
        );
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.8"));
        headers.insert(HOST, HeaderValue::from_static("backend.internal"));

        let settings = trusted_settings();
        let peer_ip = Some(IpAddr::from([192, 168, 1, 4]));

        assert_eq!(
            extract_effective_host(&headers, peer_ip, &settings).as_deref(),
            Some("backend.internal")
        );
        assert_eq!(
            extract_effective_client_ip(&headers, peer_ip, &settings),
            peer_ip
        );
    }
}

use std::collections::HashSet;
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
        // Only the proxy-appended value is authoritative. An opaque byte
        // in an earlier caller-controlled value must not poison that suffix.
        .and_then(|value| value.as_bytes().rsplit(|byte| *byte == b',').next())
        .filter(|entry| entry.is_ascii())
        .and_then(|entry| std::str::from_utf8(entry).ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// Split from the trusted end without interpreting an earlier, untrusted
/// prefix. RFC Forwarded permits delimiters inside quoted-string values.
fn forwarded_parts_from_right(
    bytes: &[u8],
    separator: u8,
    respect_quotes: bool,
) -> impl Iterator<Item = &[u8]> + '_ {
    let mut remaining = Some(bytes);
    std::iter::from_fn(move || {
        let field = remaining.take()?;
        let mut in_quotes = false;
        for index in (0..field.len()).rev() {
            let byte = field[index];
            if respect_quotes && byte == b'"' {
                let escaped = field[..index]
                    .iter()
                    .rev()
                    .take_while(|&&previous| previous == b'\\')
                    .count()
                    % 2
                    != 0;
                if !escaped {
                    in_quotes = !in_quotes;
                }
            } else if byte == separator && !in_quotes {
                remaining = Some(&field[..index]);
                return Some(&field[index + 1..]);
            }
        }
        Some(field)
    })
}

fn forwarded_parameter_value(value: &str) -> Result<&str, ()> {
    if let Some(quoted) = value.strip_prefix('"') {
        let inner = quoted.strip_suffix('"').ok_or(())?;
        let mut escaped = false;
        for byte in inner.bytes() {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                return Err(());
            }
        }
        if escaped {
            return Err(());
        }
        Ok(inner)
    } else if value.is_empty()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'"' | b'\\' | b',' | b';'))
    {
        Err(())
    } else {
        Ok(value)
    }
}

/// Distinguish an absent selector in a valid element from malformed input.
/// Never select the first of multiple case-insensitively identical parameters.
fn parse_forwarded_pair<'a>(element: &'a str, key: &str) -> Result<Option<&'a str>, ()> {
    let mut seen = HashSet::new();
    let mut selected = None;
    for part in forwarded_parts_from_right(element.as_bytes(), b';', true) {
        let part = std::str::from_utf8(part).map_err(|_| ())?;
        let (name, value) = part.trim().split_once('=').ok_or(())?;
        let name = name.trim();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
            || !seen.insert(name.to_ascii_lowercase())
        {
            return Err(());
        }
        let value = forwarded_parameter_value(value.trim())?;
        if name.eq_ignore_ascii_case(key) {
            // Authority selectors need no quoted-pair escapes or whitespace.
            // Reject them rather than passing a differently interpreted value
            // to a downstream host, IP or scheme parser.
            if value.is_empty()
                || value.bytes().any(|byte| {
                    byte.is_ascii_whitespace() || matches!(byte, b'\\' | b'"' | b',' | b';')
                })
            {
                return Err(());
            }
            selected = Some(value);
        }
    }
    Ok(selected)
}

fn last_forwarded_pair(headers: &HeaderMap, key: &str) -> Result<Option<String>, ()> {
    let values = headers.get_all(FORWARDED);
    let Some(field) = values.iter().next_back() else {
        return Ok(None);
    };
    let entry = forwarded_parts_from_right(field.as_bytes(), b',', true)
        .next()
        .ok_or(())?;
    if !entry.is_ascii() {
        return Err(());
    }
    let entry = std::str::from_utf8(entry).map_err(|_| ())?;
    parse_forwarded_pair(entry, key).map(|value| value.map(ToOwned::to_owned))
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
        for entry in forwarded_parts_from_right(
            field.as_bytes(),
            b',',
            header_name.eq_ignore_ascii_case(FORWARDED.as_str()),
        ) {
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
    parse_forwarded_pair(element, "for")
        .ok()
        .flatten()
        .and_then(parse_forwarded_for_ip)
}

pub fn extract_effective_host(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &RequestTrustSettings,
) -> Option<String> {
    let host_values = headers.get_all(HOST);
    let mut host_values = host_values.iter();
    let direct_host = host_values.next();
    if host_values.next().is_some() {
        return None;
    }
    let direct_host = || {
        direct_host
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };

    if forwarded_headers_trusted(peer_ip, settings) {
        // X-Forwarded-Host takes precedence when it is present. An invalid
        // last hop must fail closed instead of switching to a potentially
        // conflicting Forwarded or Host value from another authority.
        if headers.contains_key("x-forwarded-host") {
            return last_forwarded_header_value(headers, "x-forwarded-host");
        }
        return match last_forwarded_pair(headers, "host") {
            Ok(Some(host)) => Some(host),
            Ok(None) => direct_host(),
            Err(()) => None,
        };
    }

    warn_ignored_forwarded_headers(headers, peer_ip, "untrusted_or_ignored_forwarded_host");
    direct_host()
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
            last_forwarded_pair(headers, "proto").ok().flatten()
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
    fn opaque_earlier_values_do_not_poison_proxy_appended_host_and_proto() {
        let peer = Some(IpAddr::from([10, 1, 2, 3]));
        let settings = trusted_settings();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_bytes(b"\xff, tenant.example.test").unwrap(),
        );
        headers.insert(
            "x-forwarded-proto",
            HeaderValue::from_bytes(b"\xff, https").unwrap(),
        );
        assert_eq!(
            extract_effective_host(&headers, peer, &settings).as_deref(),
            Some("tenant.example.test")
        );
        assert_eq!(
            extract_effective_proto(&headers, peer, &settings).as_deref(),
            Some("https")
        );
        assert!(secure_cookie_required_for_environment(
            &headers, peer, &settings, false,
        ));

        // Separate header fields from the trusted proxy are equivalent.
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_bytes(b"\xff").unwrap(),
        );
        headers.append(
            "x-forwarded-host",
            HeaderValue::from_static("tenant.example.test"),
        );
        headers.insert(
            "x-forwarded-proto",
            HeaderValue::from_bytes(b"\xff").unwrap(),
        );
        headers.append("x-forwarded-proto", HeaderValue::from_static("https"));
        assert_eq!(
            extract_effective_host(&headers, peer, &settings).as_deref(),
            Some("tenant.example.test")
        );
        assert_eq!(
            extract_effective_proto(&headers, peer, &settings).as_deref(),
            Some("https")
        );

        // Invalid nearest values must not select earlier or conflicting values.
        headers.insert(
            FORWARDED,
            HeaderValue::from_static("host=other.example.test;proto=https"),
        );
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_bytes(b"tenant.example.test, \xff").unwrap(),
        );
        headers.insert(
            "x-forwarded-proto",
            HeaderValue::from_bytes(b"https, \xff").unwrap(),
        );
        assert_eq!(extract_effective_host(&headers, peer, &settings), None);
        assert_eq!(extract_effective_proto(&headers, peer, &settings), None);
    }

    #[test]
    fn opaque_earlier_rfc_forwarded_values_do_not_poison_last_element() {
        let peer = Some(IpAddr::from([10, 1, 2, 3]));
        let settings = trusted_settings();
        let mut headers = HeaderMap::new();
        headers.insert(
            FORWARDED,
            HeaderValue::from_bytes(
                b"for=\xff, for=203.0.113.8;host=tenant.example.test;proto=https",
            )
            .unwrap(),
        );
        assert_eq!(
            extract_effective_host(&headers, peer, &settings).as_deref(),
            Some("tenant.example.test")
        );
        assert_eq!(
            extract_effective_proto(&headers, peer, &settings).as_deref(),
            Some("https")
        );
        assert_eq!(
            extract_effective_host(&headers, peer, &RequestTrustSettings::default()),
            None
        );
        assert_eq!(
            extract_effective_proto(&headers, peer, &RequestTrustSettings::default()),
            None
        );

        headers.insert(
            FORWARDED,
            HeaderValue::from_bytes(b"host=tenant.example.test;proto=https, for=\xff").unwrap(),
        );
        assert_eq!(extract_effective_host(&headers, peer, &settings), None);
        assert_eq!(extract_effective_proto(&headers, peer, &settings), None);
    }

    #[test]
    fn rfc_forwarded_rejects_ambiguous_or_malformed_authority_parameters() {
        let peer = Some(IpAddr::from([10, 1, 2, 3]));
        let settings = trusted_settings();
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("backend.internal"));
        for value in [
            "for=203.0.113.8;FOR=198.51.100.1;host=tenant.example.test;proto=https",
            "for=203.0.113.8;host=tenant.example.test;HOST=other.example.test;proto=https",
            "for=203.0.113.8;host=tenant.example.test;proto=https;Proto=http",
            "for=203.0.113.8;host=\"tenant.example.test;proto=https",
            "for=203.0.113.8;host=;proto=https",
        ] {
            headers.insert(FORWARDED, HeaderValue::from_str(value).unwrap());
            assert_eq!(
                extract_effective_host(&headers, peer, &settings),
                None,
                "{value}"
            );
        }
        headers.insert(
            FORWARDED,
            HeaderValue::from_static("for=203.0.113.8;FOR=198.51.100.1;proto=https"),
        );
        assert_eq!(extract_effective_client_ip(&headers, peer, &settings), peer);
        assert_eq!(extract_effective_proto(&headers, peer, &settings), None);
    }

    #[test]
    fn rfc_forwarded_quoted_delimiters_do_not_create_authority_parameters() {
        let peer = Some(IpAddr::from([10, 1, 2, 3]));
        let settings = trusted_settings();
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("backend.internal"));
        headers.insert(
            FORWARDED,
            HeaderValue::from_static(
                "for=198.51.100.1, for=203.0.113.8;host=\"tenant.example.test\";proto=https;ext=\"a,b;host=other.example.test;proto=http\"",
            ),
        );
        assert_eq!(
            extract_effective_host(&headers, peer, &settings).as_deref(),
            Some("tenant.example.test")
        );
        assert_eq!(
            extract_effective_proto(&headers, peer, &settings).as_deref(),
            Some("https")
        );
        assert_eq!(
            extract_effective_client_ip(&headers, peer, &settings),
            Some(IpAddr::from([203, 0, 113, 8]))
        );

        // The quoted extension does not supply a host when no host exists.
        headers.insert(
            FORWARDED,
            HeaderValue::from_static("for=203.0.113.8;ext=\"a;host=other.example.test\""),
        );
        assert_eq!(
            extract_effective_host(&headers, peer, &settings).as_deref(),
            Some("backend.internal")
        );
    }

    #[test]
    fn duplicate_direct_host_is_never_an_authoritative_value() {
        let mut headers = HeaderMap::new();
        headers.append(HOST, HeaderValue::from_static("first.example.test"));
        headers.append(HOST, HeaderValue::from_static("second.example.test"));
        let peer = Some(IpAddr::from([10, 1, 2, 3]));
        assert_eq!(
            extract_effective_host(&headers, peer, &RequestTrustSettings::default()),
            None
        );
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("tenant.example.test"),
        );
        assert_eq!(
            extract_effective_host(&headers, peer, &trusted_settings()),
            None
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
        headers.insert(
            FORWARDED,
            HeaderValue::from_static("host=other.example.test"),
        );
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
            ("x-forwarded-for", &b"\xff, 203.0.113.8"[..], "203.0.113.8"),
            (
                "forwarded",
                &b"for=\xff, for=203.0.113.8"[..],
                "for=203.0.113.8",
            ),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(name, HeaderValue::from_bytes(combined).unwrap());
            assert_eq!(
                extract_effective_client_ip(&headers, peer, &settings),
                client
            );

            // The same boundary applies across multiple header field lines.
            headers.insert(name, HeaderValue::from_bytes(b"\xff").unwrap());
            headers.append(name, HeaderValue::from_static(appended));
            assert_eq!(
                extract_effective_client_ip(&headers, peer, &settings),
                client
            );

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

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use axum::extract::connect_info::ConnectInfo;
use axum::http::Extensions;
use axum::http::{
    header::{FORWARDED, HOST},
    HeaderMap,
};
use ipnet::IpNet;

use super::settings::{ForwardedHeadersMode, RequestTrustSettings};

fn trusted_proxy_nets(settings: &RequestTrustSettings) -> Vec<IpNet> {
    settings
        .trusted_proxy_cidrs
        .iter()
        .filter_map(|value| IpNet::from_str(value.trim()).ok())
        .collect()
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

fn first_header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
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

fn forwarded_value<'a>(headers: &'a HeaderMap, key: &str) -> Option<&'a str> {
    headers
        .get(FORWARDED)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_forwarded_pair(value, key))
}

fn parse_forwarded_for_ip(value: &str) -> Option<IpAddr> {
    let candidate = value.trim();
    if let Some(inner) = candidate
        .strip_prefix('[')
        .and_then(|inner| inner.split(']').next())
    {
        return IpAddr::from_str(inner).ok();
    }

    if let Ok(ip) = IpAddr::from_str(candidate) {
        return Some(ip);
    }

    candidate
        .rsplit_once(':')
        .and_then(|(host, _)| IpAddr::from_str(host).ok())
}

pub fn extract_effective_host(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &RequestTrustSettings,
) -> Option<String> {
    if forwarded_headers_trusted(peer_ip, settings) {
        return first_header_value(headers, "x-forwarded-host")
            .or_else(|| forwarded_value(headers, "host"))
            .or_else(|| headers.get(HOST).and_then(|value| value.to_str().ok()))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
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
        if let Some(forwarded_for) = first_header_value(headers, "x-forwarded-for")
            .and_then(|value| IpAddr::from_str(value).ok())
        {
            return Some(forwarded_for);
        }

        if let Some(forwarded) = forwarded_value(headers, "for").and_then(parse_forwarded_for_ip) {
            return Some(forwarded);
        }
    } else {
        warn_ignored_forwarded_headers(headers, peer_ip, "untrusted_or_ignored_forwarded_for");
    }

    peer_ip
}

pub fn extract_effective_proto(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &RequestTrustSettings,
) -> Option<String> {
    if forwarded_headers_trusted(peer_ip, settings) {
        return first_header_value(headers, "x-forwarded-proto")
            .or_else(|| forwarded_value(headers, "proto"))
            .map(|value| value.to_ascii_lowercase());
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

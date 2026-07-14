use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

use rustok_core::security::{SsrfProtection, ValidationResult};

const MAX_SITEMAP_SUBMIT_URL_LEN: usize = 2_048;
const ALLOW_INSECURE_HTTP_ENV: &str = "RUSTOK_SEO_ALLOW_INSECURE_SITEMAP_SUBMISSION";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SitemapSubmitEndpoint {
    pub(super) endpoint: String,
    pub(super) request_url: String,
}

#[async_trait::async_trait]
pub(super) trait SitemapSubmissionAdapter: Send + Sync {
    async fn submit_sitemap_index(&self, endpoint: SitemapSubmitEndpoint) -> Result<(), String>;
}

pub(super) struct HttpSitemapSubmissionAdapter {
    timeout: Duration,
}

pub(super) struct SitemapSubmissionRuntime {
    adapter: Box<dyn SitemapSubmissionAdapter>,
}

#[derive(Debug)]
struct ParsedSitemapSubmitTarget {
    url: reqwest::Url,
    host: String,
    port: u16,
}

#[derive(Debug)]
struct ResolvedSitemapSubmitTarget {
    url: reqwest::Url,
    host: String,
    address: SocketAddr,
}

impl SitemapSubmissionRuntime {
    pub(super) fn default_with_timeout(timeout_secs: u64) -> Result<Self, String> {
        if timeout_secs == 0 {
            return Err("sitemap submission timeout must be greater than zero".to_string());
        }
        Ok(Self {
            adapter: Box::new(HttpSitemapSubmissionAdapter {
                timeout: Duration::from_secs(timeout_secs),
            }),
        })
    }

    pub(super) fn adapter(&self) -> &dyn SitemapSubmissionAdapter {
        self.adapter.as_ref()
    }
}

#[async_trait::async_trait]
impl SitemapSubmissionAdapter for HttpSitemapSubmissionAdapter {
    async fn submit_sitemap_index(&self, endpoint: SitemapSubmitEndpoint) -> Result<(), String> {
        let target = resolve_sitemap_submit_target(endpoint.request_url.as_str()).await?;
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .connect_timeout(self.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .resolve(target.host.as_str(), target.address)
            .build()
            .map_err(|error| {
                format!(
                    "failed to create secure sitemap submission client for endpoint `{}`: {error}",
                    endpoint.endpoint
                )
            })?;
        let response = client.get(target.url).send().await.map_err(|error| {
            if error.is_timeout() {
                format!(
                    "request timeout for endpoint `{}`: {error}",
                    endpoint.endpoint
                )
            } else {
                format!(
                    "request failed for endpoint `{}` with error: {error}",
                    endpoint.endpoint
                )
            }
        })?;
        if response.status().is_redirection() {
            return Err(format!(
                "endpoint `{}` attempted a redirect with status {}; redirects are not allowed",
                endpoint.endpoint,
                response.status()
            ));
        }
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "endpoint `{}` responded with status {}",
                endpoint.endpoint,
                response.status()
            ))
        }
    }
}

fn validate_sitemap_submit_target(raw_url: &str) -> Result<ParsedSitemapSubmitTarget, String> {
    if raw_url.len() > MAX_SITEMAP_SUBMIT_URL_LEN {
        return Err(format!(
            "sitemap submission URL exceeds the {MAX_SITEMAP_SUBMIT_URL_LEN} byte limit"
        ));
    }

    match SsrfProtection::new().validate_url(raw_url) {
        ValidationResult::Valid => {}
        ValidationResult::Invalid { reason } => {
            return Err(format!("sitemap submission URL rejected: {reason}"));
        }
        ValidationResult::Sanitized { .. } => {
            return Err("sitemap submission URL returned an unexpected validation result".to_string());
        }
    }

    let mut url = reqwest::Url::parse(raw_url)
        .map_err(|error| format!("invalid sitemap submission URL: {error}"))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("sitemap submission URL userinfo is not allowed".to_string());
    }
    match url.scheme() {
        "https" => {}
        "http" if insecure_http_allowed() => {}
        "http" => {
            return Err(format!(
                "insecure sitemap submission HTTP is disabled; platform operators may opt in with {ALLOW_INSECURE_HTTP_ENV}"
            ));
        }
        scheme => {
            return Err(format!(
                "unsupported sitemap submission URL scheme `{scheme}`"
            ));
        }
    }

    let host = url
        .host_str()
        .ok_or_else(|| "sitemap submission URL host is required".to_string())?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".home.arpa")
    {
        return Err("local or internal sitemap submission hostnames are not allowed".to_string());
    }
    url.set_host(Some(host.as_str()))
        .map_err(|_| "sitemap submission URL host could not be canonicalized".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "sitemap submission destination port is required".to_string())?;

    Ok(ParsedSitemapSubmitTarget { url, host, port })
}

async fn resolve_sitemap_submit_target(
    raw_url: &str,
) -> Result<ResolvedSitemapSubmitTarget, String> {
    let target = validate_sitemap_submit_target(raw_url)?;
    let addresses = if let Ok(ip) = IpAddr::from_str(target.host.as_str()) {
        vec![SocketAddr::new(ip, target.port)]
    } else {
        tokio::net::lookup_host((target.host.as_str(), target.port))
            .await
            .map_err(|error| format!("sitemap submission DNS resolution failed: {error}"))?
            .collect::<Vec<_>>()
    };
    if addresses.is_empty() {
        return Err("sitemap submission destination resolved to no addresses".to_string());
    }
    if addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(
            "sitemap submission destination resolves to a private, local, reserved, or multicast address"
                .to_string(),
        );
    }

    Ok(ResolvedSitemapSubmitTarget {
        url: target.url,
        host: target.host,
        address: addresses[0],
    })
}

fn insecure_http_allowed() -> bool {
    cfg!(test)
        || std::env::var(ALLOW_INSECURE_HTTP_ENV)
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || a == 0
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 192 && b == 0)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && ip.octets()[2] == 100)
        || (a == 203 && b == 0 && ip.octets()[2] == 113))
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if let Some(ipv4) = ip.to_ipv4() {
        return is_public_ipv4(ipv4);
    }

    let segments = ip.segments();
    let first = segments[0];
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
        || (first & 0xffc0) == 0xfec0
        || (first & 0xff00) == 0xff00
        || (segments[0] == 0x0064 && segments[1] == 0xff9b)
        || (segments[0] == 0x0100 && segments[1] == 0)
        || (segments[0] == 0x2001 && segments[1] == 0)
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || segments[0] == 0x2002)
}

#[cfg(test)]
mod tests {
    use super::{is_public_ip, validate_sitemap_submit_target};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn rejects_private_metadata_and_local_destinations() {
        for url in [
            "http://127.0.0.1/ping",
            "http://10.0.0.1/ping",
            "http://169.254.169.254/latest/meta-data",
            "http://[::1]/ping",
            "https://service.internal/ping",
            "https://service.local/ping",
        ] {
            assert!(
                validate_sitemap_submit_target(url).is_err(),
                "{url} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_userinfo_and_non_http_schemes() {
        assert!(validate_sitemap_submit_target("https://user:secret@example.com/ping").is_err());
        assert!(validate_sitemap_submit_target("file:///etc/passwd").is_err());
    }

    #[test]
    fn canonicalizes_public_destination_host() {
        let target = validate_sitemap_submit_target("https://Example.COM./ping")
            .expect("public HTTPS destination should pass static validation");
        assert_eq!(target.host, "example.com");
        assert_eq!(target.url.host_str(), Some("example.com"));
        assert_eq!(target.port, 443);
    }

    #[test]
    fn public_ip_policy_blocks_reserved_ranges() {
        for ip in [
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6("fd00::1".parse().expect("valid IPv6")),
            IpAddr::V6("::ffff:127.0.0.1".parse().expect("valid mapped IPv6")),
        ] {
            assert!(!is_public_ip(ip), "{ip} must be blocked");
        }
        assert!(is_public_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }
}

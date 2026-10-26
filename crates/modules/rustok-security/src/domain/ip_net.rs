/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use crate::error::{SecurityError, SecurityResult};

/// Representation of an IP network prefix or single IP address (CIDR).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IpNet {
    V4 { network: u32, mask: u32, prefix_len: u8 },
    V6 { network: u128, mask: u128, prefix_len: u8 },
}

impl IpNet {
    /// Parses an IP address or CIDR string (e.g., "192.168.1.1", "10.0.0.0/8", "2001:db8::/32").
    pub fn parse(s: &str) -> SecurityResult<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(SecurityError::Validation("IP/CIDR string cannot be empty".to_string()));
        }

        if let Some((ip_str, prefix_str)) = trimmed.split_once('/') {
            let prefix_len: u8 = prefix_str
                .parse()
                .map_err(|_| SecurityError::Validation(format!("Invalid CIDR prefix length: '{}'", prefix_str)))?;

            if let Ok(v4) = Ipv4Addr::from_str(ip_str) {
                if prefix_len > 32 {
                    return Err(SecurityError::Validation(format!(
                        "IPv4 CIDR prefix must be between 0 and 32, got {}",
                        prefix_len
                    )));
                }
                let net_u32 = u32::from_be_bytes(v4.octets());
                let mask = if prefix_len == 0 {
                    0
                } else {
                    !0u32 << (32 - prefix_len)
                };
                Ok(Self::V4 {
                    network: net_u32 & mask,
                    mask,
                    prefix_len,
                })
            } else if let Ok(v6) = Ipv6Addr::from_str(ip_str) {
                if prefix_len > 128 {
                    return Err(SecurityError::Validation(format!(
                        "IPv6 CIDR prefix must be between 0 and 128, got {}",
                        prefix_len
                    )));
                }
                let net_u128 = u128::from_be_bytes(v6.octets());
                let mask = if prefix_len == 0 {
                    0
                } else {
                    !0u128 << (128 - prefix_len)
                };
                Ok(Self::V6 {
                    network: net_u128 & mask,
                    mask,
                    prefix_len,
                })
            } else {
                Err(SecurityError::Validation(format!("Invalid IP address in CIDR: '{}'", ip_str)))
            }
        } else {
            // Single IP address
            match IpAddr::from_str(trimmed) {
                Ok(IpAddr::V4(v4)) => {
                    let net_u32 = u32::from_be_bytes(v4.octets());
                    Ok(Self::V4 {
                        network: net_u32,
                        mask: !0u32,
                        prefix_len: 32,
                    })
                }
                Ok(IpAddr::V6(v6)) => {
                    let net_u128 = u128::from_be_bytes(v6.octets());
                    Ok(Self::V6 {
                        network: net_u128,
                        mask: !0u128,
                        prefix_len: 128,
                    })
                }
                Err(_) => Err(SecurityError::Validation(format!("Invalid IP address: '{}'", trimmed))),
            }
        }
    }

    /// Checks if a given IP address belongs to this network or matches this IP.
    pub fn contains(&self, ip: &IpAddr) -> bool {
        match (self, ip) {
            (Self::V4 { network, mask, .. }, IpAddr::V4(v4)) => {
                let ip_u32 = u32::from_be_bytes(v4.octets());
                (ip_u32 & mask) == *network
            }
            (Self::V6 { network, mask, .. }, IpAddr::V6(v6)) => {
                let ip_u128 = u128::from_be_bytes(v6.octets());
                (ip_u128 & mask) == *network
            }
            _ => false,
        }
    }

    /// Returns the prefix length.
    pub fn prefix_len(&self) -> u8 {
        match self {
            Self::V4 { prefix_len, .. } => *prefix_len,
            Self::V6 { prefix_len, .. } => *prefix_len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_ipv4() {
        let net = IpNet::parse("192.168.1.10").unwrap();
        assert!(net.contains(&"192.168.1.10".parse().unwrap()));
        assert!(!net.contains(&"192.168.1.11".parse().unwrap()));
        assert!(!net.contains(&"::1".parse().unwrap()));
    }

    #[test]
    fn test_cidr_ipv4() {
        let net = IpNet::parse("192.168.1.0/24").unwrap();
        assert!(net.contains(&"192.168.1.0".parse().unwrap()));
        assert!(net.contains(&"192.168.1.100".parse().unwrap()));
        assert!(net.contains(&"192.168.1.255".parse().unwrap()));
        assert!(!net.contains(&"192.168.2.1".parse().unwrap()));
    }

    #[test]
    fn test_cidr_ipv6() {
        let net = IpNet::parse("2001:db8::/32").unwrap();
        assert!(net.contains(&"2001:db8::1".parse().unwrap()));
        assert!(net.contains(&"2001:db8:ffff:ffff::1".parse().unwrap()));
        assert!(!net.contains(&"2001:db9::1".parse().unwrap()));
        assert!(!net.contains(&"192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn test_invalid_cidrs() {
        assert!(IpNet::parse("").is_err());
        assert!(IpNet::parse("not-an-ip").is_err());
        assert!(IpNet::parse("192.168.1.1/33").is_err());
        assert!(IpNet::parse("2001:db8::/129").is_err());
        assert!(IpNet::parse("192.168.1.1/abc").is_err());
    }
}

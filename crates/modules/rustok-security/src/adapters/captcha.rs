/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use crate::error::{SecurityError, SecurityResult};
use crate::ports::CaptchaVerifierPort;
use async_trait::async_trait;
use std::net::IpAddr;

/// Zero-dependency Honeypot verifier.
/// Expects token in the format "timestamp_ms:honeypot_value".
/// Fails if honeypot_value is non-empty or if the submission occurred faster than min_duration_ms.
#[derive(Clone, Debug)]
pub struct HoneypotVerifier {
    pub min_duration_ms: u64,
}

impl Default for HoneypotVerifier {
    fn default() -> Self {
        Self {
            min_duration_ms: 1500, // minimum 1.5 seconds for a human to fill a form
        }
    }
}

#[async_trait]
impl CaptchaVerifierPort for HoneypotVerifier {
    async fn verify(&self, token: &str, _remote_ip: Option<&IpAddr>) -> SecurityResult<bool> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(SecurityError::CaptchaFailed(
                "Empty honeypot token".to_string(),
            ));
        }

        // Expected format: "<rendered_timestamp_ms>:<honeypot_field_value>"
        if let Some((rendered_ts_str, trap_value)) = trimmed.split_once(':') {
            // Trap must be empty!
            if !trap_value.is_empty() {
                return Ok(false);
            }

            let rendered_ts: u64 = rendered_ts_str.parse().map_err(|_| {
                SecurityError::CaptchaFailed("Malformed timestamp in honeypot".to_string())
            })?;

            let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
            let elapsed_ms = now_ms.saturating_sub(rendered_ts);

            if elapsed_ms < self.min_duration_ms {
                // Form submitted impossibly fast for a human
                return Ok(false);
            }

            Ok(true)
        } else {
            // Simple presence check fallback
            Ok(!trimmed.is_empty())
        }
    }

    fn provider_name(&self) -> &'static str {
        "honeypot"
    }
}

/// No-op verifier for testing and development environments.
#[derive(Clone, Debug, Default)]
pub struct NoopCaptchaVerifier;

#[async_trait]
impl CaptchaVerifierPort for NoopCaptchaVerifier {
    async fn verify(&self, _token: &str, _remote_ip: Option<&IpAddr>) -> SecurityResult<bool> {
        Ok(true)
    }

    fn provider_name(&self) -> &'static str {
        "noop"
    }
}

#[cfg(feature = "captcha-http")]
pub mod http_adapters {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct VerificationApiResponse {
        success: bool,
        #[serde(rename = "error-codes")]
        error_codes: Option<Vec<String>>,
    }

    /// Cloudflare Turnstile verifier using HTTP client.
    #[derive(Clone)]
    pub struct TurnstileVerifier {
        client: reqwest::Client,
        secret_key: String,
    }

    impl TurnstileVerifier {
        pub fn new(secret_key: impl Into<String>) -> Self {
            Self {
                client: reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                    .unwrap_or_default(),
                secret_key: secret_key.into(),
            }
        }
    }

    #[async_trait]
    impl CaptchaVerifierPort for TurnstileVerifier {
        async fn verify(&self, token: &str, remote_ip: Option<&IpAddr>) -> SecurityResult<bool> {
            let mut params = vec![("secret", self.secret_key.as_str()), ("response", token)];
            let ip_str;
            if let Some(ip) = remote_ip {
                ip_str = ip.to_string();
                params.push(("remoteip", &ip_str));
            }

            let response = self
                .client
                .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
                .form(&params)
                .send()
                .await
                .map_err(|e| {
                    SecurityError::Internal(format!("Turnstile HTTP request failed: {}", e))
                })?;

            let body: VerificationApiResponse = response.json().await.map_err(|e| {
                SecurityError::Internal(format!("Failed to parse Turnstile response: {}", e))
            })?;

            if !body.success {
                tracing::warn!(
                    error_codes = ?body.error_codes,
                    "Cloudflare Turnstile token validation failed"
                );
            }

            Ok(body.success)
        }

        fn provider_name(&self) -> &'static str {
            "turnstile"
        }
    }

    /// Google reCAPTCHA v2 / v3 verifier using HTTP client.
    #[derive(Clone)]
    pub struct RecaptchaVerifier {
        client: reqwest::Client,
        secret_key: String,
    }

    impl RecaptchaVerifier {
        pub fn new(secret_key: impl Into<String>) -> Self {
            Self {
                client: reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                    .unwrap_or_default(),
                secret_key: secret_key.into(),
            }
        }
    }

    #[async_trait]
    impl CaptchaVerifierPort for RecaptchaVerifier {
        async fn verify(&self, token: &str, remote_ip: Option<&IpAddr>) -> SecurityResult<bool> {
            let mut params = vec![("secret", self.secret_key.as_str()), ("response", token)];
            let ip_str;
            if let Some(ip) = remote_ip {
                ip_str = ip.to_string();
                params.push(("remoteip", &ip_str));
            }

            let response = self
                .client
                .post("https://www.google.com/recaptcha/api/siteverify")
                .form(&params)
                .send()
                .await
                .map_err(|e| {
                    SecurityError::Internal(format!("reCAPTCHA HTTP request failed: {}", e))
                })?;

            let body: VerificationApiResponse = response.json().await.map_err(|e| {
                SecurityError::Internal(format!("Failed to parse reCAPTCHA response: {}", e))
            })?;

            Ok(body.success)
        }

        fn provider_name(&self) -> &'static str {
            "recaptcha"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_verifier_always_succeeds() {
        let verifier = NoopCaptchaVerifier;
        assert!(verifier.verify("any_token", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_honeypot_traps_bot() {
        let verifier = HoneypotVerifier::default();
        let now_ms = chrono::Utc::now().timestamp_millis() - 5000;

        // Bot filled the trap field
        let token_bot = format!("{}:i_am_a_bot", now_ms);
        assert!(!verifier.verify(&token_bot, None).await.unwrap());

        // Human left trap field empty and took > 1.5s
        let token_human = format!("{}:", now_ms);
        assert!(verifier.verify(&token_human, None).await.unwrap());

        // Human submitted in 100ms (too fast)
        let token_too_fast = format!("{}:", chrono::Utc::now().timestamp_millis() - 100);
        assert!(!verifier.verify(&token_too_fast, None).await.unwrap());
    }
}

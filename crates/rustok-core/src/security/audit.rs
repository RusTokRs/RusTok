//! Security Audit Logging - OWASP Logging Failures Prevention
//!
//! Comprehensive security event logging for:
//! - Authentication events (login, logout, failed attempts)
//! - Authorization events (access denied, privilege escalation)
//! - Data access events (sensitive data access)
//! - System events (configuration changes, errors)
//! - Security violations (rate limiting, validation failures)
//!
//! SIEM integration supports forwarding events to external systems via HTTP
//! webhook (compatible with Splunk HEC, Logstash, Datadog Logs API, and any
//! generic HTTP endpoint that accepts JSON payloads).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};
use uuid::Uuid;

use super::{SecurityCategory, SecurityFinding, Severity};
use crate::security::SecurityConfig;

// ─── SIEM Configuration ──────────────────────────────────────────────────────

/// Configuration for forwarding audit events to an external SIEM system.
///
/// Supported transport: HTTP webhook compatible with Splunk HEC, Logstash,
/// Datadog Logs, and any system accepting JSON POST requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SiemConfig {
    /// SIEM forwarding disabled (default)
    Disabled,

    /// Forward events to an HTTP webhook endpoint
    Webhook(SiemWebhookConfig),
}

impl Default for SiemConfig {
    fn default() -> Self {
        Self::Disabled
    }
}

/// HTTP webhook configuration for SIEM forwarding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiemWebhookConfig {
    /// Destination URL — e.g. `https://splunk:8088/services/collector/event`
    /// or `https://http-intake.logs.datadoghq.com/api/v2/logs`
    pub url: String,

    /// Optional `Authorization` header value (e.g. `"Splunk <token>"`, `"Bearer <api_key>"`)
    pub auth_header: Option<String>,

    /// Additional static headers to include in every request (e.g. `DD-API-KEY`)
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,

    /// How many events to accumulate before flushing in a single HTTP call
    /// (minimum 1, default 10)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// HTTP request timeout in seconds (default 5)
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_batch_size() -> usize {
    10
}
fn default_timeout_secs() -> u64 {
    5
}

// ─── SIEM Exporter ───────────────────────────────────────────────────────────

/// Forwards batches of `AuditEvent`s to an HTTP webhook.
struct SiemWebhookExporter {
    client: reqwest::Client,
    config: SiemWebhookConfig,
}

impl SiemWebhookExporter {
    fn new(config: SiemWebhookConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();

        Self { client, config }
    }

    /// Send a batch of events to the webhook.
    ///
    /// Uses a JSON array payload — compatible with Logstash and Datadog.
    /// For Splunk HEC the caller should use a single-event envelope; this
    /// implementation wraps events in an array which works with most SIEM
    /// HTTP inputs.
    async fn send_batch(&self, events: &[AuditEvent]) {
        if events.is_empty() {
            return;
        }

        let payload = match serde_json::to_value(events) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "SIEM: failed to serialize audit events");
                return;
            }
        };

        let mut req = self
            .client
            .post(&self.config.url)
            .json(&payload);

        if let Some(auth) = &self.config.auth_header {
            req = req.header(reqwest::header::AUTHORIZATION, auth);
        }

        for (k, v) in &self.config.extra_headers {
            req = req.header(k.as_str(), v.as_str());
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!(
                    url = %self.config.url,
                    count = events.len(),
                    "SIEM: batch sent successfully"
                );
            }
            Ok(resp) => {
                warn!(
                    url = %self.config.url,
                    status = %resp.status(),
                    count = events.len(),
                    "SIEM: webhook returned non-success status"
                );
            }
            Err(e) => {
                warn!(
                    url = %self.config.url,
                    error = %e,
                    count = events.len(),
                    "SIEM: failed to send audit events to webhook"
                );
            }
        }
    }
}

/// Security audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: AuditEventType,
    /// Severity level
    pub severity: Severity,
    /// User ID (if authenticated)
    pub user_id: Option<Uuid>,
    /// Tenant ID
    pub tenant_id: Option<Uuid>,
    /// IP address
    pub ip_address: Option<IpAddr>,
    /// Request ID
    pub request_id: Option<String>,
    /// Event description
    pub description: String,
    /// Additional context
    pub context: HashMap<String, String>,
    /// Resource being accessed
    pub resource: Option<String>,
    /// Action performed
    pub action: Option<String>,
    /// Success or failure
    pub success: bool,
}

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    Authentication,
    Authorization,
    DataAccess,
    DataModification,
    ConfigurationChange,
    SecurityViolation,
    SystemError,
    RateLimitExceeded,
    ValidationFailure,
    SuspiciousActivity,
}

impl AuditEvent {
    /// Create new audit event
    pub fn new(event_type: AuditEventType) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            severity: Severity::Info,
            user_id: None,
            tenant_id: None,
            ip_address: None,
            request_id: None,
            description: String::new(),
            context: HashMap::new(),
            resource: None,
            action: None,
            success: true,
        }
    }

    /// Set severity
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Set user ID
    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Set tenant ID
    pub fn with_tenant(mut self, tenant_id: Uuid) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    /// Set IP address
    pub fn with_ip(mut self, ip: IpAddr) -> Self {
        self.ip_address = Some(ip);
        self
    }

    /// Set request ID
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set resource
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    /// Set action
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// Set success
    pub fn with_success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    /// Add context
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Create authentication event
    pub fn authentication(user_id: Uuid, success: bool, ip: IpAddr) -> Self {
        let severity = if success {
            Severity::Info
        } else {
            Severity::Warning
        };
        Self::new(AuditEventType::Authentication)
            .with_user(user_id)
            .with_ip(ip)
            .with_success(success)
            .with_severity(severity)
            .with_description(if success {
                "User authenticated successfully"
            } else {
                "Authentication failed"
            })
    }

    /// Create authorization failure event
    pub fn authorization_denied(user_id: Uuid, resource: &str, action: &str, ip: IpAddr) -> Self {
        Self::new(AuditEventType::Authorization)
            .with_user(user_id)
            .with_resource(resource.to_string())
            .with_action(action.to_string())
            .with_ip(ip)
            .with_success(false)
            .with_severity(Severity::Warning)
            .with_description(format!(
                "Access denied to {} for action {}",
                resource, action
            ))
    }

    /// Create rate limit event
    pub fn rate_limit_exceeded(ip: IpAddr, identifier: &str) -> Self {
        Self::new(AuditEventType::RateLimitExceeded)
            .with_ip(ip)
            .with_severity(Severity::Warning)
            .with_description(format!("Rate limit exceeded for {}", identifier))
            .with_context("identifier", identifier)
    }

    /// Create validation failure event
    pub fn validation_failure(ip: IpAddr, reason: &str) -> Self {
        Self::new(AuditEventType::ValidationFailure)
            .with_ip(ip)
            .with_severity(Severity::Warning)
            .with_description(format!("Input validation failed: {}", reason))
    }

    /// Create suspicious activity event
    pub fn suspicious_activity(ip: IpAddr, description: &str) -> Self {
        Self::new(AuditEventType::SuspiciousActivity)
            .with_ip(ip)
            .with_severity(Severity::High)
            .with_description(description.to_string())
    }
}

/// Security audit logger
#[derive(Debug, Clone)]
pub struct AuditLogger {
    sender: mpsc::Sender<AuditEvent>,
    enabled: bool,
}

impl AuditLogger {
    /// Create new audit logger
    pub fn new(enabled: bool) -> (Self, mpsc::Receiver<AuditEvent>) {
        let (sender, receiver) = mpsc::channel(1000);
        (Self { sender, enabled }, receiver)
    }

    /// Log an audit event
    pub async fn log(&self, event: AuditEvent) {
        if !self.enabled {
            return;
        }

        // Send to channel (non-blocking)
        if self.sender.try_send(event.clone()).is_err() {
            // Channel full, log directly
            self.write_to_log(&event);
        }
    }

    /// Log authentication event
    pub async fn log_auth(&self, user_id: Uuid, success: bool, ip: IpAddr) {
        self.log(AuditEvent::authentication(user_id, success, ip))
            .await;
    }

    /// Log authorization failure
    pub async fn log_authz_denied(&self, user_id: Uuid, resource: &str, action: &str, ip: IpAddr) {
        self.log(AuditEvent::authorization_denied(
            user_id, resource, action, ip,
        ))
        .await;
    }

    /// Log rate limit exceeded
    pub async fn log_rate_limit(&self, ip: IpAddr, identifier: &str) {
        self.log(AuditEvent::rate_limit_exceeded(ip, identifier))
            .await;
    }

    /// Log validation failure
    pub async fn log_validation_failure(&self, ip: IpAddr, reason: &str) {
        self.log(AuditEvent::validation_failure(ip, reason)).await;
    }

    /// Log suspicious activity
    pub async fn log_suspicious(&self, ip: IpAddr, description: &str) {
        self.log(AuditEvent::suspicious_activity(ip, description))
            .await;
    }

    fn write_to_log(&self, event: &AuditEvent) {
        let json = serde_json::to_string(event).unwrap_or_default();

        match event.severity {
            Severity::Critical | Severity::High => {
                warn!(target: "security_audit", "{}", json);
            }
            _ => {
                info!(target: "security_audit", "{}", json);
            }
        }
    }
}

/// Security audit processor
pub struct SecurityAudit {
    logger: AuditLogger,
    receiver: mpsc::Receiver<AuditEvent>,
    siem: SiemConfig,
}

impl SecurityAudit {
    /// Create new security audit (SIEM disabled)
    pub fn new(enabled: bool) -> Self {
        Self::with_siem(enabled, SiemConfig::Disabled)
    }

    /// Create new security audit with optional SIEM forwarding
    pub fn with_siem(enabled: bool, siem: SiemConfig) -> Self {
        let (logger, receiver) = AuditLogger::new(enabled);
        Self {
            logger,
            receiver,
            siem,
        }
    }

    /// Get logger instance
    pub fn logger(&self) -> &AuditLogger {
        &self.logger
    }

    /// Process audit events (run in background task).
    ///
    /// Events are:
    /// 1. Written to local structured tracing (`security_audit` target)
    /// 2. Forwarded in batches to the configured SIEM endpoint (if any)
    pub async fn run(mut self) {
        let exporter = match &self.siem {
            SiemConfig::Webhook(cfg) => {
                tracing::info!(
                    url = %cfg.url,
                    batch_size = cfg.batch_size,
                    "SIEM: webhook exporter enabled"
                );
                Some(SiemWebhookExporter::new(cfg.clone()))
            }
            SiemConfig::Disabled => None,
        };

        let batch_size = match &self.siem {
            SiemConfig::Webhook(cfg) => cfg.batch_size.max(1),
            SiemConfig::Disabled => 1,
        };

        let mut batch: Vec<AuditEvent> = Vec::with_capacity(batch_size);

        while let Some(event) = self.receiver.recv().await {
            // 1. Write to local structured log
            let json = serde_json::to_string(&event).unwrap_or_default();
            match event.severity {
                Severity::Critical | Severity::High => {
                    warn!(target: "security_audit", "{}", json);
                }
                _ => {
                    info!(target: "security_audit", "{}", json);
                }
            }

            // 2. Accumulate for SIEM batch
            if let Some(ref exp) = exporter {
                batch.push(event);
                if batch.len() >= batch_size {
                    exp.send_batch(&batch).await;
                    batch.clear();
                }
            }
        }

        // Flush remaining events when channel closes
        if let Some(ref exp) = exporter {
            if !batch.is_empty() {
                exp.send_batch(&batch).await;
            }
        }
    }

    /// Run processor for a single event (for testing)
    pub async fn process_one(&mut self) -> Option<AuditEvent> {
        self.receiver.recv().await
    }
}

/// Audit logging configuration check
pub async fn audit_logging(config: &SecurityConfig) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();

    if !config.audit_logging {
        findings.push(SecurityFinding {
            category: SecurityCategory::LoggingFailures,
            severity: Severity::High,
            description: "Security audit logging is disabled".to_string(),
            remediation: "Enable audit_logging for security event tracking".to_string(),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_audit_event_builder() {
        let event = AuditEvent::new(AuditEventType::Authentication)
            .with_user(Uuid::new_v4())
            .with_description("Test event")
            .with_success(true)
            .with_severity(Severity::Info);

        assert!(event.success);
        assert_eq!(event.severity, Severity::Info);
    }

    #[test]
    fn test_authentication_event() {
        let user_id = Uuid::new_v4();
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        let event = AuditEvent::authentication(user_id, true, ip);

        assert_eq!(event.event_type, AuditEventType::Authentication);
        assert!(event.success);
        assert_eq!(event.user_id, Some(user_id));
    }

    #[test]
    fn test_authorization_denied_event() {
        let user_id = Uuid::new_v4();
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        let event = AuditEvent::authorization_denied(user_id, "/admin", "DELETE", ip);

        assert_eq!(event.event_type, AuditEventType::Authorization);
        assert!(!event.success);
        assert_eq!(event.severity, Severity::Warning);
    }

    #[tokio::test]
    async fn test_audit_logger() {
        let (logger, mut receiver) = AuditLogger::new(true);

        let event = AuditEvent::new(AuditEventType::SystemError).with_description("Test error");

        logger.log(event.clone()).await;

        let received = receiver.recv().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().description, "Test error");
    }
}

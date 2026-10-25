use rustok_core::Result;
use rustok_iggy_connector::IggyConnector;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub status: HealthStatus,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl HealthCheckResult {
    pub fn healthy(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Healthy,
            message: message.into(),
            details: None,
        }
    }

    pub fn degraded(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Degraded,
            message: message.into(),
            details: None,
        }
    }

    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

pub async fn health_check(connector: &dyn IggyConnector) -> Result<HealthCheckResult> {
    if !connector.is_connected() {
        return Ok(HealthCheckResult::unhealthy("Connector not connected"));
    }

    info!("Iggy health check passed");
    Ok(
        HealthCheckResult::healthy("Iggy transport is healthy").with_details(serde_json::json!({
            "connected": true,
            "mode": "local_or_remote"
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "unhealthy");
    }

    #[test]
    fn health_check_result_builders() {
        let healthy = HealthCheckResult::healthy("All good");
        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert_eq!(healthy.message, "All good");
        assert!(healthy.details.is_none());

        let degraded = HealthCheckResult::degraded("Some issues");
        assert_eq!(degraded.status, HealthStatus::Degraded);

        let unhealthy = HealthCheckResult::unhealthy("Critical failure");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn health_check_result_with_details() {
        let result =
            HealthCheckResult::healthy("OK").with_details(serde_json::json!({ "latency_ms": 5 }));

        assert!(result.details.is_some());
        let details = result.details.unwrap();
        assert_eq!(details["latency_ms"], 5);
    }
}

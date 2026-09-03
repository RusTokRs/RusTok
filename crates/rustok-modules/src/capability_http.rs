use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, Method};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityName, CapabilityResponse,
    SandboxError, SandboxResult,
};

use crate::artifact_capability_router::{
    resolve_granted_artifact_capability, ArtifactCapabilityBrokerResolver,
    ArtifactCapabilityExecution,
};

/// Production broker for the `platform.http` sandbox capability.
///
/// Executes outbound HTTP requests on behalf of admitted guest modules within
/// the bounds enforced by `HttpCapabilityConstraints`.
#[derive(Clone)]
pub struct ArtifactHttpCapabilityBroker {
    client: Client,
    timeout: Duration,
}

impl ArtifactHttpCapabilityBroker {
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(10))
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self { client, timeout }
    }
}

impl Default for ArtifactHttpCapabilityBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CapabilityBroker for ArtifactHttpCapabilityBroker {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        let input = call
            .input
            .as_object()
            .ok_or_else(|| SandboxError::InvalidRequest("HTTP input must be an object".into()))?;

        let method_str = input
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| SandboxError::InvalidRequest("HTTP method is required".into()))?;

        let url_str = input
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| SandboxError::InvalidRequest("HTTP url is required".into()))?;

        let method = match method_str.to_ascii_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "PATCH" => Method::PATCH,
            "DELETE" => Method::DELETE,
            "HEAD" => Method::HEAD,
            "OPTIONS" => Method::OPTIONS,
            other => {
                return Err(SandboxError::InvalidRequest(format!(
                    "unsupported HTTP method `{other}`"
                )));
            }
        };

        let mut request_builder = self.client.request(method, url_str).timeout(self.timeout);

        if let Some(headers_obj) = input.get("headers").and_then(Value::as_object) {
            for (key, val) in headers_obj {
                if let Some(val_str) = val.as_str() {
                    request_builder = request_builder.header(key.as_str(), val_str);
                }
            }
        }

        if let Some(body_val) = input.get("body") {
            if let Some(body_str) = body_val.as_str() {
                request_builder = request_builder.body(body_str.to_string());
            } else if !body_val.is_null() {
                request_builder = request_builder.json(body_val);
            }
        }

        let response = request_builder.send().await.map_err(|err| {
            SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("HTTP request failed: {err}"),
            }
        })?;

        let status = response.status().as_u16();
        let mut response_headers = HashMap::new();
        for (key, val) in response.headers() {
            if let Ok(val_str) = val.to_str() {
                response_headers.insert(key.as_str().to_string(), val_str.to_string());
            }
        }

        let body_text = response.text().await.map_err(|err| {
            SandboxError::HostCapability {
                capability: call.capability.clone(),
                message: format!("reading HTTP response body failed: {err}"),
            }
        })?;

        Ok(CapabilityResponse {
            output: json!({
                "status": status,
                "headers": response_headers,
                "body": body_text,
            }),
        })
    }
}

/// Deployment-owned resolver for the `platform.http` capability broker.
#[derive(Clone)]
pub struct SeaOrmArtifactHttpCapabilityBrokerResolver {
    db: DatabaseConnection,
    broker: Arc<ArtifactHttpCapabilityBroker>,
}

impl SeaOrmArtifactHttpCapabilityBrokerResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            broker: Arc::new(ArtifactHttpCapabilityBroker::new()),
        }
    }

    pub fn with_timeout(db: DatabaseConnection, timeout: Duration) -> Self {
        Self {
            db,
            broker: Arc::new(ArtifactHttpCapabilityBroker::with_timeout(timeout)),
        }
    }
}

#[async_trait]
impl ArtifactCapabilityBrokerResolver for SeaOrmArtifactHttpCapabilityBrokerResolver {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.http" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        Ok(self.broker.clone())
    }
}

use async_trait::async_trait;
use serde_json::Value;
use tracing::info;

use super::{StepContext, StepOutput, WorkflowStep};
use crate::error::{WorkflowError, WorkflowResult};

/// HTTP step — performs an outbound HTTP request.
///
/// Step config format:
/// ```json
/// {
///   "method": "POST",
///   "url": "https://api.example.com/webhook",
///   "headers": { "Authorization": "Bearer {{context.token}}" },
///   "body": { "order_id": "{{context.order_id}}" },
///   "timeout_secs": 30
/// }
/// ```
pub struct HttpStep {
    client: reqwest::Client,
}

impl HttpStep {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl Default for HttpStep {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkflowStep for HttpStep {
    fn step_type(&self) -> &'static str {
        "http"
    }

    async fn execute(&self, config: &Value, context: StepContext) -> WorkflowResult<StepOutput> {
        let method = config
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("POST")
            .to_uppercase();

        let url = config
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| WorkflowError::InvalidStepConfig("http: missing 'url'".into()))?;

        let timeout_secs = config
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(30);

        info!(method = %method, url = url, "Executing http step");

        let mut req = match method.as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "PATCH" => self.client.patch(url),
            "DELETE" => self.client.delete(url),
            m => {
                return Err(WorkflowError::InvalidStepConfig(format!(
                    "http: unsupported method '{m}'"
                )))
            }
        };

        req = req.timeout(std::time::Duration::from_secs(timeout_secs));

        // Set headers
        if let Some(headers) = config.get("headers").and_then(Value::as_object) {
            for (key, val) in headers {
                if let Some(v) = val.as_str() {
                    req = req.header(key.as_str(), v);
                }
            }
        }

        // Set body for non-GET requests
        if method != "GET" {
            if let Some(body) = config.get("body") {
                req = req.json(body);
            }
        }

        let resp = req
            .send()
            .await
            .map_err(|e| WorkflowError::StepFailed(format!("http: request failed: {e}")))?;

        let status = resp.status().as_u16();
        let ok = resp.status().is_success();

        let body: Value = resp.json().await.unwrap_or(Value::Null);

        if !ok {
            return Err(WorkflowError::StepFailed(format!(
                "http: request returned status {status}"
            )));
        }

        let mut new_context = context.clone();
        new_context.set("http_response", body.clone());

        Ok(StepOutput::continue_with(
            new_context,
            serde_json::json!({ "status": status, "body": body }),
        ))
    }
}

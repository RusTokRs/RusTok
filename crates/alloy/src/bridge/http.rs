use std::collections::HashMap;

use rhai::{Dynamic, Engine, Map};
use rustok_sandbox::rhai::RhaiHostExtension;
use rustok_sandbox::{
    CapabilityCall, CapabilityName, SandboxError, SandboxHost, SandboxRequest, SandboxSubject,
};
use serde_json::{Value, json};

use crate::utils::{dynamic_to_json, json_to_dynamic};

const HTTP_CAPABILITY: &str = "platform.http";

/// Registers broker-backed HTTP functions for a single sandbox request.
///
/// This adapter deliberately has no HTTP client. The platform capability broker
/// owns egress allowlists, credentials, rate limits and audit records for Alloy
/// drafts and marketplace artifacts alike.
#[derive(Debug, Default)]
pub struct HttpCapabilityBridge;

impl RhaiHostExtension for HttpCapabilityBridge {
    fn register(&self, engine: &mut Engine, request: &SandboxRequest, host: SandboxHost) {
        let context = CapabilityContext::from_request(request);
        register_http(engine, host, context);
    }
}

#[derive(Clone)]
struct CapabilityContext {
    execution_id: uuid::Uuid,
    subject: SandboxSubject,
}

impl CapabilityContext {
    fn from_request(request: &SandboxRequest) -> Self {
        Self {
            execution_id: request.context.execution_id,
            subject: request.subject.clone(),
        }
    }
}

fn register_http(engine: &mut Engine, host: SandboxHost, context: CapabilityContext) {
    let get_host = host.clone();
    let get_context = context.clone();
    engine.register_fn("http_get", move |url: &str| {
        invoke_http(
            &get_host,
            &get_context,
            "GET",
            url,
            Value::Null,
            HashMap::new(),
        )
    });

    let get_headers_host = host.clone();
    let get_headers_context = context.clone();
    engine.register_fn("http_get", move |url: &str, headers: Map| {
        invoke_http(
            &get_headers_host,
            &get_headers_context,
            "GET",
            url,
            Value::Null,
            extract_headers(headers),
        )
    });

    let post_host = host.clone();
    let post_context = context.clone();
    engine.register_fn("http_post", move |url: &str, body: Dynamic| {
        invoke_http(
            &post_host,
            &post_context,
            "POST",
            url,
            dynamic_to_json(body),
            HashMap::new(),
        )
    });

    let post_headers_host = host.clone();
    let post_headers_context = context.clone();
    engine.register_fn(
        "http_post",
        move |url: &str, body: Dynamic, headers: Map| {
            invoke_http(
                &post_headers_host,
                &post_headers_context,
                "POST",
                url,
                dynamic_to_json(body),
                extract_headers(headers),
            )
        },
    );

    engine.register_fn(
        "http_request",
        move |method: &str, url: &str, body: Dynamic, headers: Map| {
            invoke_http(
                &host,
                &context,
                &method.to_ascii_uppercase(),
                url,
                dynamic_to_json(body),
                extract_headers(headers),
            )
        },
    );
}

fn invoke_http(
    host: &SandboxHost,
    context: &CapabilityContext,
    method: &str,
    url: &str,
    body: Value,
    headers: HashMap<String, String>,
) -> Map {
    let capability = match CapabilityName::new(HTTP_CAPABILITY) {
        Ok(capability) => capability,
        Err(error) => return capability_error_map(error),
    };
    let call = CapabilityCall {
        execution_id: context.execution_id,
        subject: context.subject.clone(),
        capability,
        operation: "request".to_string(),
        input: json!({
            "method": method,
            "url": url,
            "headers": headers,
            "body": body,
        }),
    };

    match invoke_broker(host.clone(), call) {
        Ok(output) => response_map(output),
        Err(error) => capability_error_map(error),
    }
}

fn invoke_broker(host: SandboxHost, call: CapabilityCall) -> Result<Value, SandboxError> {
    host.invoke_blocking(&call).map(|response| response.output)
}

fn response_map(output: Value) -> Map {
    let body = json_to_dynamic(output);
    if body.is_map() {
        body.cast::<Map>()
    } else {
        let mut map = Map::new();
        map.insert("ok".into(), Dynamic::from(true));
        map.insert("body".into(), body);
        map
    }
}

fn capability_error_map(error: SandboxError) -> Map {
    let mut map = Map::new();
    map.insert("ok".into(), Dynamic::from(false));
    map.insert("status".into(), Dynamic::from(0_i64));
    map.insert("error_code".into(), Dynamic::from(error.code().to_string()));
    map.insert("error".into(), Dynamic::from(error.to_string()));
    map
}

fn extract_headers(headers: Map) -> HashMap<String, String> {
    headers
        .into_iter()
        .filter_map(|(key, value)| value.try_cast::<String>().map(|v| (key.to_string(), v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use chrono::Utc;
    use rustok_sandbox::{
        CapabilityBroker, CapabilityGrant, CapabilityResponse, ExecutionPhase, ExecutorRegistry,
        SandboxContext, SandboxPayload, SandboxPolicy, SandboxRuntime,
    };
    use serde_json::json;

    use super::*;

    #[derive(Default)]
    struct CapturingBroker(Mutex<Vec<CapabilityCall>>);

    #[async_trait]
    impl CapabilityBroker for CapturingBroker {
        async fn invoke(
            &self,
            call: &CapabilityCall,
            _grant: &CapabilityGrant,
        ) -> Result<CapabilityResponse, SandboxError> {
            self.0.lock().expect("calls lock").push(call.clone());
            Ok(CapabilityResponse {
                output: json!({ "ok": true, "status": 200, "body": { "source": "broker" } }),
            })
        }
    }

    fn request(granted: bool) -> SandboxRequest {
        SandboxRequest {
            subject: SandboxSubject::AlloyDraft {
                draft_id: uuid::Uuid::new_v4(),
                revision: 1,
            },
            context: SandboxContext {
                execution_id: uuid::Uuid::new_v4(),
                phase: ExecutionPhase::Test,
                timestamp: Utc::now(),
                tenant_id: None,
                actor_id: None,
                trace_id: None,
            },
            payload: SandboxPayload {
                executor: rustok_sandbox::SandboxExecutorKind::Rhai,
                media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
                digest: "sha256:test".to_string(),
                entrypoint: "main".to_string(),
                bytes: b"http_get(\"https://service.example/test\")".to_vec(),
            },
            input: Value::Null,
            policy: SandboxPolicy {
                grants: granted
                    .then(|| CapabilityGrant {
                        name: CapabilityName::new(HTTP_CAPABILITY).expect("capability"),
                        constraints: json!({ "hosts": ["service.example"] }),
                    })
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn http_bridge_uses_the_shared_broker_and_respects_default_deny() {
        let broker = Arc::new(CapturingBroker::default());
        let mut executors = ExecutorRegistry::new();
        executors
            .register(
                rustok_sandbox::rhai::RhaiExecutor::new()
                    .with_extension(Arc::new(HttpCapabilityBridge)),
            )
            .expect("executor");
        let runtime = SandboxRuntime::new(executors, broker.clone());

        let granted = runtime
            .execute(request(true))
            .await
            .expect("granted execution");
        assert_eq!(granted.output["status"], 200);
        assert_eq!(broker.0.lock().expect("calls lock").len(), 1);

        let denied = runtime
            .execute(request(false))
            .await
            .expect("denied script response");
        assert_eq!(denied.output["error_code"], "CAPABILITY_DENIED");
        assert_eq!(broker.0.lock().expect("calls lock").len(), 1);
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::{
    CapabilityBroker, CapabilityBrokerRouter, CapabilityCall, CapabilityCallContext,
    CapabilityGrant, CapabilityName, CapabilityResponse, DataCapabilityConstraints,
    EventCapabilityConstraints, HttpCapabilityConstraints, McpCapabilityConstraints,
    ObjectCapabilityConstraints, SecretReferenceCapabilityConstraints,
};
use crate::{ExecutionPhase, SandboxError, SandboxResult, SandboxSubject};

fn call(operation: &str, input: serde_json::Value) -> CapabilityCall {
    CapabilityCall {
        execution_id: Uuid::nil(),
        subject: SandboxSubject::ModuleArtifact {
            installation_id: Uuid::new_v4(),
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: "sha256:sample".to_string(),
        },
        context: CapabilityCallContext {
            phase: ExecutionPhase::Lifecycle,
            tenant_id: None,
            actor_id: None,
            trace_id: None,
        },
        capability: CapabilityName::new("platform.secrets").expect("capability name"),
        operation: operation.to_string(),
        input,
    }
}

#[test]
fn secret_reference_constraints_allow_only_declared_logical_handle_calls() {
    let grant = CapabilityGrant {
        name: CapabilityName::new("platform.secrets").expect("capability name"),
        constraints: json!({
            "references": ["payment_api"],
            "operations": ["acquire_handle"]
        }),
    };
    let constraints =
        SecretReferenceCapabilityConstraints::from_grant(&grant).expect("valid constraints");

    assert!(
        constraints
            .validate(&call(
                "acquire_handle",
                json!({ "reference": "payment_api" })
            ))
            .is_ok()
    );
    assert!(
        constraints
            .validate(&call("read", json!({ "reference": "payment_api" })))
            .is_err()
    );
    assert!(
        constraints
            .validate(&call("acquire_handle", json!({ "reference": "other" })))
            .is_err()
    );
    assert!(
        constraints
            .validate(&call(
                "acquire_handle",
                json!({ "reference": "payment_api", "resolver": "env" }),
            ))
            .is_err()
    );
}

#[test]
fn event_constraints_allow_only_declared_publish_topics() {
    let grant = CapabilityGrant {
        name: CapabilityName::new("platform.events").expect("capability name"),
        constraints: json!({
            "topics": ["order.*"],
            "operations": ["publish"]
        }),
    };
    let constraints =
        EventCapabilityConstraints::from_grant(&grant).expect("valid event constraints");
    let mut event_call = call(
        "publish",
        json!({ "topic": "order.completed", "payload": {} }),
    );
    event_call.capability = CapabilityName::new("platform.events").expect("capability name");
    assert!(constraints.validate(&event_call).is_ok());

    event_call.input = json!({ "topic": "orders.completed" });
    assert!(constraints.validate(&event_call).is_err());
    event_call.input = json!({ "topic": "order.completed", "resolver": "vault" });
    assert!(constraints.validate(&event_call).is_err());
}

#[test]
fn data_constraints_keep_calls_inside_declared_logical_prefixes() {
    let grant = CapabilityGrant {
        name: CapabilityName::new("platform.data").expect("capability name"),
        constraints: json!({
            "key_prefixes": ["state/"],
            "operations": ["get", "put", "put_batch", "delete", "list", "query_index"]
        }),
    };
    let constraints =
        DataCapabilityConstraints::from_grant(&grant).expect("valid data constraints");
    let mut data_call = call(
        "put",
        json!({
            "key": "state/answer",
            "value": 42,
            "idempotency_key": Uuid::new_v4().to_string()
        }),
    );
    data_call.capability = CapabilityName::new("platform.data").expect("capability name");
    assert!(constraints.validate(&data_call).is_ok());

    data_call.input = json!({ "key": "other/answer" });
    data_call.operation = "get".to_string();
    assert!(constraints.validate(&data_call).is_err());
    data_call.input = json!({ "prefix": "state/", "table": "module_artifact_data" });
    data_call.operation = "list".to_string();
    assert!(constraints.validate(&data_call).is_err());

    data_call.operation = "query_index".to_string();
    data_call.input = json!({
        "index": "status",
        "value": "active",
        "prefix": "state/",
        "after_key": "state/one",
        "limit": 10,
    });
    assert!(constraints.validate(&data_call).is_ok());
    data_call.input["value"] = json!({ "not": "a scalar" });
    assert!(constraints.validate(&data_call).is_err());

    data_call.operation = "delete".to_string();
    data_call.input = json!({
        "key": "state/answer",
        "expected_revision": 2,
        "idempotency_key": Uuid::new_v4().to_string()
    });
    assert!(constraints.validate(&data_call).is_ok());
    data_call.input["key"] = json!("other/answer");
    assert!(constraints.validate(&data_call).is_err());

    data_call.operation = "put_batch".to_string();
    data_call.input = json!({
        "writes": [
            {
                "key": "state/one",
                "value": 1,
                "idempotency_key": Uuid::new_v4().to_string()
            },
            {
                "key": "state/two",
                "value": 2,
                "idempotency_key": Uuid::new_v4().to_string()
            }
        ]
    });
    assert!(constraints.validate(&data_call).is_ok());

    data_call.input = json!({
        "writes": [{
            "key": "other/one",
            "value": 1,
            "idempotency_key": Uuid::new_v4().to_string()
        }]
    });
    assert!(constraints.validate(&data_call).is_err());
}

#[test]
fn object_data_constraints_reject_physical_identity_and_ungranted_names() {
    let grant = CapabilityGrant {
        name: CapabilityName::new("platform.data.objects").expect("capability name"),
        constraints: json!({
            "object_prefixes": ["exports/"],
            "operations": ["get_metadata", "read", "put", "delete", "list", "begin_upload", "append_chunk", "complete_upload"]
        }),
    };
    let constraints =
        ObjectCapabilityConstraints::from_grant(&grant).expect("valid object constraints");
    let mut object_call = call(
        "put",
        json!({
            "name": "exports/report.json",
            "content_type": "application/json",
            "data_base64": "e30=",
            "idempotency_key": Uuid::new_v4().to_string(),
        }),
    );
    object_call.capability =
        CapabilityName::new("platform.data.objects").expect("capability name");
    assert!(constraints.validate(&object_call).is_ok());

    object_call.operation = "begin_upload".to_string();
    object_call.input = json!({
        "name": "exports/report.json",
        "content_type": "application/json",
        "idempotency_key": Uuid::new_v4().to_string(),
    });
    assert!(constraints.validate(&object_call).is_ok());
    object_call.operation = "append_chunk".to_string();
    object_call.input = json!({
        "session_id": Uuid::new_v4().to_string(),
        "sequence": 0,
        "data_base64": "e30=",
    });
    assert!(constraints.validate(&object_call).is_ok());

    object_call.operation = "delete".to_string();
    object_call.input = json!({
        "name": "exports/report.json",
        "expected_revision": 3,
        "idempotency_key": Uuid::new_v4().to_string(),
    });
    assert!(constraints.validate(&object_call).is_ok());
    object_call.input["expected_revision"] = json!(0);
    assert!(constraints.validate(&object_call).is_err());

    object_call.input = json!({ "name": "other/report.json" });
    object_call.operation = "read".to_string();
    assert!(constraints.validate(&object_call).is_err());
    object_call.input = json!({
        "name": "exports/report.json",
        "content_type": "application/json",
        "data_base64": "e30=",
        "idempotency_key": Uuid::new_v4().to_string(),
        "storage_key": "host/private/key",
    });
    object_call.operation = "put".to_string();
    assert!(constraints.validate(&object_call).is_err());
}

#[test]
fn mcp_constraints_allow_only_declared_server_tool_pairs() {
    let grant = CapabilityGrant {
        name: CapabilityName::new("platform.mcp").expect("capability name"),
        constraints: json!({
            "tools": [{ "server": "rustok", "tool": "module_details" }],
            "operations": ["call"]
        }),
    };
    let constraints =
        McpCapabilityConstraints::from_grant(&grant).expect("valid MCP constraints");
    let mut mcp_call = call(
        "call",
        json!({
            "server": "rustok",
            "tool": "module_details",
            "arguments": { "slug": "content" }
        }),
    );
    mcp_call.capability = CapabilityName::new("platform.mcp").expect("capability name");
    assert!(constraints.validate(&mcp_call).is_ok());

    mcp_call.input = json!({ "server": "rustok", "tool": "list_modules" });
    assert!(constraints.validate(&mcp_call).is_err());
    mcp_call.input = json!({
        "server": "rustok",
        "tool": "module_details",
        "endpoint": "https://attacker.invalid"
    });
    assert!(constraints.validate(&mcp_call).is_err());
}

struct StaticBroker(&'static str);

#[async_trait]
impl CapabilityBroker for StaticBroker {
    async fn invoke(
        &self,
        _call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        Ok(CapabilityResponse {
            output: json!({ "owner": self.0 }),
        })
    }
}

#[tokio::test]
async fn router_uses_only_the_exact_capability_owner() {
    let secrets = CapabilityName::new("platform.secrets").expect("capability name");
    let data = CapabilityName::new("platform.data").expect("capability name");
    let router = CapabilityBrokerRouter::new()
        .route(secrets.clone(), Arc::new(StaticBroker("secrets")))
        .expect("first route")
        .route(data.clone(), Arc::new(StaticBroker("data")))
        .expect("second route");

    let mut secret_call = call("acquire_handle", json!({ "reference": "payment_api" }));
    secret_call.capability = secrets.clone();
    let response = router
        .invoke(
            &secret_call,
            &CapabilityGrant {
                name: secrets,
                constraints: json!({}),
            },
        )
        .await
        .expect("secret owner response");
    assert_eq!(response.output, json!({ "owner": "secrets" }));

    secret_call.capability = CapabilityName::new("platform.events").expect("capability name");
    let error = router
        .invoke(
            &secret_call,
            &CapabilityGrant {
                name: secret_call.capability.clone(),
                constraints: json!({}),
            },
        )
        .await
        .expect_err("unregistered capability must remain denied");
    assert!(matches!(error, SandboxError::CapabilityDenied(_)));
}

#[test]
fn router_rejects_duplicate_capability_owners() {
    let capability = CapabilityName::new("platform.data").expect("capability name");
    let result = CapabilityBrokerRouter::new()
        .route(capability.clone(), Arc::new(StaticBroker("first")))
        .expect("first route")
        .route(capability, Arc::new(StaticBroker("second")));
    assert!(matches!(result, Err(SandboxError::InvalidRequest(_))));
}

#[test]
fn http_constraints_enforce_https_by_default() {
    let constraints = HttpCapabilityConstraints {
        hosts: vec!["api.example.com".to_string()],
        methods: vec!["GET".to_string()],
        path_prefixes: vec!["/v1/".to_string()],
        allow_plain_http: false,
    };

    let mut http_call = call(
        "invoke",
        json!({
            "method": "GET",
            "url": "http://api.example.com/v1/resource"
        }),
    );
    http_call.capability = CapabilityName::new("platform.http").expect("cap");
    let err = constraints
        .validate(&http_call)
        .expect_err("plain http must be rejected by default");
    assert!(matches!(
        err,
        SandboxError::CapabilityConstraintDenied { .. }
    ));

    // With https -> passes
    http_call.input = json!({
        "method": "GET",
        "url": "https://api.example.com/v1/resource"
    });
    assert!(constraints.validate(&http_call).is_ok());

    // Embedded userinfo -> rejected
    http_call.input = json!({
        "method": "GET",
        "url": "https://user:pass@api.example.com/v1/resource"
    });
    let err = constraints
        .validate(&http_call)
        .expect_err("embedded userinfo credentials must be rejected");
    assert!(matches!(
        err,
        SandboxError::CapabilityConstraintDenied { .. }
    ));
}

#[test]
fn http_constraints_allow_plain_http_when_explicitly_configured() {
    let constraints = HttpCapabilityConstraints {
        hosts: vec!["api.example.com".to_string()],
        methods: vec!["GET".to_string()],
        path_prefixes: vec!["/v1/".to_string()],
        allow_plain_http: true,
    };

    let mut http_call = call(
        "invoke",
        json!({
            "method": "GET",
            "url": "http://api.example.com/v1/resource"
        }),
    );
    http_call.capability = CapabilityName::new("platform.http").expect("cap");
    assert!(constraints.validate(&http_call).is_ok());

    // ftp or other schemes -> still rejected
    http_call.input = json!({
        "method": "GET",
        "url": "ftp://api.example.com/v1/resource"
    });
    let err = constraints
        .validate(&http_call)
        .expect_err("ftp scheme must be rejected");
    assert!(matches!(
        err,
        SandboxError::CapabilityConstraintDenied { .. }
    ));
}

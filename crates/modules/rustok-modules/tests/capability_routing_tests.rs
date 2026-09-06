use sea_orm::{Database, DatabaseConnection};
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

use rustok_modules::{
    ArtifactCapabilityBrokerResolver, ArtifactCapabilityExecution, ArtifactEventCapabilityBroker,
    ArtifactHttpCapabilityBroker, SeaOrmArtifactEventCapabilityBrokerResolver,
    SeaOrmArtifactHttpCapabilityBrokerResolver,
};
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityCallContext, CapabilityGrant, CapabilityName,
    ExecutionPhase, SandboxSubject,
};

fn test_call(capability: &str, input: serde_json::Value) -> CapabilityCall {
    CapabilityCall {
        execution_id: Uuid::new_v4(),
        subject: SandboxSubject::ModuleArtifact {
            installation_id: Uuid::new_v4(),
            slug: "test_module".to_string(),
            version: "1.0.0".to_string(),
            digest: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
        },
        context: CapabilityCallContext {
            phase: ExecutionPhase::Manual,
            tenant_id: Some(Uuid::new_v4()),
            actor_id: None,
            trace_id: None,
        },
        capability: CapabilityName::new(capability).expect("capability"),
        operation: "invoke".to_string(),
        input,
    }
}

#[tokio::test]
async fn test_http_capability_broker_validation() {
    let broker = ArtifactHttpCapabilityBroker::with_timeout(Duration::from_millis(500));
    let grant = CapabilityGrant {
        name: CapabilityName::new("platform.http").expect("capability"),
        constraints: json!({
            "hosts": ["api.example.com"],
            "methods": ["GET", "POST"],
            "path_prefixes": ["/v1/"]
        }),
    };

    // Missing method -> error
    let invalid_call = test_call(
        "platform.http",
        json!({ "url": "https://api.example.com/v1/test" }),
    );
    let res = broker.invoke(&invalid_call, &grant).await;
    assert!(res.is_err(), "Call missing method should fail");

    // Missing url -> error
    let invalid_call2 = test_call("platform.http", json!({ "method": "GET" }));
    let res2 = broker.invoke(&invalid_call2, &grant).await;
    assert!(res2.is_err(), "Call missing url should fail");

    // Unsupported method -> error
    let invalid_call3 = test_call(
        "platform.http",
        json!({ "method": "INVALID", "url": "https://api.example.com/v1/test" }),
    );
    let res3 = broker.invoke(&invalid_call3, &grant).await;
    assert!(res3.is_err(), "Unsupported method should fail");

    // SSRF: Localhost destination -> error
    let ssrf_call1 = test_call(
        "platform.http",
        json!({ "method": "GET", "url": "https://localhost/v1/test" }),
    );
    let res_ssrf1 = broker.invoke(&ssrf_call1, &grant).await;
    assert!(res_ssrf1.is_err(), "Localhost destination must be rejected");

    // SSRF: 127.0.0.1 destination -> error
    let ssrf_call2 = test_call(
        "platform.http",
        json!({ "method": "GET", "url": "https://127.0.0.1/v1/test" }),
    );
    let res_ssrf2 = broker.invoke(&ssrf_call2, &grant).await;
    assert!(res_ssrf2.is_err(), "127.0.0.1 destination must be rejected");

    // SSRF: Cloud metadata 169.254.169.254 -> error
    let ssrf_call3 = test_call(
        "platform.http",
        json!({ "method": "GET", "url": "https://169.254.169.254/v1/test" }),
    );
    let res_ssrf3 = broker.invoke(&ssrf_call3, &grant).await;
    assert!(
        res_ssrf3.is_err(),
        "Cloud metadata destination must be rejected"
    );

    // SSRF: Private network 10.0.0.1 -> error
    let ssrf_call4 = test_call(
        "platform.http",
        json!({ "method": "GET", "url": "https://10.0.0.1/v1/test" }),
    );
    let res_ssrf4 = broker.invoke(&ssrf_call4, &grant).await;
    assert!(
        res_ssrf4.is_err(),
        "Private IP destination must be rejected"
    );

    // SSRF: Internal .internal domain -> error
    let ssrf_call5 = test_call(
        "platform.http",
        json!({ "method": "GET", "url": "https://service.internal/v1/test" }),
    );
    let res_ssrf5 = broker.invoke(&ssrf_call5, &grant).await;
    assert!(res_ssrf5.is_err(), "Internal domain must be rejected");
}

#[tokio::test]
async fn test_event_capability_broker_validation() {
    let db: DatabaseConnection = Database::connect("sqlite::memory:")
        .await
        .expect("db connect");
    let infrastructure = rustok_modules::ControlPlaneInfrastructure::default();
    let broker = ArtifactEventCapabilityBroker::new(db, infrastructure, "test_module".to_string());
    let grant = CapabilityGrant {
        name: CapabilityName::new("platform.events").expect("capability"),
        constraints: json!({
            "topics": ["test.*"],
            "operations": ["emit"]
        }),
    };

    // Missing topic -> error
    let invalid_call = test_call("platform.events", json!({ "payload": { "foo": "bar" } }));
    let res = broker.invoke(&invalid_call, &grant).await;
    assert!(res.is_err(), "Call missing topic should fail");

    // Non-object input -> error
    let invalid_call2 = test_call("platform.events", json!("not an object"));
    let res2 = broker.invoke(&invalid_call2, &grant).await;
    assert!(res2.is_err(), "Non-object input should fail");
}

#[tokio::test]
async fn test_capability_resolvers_fail_closed_on_unadmitted_execution() {
    let db: DatabaseConnection = Database::connect("sqlite::memory:")
        .await
        .expect("db connect");

    let http_resolver = SeaOrmArtifactHttpCapabilityBrokerResolver::new(db.clone());
    let events_resolver = SeaOrmArtifactEventCapabilityBrokerResolver::new(db);

    let execution = ArtifactCapabilityExecution {
        installation_id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        slug: "uninstalled_module".to_string(),
        version: "1.0.0".to_string(),
        digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
    };

    let http_cap = CapabilityName::new("platform.http").expect("cap");
    let events_cap = CapabilityName::new("platform.events").expect("cap");
    let data_cap = CapabilityName::new("platform.data").expect("cap");

    // Mismatched capability name fails closed
    assert!(
        http_resolver
            .resolve_broker(&execution, &data_cap)
            .await
            .is_err()
    );
    assert!(
        events_resolver
            .resolve_broker(&execution, &data_cap)
            .await
            .is_err()
    );

    // Uninstalled execution fails closed
    assert!(
        http_resolver
            .resolve_broker(&execution, &http_cap)
            .await
            .is_err()
    );
    assert!(
        events_resolver
            .resolve_broker(&execution, &events_cap)
            .await
            .is_err()
    );
}

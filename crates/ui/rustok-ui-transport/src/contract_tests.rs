use super::*;

#[test]
fn test_transport_path_as_str_and_serde() {
    assert_eq!(UiTransportPath::NativeServer.as_str(), "native_server");
    assert_eq!(UiTransportPath::Graphql.as_str(), "graphql");

    let json = serde_json::to_string(&UiTransportPath::NativeServer).expect("serialize");
    assert_eq!(json, "\"native_server\"");

    let deserialized: UiTransportPath = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized, UiTransportPath::NativeServer);
}

#[test]
fn test_retry_safety_serde() {
    let json_safe = serde_json::to_string(&UiTransportRetrySafety::SafeToRetry).expect("serialize");
    assert_eq!(json_safe, "\"safe_to_retry\"");

    let json_once = serde_json::to_string(&UiTransportRetrySafety::AtMostOnce).expect("serialize");
    assert_eq!(json_once, "\"at_most_once\"");
}

#[test]
fn test_transport_error_display() {
    let err_native = UiTransportError::native("catalog", "connection timeout");
    assert_eq!(err_native.surface, "catalog");
    assert_eq!(err_native.failed_path, UiTransportPath::NativeServer);
    assert!(!err_native.fallback_attempted);
    assert!(err_native.to_string().contains("native_server"));
    assert!(err_native.to_string().contains("connection timeout"));

    let err_graphql = UiTransportError::graphql("catalog", "field not found");
    assert_eq!(err_graphql.failed_path, UiTransportPath::Graphql);
    assert!(err_graphql.to_string().contains("graphql"));

    let err_fallback = UiTransportError::fallback_failed("catalog", "native down", "graphql 503");
    assert!(err_fallback.fallback_attempted);
    assert!(err_fallback.to_string().contains("fallback failed"));
    assert!(
        err_fallback
            .to_string()
            .contains("native_server=native down")
    );
    assert!(err_fallback.to_string().contains("graphql=graphql 503"));
}

#[tokio::test]
async fn test_contract_execute_selected() {
    let ok = execute_selected_transport(
        "products",
        UiTransportPath::NativeServer,
        || async { Ok::<_, &'static str>("data") },
        || async {
            panic!("graphql should not run");
            #[allow(unreachable_code)]
            Ok::<&'static str, &'static str>("")
        },
    )
    .await
    .expect("execution succeeds");
    assert_eq!(ok, "data");

    let err = execute_selected_transport(
        "products",
        UiTransportPath::Graphql,
        || async {
            panic!("native should not run");
            #[allow(unreachable_code)]
            Ok::<(), &'static str>(())
        },
        || async { Err::<(), _>("graphql error") },
    )
    .await
    .expect_err("graphql fails");
    assert_eq!(err.failed_path, UiTransportPath::Graphql);
    assert_eq!(err.graphql_error.as_deref(), Some("graphql error"));
}

#[tokio::test]
async fn test_contract_execute_with_fallback() {
    let recovered = execute_with_fallback(
        "cart",
        UiTransportPath::NativeServer,
        || async { Err::<&'static str, _>("native offline") },
        || async { Ok::<_, &'static str>("graphql fallback ok") },
    )
    .await
    .expect("fallback succeeds");
    assert_eq!(recovered, "graphql fallback ok");
}

#[tokio::test]
async fn test_contract_idempotency_guard_blocks_fallback() {
    let error = execute_transport_policy(
        "checkout",
        UiTransportPath::NativeServer,
        UiTransportRetrySafety::AtMostOnce,
        true, // fallback_allowed is true, but AtMostOnce MUST block it!
        || async { Err::<(), _>("charge timeout") },
        || async {
            panic!("unsafe retry must NEVER be executed");
            #[allow(unreachable_code)]
            Ok::<(), &'static str>(())
        },
    )
    .await
    .expect_err("at most once fails without fallback");

    assert!(!error.fallback_attempted);
    assert_eq!(error.failed_path, UiTransportPath::NativeServer);
}

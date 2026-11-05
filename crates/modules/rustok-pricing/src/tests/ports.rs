use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_commerce_foundation::error::CommerceError;
use uuid::Uuid;

use crate::ports::diagnostics::{
    parse_port_actor_id, parse_port_tenant_id, pricing_error_to_port_error, validate_non_nil_uuid,
};

#[test]
fn canonical_port_uses_nested_owner_policy_without_payload_or_retry_loss() {
    use rustok_core::error::{ErrorKind, RichError};
    let context = PortContext::new(
        Uuid::new_v4().to_string(),
        PortActor::system(),
        "en",
        "scenario",
    );
    for (kind, expected, code, retryable) in [
        (
            ErrorKind::Forbidden,
            PortErrorKind::Forbidden,
            "pricing.access_denied",
            false,
        ),
        (
            ErrorKind::Timeout,
            PortErrorKind::Timeout,
            "pricing.timeout",
            true,
        ),
        (
            ErrorKind::RateLimited,
            PortErrorKind::Unavailable,
            "pricing.rate_limited",
            true,
        ),
    ] {
        let error = CommerceError::from(RichError::new(kind, "private-port-payload"));
        let port = pricing_error_to_port_error(&context, "scenario", error);
        assert_eq!(port.kind, expected);
        assert_eq!(port.code, code);
        assert_eq!(port.retryable, retryable);
        assert!(!port.message.contains("private-port-payload"));
    }
    let port = pricing_error_to_port_error(
        &context,
        "scenario",
        CommerceError::Core(rustok_core::Error::Cache("private-cache-payload".into())),
    );
    assert_eq!(port.kind, PortErrorKind::Unavailable);
    assert!(port.retryable);
    assert!(!port.message.contains("private-cache-payload"));
}

#[test]
fn validate_non_nil_uuid_accepts_valid_and_rejects_nil() {
    let valid = Uuid::new_v4();
    assert!(validate_non_nil_uuid(valid, "test_field").is_ok());

    let err = validate_non_nil_uuid(Uuid::nil(), "test_field").unwrap_err();
    assert_eq!(err.code, "pricing.test_field_nil");
    assert_eq!(err.kind, PortErrorKind::Validation);
}

#[test]
fn parse_port_tenant_id_accepts_valid_and_rejects_nil_or_invalid() {
    let valid_tenant = Uuid::new_v4();
    let ctx = PortContext::new(
        valid_tenant.to_string(),
        PortActor::system(),
        "en-US",
        Uuid::new_v4(),
    );
    assert_eq!(parse_port_tenant_id(&ctx, "test_op").unwrap(), valid_tenant);

    let nil_ctx = PortContext::new(
        Uuid::nil().to_string(),
        PortActor::system(),
        "en-US",
        Uuid::new_v4(),
    );
    let err = parse_port_tenant_id(&nil_ctx, "test_op").unwrap_err();
    assert_eq!(err.code, "pricing.tenant_id_nil");
    assert_eq!(err.kind, PortErrorKind::Validation);

    let invalid_ctx = PortContext::new("not-a-uuid", PortActor::system(), "en-US", Uuid::new_v4());
    let err = parse_port_tenant_id(&invalid_ctx, "test_op").unwrap_err();
    assert_eq!(err.code, "pricing.tenant_id_invalid");
    assert_eq!(err.kind, PortErrorKind::Validation);
}

#[test]
fn parse_port_actor_id_accepts_valid_and_rejects_nil_or_invalid() {
    let valid_actor = Uuid::new_v4();
    let ctx = PortContext::new(
        Uuid::new_v4().to_string(),
        PortActor::user(valid_actor.to_string()),
        "en-US",
        Uuid::new_v4(),
    );
    assert_eq!(parse_port_actor_id(&ctx, "test_op").unwrap(), valid_actor);

    let nil_ctx = PortContext::new(
        Uuid::new_v4().to_string(),
        PortActor::user(Uuid::nil().to_string()),
        "en-US",
        Uuid::new_v4(),
    );
    let err = parse_port_actor_id(&nil_ctx, "test_op").unwrap_err();
    assert_eq!(err.code, "pricing.actor_id_nil");
    assert_eq!(err.kind, PortErrorKind::Validation);

    let invalid_ctx = PortContext::new(
        Uuid::new_v4().to_string(),
        PortActor::user("not-a-uuid"),
        "en-US",
        Uuid::new_v4(),
    );
    let err = parse_port_actor_id(&invalid_ctx, "test_op").unwrap_err();
    assert_eq!(err.code, "pricing.actor_id_invalid");
    assert_eq!(err.kind, PortErrorKind::Validation);
}

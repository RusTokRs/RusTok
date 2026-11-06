use leptos::prelude::ServerFnError;
use uuid::Uuid;

use crate::core::{
    CommentThreadDetailRequest, CommentThreadsRequest, CommentsAdminValidationError,
    DEFAULT_DETAIL_PER_PAGE, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, SetCommentStatusCommand,
    SetThreadStatusCommand,
};
use crate::transport::{
    ACTIVE_TRANSPORT_PATH, CommentsAdminTransportError, CommentsAdminTransportPath,
};
use rustok_comments::{CommentStatus, CommentThreadStatus};

#[test]
fn test_crate_api_compliance() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(api_doc.contains("## Platform Capability Bindings"));
    assert!(api_doc.contains("## Domain Collaborations"));
    assert!(api_doc.contains("## Minimum Contract Set"));
    assert!(api_doc.contains("### Input DTOs/Commands"));
    assert!(api_doc.contains("### Domain Invariants"));
    assert!(api_doc.contains("### Events / Outbox Side Effects"));
    assert!(api_doc.contains("### Errors / Failure Codes"));
}

#[test]
fn ffa_core_boundary_invariants() {
    let core_source = include_str!("core.rs");
    assert!(
        !core_source.contains("use leptos"),
        "core.rs must not import leptos"
    );
    assert!(
        !core_source.contains("leptos::"),
        "core.rs must not call leptos functions directly"
    );
    assert!(
        !core_source.contains("#[component]"),
        "core.rs must not declare leptos components"
    );
    assert!(
        !core_source.contains("#[server"),
        "core.rs must not declare server functions"
    );
    assert!(
        !core_source.contains("LocalResource"),
        "core.rs must not use framework resource primitives"
    );
}

#[test]
fn transport_error_display_and_traits() {
    let err = CommentsAdminTransportError::ServerFn("rpc timeout".to_string());
    assert_eq!(err.to_string(), "rpc timeout");
    assert_eq!(
        err,
        CommentsAdminTransportError::ServerFn("rpc timeout".to_string())
    );
    assert_ne!(
        err,
        CommentsAdminTransportError::ServerFn("other error".to_string())
    );

    let from_server_fn: CommentsAdminTransportError =
        ServerFnError::new("internal error").into();
    assert_eq!(
        from_server_fn,
        CommentsAdminTransportError::ServerFn(
            "error running server function: internal error".to_string()
        )
    );

    let serialized = serde_json::to_string(&err).expect("transport error serializes");
    let deserialized: CommentsAdminTransportError =
        serde_json::from_str(&serialized).expect("transport error deserializes");
    assert_eq!(err, deserialized);
}

#[test]
fn validation_error_exhaustive_coverage() {
    let variants = [
        (
            CommentsAdminValidationError::EmptyThreadId,
            "thread_id must not be empty",
        ),
        (
            CommentsAdminValidationError::InvalidThreadId("parse err".to_string()),
            "invalid thread_id UUID: parse err",
        ),
        (
            CommentsAdminValidationError::NilThreadId,
            "thread_id must not be nil UUID",
        ),
        (
            CommentsAdminValidationError::EmptyCommentId,
            "comment_id must not be empty",
        ),
        (
            CommentsAdminValidationError::InvalidCommentId("parse err".to_string()),
            "invalid comment_id UUID: parse err",
        ),
        (
            CommentsAdminValidationError::NilCommentId,
            "comment_id must not be nil UUID",
        ),
    ];

    for (variant, expected_message) in &variants {
        assert_eq!(&variant.to_string(), expected_message);
        assert_eq!(variant, variant);
    }

    assert_ne!(
        CommentsAdminValidationError::EmptyThreadId,
        CommentsAdminValidationError::NilThreadId
    );
    assert_ne!(
        CommentsAdminValidationError::EmptyCommentId,
        CommentsAdminValidationError::NilCommentId
    );
}

#[test]
fn transport_path_enum_contract() {
    assert_eq!(
        ACTIVE_TRANSPORT_PATH,
        CommentsAdminTransportPath::NativeServer
    );
    let path = CommentsAdminTransportPath::NativeServer;
    let cloned = path;
    assert_eq!(path, cloned);
    assert_eq!(format!("{path:?}"), "NativeServer");
}

#[test]
fn pagination_and_bound_constants() {
    assert_eq!(DEFAULT_PAGE_SIZE, 20);
    assert_eq!(DEFAULT_DETAIL_PER_PAGE, 100);
    assert_eq!(MAX_PAGE_SIZE, 100);

    let req = CommentThreadsRequest::from_filters(0, 0, "".to_string(), "open", "pending");
    assert_eq!(req.page, 1);
    assert_eq!(req.per_page, 1);

    let req_overflow = CommentThreadsRequest::from_filters(
        u64::MAX,
        u64::MAX,
        "blog".to_string(),
        "open",
        "pending",
    );
    assert_eq!(req_overflow.page, u64::MAX);
    assert_eq!(req_overflow.per_page, MAX_PAGE_SIZE);
}

#[test]
fn command_validation_boundary_cases() {
    let nil_uuid = Uuid::nil().to_string();

    let cmd_thread_nil = SetThreadStatusCommand::new(nil_uuid.clone(), CommentThreadStatus::Open);
    assert_eq!(
        cmd_thread_nil.validate().unwrap_err(),
        CommentsAdminValidationError::NilThreadId
    );

    let cmd_thread_empty = SetThreadStatusCommand::new("   ".to_string(), CommentThreadStatus::Open);
    assert_eq!(
        cmd_thread_empty.validate().unwrap_err(),
        CommentsAdminValidationError::EmptyThreadId
    );

    let cmd_comment_nil =
        SetCommentStatusCommand::new(nil_uuid.clone(), CommentStatus::Spam, "en".to_string());
    assert_eq!(
        cmd_comment_nil.validate().unwrap_err(),
        CommentsAdminValidationError::NilCommentId
    );

    let cmd_comment_empty =
        SetCommentStatusCommand::new("   ".to_string(), CommentStatus::Spam, "en".to_string());
    assert_eq!(
        cmd_comment_empty.validate().unwrap_err(),
        CommentsAdminValidationError::EmptyCommentId
    );

    let valid_id = Uuid::new_v4();
    let detail_req =
        CommentThreadDetailRequest::new(valid_id.to_string(), "  ru  ".to_string(), 0, 999);
    assert_eq!(detail_req.validate().unwrap(), valid_id);
    assert_eq!(detail_req.page, 1);
    assert_eq!(detail_req.per_page, MAX_PAGE_SIZE);
    assert_eq!(detail_req.locale, "ru");
}

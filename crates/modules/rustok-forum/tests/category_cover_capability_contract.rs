use rustok_forum::ForumError;

#[test]
fn unavailable_category_cover_capability_has_stable_non_retryable_code() {
    let error = ForumError::capability_unavailable(
        "forum.category_cover.media",
        "FORUM_CATEGORY_COVER_MEDIA_CAPABILITY_UNAVAILABLE",
    );

    assert_eq!(
        error.stable_code(),
        "FORUM_CATEGORY_COVER_MEDIA_CAPABILITY_UNAVAILABLE"
    );
    assert!(!error.is_retryable());
}

#[test]
fn provider_failure_preserves_diagnostics_without_exposing_internal_message() {
    let error = ForumError::capability_failure(
        "forum.category_cover.media",
        "media.database",
        "connection string and internal database detail",
        true,
    );

    assert_eq!(error.stable_code(), "FORUM_CAPABILITY_FAILURE");
    assert!(error.is_retryable());
    assert!(!error.to_string().contains("connection string"));

    match error {
        ForumError::CapabilityFailure {
            source_code,
            message,
            retryable,
            ..
        } => {
            assert_eq!(source_code, "media.database");
            assert_eq!(message, "connection string and internal database detail");
            assert!(retryable);
        }
        other => panic!("expected capability failure, got {other:?}"),
    }
}

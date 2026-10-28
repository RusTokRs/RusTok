use sea_orm::DbErr;
use uuid::Uuid;

use super::comment_thread::{THREAD_IDENTITY_CONFLICT_MARKER, is_thread_identity_conflict};
use crate::error::CommentsError;

fn identity_conflict_error(
    tenant_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    existing_thread_id: Uuid,
) -> DbErr {
    DbErr::Custom(format!(
        "{THREAD_IDENTITY_CONFLICT_MARKER}:{tenant_id}:{target_type}:{target_id}:{existing_thread_id}"
    ))
}

#[test]
fn thread_identity_conflict_classifier_accepts_exact_scope_and_owner_uuid() {
    let tenant_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let error = identity_conflict_error(tenant_id, "blog_post", target_id, Uuid::new_v4());

    assert!(is_thread_identity_conflict(
        &error,
        tenant_id,
        "blog_post",
        target_id,
    ));
}

#[test]
fn thread_identity_conflict_classifier_rejects_malformed_owner_uuid() {
    let tenant_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let error = DbErr::Custom(format!(
        "{THREAD_IDENTITY_CONFLICT_MARKER}:{tenant_id}:blog_post:{target_id}:not-a-uuid"
    ));

    assert!(!is_thread_identity_conflict(
        &error,
        tenant_id,
        "blog_post",
        target_id,
    ));
}

#[test]
fn thread_identity_conflict_classifier_rejects_wrong_scope() {
    let tenant_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let error = identity_conflict_error(tenant_id, "blog_post", target_id, Uuid::new_v4());

    assert!(!is_thread_identity_conflict(
        &error,
        Uuid::new_v4(),
        "blog_post",
        target_id,
    ));
    assert!(!is_thread_identity_conflict(
        &error,
        tenant_id,
        "catalog_item",
        target_id,
    ));
    assert!(!is_thread_identity_conflict(
        &error,
        tenant_id,
        "blog_post",
        Uuid::new_v4(),
    ));
}

#[test]
fn unrelated_custom_error_remains_a_database_error() {
    let error = CommentsError::from(DbErr::Custom(
        "connection reset while inserting comment thread".to_string(),
    ));

    assert!(matches!(
        error,
        CommentsError::Database(DbErr::Custom(message))
            if message == "connection reset while inserting comment thread"
    ));
}

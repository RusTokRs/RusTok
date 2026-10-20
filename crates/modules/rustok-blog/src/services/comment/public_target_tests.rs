use super::{comments_write_port_context, is_public_comment_target};
use crate::BlogPostStatus;
use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActorKind};
use rustok_core::SecurityContext;
use uuid::Uuid;

#[test]
fn public_comment_target_requires_a_published_post() {
    assert!(!is_public_comment_target(
        BlogPostStatus::Draft,
        &[],
        Some("web")
    ));
    assert!(!is_public_comment_target(
        BlogPostStatus::Archived,
        &[],
        Some("web")
    ));
}

#[test]
fn compensation_write_context_uses_the_trusted_system_actor() {
    let context = comments_write_port_context(
        Uuid::new_v4(),
        &SecurityContext::system(),
        PLATFORM_FALLBACK_LOCALE,
        "delete-after-public-target-loss",
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
    .expect("system compensation context should be valid");

    assert_eq!(context.actor.kind, PortActorKind::System);
    assert_eq!(context.actor.id, "system");
}

#[test]
fn public_comment_target_enforces_the_channel_allowlist() {
    let channels = vec!["web".to_string()];
    assert!(is_public_comment_target(
        BlogPostStatus::Published,
        &channels,
        Some("web")
    ));
    assert!(!is_public_comment_target(
        BlogPostStatus::Published,
        &channels,
        Some("mobile")
    ));
    assert!(!is_public_comment_target(
        BlogPostStatus::Published,
        &channels,
        None
    ));
}

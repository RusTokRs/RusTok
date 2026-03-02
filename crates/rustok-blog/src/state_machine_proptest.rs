//! Property-based tests for Blog Post state machine
//!
//! Uses proptest to verify state machine invariants hold for all possible inputs.

use super::state_machine::*;
use proptest::prelude::*;
use uuid::Uuid;

// ============================================================================
// Strategies for generating test data
// ============================================================================

fn uuid_strategy() -> impl Strategy<Value = Uuid> {
    (any::<[u8; 16]>()).prop_map(Uuid::from_bytes)
}

fn non_empty_string_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,50}"
}

fn tags_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(non_empty_string_strategy(), 0..10)
}

// ============================================================================
// State Machine Invariants
// ============================================================================

proptest! {
    /// Test: All draft posts can be published
    #[test]
    fn draft_can_always_be_published(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale);
        let published = post.publish();

        prop_assert_eq!(published.id, id);
        prop_assert_eq!(published.tenant_id, tenant_id);
        prop_assert_eq!(published.to_status(), BlogPostStatus::Published);
    }

    /// Test: All published posts can be archived
    #[test]
    fn published_can_always_be_archived(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
        reason in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale)
            .publish()
            .archive(reason);

        prop_assert_eq!(post.id, id);
        prop_assert_eq!(post.to_status(), BlogPostStatus::Archived);
    }

    /// Test: All published posts can be unpublished
    #[test]
    fn published_can_always_be_unpublished(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale)
            .publish()
            .unpublish();

        prop_assert_eq!(post.id, id);
        prop_assert_eq!(post.to_status(), BlogPostStatus::Draft);
    }

    /// Test: All archived posts can be restored
    #[test]
    fn archived_can_always_be_restored(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
        reason in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale)
            .publish()
            .archive(reason)
            .restore_to_draft();

        prop_assert_eq!(post.id, id);
        prop_assert_eq!(post.to_status(), BlogPostStatus::Draft);
    }

    /// Test: ID is preserved through all valid transitions
    #[test]
    fn id_preserved_through_transitions(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
        reason in non_empty_string_strategy(),
    ) {
        // Draft -> Published
        let post = BlogPost::new_draft(id, tenant_id, author_id, title.clone(), slug.clone(), locale.clone());
        prop_assert_eq!(post.id, id);

        let post = post.publish();
        prop_assert_eq!(post.id, id);

        // Published -> Archived
        let post = post.archive(reason);
        prop_assert_eq!(post.id, id);

        // Archived -> Draft
        let post = post.restore_to_draft();
        prop_assert_eq!(post.id, id);

        // Draft -> Published -> Draft
        let post = post.publish().unpublish();
        prop_assert_eq!(post.id, id);
    }

    /// Test: Tenant ID is preserved through all transitions
    #[test]
    fn tenant_id_preserved_through_transitions(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale);
        let post = post.publish();
        let post = post.archive("test".to_string());
        let post = post.restore_to_draft();

        prop_assert_eq!(post.tenant_id, tenant_id);
    }

    /// Test: Author ID is preserved through all transitions
    #[test]
    fn author_id_preserved_through_transitions(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale);
        let post = post.publish();
        let post = post.archive("test".to_string());
        let post = post.restore_to_draft();

        prop_assert_eq!(post.author_id, author_id);
    }

    /// Test: Title and slug are preserved
    #[test]
    fn metadata_preserved_through_transitions(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title.clone(), slug.clone(), locale.clone());
        let post = post.publish();
        let post = post.archive("test".to_string());
        let post = post.restore_to_draft();

        prop_assert_eq!(post.title, title);
        prop_assert_eq!(post.slug, slug);
        prop_assert_eq!(post.locale, locale);
    }

    /// Test: Tags are preserved through state transitions
    #[test]
    fn tags_preserved_through_transitions(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
        tags in tags_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale)
            .set_tags(tags.clone());
        let post = post.publish();
        let post = post.archive("test".to_string());
        let post = post.restore_to_draft();

        prop_assert_eq!(post.tags, tags);
    }

    /// Test: Published_at is always in the past or now
    #[test]
    fn published_at_not_in_future(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale)
            .publish();

        prop_assert!(post.state.published_at <= chrono::Utc::now());
    }

    /// Test: Archived_at is always in the past or now
    #[test]
    fn archived_at_not_in_future(
        id in uuid_strategy(),
        tenant_id in uuid_strategy(),
        author_id in uuid_strategy(),
        title in non_empty_string_strategy(),
        slug in non_empty_string_strategy(),
        locale in non_empty_string_strategy(),
        reason in non_empty_string_strategy(),
    ) {
        let post = BlogPost::new_draft(id, tenant_id, author_id, title, slug, locale)
            .publish()
            .archive(reason);

        prop_assert!(post.state.archived_at <= chrono::Utc::now());
    }
}

// ============================================================================
// Comment Status Property Tests
// ============================================================================

proptest! {
    /// Test: Comment visibility is correct for each status
    #[test]
    fn comment_visibility_is_correct(
        status in prop_oneof![
            Just(CommentStatus::Pending),
            Just(CommentStatus::Approved),
            Just(CommentStatus::Spam),
            Just(CommentStatus::Trash),
        ]
    ) {
        let is_visible = status.is_visible();
        prop_assert_eq!(is_visible, status == CommentStatus::Approved);
    }

    /// Test: Approved comments remain approved after approve()
    #[test]
    fn approved_stays_approved_on_approve() {
        let status = CommentStatus::Approved;
        prop_assert_eq!(status.approve(), CommentStatus::Approved);
    }

    /// Test: Trash is terminal
    #[test]
    fn trash_is_terminal() {
        let status = CommentStatus::Trash;
        prop_assert_eq!(status.approve(), CommentStatus::Trash);
        prop_assert_eq!(status.mark_spam(), CommentStatus::Trash);
    }
}

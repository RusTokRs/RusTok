//! Property-Based Tests for Content State Machine
//!
//! These tests use proptest to verify state machine invariants across
//! randomly generated inputs, ensuring robustness and catching edge cases.
//!
//! Properties tested:
//! - ID preservation across all valid transitions
//! - Tenant isolation (tenant_id never changes)
//! - Timestamp monotonicity
//! - State transition validity

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::entities::node::ContentStatus;
    use proptest::prelude::*;
    use uuid::Uuid;

    // ============================================================================
    // Strategy Definitions
    // ============================================================================

    /// Generate random UUIDs
    fn uuid_strategy() -> impl Strategy<Value = Uuid> {
        prop::array::uniform16(0u8..=255).prop_map(Uuid::from_bytes)
    }

    /// Generate valid content kinds
    fn content_kind_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["article", "page", "post", "product", "category"])
            .prop_map(String::from)
    }

    /// Generate archive reasons
    fn archive_reason_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec![
            "Content outdated",
            "Replaced by new version",
            "Legal compliance",
            "User request",
            "Administrative decision",
        ])
        .prop_map(String::from)
    }

    /// Generate complete draft nodes
    fn draft_node_strategy() -> impl Strategy<Value = ContentNode<Draft>> {
        (
            uuid_strategy(),
            uuid_strategy(),
            uuid_strategy(),
            content_kind_strategy(),
        )
            .prop_map(|(id, tenant_id, author_id, kind)| {
                ContentNode::new_draft(id, tenant_id, Some(author_id), kind)
            })
    }

    // ============================================================================
    // Property Tests: ID Preservation
    // ============================================================================

    proptest! {
        /// Property: ID is preserved through Draft -> Published transition
        #[test]
        fn id_preserved_draft_to_published(
            node in draft_node_strategy()
        ) {
            let original_id = node.id;
            let published = node.publish();
            prop_assert_eq!(published.id, original_id);
        }

        /// Property: ID is preserved through Published -> Archived transition
        #[test]
        fn id_preserved_published_to_archived(
            (node, reason) in (draft_node_strategy(), archive_reason_strategy())
        ) {
            let original_id = node.id;
            let published = node.publish();
            let archived = published.archive(reason);
            prop_assert_eq!(archived.id, original_id);
        }

        /// Property: ID is preserved through full lifecycle
        #[test]
        fn id_preserved_full_lifecycle(
            (node, reason) in (draft_node_strategy(), archive_reason_strategy())
        ) {
            let original_id = node.id;
            let restored = node
                .publish()
                .archive(reason)
                .restore_to_draft();
            prop_assert_eq!(restored.id, original_id);
        }
    }

    // ============================================================================
    // Property Tests: Tenant Isolation
    // ============================================================================

    proptest! {
        /// Property: Tenant ID never changes across any transition
        #[test]
        fn tenant_id_preserved_all_transitions(
            (node, reason) in (draft_node_strategy(), archive_reason_strategy())
        ) {
            let original_tenant_id = node.tenant_id;

            // Draft -> Published
            let published = node.publish();
            prop_assert_eq!(published.tenant_id, original_tenant_id);

            // Published -> Archived
            let archived = published.archive(reason);
            prop_assert_eq!(archived.tenant_id, original_tenant_id);

            // Archived -> Draft
            let restored = archived.restore_to_draft();
            prop_assert_eq!(restored.tenant_id, original_tenant_id);
        }
    }

    // ============================================================================
    // Property Tests: Author Preservation
    // ============================================================================

    proptest! {
        /// Property: Author ID is preserved through all transitions
        #[test]
        fn author_id_preserved_all_transitions(
            node in draft_node_strategy()
        ) {
            let original_author_id = node.author_id;

            let published = node.publish();
            prop_assert_eq!(published.author_id, original_author_id);
        }

        /// Property: Content kind is preserved through all transitions
        #[test]
        fn kind_preserved_all_transitions(
            (node, reason) in (draft_node_strategy(), archive_reason_strategy())
        ) {
            let original_kind = node.kind.clone();

            let restored = node
                .publish()
                .archive(reason)
                .restore_to_draft();

            prop_assert_eq!(restored.kind, original_kind);
        }
    }

    // ============================================================================
    // Property Tests: State Transitions
    // ============================================================================

    proptest! {
        /// Property: Published state has valid published_at timestamp
        #[test]
        fn published_has_valid_timestamp(
            node in draft_node_strategy()
        ) {
            let before_publish = chrono::Utc::now();
            let published = node.publish();
            let after_publish = chrono::Utc::now();

            prop_assert!(
                published.state.published_at >= before_publish
                    && published.state.published_at <= after_publish
            );
        }

        /// Property: Archived state stores the correct reason
        #[test]
        fn archived_stores_correct_reason(
            (node, reason) in (draft_node_strategy(), archive_reason_strategy())
        ) {
            let archived = node.publish().archive(reason.clone());
            prop_assert_eq!(archived.state.reason, reason);
        }

        /// Property: Restored draft has fresh timestamps
        #[test]
        fn restored_draft_has_fresh_timestamps(
            (node, reason) in (draft_node_strategy(), archive_reason_strategy())
        ) {
            let before_restore = chrono::Utc::now();
            let restored = node
                .publish()
                .archive(reason)
                .restore_to_draft();
            let after_restore = chrono::Utc::now();

            prop_assert!(
                restored.state.created_at >= before_restore
                    && restored.state.created_at <= after_restore
            );
            prop_assert!(
                restored.state.updated_at >= before_restore
                    && restored.state.updated_at <= after_restore
            );
        }
    }

    // ============================================================================
    // Property Tests: Status Conversion
    // ============================================================================

    proptest! {
        /// Property: Draft node converts to Draft status
        #[test]
        fn draft_converts_to_draft_status(
            node in draft_node_strategy()
        ) {
            prop_assert_eq!(node.to_status(), ContentStatus::Draft);
        }

        /// Property: Published node converts to Published status
        #[test]
        fn published_converts_to_published_status(
            node in draft_node_strategy()
        ) {
            let published = node.publish();
            prop_assert_eq!(published.to_status(), ContentStatus::Published);
        }

        /// Property: Archived node converts to Archived status
        #[test]
        fn archived_converts_to_archived_status(
            (node, reason) in (draft_node_strategy(), archive_reason_strategy())
        ) {
            let archived = node.publish().archive(reason);
            prop_assert_eq!(archived.to_status(), ContentStatus::Archived);
        }

        /// Property: Restored draft converts back to Draft status
        #[test]
        fn restored_converts_to_draft_status(
            (node, reason) in (draft_node_strategy(), archive_reason_strategy())
        ) {
            let restored = node
                .publish()
                .archive(reason)
                .restore_to_draft();
            prop_assert_eq!(restored.to_status(), ContentStatus::Draft);
        }
    }

    // ============================================================================
    // Property Tests: Metadata Operations
    // ============================================================================

    proptest! {
        /// Property: set_parent updates parent_id correctly
        #[test]
        fn set_parent_updates_parent_id(
            (node, parent_id) in (draft_node_strategy(), uuid_strategy())
        ) {
            let node = node.set_parent(parent_id);
            prop_assert_eq!(node.parent_id, Some(parent_id));
        }

        /// Property: set_category updates category_id correctly
        #[test]
        fn set_category_updates_category_id(
            (node, category_id) in (draft_node_strategy(), uuid_strategy())
        ) {
            let node = node.set_category(category_id);
            prop_assert_eq!(node.category_id, Some(category_id));
        }

        /// Property: Metadata operations are preserved through transitions
        #[test]
        fn metadata_preserved_through_transitions(
            (node, parent_id, category_id, reason) in (
                draft_node_strategy(),
                uuid_strategy(),
                uuid_strategy(),
                archive_reason_strategy()
            )
        ) {
            let node = node
                .set_parent(parent_id)
                .set_category(category_id);

            let restored = node
                .publish()
                .archive(reason)
                .restore_to_draft();

            prop_assert_eq!(restored.parent_id, Some(parent_id));
            prop_assert_eq!(restored.category_id, Some(category_id));
        }
    }

    // ============================================================================
    // Property Tests: Edge Cases
    // ============================================================================

    proptest! {
        /// Property: Empty archive reason is handled correctly
        #[test]
        fn empty_archive_reason_allowed(
            node in draft_node_strategy()
        ) {
            let archived = node.publish().archive(String::new());
            prop_assert_eq!(archived.state.reason, "");
        }

        /// Property: Long archive reasons are preserved
        #[test]
        fn long_archive_reason_preserved(
            (node, reason) in (
                draft_node_strategy(),
                "[a-zA-Z0-9]{100,1000}"
            )
        ) {
            let reason = reason.to_string();
            let archived = node.publish().archive(reason.clone());
            prop_assert_eq!(archived.state.reason, reason);
        }

        /// Property: Update operation changes updated_at timestamp
        #[test]
        fn update_changes_updated_at(
            node in draft_node_strategy()
        ) {
            let original_updated_at = node.state.updated_at;

            // Small delay to ensure timestamp difference
            std::thread::sleep(std::time::Duration::from_millis(5));

            let updated = node.update();
            prop_assert!(updated.state.updated_at > original_updated_at);
        }
    }
}

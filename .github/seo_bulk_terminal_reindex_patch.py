from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def replace_between(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    if start_index < 0:
        raise SystemExit(f"{label}: start marker not found")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise SystemExit(f"{label}: end marker not found")
    return text[:start_index] + replacement + text[end_index:]


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def patch_events() -> None:
    path = Path("crates/rustok-seo/src/services/events.rs")
    text = path.read_text()
    start = "    #[allow(clippy::too_many_arguments)]\n    pub(super) async fn publish_seo_bulk_terminal_event_in_tx("
    end = "    pub async fn index_delivery_status("
    replacement = '''    #[allow(clippy::too_many_arguments)]
    pub(super) async fn publish_seo_bulk_terminal_event_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        job_id: Uuid,
        target_kind: &str,
        locale: &str,
        status: &str,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
    ) -> SeoResult<Option<(String, DomainEvent)>> {
        let event_scope = match status {
            "partial" => "seo.bulk.partial",
            "failed" => "seo.bulk.failed",
            _ => "seo.bulk.completed",
        };
        let idempotency_key = self.build_event_key(
            event_scope,
            tenant_id,
            &[
                target_kind.to_string(),
                locale.to_string(),
                job_id.to_string(),
                status.to_string(),
                processed_count.to_string(),
                succeeded_count.to_string(),
                failed_count.to_string(),
            ],
        );
        let existing = seo_event_delivery::Entity::find()
            .filter(seo_event_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_event_delivery::Column::IdempotencyKey.eq(idempotency_key.as_str()))
            .one(txn)
            .await?;
        if existing.is_some() {
            return Ok(None);
        }

        let event = seo_bulk_terminal_event(
            job_id,
            target_kind,
            locale,
            status,
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key.clone(),
        );
        let index_reindex_event = event.clone();
        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(txn, tenant_id, None, event)
            .await
            .map_err(|error| {
                SeoError::Database(DbErr::Custom(format!(
                    "failed to enqueue bulk terminal event transactionally: {error}"
                )))
            })?;
        let now = Utc::now().fixed_offset();

        seo_event_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            event_type: Set(event_scope.to_string()),
            idempotency_key: Set(idempotency_key.clone()),
            source_kind: Set(Some("bulk_job".to_string())),
            source_id: Set(Some(job_id)),
            status: Set(DELIVERY_STATUS_SENT.to_string()),
            outbox_event_id: Set(Some(outbox_event_id)),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            dispatched_at: Set(Some(now)),
        }
        .insert(txn)
        .await?;

        Ok(Some((idempotency_key, index_reindex_event)))
    }

    pub(super) async fn dispatch_seo_bulk_terminal_reindex(
        &self,
        tenant_id: Uuid,
        dispatch: Option<(String, DomainEvent)>,
    ) {
        let Some((idempotency_key, event)) = dispatch else {
            return;
        };
        let event_type = event.event_type().to_string();
        self.dispatch_index_reindex_for_event(
            tenant_id,
            event_type.as_str(),
            idempotency_key.as_str(),
            &event,
        )
        .await;
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    async fn publish_seo_bulk_completed_event(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        target_kind: &str,
        locale: &str,
        status: &str,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
    ) {
        let txn = self
            .db
            .begin()
            .await
            .expect("bulk terminal test transaction should begin");
        let dispatch = self
            .publish_seo_bulk_terminal_event_in_tx(
                &txn,
                tenant_id,
                job_id,
                target_kind,
                locale,
                status,
                processed_count,
                succeeded_count,
                failed_count,
            )
            .await
            .expect("bulk terminal test event should enqueue transactionally");
        txn.commit()
            .await
            .expect("bulk terminal test transaction should commit");
        self.dispatch_seo_bulk_terminal_reindex(tenant_id, dispatch)
            .await;
    }

'''
    text = replace_between(text, start, end, replacement, "bulk terminal event section")

    old_assertion = '''        assert_eq!(outbox_events.len(), 1);
    }

    #[tokio::test]
    async fn seo_bulk_delivery_tracker_allows_scope_distinct_terminal_events() {
'''
    new_assertion = '''        assert_eq!(outbox_events.len(), 1);

        let index_deliveries = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("bulk index deliveries should load");
        assert_eq!(index_deliveries.len(), 1);
        assert_eq!(index_deliveries[0].target_type, "product");
        assert_eq!(index_deliveries[0].target_scope, INDEX_TARGET_SCOPE_KIND);
        assert_eq!(index_deliveries[0].status, INDEX_DELIVERY_STATUS_SENT);

        let reindex_events = outbox_entity::Entity::find()
            .filter(outbox_entity::Column::EventType.eq("index.reindex_requested"))
            .all(&db)
            .await
            .expect("bulk reindex events should load");
        assert_eq!(reindex_events.len(), 1);
    }

    #[tokio::test]
    async fn seo_bulk_delivery_tracker_allows_scope_distinct_terminal_events() {
'''
    text = replace_once(text, old_assertion, new_assertion, "bulk reindex regression assertion")
    path.write_text(text)


def patch_bulk() -> None:
    path = Path("crates/rustok-seo/src/services/bulk.rs")
    text = path.read_text()
    old = '''        self.publish_seo_bulk_terminal_event_in_tx(
            &txn,
            updated.tenant_id,
            updated.id,
            updated.target_kind.as_str(),
            updated.locale.as_str(),
            updated.status.as_str(),
            updated.processed_count,
            updated.succeeded_count,
            updated.failed_count,
        )
        .await?;
        txn.commit().await?;

        Ok(())
'''
    new = '''        let terminal_dispatch = self
            .publish_seo_bulk_terminal_event_in_tx(
                &txn,
                updated.tenant_id,
                updated.id,
                updated.target_kind.as_str(),
                updated.locale.as_str(),
                updated.status.as_str(),
                updated.processed_count,
                updated.succeeded_count,
                updated.failed_count,
            )
            .await?;
        txn.commit().await?;
        self.dispatch_seo_bulk_terminal_reindex(updated.tenant_id, terminal_dispatch)
            .await;

        Ok(())
'''
    count = text.count(old)
    if count != 2:
        raise SystemExit(f"bulk terminal call sites: expected two matches, found {count}")
    path.write_text(text.replace(old, new))


def patch_roadmap() -> None:
    path = Path("docs/roadmaps/seo-hardening-progress.md")
    text = path.read_text()
    old = "The current execution environment does not provide a Rust toolchain, and direct commits have not received GitHub status checks. These verification boxes must remain open until they are actually executed."
    new = "The connected local execution environment does not provide a Rust toolchain. PR #2022 supplied scoped Rust verification through GitHub Actions; the full-suite checkbox remains open because nine pre-existing failures outside this slice still need resolution."
    path.write_text(replace_once(text, old, new, "verification footer"))


def guard_paths() -> None:
    allowed = {
        ".github/seo_bulk_terminal_reindex_patch.py",
        ".github/workflows/seo-bulk-terminal-reindex-bootstrap.yml",
        "crates/rustok-seo/src/services/bulk.rs",
        "crates/rustok-seo/src/services/events.rs",
        "docs/roadmaps/seo-hardening-progress.md",
    }
    changed = set(
        subprocess.check_output(
            ["git", "diff", "--cached", "--name-only"], text=True
        ).splitlines()
    )
    unexpected = sorted(changed - allowed)
    required = {
        "crates/rustok-seo/src/services/bulk.rs",
        "crates/rustok-seo/src/services/events.rs",
        "docs/roadmaps/seo-hardening-progress.md",
    }
    missing = sorted(required - changed)
    if unexpected or missing:
        raise SystemExit(
            f"invalid reindex preservation paths: unexpected={unexpected}, missing={missing}"
        )


if "--guard" in sys.argv:
    guard_paths()
else:
    patch_events()
    patch_bulk()
    patch_roadmap()

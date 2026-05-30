use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_core::{simple_hash, DomainEvent};

use crate::entities::seo_event_delivery;

use super::SeoService;

const DELIVERY_STATUS_PENDING: &str = "pending";
const DELIVERY_STATUS_SENT: &str = "sent";
const DELIVERY_STATUS_FAILED: &str = "failed";
const MAX_DELIVERY_ERROR_LEN: usize = 2048;

#[allow(clippy::too_many_arguments)]
fn seo_bulk_terminal_event(
    job_id: Uuid,
    target_kind: &str,
    locale: &str,
    status: &str,
    processed_count: i32,
    succeeded_count: i32,
    failed_count: i32,
    idempotency_key: String,
) -> DomainEvent {
    match status {
        "partial" => DomainEvent::SeoBulkPartial {
            job_id,
            target_kind: target_kind.to_string(),
            locale: locale.to_string(),
            status: status.to_string(),
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key,
        },
        "failed" => DomainEvent::SeoBulkFailed {
            job_id,
            target_kind: target_kind.to_string(),
            locale: locale.to_string(),
            status: status.to_string(),
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key,
        },
        _ => DomainEvent::SeoBulkCompleted {
            job_id,
            target_kind: target_kind.to_string(),
            locale: locale.to_string(),
            status: status.to_string(),
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key,
        },
    }
}

fn build_seo_event_key(scope: &str, tenant_id: Uuid, parts: &[String]) -> String {
    let mut payload = format!("{scope}|{tenant_id}");
    for part in parts {
        payload.push('|');
        payload.push_str(part.as_str());
    }
    format!("{scope}:{:016x}", simple_hash(payload.as_str()))
}

#[derive(Debug, Clone)]
struct SeoEventDeliveryMetadata {
    idempotency_key: String,
    source_kind: Option<String>,
    source_id: Option<Uuid>,
}

impl SeoService {
    pub(super) async fn publish_seo_meta_upserted_event(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        locale: &str,
        source: &str,
        transition_ref: Option<&str>,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.meta.upserted",
            tenant_id,
            &[
                target_kind.to_string(),
                target_id.to_string(),
                locale.to_string(),
                transition_ref.unwrap_or("direct").to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoMetaUpserted {
                target_kind: target_kind.to_string(),
                target_id,
                locale: locale.to_string(),
                source: source.to_string(),
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_revision_published_event(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        revision: i32,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.revision.published",
            tenant_id,
            &[
                target_kind.to_string(),
                target_id.to_string(),
                revision.to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoRevisionPublished {
                target_kind: target_kind.to_string(),
                target_id,
                revision,
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_revision_rolled_back_event(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        revision: i32,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.revision.rolled_back",
            tenant_id,
            &[
                target_kind.to_string(),
                target_id.to_string(),
                revision.to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoRevisionRolledBack {
                target_kind: target_kind.to_string(),
                target_id,
                revision,
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_redirect_upserted_event(
        &self,
        tenant_id: Uuid,
        redirect_id: Uuid,
        source_pattern: &str,
        target_url: &str,
        status_code: i32,
        is_active: bool,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.redirect.upserted",
            tenant_id,
            &[
                redirect_id.to_string(),
                source_pattern.to_string(),
                target_url.to_string(),
                status_code.to_string(),
                is_active.to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoRedirectUpserted {
                redirect_id,
                source_pattern: source_pattern.to_string(),
                target_url: target_url.to_string(),
                status_code,
                is_active,
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_redirect_disabled_event(
        &self,
        tenant_id: Uuid,
        redirect_id: Uuid,
        source_pattern: &str,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.redirect.disabled",
            tenant_id,
            &[redirect_id.to_string(), source_pattern.to_string()],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoRedirectDisabled {
                redirect_id,
                source_pattern: source_pattern.to_string(),
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_sitemap_generated_event(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        file_count: i32,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.sitemap.generated",
            tenant_id,
            &[job_id.to_string(), file_count.to_string()],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoSitemapGenerated {
                job_id,
                file_count,
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_sitemap_submitted_event(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        endpoint_count: i32,
        success: bool,
        error: Option<String>,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.sitemap.submitted",
            tenant_id,
            &[
                job_id.to_string(),
                endpoint_count.to_string(),
                success.to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoSitemapSubmitted {
                job_id,
                endpoint_count,
                success,
                error,
                idempotency_key,
            },
        )
        .await;
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn publish_seo_bulk_completed_event(
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
        let event = seo_bulk_terminal_event(
            job_id,
            target_kind,
            locale,
            status,
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key,
        );
        self.publish_seo_event(tenant_id, event).await;
    }

    async fn publish_seo_event(&self, tenant_id: Uuid, event: DomainEvent) {
        let event_type = event.event_type().to_string();
        let Some(metadata) = event_delivery_metadata(&event) else {
            self.publish_seo_event_without_delivery_tracking(tenant_id, event)
                .await;
            return;
        };

        match self
            .load_delivery_by_idempotency_key(tenant_id, metadata.idempotency_key.as_str())
            .await
        {
            Ok(Some(existing)) => {
                tracing::debug!(
                    tenant_id = %tenant_id,
                    event_type = %event_type,
                    idempotency_key = %existing.idempotency_key,
                    "skipping duplicate SEO domain event emission"
                );
                return;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    event_type = %event_type,
                    error = %error,
                    "failed to query SEO event delivery tracker; publishing without duplicate guard"
                );
                self.publish_seo_event_without_delivery_tracking(tenant_id, event)
                    .await;
                return;
            }
        }

        let delivery = match self
            .insert_pending_delivery(tenant_id, event_type.as_str(), &metadata)
            .await
        {
            Ok(delivery) => delivery,
            Err(error) if is_duplicate_delivery_insert_error(&error) => {
                tracing::debug!(
                    tenant_id = %tenant_id,
                    event_type = %event_type,
                    idempotency_key = %metadata.idempotency_key,
                    "skipping duplicate SEO domain event emission after delivery insert conflict"
                );
                return;
            }
            Err(error) => {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    event_type = %event_type,
                    error = %error,
                    "failed to persist SEO event delivery tracker; publishing without duplicate guard"
                );
                self.publish_seo_event_without_delivery_tracking(tenant_id, event)
                    .await;
                return;
            }
        };

        match self
            .event_bus
            .publish_with_envelope_id(tenant_id, None, event)
            .await
        {
            Ok(outbox_event_id) => {
                if let Err(error) = self
                    .mark_delivery_sent(delivery.id, outbox_event_id)
                    .await
                {
                    tracing::warn!(
                        tenant_id = %tenant_id,
                        event_type = %event_type,
                        delivery_id = %delivery.id,
                        outbox_event_id = %outbox_event_id,
                        error = %error,
                        "failed to mark SEO event delivery as sent"
                    );
                }
            }
            Err(error) => {
                let error_message = limit_delivery_error_message(error.to_string());
                if let Err(update_error) = self
                    .mark_delivery_failed(delivery.id, error_message.as_str())
                    .await
                {
                    tracing::warn!(
                        tenant_id = %tenant_id,
                        event_type = %event_type,
                        delivery_id = %delivery.id,
                        error = %update_error,
                        "failed to mark SEO event delivery as failed"
                    );
                }
                tracing::warn!(
                    tenant_id = %tenant_id,
                    event_type = %event_type,
                    error = %error,
                    "failed to publish SEO domain event"
                );
            }
        }
    }

    async fn publish_seo_event_without_delivery_tracking(&self, tenant_id: Uuid, event: DomainEvent) {
        if let Err(error) = self.event_bus.publish(tenant_id, None, event.clone()).await {
            tracing::warn!(
                tenant_id = %tenant_id,
                event_type = event.event_type(),
                error = %error,
                "failed to publish SEO domain event"
            );
        }
    }

    async fn load_delivery_by_idempotency_key(
        &self,
        tenant_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<seo_event_delivery::Model>, DbErr> {
        seo_event_delivery::Entity::find()
            .filter(seo_event_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_event_delivery::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
    }

    async fn insert_pending_delivery(
        &self,
        tenant_id: Uuid,
        event_type: &str,
        metadata: &SeoEventDeliveryMetadata,
    ) -> Result<seo_event_delivery::Model, DbErr> {
        let now = Utc::now().fixed_offset();
        seo_event_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            event_type: Set(event_type.to_string()),
            idempotency_key: Set(metadata.idempotency_key.clone()),
            source_kind: Set(metadata.source_kind.clone()),
            source_id: Set(metadata.source_id),
            status: Set(DELIVERY_STATUS_PENDING.to_string()),
            outbox_event_id: Set(None),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            dispatched_at: Set(None),
        }
        .insert(&self.db)
        .await
    }

    async fn mark_delivery_sent(&self, delivery_id: Uuid, outbox_event_id: Uuid) -> Result<(), DbErr> {
        let Some(delivery) = seo_event_delivery::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let now = Utc::now().fixed_offset();
        let mut active: seo_event_delivery::ActiveModel = delivery.into();
        active.status = Set(DELIVERY_STATUS_SENT.to_string());
        active.outbox_event_id = Set(Some(outbox_event_id));
        active.last_error = Set(None);
        active.updated_at = Set(now);
        active.dispatched_at = Set(Some(now));
        active.update(&self.db).await?;
        Ok(())
    }

    async fn mark_delivery_failed(&self, delivery_id: Uuid, error: &str) -> Result<(), DbErr> {
        let Some(delivery) = seo_event_delivery::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let now = Utc::now().fixed_offset();
        let mut active: seo_event_delivery::ActiveModel = delivery.into();
        active.status = Set(DELIVERY_STATUS_FAILED.to_string());
        active.last_error = Set(Some(error.to_string()));
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    fn build_event_key(&self, scope: &str, tenant_id: Uuid, parts: &[String]) -> String {
        build_seo_event_key(scope, tenant_id, parts)
    }
}

fn event_delivery_metadata(event: &DomainEvent) -> Option<SeoEventDeliveryMetadata> {
    match event {
        DomainEvent::SeoMetaUpserted {
            target_kind,
            target_id,
            idempotency_key,
            ..
        }
        | DomainEvent::SeoRevisionPublished {
            target_kind,
            target_id,
            idempotency_key,
            ..
        }
        | DomainEvent::SeoRevisionRolledBack {
            target_kind,
            target_id,
            idempotency_key,
            ..
        } => Some(SeoEventDeliveryMetadata {
            idempotency_key: idempotency_key.clone(),
            source_kind: Some(target_kind.clone()),
            source_id: Some(*target_id),
        }),
        DomainEvent::SeoRedirectUpserted {
            redirect_id,
            idempotency_key,
            ..
        }
        | DomainEvent::SeoRedirectDisabled {
            redirect_id,
            idempotency_key,
            ..
        } => Some(SeoEventDeliveryMetadata {
            idempotency_key: idempotency_key.clone(),
            source_kind: Some("redirect".to_string()),
            source_id: Some(*redirect_id),
        }),
        DomainEvent::SeoSitemapGenerated {
            job_id,
            idempotency_key,
            ..
        }
        | DomainEvent::SeoSitemapSubmitted {
            job_id,
            idempotency_key,
            ..
        } => Some(SeoEventDeliveryMetadata {
            idempotency_key: idempotency_key.clone(),
            source_kind: Some("sitemap_job".to_string()),
            source_id: Some(*job_id),
        }),
        DomainEvent::SeoBulkCompleted {
            job_id,
            idempotency_key,
            ..
        }
        | DomainEvent::SeoBulkPartial {
            job_id,
            idempotency_key,
            ..
        }
        | DomainEvent::SeoBulkFailed {
            job_id,
            idempotency_key,
            ..
        } => Some(SeoEventDeliveryMetadata {
            idempotency_key: idempotency_key.clone(),
            source_kind: Some("bulk_job".to_string()),
            source_id: Some(*job_id),
        }),
        _ => None,
    }
}

fn limit_delivery_error_message(message: String) -> String {
    if message.len() <= MAX_DELIVERY_ERROR_LEN {
        return message;
    }

    message
        .chars()
        .take(MAX_DELIVERY_ERROR_LEN)
        .collect::<String>()
}

fn is_duplicate_delivery_insert_error(error: &DbErr) -> bool {
    let lowered = error.to_string().to_ascii_lowercase();
    lowered.contains("unique")
        && (lowered.contains("seo_event_deliveries")
            || lowered.contains("idx_seo_event_deliveries_idempotency"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustok_outbox::{entity as outbox_entity, OutboxTransport, SysEventsMigration, TransactionalEventBus};
    use rustok_seo_targets::SeoTargetRegistry;
    use sea_orm::{
        ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter,
    };
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use crate::migrations as seo_migrations;

    use super::*;

    async fn test_db() -> DatabaseConnection {
        let db_url = format!("sqlite:file:seo_events_{}?mode=memory&cache=shared", Uuid::new_v4());
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);
        Database::connect(opts)
            .await
            .expect("failed to connect seo events sqlite db")
    }

    async fn run_migrations(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
        for migration in seo_migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("seo migration should apply");
        }
    }

    fn service_with_outbox(db: DatabaseConnection) -> SeoService {
        let transport = Arc::new(OutboxTransport::new(db.clone()));
        SeoService::new(
            db,
            TransactionalEventBus::new(transport),
            Arc::new(SeoTargetRegistry::default()),
        )
    }

    #[test]
    fn seo_bulk_terminal_event_uses_status_specific_variants() {
        let job_id = Uuid::from_u128(42);

        let completed = seo_bulk_terminal_event(
            job_id,
            "product",
            "en-US",
            "completed",
            3,
            3,
            0,
            "completed-key".to_string(),
        );
        assert!(matches!(completed, DomainEvent::SeoBulkCompleted { .. }));
        assert_eq!(completed.event_type(), "seo.bulk.completed");

        let partial = seo_bulk_terminal_event(
            job_id,
            "product",
            "en-US",
            "partial",
            3,
            2,
            1,
            "partial-key".to_string(),
        );
        assert!(matches!(partial, DomainEvent::SeoBulkPartial { .. }));
        assert_eq!(partial.event_type(), "seo.bulk.partial");

        let failed = seo_bulk_terminal_event(
            job_id,
            "product",
            "en-US",
            "failed",
            3,
            0,
            3,
            "failed-key".to_string(),
        );
        assert!(matches!(failed, DomainEvent::SeoBulkFailed { .. }));
        assert_eq!(failed.event_type(), "seo.bulk.failed");
    }

    #[test]
    fn seo_event_keys_are_deterministic_and_scope_sensitive() {
        let tenant_id = Uuid::from_u128(7);
        let completed = build_seo_event_key(
            "seo.bulk.completed",
            tenant_id,
            &[
                "product".to_string(),
                "en-US".to_string(),
                "job-1".to_string(),
                "completed".to_string(),
            ],
        );
        let repeated = build_seo_event_key(
            "seo.bulk.completed",
            tenant_id,
            &[
                "product".to_string(),
                "en-US".to_string(),
                "job-1".to_string(),
                "completed".to_string(),
            ],
        );
        let partial = build_seo_event_key(
            "seo.bulk.partial",
            tenant_id,
            &[
                "product".to_string(),
                "en-US".to_string(),
                "job-1".to_string(),
                "partial".to_string(),
            ],
        );

        assert_eq!(completed, repeated);
        assert_ne!(completed, partial);
        assert!(completed.starts_with("seo.bulk.completed:"));
        assert!(partial.starts_with("seo.bulk.partial:"));
    }

    #[tokio::test]
    async fn seo_bulk_delivery_tracker_skips_duplicate_terminal_emission() {
        let db = test_db().await;
        run_migrations(&db).await;
        let service = service_with_outbox(db.clone());

        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();

        service
            .publish_seo_bulk_completed_event(
                tenant_id, job_id, "product", "en-US", "completed", 10, 10, 0,
            )
            .await;
        service
            .publish_seo_bulk_completed_event(
                tenant_id, job_id, "product", "en-US", "completed", 10, 10, 0,
            )
            .await;

        let deliveries = seo_event_delivery::Entity::find()
            .filter(seo_event_delivery::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("seo deliveries should load");
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].status, DELIVERY_STATUS_SENT);
        assert!(deliveries[0].outbox_event_id.is_some());

        let outbox_events = outbox_entity::Entity::find()
            .filter(outbox_entity::Column::EventType.eq("seo.bulk.completed"))
            .all(&db)
            .await
            .expect("outbox events should load");
        assert_eq!(outbox_events.len(), 1);
    }

    #[tokio::test]
    async fn seo_bulk_delivery_tracker_allows_scope_distinct_terminal_events() {
        let db = test_db().await;
        run_migrations(&db).await;
        let service = service_with_outbox(db.clone());

        let tenant_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();

        service
            .publish_seo_bulk_completed_event(
                tenant_id, job_id, "product", "en-US", "completed", 10, 8, 2,
            )
            .await;
        service
            .publish_seo_bulk_completed_event(
                tenant_id, job_id, "product", "en-US", "partial", 10, 8, 2,
            )
            .await;

        let deliveries = seo_event_delivery::Entity::find()
            .filter(seo_event_delivery::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("seo deliveries should load");
        assert_eq!(deliveries.len(), 2);
        assert!(deliveries.iter().all(|item| item.outbox_event_id.is_some()));

        let outbox_events = outbox_entity::Entity::find()
            .filter(outbox_entity::Column::EventType.contains("seo.bulk."))
            .all(&db)
            .await
            .expect("outbox events should load");
        assert_eq!(outbox_events.len(), 2);
    }
}

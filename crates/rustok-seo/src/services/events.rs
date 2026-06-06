use std::collections::BTreeSet;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbErr, EntityTrait, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use rustok_core::{simple_hash, DomainEvent};

use crate::dto::{
    SeoIndexCursorRecord, SeoIndexDeliveryStatusRecord, SeoIndexRepairReplayResultRecord,
    SeoIndexReplayMode,
};
use crate::entities::{seo_event_delivery, seo_index_cursor, seo_index_delivery};
use crate::{SeoError, SeoResult};

use super::SeoService;

const DELIVERY_STATUS_PENDING: &str = "pending";
const DELIVERY_STATUS_SENT: &str = "sent";
const DELIVERY_STATUS_FAILED: &str = "failed";

const INDEX_DELIVERY_STATUS_PENDING: &str = "pending";
const INDEX_DELIVERY_STATUS_SENT: &str = "sent";
const INDEX_DELIVERY_STATUS_FAILED: &str = "failed";
const INDEX_DELIVERY_STATUS_DEAD_LETTER: &str = "dead_letter";

const INDEX_TARGET_SCOPE_ENTITY: &str = "entity";
const INDEX_TARGET_SCOPE_KIND: &str = "kind";
const INDEX_SCOPE_KEY_ALL: &str = "*";

const INDEX_CURSOR_REPLAY_MODE_NOT_STARTED: &str = "not_started";
const INDEX_CURSOR_REPLAY_MODE_REPAIR_ONLY: &str = "repair_only";
const INDEX_CURSOR_REPLAY_MODE_REPLAY_REQUESTED: &str = "replay_requested";
const INDEX_CURSOR_REPLAY_MODE_REPLAYING: &str = "replaying";
const INDEX_CURSOR_REPLAY_MODE_REPLAY_COMPLETED: &str = "replay_completed";

const INDEX_RETRY_MAX_ATTEMPTS: i32 = 3;
const INDEX_RETRY_BASE_BACKOFF_MS: u64 = 100;

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

#[derive(Debug, Clone)]
struct SeoIndexReindexTrigger {
    target_type: String,
    target_id: Option<Uuid>,
    target_scope: String,
    target_scope_key: String,
}

#[derive(Debug, Clone, Default)]
struct SeoHistoricalReplayStats {
    events_scanned: usize,
    replayed_count: usize,
    replay_run_id: Option<Uuid>,
}

impl SeoIndexReindexTrigger {
    fn entity(target_type: &str, target_id: Uuid) -> Self {
        Self {
            target_type: target_type.to_string(),
            target_id: Some(target_id),
            target_scope: INDEX_TARGET_SCOPE_ENTITY.to_string(),
            target_scope_key: target_id.to_string(),
        }
    }

    fn kind(target_type: &str) -> Self {
        Self {
            target_type: target_type.to_string(),
            target_id: None,
            target_scope: INDEX_TARGET_SCOPE_KIND.to_string(),
            target_scope_key: INDEX_SCOPE_KEY_ALL.to_string(),
        }
    }

    fn from_delivery(model: &seo_index_delivery::Model) -> Self {
        Self {
            target_type: model.target_type.clone(),
            target_id: model.target_id,
            target_scope: model.target_scope.clone(),
            target_scope_key: model.target_scope_key.clone(),
        }
    }
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

    pub async fn index_delivery_status(
        &self,
        tenant_id: Uuid,
        target_type: Option<&str>,
    ) -> SeoResult<SeoIndexDeliveryStatusRecord> {
        let normalized_target_type = normalize_index_target_type(target_type)?;

        let mut query = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id));
        if let Some(target_type) = normalized_target_type.as_deref() {
            query = query.filter(seo_index_delivery::Column::TargetType.eq(target_type));
        }

        let deliveries = query.all(&self.db).await?;
        let mut summary = SeoIndexDeliveryStatusRecord {
            target_type: normalized_target_type.clone(),
            ..SeoIndexDeliveryStatusRecord::default()
        };

        for delivery in deliveries {
            match delivery.status.as_str() {
                INDEX_DELIVERY_STATUS_PENDING => summary.pending_count += 1,
                INDEX_DELIVERY_STATUS_SENT => summary.sent_count += 1,
                INDEX_DELIVERY_STATUS_FAILED if delivery.attempt_count > 0 => {
                    summary.retry_count += 1
                }
                INDEX_DELIVERY_STATUS_FAILED => summary.failed_count += 1,
                INDEX_DELIVERY_STATUS_DEAD_LETTER => summary.dead_letter_count += 1,
                _ => {}
            }
        }

        let mut cursor_query = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id));
        if let Some(target_type) = normalized_target_type.as_deref() {
            cursor_query =
                cursor_query.filter(seo_index_cursor::Column::TargetType.eq(target_type));
        }
        let mut cursors = cursor_query.all(&self.db).await?;
        cursors.sort_by(|left, right| left.target_type.cmp(&right.target_type));

        summary.cursors = cursors
            .into_iter()
            .map(|cursor| SeoIndexCursorRecord {
                target_type: cursor.target_type,
                initial_cursor_at: cursor.initial_cursor_at.with_timezone(&Utc),
                high_water_mark_at: cursor.high_water_mark_at.with_timezone(&Utc),
                last_repair_cursor_at: cursor
                    .last_repair_cursor_at
                    .map(|value| value.with_timezone(&Utc)),
                replay_mode: parse_replay_mode(cursor.replay_mode.as_str()),
                replay_requested_at: cursor
                    .replay_requested_at
                    .map(|value| value.with_timezone(&Utc)),
                replay_completed_at: cursor
                    .replay_completed_at
                    .map(|value| value.with_timezone(&Utc)),
            })
            .collect();

        Ok(summary)
    }

    pub async fn run_index_repair_replay(
        &self,
        tenant_id: Uuid,
        target_type: Option<&str>,
        limit: usize,
        replay_historical: bool,
    ) -> SeoResult<SeoIndexRepairReplayResultRecord> {
        let normalized_target_type = normalize_index_target_type(target_type)?;
        let bounded_limit = limit.clamp(1, 500);

        let repaired_count = self
            .repair_index_delivery_backlog(
                tenant_id,
                normalized_target_type.as_deref(),
                bounded_limit,
            )
            .await?;

        let replay_stats = if replay_historical {
            self.replay_historical_index_changes(
                tenant_id,
                normalized_target_type.as_deref(),
                bounded_limit,
            )
            .await?
        } else {
            SeoHistoricalReplayStats::default()
        };

        Ok(SeoIndexRepairReplayResultRecord {
            target_type: normalized_target_type,
            limit: bounded_limit as i32,
            replay_mode: if replay_historical {
                SeoIndexReplayMode::ReplayCompleted
            } else {
                SeoIndexReplayMode::RepairOnly
            },
            repaired_count: repaired_count as i32,
            replayed_count: replay_stats.replayed_count as i32,
            historical_events_scanned: replay_stats.events_scanned as i32,
            replay_run_id: replay_stats.replay_run_id,
        })
    }

    pub async fn repair_index_delivery_backlog(
        &self,
        tenant_id: Uuid,
        target_type: Option<&str>,
        limit: usize,
    ) -> SeoResult<usize> {
        let normalized_target_type = normalize_index_target_type(target_type)?;
        let bounded_limit = limit.clamp(1, 500);

        let mut query = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .filter(
                Condition::any()
                    .add(seo_index_delivery::Column::Status.eq(INDEX_DELIVERY_STATUS_FAILED))
                    .add(seo_index_delivery::Column::Status.eq(INDEX_DELIVERY_STATUS_DEAD_LETTER)),
            )
            .order_by_asc(seo_index_delivery::Column::CreatedAt)
            .limit(bounded_limit as u64);

        if let Some(target_type) = normalized_target_type.as_deref() {
            query = query.filter(seo_index_delivery::Column::TargetType.eq(target_type));
        }

        let deliveries = query.all(&self.db).await?;
        let mut repaired = 0usize;

        for delivery in deliveries {
            self.reset_index_delivery_for_repair(delivery.id).await?;
            let Some(delivery) = seo_index_delivery::Entity::find_by_id(delivery.id)
                .one(&self.db)
                .await?
            else {
                continue;
            };
            let trigger = SeoIndexReindexTrigger::from_delivery(&delivery);
            self.dispatch_index_reindex_trigger(tenant_id, &delivery, &trigger)
                .await;
            if let Err(error) = self
                .mark_index_cursor_repair_progress(
                    tenant_id,
                    trigger.target_type.as_str(),
                    delivery.created_at,
                    INDEX_CURSOR_REPLAY_MODE_REPAIR_ONLY,
                )
                .await
            {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    target_type = %trigger.target_type,
                    error = %error,
                    "failed to mark SEO index cursor repair progress"
                );
            }
            repaired += 1;
        }

        Ok(repaired)
    }

    async fn replay_historical_index_changes(
        &self,
        tenant_id: Uuid,
        target_type: Option<&str>,
        limit: usize,
    ) -> SeoResult<SeoHistoricalReplayStats> {
        let normalized_target_type = normalize_index_target_type(target_type)?;
        let bounded_limit = limit.clamp(1, 500);

        let historical_events = seo_event_delivery::Entity::find()
            .filter(seo_event_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_event_delivery::Column::Status.eq(DELIVERY_STATUS_SENT))
            .order_by_asc(seo_event_delivery::Column::CreatedAt)
            .limit(bounded_limit as u64)
            .all(&self.db)
            .await?;

        let mut replay_work = Vec::<(seo_event_delivery::Model, SeoIndexReindexTrigger)>::new();
        for event_delivery in &historical_events {
            let triggers = index_reindex_triggers_for_delivery_event(
                event_delivery.event_type.as_str(),
                event_delivery.source_kind.as_deref(),
                event_delivery.source_id,
                normalized_target_type.as_deref(),
            );
            for trigger in triggers {
                replay_work.push((event_delivery.clone(), trigger));
            }
        }

        if replay_work.is_empty() {
            return Ok(SeoHistoricalReplayStats {
                events_scanned: historical_events.len(),
                replayed_count: 0,
                replay_run_id: None,
            });
        }

        let replay_run_id = Uuid::new_v4();
        let touched_target_types = replay_work
            .iter()
            .map(|(_, trigger)| trigger.target_type.clone())
            .collect::<BTreeSet<_>>();

        let requested_at = Utc::now().fixed_offset();
        for target in &touched_target_types {
            self.upsert_index_cursor(tenant_id, target.as_str(), requested_at)
                .await?;
            self.mark_index_cursor_replay_requested(tenant_id, target.as_str(), requested_at)
                .await?;
        }

        let mut replayed_count = 0usize;
        for (event_delivery, trigger) in replay_work {
            let replay_idempotency_key = self.build_event_key(
                "seo.index.replay.historical",
                tenant_id,
                &[
                    event_delivery.idempotency_key.clone(),
                    trigger.target_type.clone(),
                    trigger.target_scope_key.clone(),
                    replay_run_id.to_string(),
                ],
            );

            let delivery = self
                .upsert_index_delivery(
                    tenant_id,
                    event_delivery.event_type.as_str(),
                    replay_idempotency_key.as_str(),
                    &trigger,
                )
                .await?;
            self.dispatch_index_reindex_trigger(tenant_id, &delivery, &trigger)
                .await;
            if let Err(error) = self
                .mark_index_cursor_repair_progress(
                    tenant_id,
                    trigger.target_type.as_str(),
                    event_delivery.created_at,
                    INDEX_CURSOR_REPLAY_MODE_REPLAYING,
                )
                .await
            {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    target_type = %trigger.target_type,
                    error = %error,
                    "failed to mark SEO index cursor replay progress"
                );
            }
            replayed_count += 1;
        }

        let completed_at = Utc::now().fixed_offset();
        for target in &touched_target_types {
            self.mark_index_cursor_replay_completed(tenant_id, target.as_str(), completed_at)
                .await?;
        }

        Ok(SeoHistoricalReplayStats {
            events_scanned: historical_events.len(),
            replayed_count,
            replay_run_id: Some(replay_run_id),
        })
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

        let index_reindex_event = event.clone();
        let index_reindex_idempotency_key = metadata.idempotency_key.clone();

        match self
            .event_bus
            .publish_with_envelope_id(tenant_id, None, event)
            .await
        {
            Ok(outbox_event_id) => {
                if let Err(error) = self.mark_delivery_sent(delivery.id, outbox_event_id).await {
                    tracing::warn!(
                        tenant_id = %tenant_id,
                        event_type = %event_type,
                        delivery_id = %delivery.id,
                        outbox_event_id = %outbox_event_id,
                        error = %error,
                        "failed to mark SEO event delivery as sent"
                    );
                }
                self.dispatch_index_reindex_for_event(
                    tenant_id,
                    event_type.as_str(),
                    index_reindex_idempotency_key.as_str(),
                    &index_reindex_event,
                )
                .await;
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

    async fn publish_seo_event_without_delivery_tracking(
        &self,
        tenant_id: Uuid,
        event: DomainEvent,
    ) {
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

    async fn mark_delivery_sent(
        &self,
        delivery_id: Uuid,
        outbox_event_id: Uuid,
    ) -> Result<(), DbErr> {
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

    async fn dispatch_index_reindex_for_event(
        &self,
        tenant_id: Uuid,
        seo_event_type: &str,
        idempotency_key: &str,
        event: &DomainEvent,
    ) {
        let triggers = index_reindex_triggers_for_event(event);
        if triggers.is_empty() {
            return;
        }

        for trigger in triggers {
            let delivery = match self
                .upsert_index_delivery(tenant_id, seo_event_type, idempotency_key, &trigger)
                .await
            {
                Ok(delivery) => delivery,
                Err(error) => {
                    tracing::warn!(
                        tenant_id = %tenant_id,
                        seo_event_type,
                        idempotency_key,
                        target_type = %trigger.target_type,
                        target_id = ?trigger.target_id,
                        error = %error,
                        "failed to prepare SEO index delivery"
                    );
                    continue;
                }
            };

            if let Err(error) = self
                .upsert_index_cursor(tenant_id, trigger.target_type.as_str(), delivery.created_at)
                .await
            {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    target_type = %trigger.target_type,
                    error = %error,
                    "failed to update SEO index high-water cursor"
                );
            }

            self.dispatch_index_reindex_trigger(tenant_id, &delivery, &trigger)
                .await;
        }
    }

    async fn dispatch_index_reindex_trigger(
        &self,
        tenant_id: Uuid,
        delivery: &seo_index_delivery::Model,
        trigger: &SeoIndexReindexTrigger,
    ) {
        if delivery.status == INDEX_DELIVERY_STATUS_SENT
            || delivery.status == INDEX_DELIVERY_STATUS_DEAD_LETTER
        {
            return;
        }

        let mut attempts = delivery.attempt_count.max(0);
        while attempts < INDEX_RETRY_MAX_ATTEMPTS {
            attempts += 1;
            match self
                .event_bus
                .publish_with_envelope_id(
                    tenant_id,
                    None,
                    DomainEvent::ReindexRequested {
                        target_type: trigger.target_type.clone(),
                        target_id: trigger.target_id,
                    },
                )
                .await
            {
                Ok(outbox_event_id) => {
                    if let Err(error) = self
                        .mark_index_delivery_sent(delivery.id, attempts, outbox_event_id)
                        .await
                    {
                        tracing::warn!(
                            tenant_id = %tenant_id,
                            target_type = %trigger.target_type,
                            target_id = ?trigger.target_id,
                            delivery_id = %delivery.id,
                            outbox_event_id = %outbox_event_id,
                            error = %error,
                            "failed to mark SEO index delivery as sent"
                        );
                    }
                    return;
                }
                Err(error) => {
                    let error_message = limit_delivery_error_message(error.to_string());
                    let is_terminal = attempts >= INDEX_RETRY_MAX_ATTEMPTS;
                    if is_terminal {
                        if let Err(update_error) = self
                            .mark_index_delivery_dead_letter(
                                delivery.id,
                                attempts,
                                error_message.as_str(),
                            )
                            .await
                        {
                            tracing::warn!(
                                tenant_id = %tenant_id,
                                target_type = %trigger.target_type,
                                target_id = ?trigger.target_id,
                                delivery_id = %delivery.id,
                                error = %update_error,
                                "failed to mark SEO index delivery as dead-letter"
                            );
                        }
                        tracing::warn!(
                            tenant_id = %tenant_id,
                            target_type = %trigger.target_type,
                            target_id = ?trigger.target_id,
                            delivery_id = %delivery.id,
                            attempts,
                            error = %error,
                            "SEO index trigger reached bounded retry limit"
                        );
                        return;
                    }

                    let next_attempt_at = Utc::now().fixed_offset()
                        + ChronoDuration::from_std(index_retry_backoff(attempts))
                            .unwrap_or_else(|_| ChronoDuration::milliseconds(100));
                    if let Err(update_error) = self
                        .mark_index_delivery_failed(
                            delivery.id,
                            attempts,
                            error_message.as_str(),
                            Some(next_attempt_at),
                        )
                        .await
                    {
                        tracing::warn!(
                            tenant_id = %tenant_id,
                            target_type = %trigger.target_type,
                            target_id = ?trigger.target_id,
                            delivery_id = %delivery.id,
                            error = %update_error,
                            "failed to persist SEO index retry attempt"
                        );
                    }
                    tokio::time::sleep(index_retry_backoff(attempts)).await;
                }
            }
        }
    }

    async fn upsert_index_delivery(
        &self,
        tenant_id: Uuid,
        seo_event_type: &str,
        idempotency_key: &str,
        trigger: &SeoIndexReindexTrigger,
    ) -> Result<seo_index_delivery::Model, DbErr> {
        if let Some(existing) = self
            .load_index_delivery(
                tenant_id,
                idempotency_key,
                trigger.target_type.as_str(),
                trigger.target_scope_key.as_str(),
            )
            .await?
        {
            return Ok(existing);
        }

        let now = Utc::now().fixed_offset();
        let insert = seo_index_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            seo_event_type: Set(seo_event_type.to_string()),
            idempotency_key: Set(idempotency_key.to_string()),
            target_type: Set(trigger.target_type.clone()),
            target_id: Set(trigger.target_id),
            target_scope: Set(trigger.target_scope.clone()),
            target_scope_key: Set(trigger.target_scope_key.clone()),
            status: Set(INDEX_DELIVERY_STATUS_PENDING.to_string()),
            attempt_count: Set(0),
            outbox_event_id: Set(None),
            next_attempt_at: Set(None),
            last_error: Set(None),
            dead_lettered_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            dispatched_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(error) if is_duplicate_index_delivery_insert_error(&error) => self
                .load_index_delivery(
                    tenant_id,
                    idempotency_key,
                    trigger.target_type.as_str(),
                    trigger.target_scope_key.as_str(),
                )
                .await?
                .ok_or(error),
            Err(error) => Err(error),
        }
    }

    async fn load_index_delivery(
        &self,
        tenant_id: Uuid,
        idempotency_key: &str,
        target_type: &str,
        target_scope_key: &str,
    ) -> Result<Option<seo_index_delivery::Model>, DbErr> {
        seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_index_delivery::Column::IdempotencyKey.eq(idempotency_key))
            .filter(seo_index_delivery::Column::TargetType.eq(target_type))
            .filter(seo_index_delivery::Column::TargetScopeKey.eq(target_scope_key))
            .one(&self.db)
            .await
    }

    async fn mark_index_delivery_sent(
        &self,
        delivery_id: Uuid,
        attempt_count: i32,
        outbox_event_id: Uuid,
    ) -> Result<(), DbErr> {
        let Some(delivery) = seo_index_delivery::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let now = Utc::now().fixed_offset();
        let mut active: seo_index_delivery::ActiveModel = delivery.into();
        active.status = Set(INDEX_DELIVERY_STATUS_SENT.to_string());
        active.attempt_count = Set(attempt_count);
        active.outbox_event_id = Set(Some(outbox_event_id));
        active.next_attempt_at = Set(None);
        active.last_error = Set(None);
        active.dead_lettered_at = Set(None);
        active.updated_at = Set(now);
        active.dispatched_at = Set(Some(now));
        active.update(&self.db).await?;
        Ok(())
    }

    async fn mark_index_delivery_failed(
        &self,
        delivery_id: Uuid,
        attempt_count: i32,
        error: &str,
        next_attempt_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> Result<(), DbErr> {
        let Some(delivery) = seo_index_delivery::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let now = Utc::now().fixed_offset();
        let mut active: seo_index_delivery::ActiveModel = delivery.into();
        active.status = Set(INDEX_DELIVERY_STATUS_FAILED.to_string());
        active.attempt_count = Set(attempt_count);
        active.next_attempt_at = Set(next_attempt_at);
        active.last_error = Set(Some(error.to_string()));
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn mark_index_delivery_dead_letter(
        &self,
        delivery_id: Uuid,
        attempt_count: i32,
        error: &str,
    ) -> Result<(), DbErr> {
        let Some(delivery) = seo_index_delivery::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let now = Utc::now().fixed_offset();
        let mut active: seo_index_delivery::ActiveModel = delivery.into();
        active.status = Set(INDEX_DELIVERY_STATUS_DEAD_LETTER.to_string());
        active.attempt_count = Set(attempt_count);
        active.next_attempt_at = Set(None);
        active.last_error = Set(Some(error.to_string()));
        active.dead_lettered_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn reset_index_delivery_for_repair(&self, delivery_id: Uuid) -> Result<(), DbErr> {
        let Some(delivery) = seo_index_delivery::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let now = Utc::now().fixed_offset();
        let mut active: seo_index_delivery::ActiveModel = delivery.into();
        active.status = Set(INDEX_DELIVERY_STATUS_PENDING.to_string());
        active.attempt_count = Set(0);
        active.next_attempt_at = Set(None);
        active.last_error = Set(None);
        active.dead_lettered_at = Set(None);
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn upsert_index_cursor(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        observed_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<(), DbErr> {
        let existing = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
            .filter(seo_index_cursor::Column::TargetType.eq(target_type))
            .one(&self.db)
            .await?;

        if let Some(existing) = existing {
            if existing.high_water_mark_at >= observed_at {
                return Ok(());
            }

            let mut active: seo_index_cursor::ActiveModel = existing.into();
            active.high_water_mark_at = Set(observed_at);
            active.updated_at = Set(Utc::now().fixed_offset());
            active.update(&self.db).await?;
            return Ok(());
        }

        seo_index_cursor::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            target_type: Set(target_type.to_string()),
            initial_cursor_at: Set(observed_at),
            high_water_mark_at: Set(observed_at),
            last_repair_cursor_at: Set(None),
            replay_mode: Set(INDEX_CURSOR_REPLAY_MODE_NOT_STARTED.to_string()),
            replay_requested_at: Set(None),
            replay_completed_at: Set(None),
            created_at: Set(Utc::now().fixed_offset()),
            updated_at: Set(Utc::now().fixed_offset()),
        }
        .insert(&self.db)
        .await?;

        Ok(())
    }

    async fn mark_index_cursor_repair_progress(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        cursor: chrono::DateTime<chrono::FixedOffset>,
        replay_mode: &str,
    ) -> Result<(), DbErr> {
        let Some(existing) = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
            .filter(seo_index_cursor::Column::TargetType.eq(target_type))
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let mut active: seo_index_cursor::ActiveModel = existing.clone().into();
        active.last_repair_cursor_at = Set(Some(cursor));
        active.replay_mode = Set(advance_replay_mode(
            existing.replay_mode.as_str(),
            replay_mode,
        ));
        active.updated_at = Set(Utc::now().fixed_offset());
        active.update(&self.db).await?;
        Ok(())
    }

    async fn mark_index_cursor_replay_requested(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        requested_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<(), DbErr> {
        let Some(existing) = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
            .filter(seo_index_cursor::Column::TargetType.eq(target_type))
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let mut active: seo_index_cursor::ActiveModel = existing.clone().into();
        active.replay_mode = Set(advance_replay_mode(
            existing.replay_mode.as_str(),
            INDEX_CURSOR_REPLAY_MODE_REPLAY_REQUESTED,
        ));
        active.replay_requested_at = Set(Some(requested_at));
        active.updated_at = Set(Utc::now().fixed_offset());
        active.update(&self.db).await?;
        Ok(())
    }

    async fn mark_index_cursor_replay_completed(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        completed_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<(), DbErr> {
        let Some(existing) = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
            .filter(seo_index_cursor::Column::TargetType.eq(target_type))
            .one(&self.db)
            .await?
        else {
            return Ok(());
        };

        let mut active: seo_index_cursor::ActiveModel = existing.clone().into();
        active.replay_mode = Set(advance_replay_mode(
            existing.replay_mode.as_str(),
            INDEX_CURSOR_REPLAY_MODE_REPLAY_COMPLETED,
        ));
        active.replay_completed_at = Set(Some(completed_at));
        active.updated_at = Set(Utc::now().fixed_offset());
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

fn is_duplicate_index_delivery_insert_error(error: &DbErr) -> bool {
    let lowered = error.to_string().to_ascii_lowercase();
    lowered.contains("unique")
        && (lowered.contains("seo_index_deliveries")
            || lowered.contains("idx_seo_index_deliveries_unique_transition"))
}

fn index_retry_backoff(attempt: i32) -> Duration {
    let exponent = (attempt.max(1) as u32).saturating_sub(1).min(4);
    let multiplier = 1u64 << exponent;
    Duration::from_millis(INDEX_RETRY_BASE_BACKOFF_MS * multiplier)
}

fn index_reindex_triggers_for_event(event: &DomainEvent) -> Vec<SeoIndexReindexTrigger> {
    match event {
        DomainEvent::SeoMetaUpserted {
            target_kind,
            target_id,
            ..
        }
        | DomainEvent::SeoRevisionPublished {
            target_kind,
            target_id,
            ..
        }
        | DomainEvent::SeoRevisionRolledBack {
            target_kind,
            target_id,
            ..
        } => map_target_type_for_seo_kind(target_kind)
            .map(|target_type| vec![SeoIndexReindexTrigger::entity(target_type, *target_id)])
            .unwrap_or_default(),
        DomainEvent::SeoBulkCompleted { target_kind, .. }
        | DomainEvent::SeoBulkPartial { target_kind, .. }
        | DomainEvent::SeoBulkFailed { target_kind, .. } => {
            map_target_type_for_seo_kind(target_kind)
                .map(|target_type| vec![SeoIndexReindexTrigger::kind(target_type)])
                .unwrap_or_default()
        }
        DomainEvent::SeoRedirectUpserted { .. } | DomainEvent::SeoRedirectDisabled { .. } => {
            vec![
                SeoIndexReindexTrigger::kind("content"),
                SeoIndexReindexTrigger::kind("product"),
            ]
        }
        _ => Vec::new(),
    }
}

fn map_target_type_for_seo_kind(target_kind: &str) -> Option<&'static str> {
    match target_kind {
        "product" => Some("product"),
        "page" | "blog_post" | "forum_category" | "forum_topic" => Some("content"),
        _ => None,
    }
}

fn index_reindex_triggers_for_delivery_event(
    event_type: &str,
    source_kind: Option<&str>,
    source_id: Option<Uuid>,
    target_type_filter: Option<&str>,
) -> Vec<SeoIndexReindexTrigger> {
    let triggers = match event_type {
        "seo.meta.upserted" | "seo.revision.published" | "seo.revision.rolled_back" => {
            let Some(source_kind) = source_kind else {
                return Vec::new();
            };
            let Some(source_id) = source_id else {
                return Vec::new();
            };
            map_target_type_for_seo_kind(source_kind)
                .map(|target_type| vec![SeoIndexReindexTrigger::entity(target_type, source_id)])
                .unwrap_or_default()
        }
        "seo.redirect.upserted" | "seo.redirect.disabled" => vec![
            SeoIndexReindexTrigger::kind("content"),
            SeoIndexReindexTrigger::kind("product"),
        ],
        "seo.bulk.completed" | "seo.bulk.partial" | "seo.bulk.failed" => {
            if let Some(target_type) = target_type_filter {
                vec![SeoIndexReindexTrigger::kind(target_type)]
            } else {
                vec![
                    SeoIndexReindexTrigger::kind("content"),
                    SeoIndexReindexTrigger::kind("product"),
                ]
            }
        }
        _ => Vec::new(),
    };

    if let Some(target_type) = target_type_filter {
        triggers
            .into_iter()
            .filter(|trigger| trigger.target_type == target_type)
            .collect()
    } else {
        triggers
    }
}

fn normalize_index_target_type(target_type: Option<&str>) -> SeoResult<Option<String>> {
    let Some(value) = target_type else {
        return Ok(None);
    };

    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Ok(None);
    }

    match normalized.as_str() {
        "content" | "product" => Ok(Some(normalized)),
        _ => Err(SeoError::validation(format!(
            "unsupported index target_type `{}`; expected `content` or `product`",
            value.trim()
        ))),
    }
}

fn replay_mode_rank(mode: &str) -> u8 {
    match mode {
        INDEX_CURSOR_REPLAY_MODE_NOT_STARTED => 0,
        INDEX_CURSOR_REPLAY_MODE_REPAIR_ONLY => 1,
        INDEX_CURSOR_REPLAY_MODE_REPLAY_REQUESTED => 2,
        INDEX_CURSOR_REPLAY_MODE_REPLAYING => 3,
        INDEX_CURSOR_REPLAY_MODE_REPLAY_COMPLETED => 4,
        _ => 0,
    }
}

fn advance_replay_mode(current: &str, next: &str) -> String {
    if replay_mode_rank(next) >= replay_mode_rank(current) {
        next.to_string()
    } else {
        current.to_string()
    }
}

fn parse_replay_mode(value: &str) -> SeoIndexReplayMode {
    SeoIndexReplayMode::parse(value).unwrap_or(SeoIndexReplayMode::NotStarted)
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::{Arc, Mutex};

    use rustok_core::events::{EventTransport, ReliabilityLevel};
    use rustok_core::{Error as CoreError, Result as CoreResult};
    use rustok_events::EventEnvelope;
    use rustok_outbox::{
        entity as outbox_entity, OutboxTransport, SysEventsMigration, TransactionalEventBus,
    };
    use rustok_seo_targets::SeoTargetRegistry;
    use sea_orm::{
        ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter,
    };
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use crate::migrations as seo_migrations;

    use super::*;

    async fn test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:seo_events_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
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

    fn service_with_transport(
        db: DatabaseConnection,
        transport: Arc<dyn EventTransport>,
    ) -> SeoService {
        SeoService::new(
            db,
            TransactionalEventBus::new(transport),
            Arc::new(SeoTargetRegistry::default()),
        )
    }

    #[derive(Default)]
    struct FlakyIndexTransport {
        fail_reindex_attempts: AtomicI32,
        published_event_types: Mutex<Vec<String>>,
    }

    impl FlakyIndexTransport {
        fn with_reindex_failures(fail_reindex_attempts: i32) -> Self {
            Self {
                fail_reindex_attempts: AtomicI32::new(fail_reindex_attempts.max(0)),
                published_event_types: Mutex::new(Vec::new()),
            }
        }

        fn set_fail_reindex_attempts(&self, attempts: i32) {
            self.fail_reindex_attempts
                .store(attempts.max(0), Ordering::SeqCst);
        }

        fn published_count(&self, event_type: &str) -> usize {
            self.published_event_types
                .lock()
                .expect("published_event_types mutex")
                .iter()
                .filter(|item| item.as_str() == event_type)
                .count()
        }
    }

    #[async_trait::async_trait]
    impl EventTransport for FlakyIndexTransport {
        async fn publish(&self, envelope: EventEnvelope) -> CoreResult<()> {
            self.published_event_types
                .lock()
                .expect("published_event_types mutex")
                .push(envelope.event_type.clone());

            if envelope.event_type == "index.reindex_requested"
                && self.fail_reindex_attempts.load(Ordering::SeqCst) > 0
            {
                self.fail_reindex_attempts.fetch_sub(1, Ordering::SeqCst);
                return Err(CoreError::External(
                    "forced index reindex trigger failure".to_string(),
                ));
            }

            Ok(())
        }

        fn reliability_level(&self) -> ReliabilityLevel {
            ReliabilityLevel::InMemory
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
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
                tenant_id,
                job_id,
                "product",
                "en-US",
                "completed",
                10,
                10,
                0,
            )
            .await;
        service
            .publish_seo_bulk_completed_event(
                tenant_id,
                job_id,
                "product",
                "en-US",
                "completed",
                10,
                10,
                0,
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
                tenant_id,
                job_id,
                "product",
                "en-US",
                "completed",
                10,
                8,
                2,
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

    #[tokio::test]
    async fn seo_meta_event_dispatches_index_reindex_trigger_and_cursor() {
        let db = test_db().await;
        run_migrations(&db).await;
        let service = service_with_outbox(db.clone());

        let tenant_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();

        service
            .publish_seo_meta_upserted_event(
                tenant_id, "product", target_id, "en-US", "explicit", None,
            )
            .await;

        let index_deliveries = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("seo index deliveries should load");
        assert_eq!(index_deliveries.len(), 1);
        assert_eq!(index_deliveries[0].status, INDEX_DELIVERY_STATUS_SENT);
        assert_eq!(index_deliveries[0].target_type, "product");
        assert_eq!(index_deliveries[0].target_id, Some(target_id));
        assert_eq!(index_deliveries[0].target_scope, INDEX_TARGET_SCOPE_ENTITY);

        let cursors = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("seo index cursors should load");
        assert_eq!(cursors.len(), 1);
        assert_eq!(cursors[0].target_type, "product");
        assert_eq!(cursors[0].replay_mode, INDEX_CURSOR_REPLAY_MODE_NOT_STARTED);
        assert!(cursors[0].high_water_mark_at >= cursors[0].initial_cursor_at);

        let reindex_events = outbox_entity::Entity::find()
            .filter(outbox_entity::Column::EventType.eq("index.reindex_requested"))
            .all(&db)
            .await
            .expect("reindex events should load");
        assert_eq!(reindex_events.len(), 1);
    }

    #[tokio::test]
    async fn seo_index_delivery_tracker_skips_duplicate_reindex_trigger() {
        let db = test_db().await;
        run_migrations(&db).await;
        let service = service_with_outbox(db.clone());

        let tenant_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();

        service
            .publish_seo_meta_upserted_event(
                tenant_id, "product", target_id, "en-US", "explicit", None,
            )
            .await;
        service
            .publish_seo_meta_upserted_event(
                tenant_id, "product", target_id, "en-US", "explicit", None,
            )
            .await;

        let index_deliveries = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await
            .expect("seo index deliveries should load");
        assert_eq!(index_deliveries.len(), 1);

        let reindex_events = outbox_entity::Entity::find()
            .filter(outbox_entity::Column::EventType.eq("index.reindex_requested"))
            .all(&db)
            .await
            .expect("reindex events should load");
        assert_eq!(reindex_events.len(), 1);
    }

    #[tokio::test]
    async fn seo_index_delivery_remains_tenant_scoped_for_same_idempotency_key() {
        let db = test_db().await;
        run_migrations(&db).await;
        let service = service_with_outbox(db.clone());

        let target_id = Uuid::new_v4();
        let event = DomainEvent::SeoMetaUpserted {
            target_kind: "product".to_string(),
            target_id,
            locale: "en-US".to_string(),
            source: "explicit".to_string(),
            idempotency_key: "shared-index-key".to_string(),
        };

        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();

        service.publish_seo_event(tenant_a, event.clone()).await;
        service.publish_seo_event(tenant_b, event).await;

        let deliveries = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::IdempotencyKey.eq("shared-index-key"))
            .all(&db)
            .await
            .expect("seo index deliveries should load");
        assert_eq!(deliveries.len(), 2);
        assert!(deliveries.iter().any(|item| item.tenant_id == tenant_a));
        assert!(deliveries.iter().any(|item| item.tenant_id == tenant_b));
    }

    #[tokio::test]
    async fn seo_index_delivery_moves_to_dead_letter_after_bounded_retries() {
        let db = test_db().await;
        run_migrations(&db).await;
        let transport = Arc::new(FlakyIndexTransport::with_reindex_failures(16));
        let service = service_with_transport(db.clone(), transport.clone());

        let tenant_id = Uuid::new_v4();
        service
            .publish_seo_meta_upserted_event(
                tenant_id,
                "product",
                Uuid::new_v4(),
                "en-US",
                "explicit",
                None,
            )
            .await;

        let index_delivery = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .one(&db)
            .await
            .expect("seo index delivery should load")
            .expect("seo index delivery should exist");
        assert_eq!(index_delivery.status, INDEX_DELIVERY_STATUS_DEAD_LETTER);
        assert_eq!(index_delivery.attempt_count, INDEX_RETRY_MAX_ATTEMPTS);
        assert!(index_delivery.outbox_event_id.is_none());
        assert!(index_delivery.last_error.is_some());
        assert_eq!(
            transport.published_count("index.reindex_requested"),
            INDEX_RETRY_MAX_ATTEMPTS as usize
        );
    }

    #[tokio::test]
    async fn seo_index_delivery_repair_replays_dead_letters_and_updates_cursor() {
        let db = test_db().await;
        run_migrations(&db).await;
        let transport = Arc::new(FlakyIndexTransport::with_reindex_failures(16));
        let service = service_with_transport(db.clone(), transport.clone());

        let tenant_id = Uuid::new_v4();
        service
            .publish_seo_meta_upserted_event(
                tenant_id,
                "product",
                Uuid::new_v4(),
                "en-US",
                "explicit",
                None,
            )
            .await;

        transport.set_fail_reindex_attempts(0);
        let repaired = service
            .repair_index_delivery_backlog(tenant_id, Some("product"), 20)
            .await
            .expect("repair backlog should succeed");
        assert_eq!(repaired, 1);

        let index_delivery = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .one(&db)
            .await
            .expect("seo index delivery should load")
            .expect("seo index delivery should exist");
        assert_eq!(index_delivery.status, INDEX_DELIVERY_STATUS_SENT);
        assert!(index_delivery.outbox_event_id.is_some());

        let cursor = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
            .filter(seo_index_cursor::Column::TargetType.eq("product"))
            .one(&db)
            .await
            .expect("seo index cursor should load")
            .expect("seo index cursor should exist");
        assert!(cursor.last_repair_cursor_at.is_some());
        assert_eq!(cursor.replay_mode, INDEX_CURSOR_REPLAY_MODE_REPAIR_ONLY);
    }

    #[tokio::test]
    async fn historical_replay_dispatches_new_index_transition_with_unique_idempotency_key() {
        let db = test_db().await;
        run_migrations(&db).await;
        let service = service_with_outbox(db.clone());

        let tenant_id = Uuid::new_v4();
        service
            .publish_seo_meta_upserted_event(
                tenant_id,
                "product",
                Uuid::new_v4(),
                "en-US",
                "explicit",
                None,
            )
            .await;

        let result = service
            .run_index_repair_replay(tenant_id, Some("product"), 20, true)
            .await
            .expect("replay mode should succeed");

        assert_eq!(result.repaired_count, 0);
        assert_eq!(result.replayed_count, 1);
        assert_eq!(result.historical_events_scanned, 1);
        assert_eq!(result.replay_mode, SeoIndexReplayMode::ReplayCompleted);
        assert!(result.replay_run_id.is_some());

        let deliveries = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_index_delivery::Column::TargetType.eq("product"))
            .all(&db)
            .await
            .expect("seo index deliveries should load");
        assert_eq!(deliveries.len(), 2);
        assert_ne!(deliveries[0].idempotency_key, deliveries[1].idempotency_key);

        let reindex_events = outbox_entity::Entity::find()
            .filter(outbox_entity::Column::EventType.eq("index.reindex_requested"))
            .all(&db)
            .await
            .expect("reindex events should load");
        assert_eq!(reindex_events.len(), 2);

        let cursor = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
            .filter(seo_index_cursor::Column::TargetType.eq("product"))
            .one(&db)
            .await
            .expect("seo index cursor should load")
            .expect("seo index cursor should exist");
        assert_eq!(
            cursor.replay_mode,
            INDEX_CURSOR_REPLAY_MODE_REPLAY_COMPLETED
        );
        assert!(cursor.replay_requested_at.is_some());
        assert!(cursor.replay_completed_at.is_some());
    }

    #[tokio::test]
    async fn index_delivery_status_reports_sent_transitions_and_cursor_state() {
        let db = test_db().await;
        run_migrations(&db).await;
        let service = service_with_outbox(db.clone());

        let tenant_id = Uuid::new_v4();
        service
            .publish_seo_meta_upserted_event(
                tenant_id,
                "product",
                Uuid::new_v4(),
                "en-US",
                "explicit",
                None,
            )
            .await;

        let summary = service
            .index_delivery_status(tenant_id, Some("product"))
            .await
            .expect("index delivery status should load");
        assert_eq!(summary.sent_count, 1);
        assert_eq!(summary.pending_count, 0);
        assert_eq!(summary.retry_count, 0);
        assert_eq!(summary.failed_count, 0);
        assert_eq!(summary.dead_letter_count, 0);
        assert_eq!(summary.cursors.len(), 1);
        assert_eq!(
            summary.cursors[0].replay_mode,
            SeoIndexReplayMode::NotStarted
        );
    }

    #[test]
    fn replay_mode_is_forward_only() {
        assert_eq!(
            advance_replay_mode(
                INDEX_CURSOR_REPLAY_MODE_REPLAY_COMPLETED,
                INDEX_CURSOR_REPLAY_MODE_REPAIR_ONLY,
            ),
            INDEX_CURSOR_REPLAY_MODE_REPLAY_COMPLETED,
        );
        assert_eq!(
            advance_replay_mode(
                INDEX_CURSOR_REPLAY_MODE_NOT_STARTED,
                INDEX_CURSOR_REPLAY_MODE_REPLAY_REQUESTED,
            ),
            INDEX_CURSOR_REPLAY_MODE_REPLAY_REQUESTED,
        );
    }
}

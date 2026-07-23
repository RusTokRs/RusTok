use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope, ForumMentionEvent,
};
use rustok_notifications::api::{
    NotificationSourceEventRef, NotificationSourceSlug, NotificationTypeKey,
};
use rustok_notifications::{
    DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE, NotificationError,
    NotificationOutboxEnvelopeDecoder, NotificationOutboxEnvelopeRecord,
    NotificationOutboxIntakeOutcome, NotificationOutboxIntakeWorker, NotificationResult,
};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV: &str =
    "RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED";
const OUTBOX_INTAKE_POLL_INTERVAL: Duration = Duration::from_millis(500);
const FORUM_SOURCE: &str = "forum";
const FORUM_TOPIC_CREATED: &str = "forum.topic.created";
const FORUM_USER_MENTION_ADDED: &str = "forum.mention.user_added";
static NOTIFICATION_OUTBOX_INTAKE_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Default)]
struct ServerNotificationOutboxEnvelopeDecoder;

impl NotificationOutboxEnvelopeDecoder for ServerNotificationOutboxEnvelopeDecoder {
    fn decode(
        &self,
        record: &NotificationOutboxEnvelopeRecord,
    ) -> NotificationResult<NotificationSourceEventRef> {
        if let Ok(envelope) =
            serde_json::from_value::<ContractEventEnvelope>(record.payload.clone())
        {
            envelope
                .validate_registered_schema()
                .map_err(|_| NotificationError::InvalidEvent)?;
            if envelope.id() != record.outbox_event_id
                || envelope.event_type() != record.event_type
                || i16::try_from(envelope.schema_version()).ok() != Some(record.schema_version)
            {
                return Err(NotificationError::InvalidEvent);
            }
            let tenant_id = envelope.tenant_id();
            let event_id = envelope.id();
            return match envelope
                .into_payload()
                .map_err(|_| NotificationError::InvalidEvent)?
            {
                ContractEventPayload::ForumMention(ForumMentionEvent::UserMentionAdded {
                    source_revision_id,
                    ..
                }) if record.event_type == FORUM_USER_MENTION_ADDED => source_event_ref(
                    tenant_id,
                    event_id,
                    FORUM_USER_MENTION_ADDED,
                    u64::try_from(source_revision_id)
                        .map_err(|_| NotificationError::InvalidEvent)?,
                ),
                _ => Err(NotificationError::InvalidEvent),
            };
        }

        let envelope = serde_json::from_value::<EventEnvelope>(record.payload.clone())?;
        envelope
            .validate_registered_schema()
            .map_err(|_| NotificationError::InvalidEvent)?;
        if envelope.id != record.outbox_event_id
            || envelope.event_type != record.event_type
            || i16::try_from(envelope.schema_version).ok() != Some(record.schema_version)
        {
            return Err(NotificationError::InvalidEvent);
        }
        match envelope.event {
            DomainEvent::ForumTopicCreated { topic_id, .. }
                if record.event_type == FORUM_TOPIC_CREATED =>
            {
                source_event_ref(envelope.tenant_id, topic_id, FORUM_TOPIC_CREATED, 1)
            }
            _ => Err(NotificationError::InvalidEvent),
        }
    }
}

pub struct NotificationOutboxIntakeWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl NotificationOutboxIntakeWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }
}

pub fn start_notification_outbox_intake_if_enabled(ctx: &ServerRuntimeContext) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<NotificationOutboxIntakeWorkerHandle>()
    {
        return Ok(());
    }
    if !outbox_intake_enabled_from_environment() {
        tracing::info!(
            variable = NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV,
            "Notification outbox intake disabled by explicit runtime flag"
        );
        return Ok(());
    }

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before notification outbox intake startup")
        .subscribe();

    let instance_id = NOTIFICATION_OUTBOX_INTAKE_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let worker = NotificationOutboxIntakeWorker::new(
        ctx.db_clone(),
        Arc::new(ServerNotificationOutboxEnvelopeDecoder),
        DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE,
    )
    .map_err(|error| Error::Message(format!("notification outbox intake is invalid: {error}")))?;

    tracing::info!(
        instance_id,
        batch_size = worker.batch_size(),
        "Starting notification outbox intake worker"
    );
    ctx.shared_insert(NotificationOutboxIntakeWorkerHandle {
        instance_id,
        _handle: tokio::spawn(notification_outbox_intake_loop(worker, stop_rx)),
    });
    Ok(())
}

async fn notification_outbox_intake_loop(
    worker: NotificationOutboxIntakeWorker,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!("Notification outbox intake stopped");
            return;
        }

        let event_ids = match worker.pending_outbox_event_ids().await {
            Ok(event_ids) => event_ids,
            Err(error) => {
                tracing::error!(
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification outbox intake failed to select committed envelopes"
                );
                Vec::new()
            }
        };

        for outbox_event_id in event_ids {
            if *stop_rx.borrow() {
                tracing::info!("Notification outbox intake stopped before next envelope");
                return;
            }

            match worker.process_outbox_event(outbox_event_id).await {
                Ok(NotificationOutboxIntakeOutcome::Accepted(result)) => tracing::debug!(
                    outbox_event_id = %result.outbox_event_id,
                    source_inbox_id = %result.source_inbox_id,
                    source_slug = result.source_slug,
                    event_type = result.event_type,
                    source_revision = result.source_revision,
                    replayed = result.replayed,
                    "Notification source envelope accepted from outbox"
                ),
                Ok(NotificationOutboxIntakeOutcome::Rejected(result)) => tracing::error!(
                    outbox_event_id = %result.outbox_event_id,
                    event_type = result.event_type,
                    schema_version = result.schema_version,
                    error_code = result.error_code,
                    replayed = result.replayed,
                    "Notification source envelope moved to permanent intake quarantine"
                ),
                Err(error) => tracing::warn!(
                    outbox_event_id = %outbox_event_id,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification outbox intake has no terminal record; envelope will be retried"
                ),
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(OUTBOX_INTAKE_POLL_INTERVAL) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!("Notification outbox intake received shutdown signal");
                    return;
                }
            }
        }
    }
}

fn source_event_ref(
    tenant_id: Uuid,
    event_id: Uuid,
    event_type: &str,
    source_revision: u64,
) -> NotificationResult<NotificationSourceEventRef> {
    NotificationSourceEventRef::new(
        tenant_id,
        event_id,
        NotificationSourceSlug::new(FORUM_SOURCE).map_err(|_| NotificationError::InvalidEvent)?,
        NotificationTypeKey::new(event_type).map_err(|_| NotificationError::InvalidEvent)?,
        source_revision,
    )
    .map_err(|_| NotificationError::InvalidEvent)
}

fn outbox_intake_enabled_from_environment() -> bool {
    match std::env::var(NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "" | "0" | "false" | "no" | "off" => false,
            _ => {
                tracing::warn!(
                    variable = NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV,
                    value,
                    "Invalid notification outbox intake flag; keeping intake disabled"
                );
                false
            }
        },
        Err(std::env::VarError::NotPresent) => false,
        Err(error) => {
            tracing::warn!(
                variable = NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV,
                error = %error,
                "Notification outbox intake flag is unreadable; keeping intake disabled"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use rustok_events::{ContractEventEnvelope, DomainEvent, EventEnvelope, ForumMentionEvent};
    use rustok_notifications::{
        NotificationOutboxEnvelopeDecoder, NotificationOutboxEnvelopeRecord,
    };
    use uuid::Uuid;

    use super::{
        FORUM_TOPIC_CREATED, FORUM_USER_MENTION_ADDED,
        ServerNotificationOutboxEnvelopeDecoder,
    };

    #[test]
    fn decoder_maps_root_topic_and_contract_mention_semantics() {
        let tenant_id = Uuid::new_v4();
        let topic_id = Uuid::new_v4();
        let topic = EventEnvelope::new(
            tenant_id,
            Some(Uuid::new_v4()),
            DomainEvent::ForumTopicCreated {
                topic_id,
                category_id: Uuid::new_v4(),
                author_id: Some(Uuid::new_v4()),
                locale: "en".to_string(),
            },
        );
        let topic_record = NotificationOutboxEnvelopeRecord {
            outbox_event_id: topic.id,
            event_type: topic.event_type.clone(),
            schema_version: i16::try_from(topic.schema_version).expect("schema fits"),
            payload: serde_json::to_value(topic).expect("topic envelope serializes"),
        };
        let decoded = ServerNotificationOutboxEnvelopeDecoder
            .decode(&topic_record)
            .expect("topic envelope decodes");
        assert_eq!(decoded.tenant_id(), tenant_id);
        assert_eq!(decoded.event_id(), topic_id);
        assert_eq!(decoded.event_type().as_str(), FORUM_TOPIC_CREATED);
        assert_eq!(decoded.source_revision(), 1);

        let mention_revision = 7_i64;
        let mention = ContractEventEnvelope::new(
            tenant_id,
            Some(Uuid::new_v4()),
            ForumMentionEvent::UserMentionAdded {
                source_kind: "topic".to_string(),
                source_id: topic_id,
                source_revision_id: mention_revision,
                source_locale: "en".to_string(),
                mentioned_user_id: Uuid::new_v4(),
            },
        )
        .expect("mention envelope validates");
        let mention_id = mention.id();
        let mention_record = NotificationOutboxEnvelopeRecord {
            outbox_event_id: mention_id,
            event_type: mention.event_type().to_string(),
            schema_version: i16::try_from(mention.schema_version()).expect("schema fits"),
            payload: serde_json::to_value(mention).expect("mention envelope serializes"),
        };
        let decoded = ServerNotificationOutboxEnvelopeDecoder
            .decode(&mention_record)
            .expect("mention envelope decodes");
        assert_eq!(decoded.event_id(), mention_id);
        assert_eq!(decoded.event_type().as_str(), FORUM_USER_MENTION_ADDED);
        assert_eq!(decoded.source_revision(), mention_revision as u64);
    }
}

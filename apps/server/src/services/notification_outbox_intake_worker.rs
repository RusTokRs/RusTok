use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
    NotificationOutboxIntakeOutcome, NotificationOutboxIntakeReceiptPort,
    NotificationOutboxIntakeWorker, NotificationResult,
};
use tokio::task::JoinHandle;
use rustok_outbox::OutboxConsumer;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::stop_handle::StopHandle;
use crate::services::server_runtime_context::{ServerRuntimeContext, ServerRuntimeStartReservation};

pub const NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV: &str =
    "RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED";
const OUTBOX_INTAKE_POLL_INTERVAL: Duration = Duration::from_millis(500);
const NOTIFICATION_OUTBOX_CONSUMER_KEY: &str = "notifications.outbox-intake";
const FORUM_SOURCE: &str = "forum";
const FORUM_TOPIC_CREATED: &str = "forum.topic.created";
const FORUM_TOPIC_REPLIED: &str = "forum.topic.replied";
const FORUM_USER_MENTION_ADDED: &str = "forum.mention.user_added";
const MESSENGER_SOURCE: &str = "messenger";
const MESSENGER_MESSAGE_DISPATCHED: &str = "messenger.message.dispatched";
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
                    FORUM_SOURCE,
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
                source_event_ref(
                    envelope.tenant_id,
                    topic_id,
                    FORUM_SOURCE,
                    FORUM_TOPIC_CREATED,
                    1,
                )
            }
            // The reply id is the source event identity: one fan-out per reply, and the
            // provider resolves the topic and the reply's audience from it.
            DomainEvent::ForumTopicReplied { reply_id, .. }
                if record.event_type == FORUM_TOPIC_REPLIED =>
            {
                source_event_ref(
                    envelope.tenant_id,
                    reply_id,
                    FORUM_SOURCE,
                    FORUM_TOPIC_REPLIED,
                    1,
                )
            }
            DomainEvent::MessengerMessageDispatched {
                conversation_id,
                message_id,
                ..
            } if record.event_type == MESSENGER_MESSAGE_DISPATCHED => source_event_ref(
                envelope.tenant_id,
                conversation_id,
                MESSENGER_SOURCE,
                MESSENGER_MESSAGE_DISPATCHED,
                u64::try_from(message_id).map_err(|_| NotificationError::InvalidEvent)?,
            ),
            _ => Err(NotificationError::InvalidEvent),
        }
    }

    fn accepted_event_types(&self) -> Vec<String> {
        // One list for the decode arms above: the Notifications intake never names
        // a module event type, and a new Forum notification type reaches intake as
        // soon as the host decodes it.
        vec![
            FORUM_TOPIC_CREATED.to_string(),
            FORUM_TOPIC_REPLIED.to_string(),
            FORUM_USER_MENTION_ADDED.to_string(),
            MESSENGER_MESSAGE_DISPATCHED.to_string(),
        ]
    }
}

#[derive(Debug, Default)]
struct ServerNotificationOutboxIntakeReceiptPort;

#[async_trait::async_trait]
impl NotificationOutboxIntakeReceiptPort for ServerNotificationOutboxIntakeReceiptPort {
    async fn acknowledge_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        consumer_key: &str,
        outbox_event_id: Uuid,
        event_type: &str,
    ) -> NotificationResult<()> {
        OutboxConsumer::acknowledge_in_transaction(
            transaction,
            consumer_key,
            outbox_event_id,
            event_type,
        )
        .await
        .map_err(NotificationError::Database)
    }
}

// The registration and durable receipt backfill are one startup lifecycle.
// A concurrent bootstrap must not run another backfill before this one ends.
struct NotificationOutboxIntakeStartupReservation;

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

    /// Await a durable worker's current operation after the shutdown signal.
    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }

    pub(crate) fn into_task(self) -> JoinHandle<()> {
        self._handle
    }
}

pub async fn start_notification_outbox_intake_if_enabled(ctx: &ServerRuntimeContext) -> Result<()> {
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

    let stop = StopHandle::ensure(ctx);
    ensure_intake_bootstrap_not_stopping(&stop)?;
    let Some(reservation) =
        ServerRuntimeStartReservation::try_claim(ctx, NotificationOutboxIntakeStartupReservation)
    else {
        // Another caller is still preparing this worker; a successful return
        // would falsely tell bootstrap it is running if that starter fails.
        if ctx.shared_contains::<NotificationOutboxIntakeWorkerHandle>() {
            return Ok(());
        }
        return Err(Error::Message(
            "Notification outbox intake worker startup is already in progress; retry".to_string(),
        ));
    };
    if ctx.shared_contains::<NotificationOutboxIntakeWorkerHandle>() {
        return Ok(());
    }

    let decoder = Arc::new(ServerNotificationOutboxEnvelopeDecoder);
    let accepted_event_types = decoder.accepted_event_types();
    let outbox_consumer = OutboxConsumer::new(ctx.db_clone());
    outbox_consumer
        .register_required_event_types(NOTIFICATION_OUTBOX_CONSUMER_KEY, &accepted_event_types)
        .await
        .map_err(|error| {
            Error::Message(format!("notification outbox consumer registration failed: {error}"))
        })?;

    ensure_intake_bootstrap_not_stopping(&stop)?;
    let worker = NotificationOutboxIntakeWorker::new(
        ctx.db_clone(),
        decoder,
        DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE,
    )
    .map_err(|error| Error::Message(format!("notification outbox intake is invalid: {error}")))?
    .with_receipt_port(
        NOTIFICATION_OUTBOX_CONSUMER_KEY,
        Arc::new(ServerNotificationOutboxIntakeReceiptPort),
    )
    .map_err(|error| {
        Error::Message(format!("notification outbox receipt port is invalid: {error}"))
    })?;

    // Reconcile terminal receipts created before the generic Outbox consumer
    // ledger existed. Registration/backfill run before runtime workers (and
    // therefore retention) are started by server_bootstrap.
    let mut cursor = None;
    loop {
        ensure_intake_bootstrap_not_stopping(&stop)?;
        let ids = worker
            .terminal_outbox_event_ids_after(cursor, DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE)
            .await
            .map_err(|error| {
                Error::Message(format!("notification outbox receipt backfill failed: {error}"))
            })?;
        ensure_intake_bootstrap_not_stopping(&stop)?;
        if ids.is_empty() {
            break;
        }
        for outbox_event_id in &ids {
            ensure_intake_bootstrap_not_stopping(&stop)?;
            outbox_consumer
                .acknowledge_existing(NOTIFICATION_OUTBOX_CONSUMER_KEY, *outbox_event_id)
                .await
                .map_err(|error| {
                    Error::Message(format!(
                        "notification outbox receipt backfill failed for {outbox_event_id}: {error}"
                    ))
                })?;
        }
        cursor = ids.last().copied();
        if ids.len() < DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE {
            break;
        }
    }

    ensure_intake_bootstrap_not_stopping(&stop)?;
    let stop_rx = stop.subscribe();
    let instance_id = NOTIFICATION_OUTBOX_INTAKE_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);

    tracing::info!(
        instance_id,
        batch_size = worker.batch_size(),
        "Starting notification outbox intake worker"
    );
    ctx.shared_insert(NotificationOutboxIntakeWorkerHandle {
        instance_id,
        _handle: tokio::spawn(notification_outbox_intake_loop(worker, stop_rx)),
    });
    reservation.commit();
    Ok(())
}

/// An interrupted initial receipt backfill is safe to resume: every receipt is
/// idempotent, and the cursor is transient rather than persisted. Stop before
/// claiming the next receipt when host shutdown has already started.
fn ensure_intake_bootstrap_not_stopping(stop: &StopHandle) -> Result<()> {
    if stop.is_stopping() {
        return Err(Error::Message(
            "notification outbox intake bootstrap interrupted by server shutdown".to_string(),
        ));
    }
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
    source_slug: &str,
    event_type: &str,
    source_revision: u64,
) -> NotificationResult<NotificationSourceEventRef> {
    NotificationSourceEventRef::new(
        tenant_id,
        event_id,
        NotificationSourceSlug::new(source_slug).map_err(|_| NotificationError::InvalidEvent)?,
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
        Err(_error) => {
            tracing::warn!(
                variable = NOTIFICATION_OUTBOX_INTAKE_ENABLED_ENV,
                failure_code = "notification_outbox_intake_flag_unreadable",
                "Notification outbox intake flag is unreadable; keeping intake disabled"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn intake_backfill_checks_host_shutdown_before_starting_more_work() {
        let (stop, _receiver) = super::StopHandle::new();
        assert!(super::ensure_intake_bootstrap_not_stopping(&stop).is_ok());
        stop.stop().await;
        assert!(super::ensure_intake_bootstrap_not_stopping(&stop).is_err());
        let late_subscriber = stop.subscribe();
        assert!(*late_subscriber.borrow());
    }

    use rustok_events::{ContractEventEnvelope, DomainEvent, EventEnvelope, ForumMentionEvent};
    use rustok_notifications::{
        NotificationOutboxEnvelopeDecoder, NotificationOutboxEnvelopeRecord,
    };
    use uuid::Uuid;

    use super::{
        FORUM_TOPIC_CREATED, FORUM_TOPIC_REPLIED, FORUM_USER_MENTION_ADDED,
        MESSENGER_MESSAGE_DISPATCHED, MESSENGER_SOURCE, ServerNotificationOutboxEnvelopeDecoder,
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

        let reply_topic_id = Uuid::new_v4();
        let reply_id = Uuid::new_v4();
        let reply = EventEnvelope::new(
            tenant_id,
            Some(Uuid::new_v4()),
            DomainEvent::ForumTopicReplied {
                topic_id: reply_topic_id,
                reply_id,
                author_id: Some(Uuid::new_v4()),
            },
        );
        let reply_record = NotificationOutboxEnvelopeRecord {
            outbox_event_id: reply.id,
            event_type: reply.event_type.clone(),
            schema_version: i16::try_from(reply.schema_version).expect("schema fits"),
            payload: serde_json::to_value(reply).expect("reply envelope serializes"),
        };
        let decoded_reply = ServerNotificationOutboxEnvelopeDecoder
            .decode(&reply_record)
            .expect("reply envelope decodes");
        assert_eq!(decoded_reply.tenant_id(), tenant_id);
        assert_eq!(decoded_reply.event_id(), reply_id);
        assert_eq!(decoded_reply.event_type().as_str(), FORUM_TOPIC_REPLIED);
        assert_eq!(decoded_reply.source_revision(), 1);

        let mismatched = NotificationOutboxEnvelopeRecord {
            outbox_event_id: reply_record.outbox_event_id,
            event_type: FORUM_TOPIC_CREATED.to_string(),
            schema_version: reply_record.schema_version,
            payload: reply_record.payload.clone(),
        };
        assert!(
            ServerNotificationOutboxEnvelopeDecoder
                .decode(&mismatched)
                .is_err(),
            "a replied payload under another event type must not be claimed as a topic"
        );

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

    #[test]
    fn intake_claims_exactly_the_decoded_event_types() {
        assert_eq!(
            ServerNotificationOutboxEnvelopeDecoder.accepted_event_types(),
            vec![
                FORUM_TOPIC_CREATED.to_string(),
                FORUM_TOPIC_REPLIED.to_string(),
                FORUM_USER_MENTION_ADDED.to_string(),
                MESSENGER_MESSAGE_DISPATCHED.to_string(),
            ]
        );
    }

    #[test]
    fn decoder_claims_and_decodes_messenger_message_dispatched() {
        let tenant_id = Uuid::new_v4();
        let conversation_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let message_id = 42_i64;

        let msg_event = EventEnvelope::new(
            tenant_id,
            Some(sender_id),
            DomainEvent::MessengerMessageDispatched {
                conversation_id,
                message_id,
                sender_id,
            },
        );

        let record = NotificationOutboxEnvelopeRecord {
            outbox_event_id: msg_event.id,
            event_type: msg_event.event_type.clone(),
            schema_version: i16::try_from(msg_event.schema_version).expect("schema fits"),
            payload: serde_json::to_value(msg_event).expect("payload serializes"),
        };

        let decoded = ServerNotificationOutboxEnvelopeDecoder
            .decode(&record)
            .expect("messenger envelope decodes");
        assert_eq!(decoded.tenant_id(), tenant_id);
        assert_eq!(decoded.event_id(), conversation_id);
        assert_eq!(decoded.source().as_str(), MESSENGER_SOURCE);
        assert_eq!(decoded.event_type().as_str(), MESSENGER_MESSAGE_DISPATCHED);
        assert_eq!(decoded.source_revision(), 42);
    }
}

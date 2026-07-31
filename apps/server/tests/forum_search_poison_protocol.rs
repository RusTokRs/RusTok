use std::time::Duration;

use rustok_iggy::{
    ConsumedContractDecodeFailure, ContractDecodeFailureKind, DlqEntry,
};
use rustok_iggy_connector::SubscriberMessageMetadata;
use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceiptState,
    ConsumerPoisonReceiptStore,
};
use rustok_search::{
    FORUM_SEARCH_CONTRACT_CONSUMER_GROUP, FORUM_SEARCH_CONTRACT_TOPIC,
};
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const SOURCE_STREAM: &str = "rustok";
const SOURCE_PARTITION: u32 = 1;
const PUBLISH_LEASE: Duration = Duration::from_secs(30);
const SEMANTIC_CONFLICT_CODE: &str =
    "forum.search_projection.contract_inbox_identity_conflict";

#[derive(Default)]
struct RecordingDlqPublisher {
    entries: Vec<DlqEntry>,
    fail_next: bool,
}

impl RecordingDlqPublisher {
    fn publish(&mut self, entry: DlqEntry) -> Result<(), &'static str> {
        if self.fail_next {
            self.fail_next = false;
            return Err("injected DLQ publication failure");
        }
        self.entries.push(entry);
        Ok(())
    }
}

#[derive(Default)]
struct RecordingAcknowledger {
    attempts: u32,
    fail_next: bool,
}

impl RecordingAcknowledger {
    fn acknowledge(&mut self) -> Result<(), &'static str> {
        self.attempts += 1;
        if self.fail_next {
            self.fail_next = false;
            return Err("injected source acknowledgement failure");
        }
        Ok(())
    }
}

async fn receipt_store() -> ConsumerPoisonReceiptStore {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite receipt database should open");
    apply_connector_migrations(&db).await;
    ConsumerPoisonReceiptStore::new(db)
}

async fn apply_connector_migrations(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_iggy_connector::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("connector migration should apply");
    }
}

fn metadata(offset: u64) -> SubscriberMessageMetadata {
    SubscriberMessageMetadata::new(
        SOURCE_STREAM,
        FORUM_SEARCH_CONTRACT_TOPIC,
        SOURCE_PARTITION,
    )
    .with_offset(offset)
    .with_message_id(format!("forum-search-poison-{offset}"))
    .with_delivery_attempt(1)
    .with_ack_token(format!("forum-search-poison-ack-{offset}"))
}

fn raw_failure(
    offset: u64,
    payload: Vec<u8>,
) -> ConsumedContractDecodeFailure {
    ConsumedContractDecodeFailure::new(
        SOURCE_STREAM.to_string(),
        FORUM_SEARCH_CONTRACT_TOPIC.to_string(),
        metadata(offset),
        payload,
        ContractDecodeFailureKind::Deserialize,
    )
    .expect("raw poison identity should be valid")
}

fn raw_identity(
    failure: &ConsumedContractDecodeFailure,
) -> ConsumerPoisonIdentity {
    ConsumerPoisonIdentity::new(
        failure.delivery_id(),
        FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        failure.stream(),
        failure.topic(),
        failure.partition(),
        failure.offset(),
        failure.raw_payload().to_vec(),
    )
    .expect("raw poison receipt identity should be valid")
}

fn semantic_descriptor(
    offset: u64,
    payload: Vec<u8>,
) -> (ConsumerPoisonIdentity, DlqEntry) {
    let connector_metadata = metadata(offset);
    let delivery = ConsumedContractDecodeFailure::new(
        SOURCE_STREAM.to_string(),
        FORUM_SEARCH_CONTRACT_TOPIC.to_string(),
        connector_metadata.clone(),
        payload.clone(),
        ContractDecodeFailureKind::SchemaValidation,
    )
    .expect("semantic poison delivery identity should be valid");
    let delivery_id = delivery.delivery_id();
    let identity = ConsumerPoisonIdentity::new(
        delivery_id,
        FORUM_SEARCH_CONTRACT_CONSUMER_GROUP,
        SOURCE_STREAM,
        FORUM_SEARCH_CONTRACT_TOPIC,
        SOURCE_PARTITION,
        offset,
        payload.clone(),
    )
    .expect("semantic poison receipt identity should be valid");
    let entry = DlqEntry::new(
        delivery_id,
        FORUM_SEARCH_CONTRACT_TOPIC,
        payload,
        SEMANTIC_CONFLICT_CODE,
        1,
    )
    .with_connector_metadata(connector_metadata)
    .with_broker_message_id(delivery_id);
    (identity, entry)
}

async fn publish_terminal_result(
    store: &ConsumerPoisonReceiptStore,
    identity: &ConsumerPoisonIdentity,
    stable_error_code: &str,
    publisher_id: Uuid,
    entry: DlqEntry,
    publisher: &mut RecordingDlqPublisher,
) -> Result<(), &'static str> {
    assert_eq!(
        store
            .reserve_and_claim(
                identity,
                stable_error_code,
                1,
                publisher_id,
                PUBLISH_LEASE,
            )
            .await
            .expect("receipt claim should be readable"),
        ConsumerPoisonPublishClaim::Claimed
    );
    if let Err(error) = publisher.publish(entry) {
        store
            .release_claim(identity, publisher_id)
            .await
            .expect("failed publication must release its durable claim");
        return Err(error);
    }
    store
        .mark_published(identity, publisher_id)
        .await
        .expect("DLQ publication must become durable before source acknowledgement");
    Ok(())
}

#[tokio::test]
async fn raw_poison_redelivery_is_ack_only_after_durable_publication() {
    let store = receipt_store().await;
    let failure = raw_failure(41, b"{not-valid-contract-json".to_vec());
    let identity = raw_identity(&failure);
    let entry = failure.to_dlq_entry(1);
    let deterministic_id = failure.delivery_id();

    assert_eq!(entry.event_id, deterministic_id);
    assert_eq!(entry.broker_message_id(), Some(deterministic_id));
    assert_eq!(entry.payload, failure.raw_payload());
    assert_eq!(entry.error.as_str(), failure.stable_error_code());

    let mut first_publisher = RecordingDlqPublisher::default();
    publish_terminal_result(
        &store,
        &identity,
        failure.stable_error_code(),
        Uuid::new_v4(),
        entry,
        &mut first_publisher,
    )
    .await
    .expect("first DLQ publication should succeed");
    assert_eq!(first_publisher.entries.len(), 1);

    let mut first_ack = RecordingAcknowledger {
        fail_next: true,
        ..Default::default()
    };
    assert!(first_ack.acknowledge().is_err());
    assert_eq!(first_ack.attempts, 1);
    assert_eq!(
        store.find(&identity).await.unwrap().unwrap().state,
        ConsumerPoisonReceiptState::Published
    );

    let restart_publisher_id = Uuid::new_v4();
    assert_eq!(
        store
            .reserve_and_claim(
                &identity,
                failure.stable_error_code(),
                9,
                restart_publisher_id,
                PUBLISH_LEASE,
            )
            .await
            .unwrap(),
        ConsumerPoisonPublishClaim::AlreadyPublished
    );
    let restart_publisher = RecordingDlqPublisher::default();
    assert!(restart_publisher.entries.is_empty());

    let mut restarted_ack = RecordingAcknowledger::default();
    restarted_ack
        .acknowledge()
        .expect("redelivery should acknowledge the already-published receipt");
    store
        .mark_acknowledged(&identity)
        .await
        .expect("acknowledged source position should advance receipt bookkeeping");
    assert_eq!(restarted_ack.attempts, 1);
    assert_eq!(
        store.find(&identity).await.unwrap().unwrap().state,
        ConsumerPoisonReceiptState::Acknowledged
    );
}

#[tokio::test]
async fn semantic_poison_reuses_the_same_durable_dlq_protocol() {
    let store = receipt_store().await;
    let payload = br#"{"event":"valid-envelope-with-conflicting-root"}"#.to_vec();
    let (identity, entry) = semantic_descriptor(42, payload.clone());
    let deterministic_id = identity.delivery_id();

    assert_eq!(entry.event_id, deterministic_id);
    assert_eq!(entry.broker_message_id(), Some(deterministic_id));
    assert_eq!(entry.payload, payload);
    assert_eq!(entry.error.as_str(), SEMANTIC_CONFLICT_CODE);

    let mut publisher = RecordingDlqPublisher::default();
    publish_terminal_result(
        &store,
        &identity,
        SEMANTIC_CONFLICT_CODE,
        Uuid::new_v4(),
        entry,
        &mut publisher,
    )
    .await
    .expect("semantic poison DLQ publication should succeed");
    assert_eq!(publisher.entries.len(), 1);

    let mut failed_ack = RecordingAcknowledger {
        fail_next: true,
        ..Default::default()
    };
    assert!(failed_ack.acknowledge().is_err());
    let retained = store.find(&identity).await.unwrap().unwrap();
    assert_eq!(retained.state, ConsumerPoisonReceiptState::Published);
    assert_eq!(retained.stable_error_code, SEMANTIC_CONFLICT_CODE);
    assert_eq!(retained.first_delivery_attempt_count, 1);

    assert_eq!(
        store
            .reserve_and_claim(
                &identity,
                SEMANTIC_CONFLICT_CODE,
                7,
                Uuid::new_v4(),
                PUBLISH_LEASE,
            )
            .await
            .unwrap(),
        ConsumerPoisonPublishClaim::AlreadyPublished
    );

    let mut restarted_ack = RecordingAcknowledger::default();
    restarted_ack.acknowledge().unwrap();
    store.mark_acknowledged(&identity).await.unwrap();
    assert_eq!(
        store.find(&identity).await.unwrap().unwrap().state,
        ConsumerPoisonReceiptState::Acknowledged
    );
}

#[tokio::test]
async fn failed_dlq_publication_releases_the_claim_for_restart() {
    let store = receipt_store().await;
    let failure = raw_failure(43, vec![0xff, 0x00, 0x7f]);
    let identity = raw_identity(&failure);
    let entry = failure.to_dlq_entry(1);

    let mut failed_publisher = RecordingDlqPublisher {
        fail_next: true,
        ..Default::default()
    };
    assert!(
        publish_terminal_result(
            &store,
            &identity,
            failure.stable_error_code(),
            Uuid::new_v4(),
            entry.clone(),
            &mut failed_publisher,
        )
        .await
        .is_err()
    );
    assert!(failed_publisher.entries.is_empty());
    assert_eq!(
        store.find(&identity).await.unwrap().unwrap().state,
        ConsumerPoisonReceiptState::Reserved
    );

    let mut restarted_publisher = RecordingDlqPublisher::default();
    publish_terminal_result(
        &store,
        &identity,
        failure.stable_error_code(),
        Uuid::new_v4(),
        entry,
        &mut restarted_publisher,
    )
    .await
    .expect("restart should reclaim and publish the released receipt");
    assert_eq!(restarted_publisher.entries.len(), 1);

    let mut acknowledger = RecordingAcknowledger::default();
    acknowledger.acknowledge().unwrap();
    store.mark_acknowledged(&identity).await.unwrap();
    assert_eq!(
        store.find(&identity).await.unwrap().unwrap().state,
        ConsumerPoisonReceiptState::Acknowledged
    );
}

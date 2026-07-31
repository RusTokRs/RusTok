use std::sync::Arc;

use async_trait::async_trait;
use rustok_events::{ContractEventEnvelope, ForumSearchProjectionEvent};
use rustok_iggy_connector::{
    ConnectorError, ConsumerCursor, SubscriberMessage, SubscriberMessageMetadata,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{PersistentContractConsumerGroup, PersistentContractDelivery};
use crate::serialization::{EventSerializer, JsonSerializer};

const STREAM: &str = "rustok";
const TOPIC: &str = "domain";
const PARTITION: u32 = 1;
const OFFSET: u64 = 41;
const ACK_TOKEN: &str = "restart-proof-ack-41";

#[derive(Clone)]
struct SharedCursorState {
    inner: Arc<Mutex<CursorState>>,
}

struct CursorState {
    message: SubscriberMessage,
    committed: bool,
    ack_failures_remaining: u32,
    receive_count: u32,
    acknowledge_count: u32,
}

impl SharedCursorState {
    fn new(payload: Vec<u8>, ack_failures_remaining: u32) -> Self {
        let metadata = SubscriberMessageMetadata::new(STREAM, TOPIC, PARTITION)
            .with_offset(OFFSET)
            .with_message_id("restart-proof-message")
            .with_delivery_attempt(1)
            .with_ack_token(ACK_TOKEN);
        Self {
            inner: Arc::new(Mutex::new(CursorState {
                message: SubscriberMessage { payload, metadata },
                committed: false,
                ack_failures_remaining,
                receive_count: 0,
                acknowledge_count: 0,
            })),
        }
    }

    async fn snapshot(&self) -> (bool, u32, u32) {
        let state = self.inner.lock().await;
        (
            state.committed,
            state.receive_count,
            state.acknowledge_count,
        )
    }
}

struct RestartableCursor {
    state: SharedCursorState,
}

#[async_trait]
impl ConsumerCursor for RestartableCursor {
    async fn receive(&mut self) -> Result<Option<SubscriberMessage>, ConnectorError> {
        let mut state = self.state.inner.lock().await;
        if state.committed {
            return Ok(None);
        }
        state.receive_count += 1;
        Ok(Some(state.message.clone()))
    }

    async fn acknowledge(&mut self, ack_token: &str) -> Result<(), ConnectorError> {
        let mut state = self.state.inner.lock().await;
        state.acknowledge_count += 1;
        if state.message.metadata.ack_token.as_deref() != Some(ack_token) {
            return Err(ConnectorError::Connection(
                "ack token does not match the delivered cursor position".to_string(),
            ));
        }
        if state.ack_failures_remaining > 0 {
            state.ack_failures_remaining -= 1;
            return Err(ConnectorError::Connection(
                "injected acknowledgement failure".to_string(),
            ));
        }
        state.committed = true;
        Ok(())
    }
}

fn group(state: SharedCursorState) -> PersistentContractConsumerGroup {
    PersistentContractConsumerGroup::new(
        STREAM.to_string(),
        TOPIC.to_string(),
        Arc::new(JsonSerializer),
        Box::new(RestartableCursor { state }),
    )
}

fn valid_payload() -> (ContractEventEnvelope, Vec<u8>) {
    let envelope = ContractEventEnvelope::new_caused_by(
        Uuid::new_v4(),
        None,
        Uuid::new_v4(),
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision: 7,
            target_type: "forum_category".to_string(),
            target_id: Some(Uuid::new_v4()),
        },
    )
    .expect("valid Forum Search contract envelope");
    let payload = JsonSerializer
        .serialize_contract(&envelope)
        .expect("contract JSON should serialize");
    (envelope, payload)
}

#[tokio::test]
async fn event_ack_failure_is_redelivered_after_consumer_reconstruction() {
    let (envelope, payload) = valid_payload();
    let state = SharedCursorState::new(payload.clone(), 1);

    let first_group = group(state.clone());
    let first = match first_group
        .receive_delivery()
        .await
        .expect("first receive should succeed")
        .expect("delivery should exist")
    {
        PersistentContractDelivery::Event(consumed) => consumed,
        PersistentContractDelivery::DecodeFailure(_) => {
            panic!("valid contract bytes must not become decode poison")
        }
    };
    assert_eq!(first.envelope.id(), envelope.id());
    assert_eq!(first.offset(), Some(OFFSET));
    assert_eq!(first.raw_payload(), payload.as_slice());
    assert_eq!(first.ack_token(), Some(ACK_TOKEN));
    assert!(first_group.acknowledge(&first).await.is_err());
    assert_eq!(state.snapshot().await, (false, 1, 1));
    drop(first_group);

    let restarted_group = group(state.clone());
    let redelivery = match restarted_group
        .receive_delivery()
        .await
        .expect("restart receive should succeed")
        .expect("uncommitted delivery should be redelivered")
    {
        PersistentContractDelivery::Event(consumed) => consumed,
        PersistentContractDelivery::DecodeFailure(_) => {
            panic!("redelivered valid bytes must remain a contract event")
        }
    };
    assert_eq!(redelivery.envelope.id(), envelope.id());
    assert_eq!(redelivery.offset(), Some(OFFSET));
    assert_eq!(redelivery.raw_payload(), payload.as_slice());
    restarted_group
        .acknowledge(&redelivery)
        .await
        .expect("restarted consumer should commit the exact offset");
    assert_eq!(state.snapshot().await, (true, 2, 2));
    assert!(
        restarted_group
            .receive_delivery()
            .await
            .expect("post-ack receive should succeed")
            .is_none()
    );
}

#[tokio::test]
async fn decode_failure_ack_failure_is_redelivered_after_consumer_reconstruction() {
    let payload = b"{not-valid-contract-json".to_vec();
    let state = SharedCursorState::new(payload.clone(), 1);

    let first_group = group(state.clone());
    let first = match first_group
        .receive_delivery()
        .await
        .expect("first raw receive should succeed")
        .expect("raw delivery should exist")
    {
        PersistentContractDelivery::DecodeFailure(failure) => failure,
        PersistentContractDelivery::Event(_) => panic!("invalid bytes must be decode poison"),
    };
    let delivery_id = first.delivery_id();
    let stable_error_code = first.stable_error_code();
    assert_eq!(first.offset(), OFFSET);
    assert_eq!(first.raw_payload(), payload.as_slice());
    assert!(
        first_group
            .acknowledge_decode_failure(&first)
            .await
            .is_err()
    );
    assert_eq!(state.snapshot().await, (false, 1, 1));
    drop(first_group);

    let restarted_group = group(state.clone());
    let redelivery = match restarted_group
        .receive_delivery()
        .await
        .expect("restart raw receive should succeed")
        .expect("uncommitted raw delivery should be redelivered")
    {
        PersistentContractDelivery::DecodeFailure(failure) => failure,
        PersistentContractDelivery::Event(_) => {
            panic!("redelivered invalid bytes must remain decode poison")
        }
    };
    assert_eq!(redelivery.delivery_id(), delivery_id);
    assert_eq!(redelivery.stable_error_code(), stable_error_code);
    assert_eq!(redelivery.offset(), OFFSET);
    assert_eq!(redelivery.raw_payload(), payload.as_slice());
    restarted_group
        .acknowledge_decode_failure(&redelivery)
        .await
        .expect("restarted consumer should commit the poison offset");
    assert_eq!(state.snapshot().await, (true, 2, 2));
    assert!(
        restarted_group
            .receive_delivery()
            .await
            .expect("post-poison-ack receive should succeed")
            .is_none()
    );
}

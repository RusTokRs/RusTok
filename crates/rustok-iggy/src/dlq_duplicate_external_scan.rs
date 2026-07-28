use std::collections::BTreeSet;

use iggy::prelude::{
    Consumer, ConsumerKind, Identifier, IggyClient, MessageClient, PollingStrategy,
};
use thiserror::Error;
use uuid::Uuid;

use crate::dlq_duplicate_inspection::{
    DlqDuplicateInspectionError, DlqDuplicateObservation, DlqDuplicateSummary,
    summarize_dlq_duplicates,
};

const DLQ_TOPIC: &str = "dlq";
const READ_ONLY_CONSUMER: &str = "rustok-dlq-duplicate-readonly-v1";
const MAX_SCAN_MESSAGES: u32 = 10_000;
const MAX_BATCH_MESSAGES: u32 = 1_000;
const MAX_SCAN_PARTITIONS: usize = 128;
const MAX_STREAM_NAME_BYTES: usize = 255;

/// Bounded explicit-offset request for a read-only physical Iggy DLQ scan.
///
/// The request contains no broker address, credentials, payload, message ID, or
/// mutation policy. Every partition starts at the same explicit offset. A caller
/// that needs partition-specific windows must issue separate requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IggyDlqDuplicateScanRequest {
    partitions: Vec<u32>,
    start_offset: u64,
    max_messages: u32,
    batch_size: u32,
}

impl IggyDlqDuplicateScanRequest {
    pub fn new(
        partitions: Vec<u32>,
        start_offset: u64,
        max_messages: u32,
        batch_size: u32,
    ) -> Result<Self, IggyDlqDuplicateScanError> {
        validate_partitions(&partitions)?;
        if max_messages == 0 || max_messages > MAX_SCAN_MESSAGES {
            return Err(IggyDlqDuplicateScanError::InvalidRequest);
        }
        if batch_size == 0
            || batch_size > MAX_BATCH_MESSAGES
            || batch_size > max_messages
        {
            return Err(IggyDlqDuplicateScanError::InvalidRequest);
        }
        Ok(Self {
            partitions,
            start_offset,
            max_messages,
            batch_size,
        })
    }

    pub fn partitions(&self) -> &[u32] {
        &self.partitions
    }

    pub const fn start_offset(&self) -> u64 {
        self.start_offset
    }

    pub const fn max_messages(&self) -> u32 {
        self.max_messages
    }

    pub const fn batch_size(&self) -> u32 {
        self.batch_size
    }
}

/// External-Iggy adapter that polls immutable physical DLQ messages without
/// storing a consumer offset.
///
/// The scanner uses a standalone consumer, explicit `PollingStrategy::offset`
/// requests, an explicit partition ID, and `auto_commit = false`. It cannot
/// acknowledge, delete, purge, replay, publish, retry, or mutate poison receipts.
/// It returns only the identifier-free [`DlqDuplicateSummary`]. Connection and
/// authentication lifecycle remain owned by the caller.
pub struct IggyDlqDuplicateScanner<'a> {
    client: &'a IggyClient,
    stream_id: Identifier,
    topic_id: Identifier,
    consumer: Consumer,
}

impl<'a> IggyDlqDuplicateScanner<'a> {
    pub fn new(
        client: &'a IggyClient,
        stream_name: &str,
    ) -> Result<Self, IggyDlqDuplicateScanError> {
        validate_stream_name(stream_name)?;
        let stream_id: Identifier = stream_name
            .to_owned()
            .try_into()
            .map_err(|_| IggyDlqDuplicateScanError::InvalidRequest)?;
        let topic_id: Identifier = DLQ_TOPIC
            .to_owned()
            .try_into()
            .map_err(|_| IggyDlqDuplicateScanError::InvalidRequest)?;
        let consumer_id: Identifier = READ_ONLY_CONSUMER
            .to_owned()
            .try_into()
            .map_err(|_| IggyDlqDuplicateScanError::InvalidRequest)?;
        Ok(Self {
            client,
            stream_id,
            topic_id,
            consumer: Consumer {
                kind: ConsumerKind::Consumer,
                id: consumer_id,
            },
        })
    }

    pub async fn summarize(
        &self,
        request: &IggyDlqDuplicateScanRequest,
    ) -> Result<DlqDuplicateSummary, IggyDlqDuplicateScanError> {
        let mut remaining = request.max_messages;
        let mut observations = Vec::with_capacity(request.max_messages as usize);

        for &partition_id in &request.partitions {
            if remaining == 0 {
                break;
            }
            let mut next_offset = request.start_offset;

            loop {
                let requested_count = request.batch_size.min(remaining);
                let polled = self
                    .client
                    .poll_messages(
                        &self.stream_id,
                        &self.topic_id,
                        Some(partition_id),
                        &self.consumer,
                        &PollingStrategy::offset(next_offset),
                        requested_count,
                        false,
                    )
                    .await
                    .map_err(|_| IggyDlqDuplicateScanError::PollFailed)?;

                if polled.partition_id != partition_id
                    || polled.count as usize != polled.messages.len()
                    || polled.count > requested_count
                {
                    return Err(IggyDlqDuplicateScanError::InvalidBrokerResponse);
                }
                if polled.messages.is_empty() {
                    break;
                }

                let received_count = polled.count;
                let mut previous_offset = None;
                for message in polled.messages {
                    let offset = message.header.offset;
                    if offset < next_offset
                        || previous_offset.is_some_and(|previous| offset <= previous)
                    {
                        return Err(IggyDlqDuplicateScanError::InvalidBrokerResponse);
                    }
                    observations.push(DlqDuplicateObservation::from_payload(
                        Uuid::from_u128(message.header.id),
                        message.payload.as_ref(),
                    )?);
                    remaining = remaining
                        .checked_sub(1)
                        .ok_or(IggyDlqDuplicateScanError::InvalidBrokerResponse)?;
                    previous_offset = Some(offset);
                }

                let last_offset = previous_offset
                    .ok_or(IggyDlqDuplicateScanError::InvalidBrokerResponse)?;
                next_offset = last_offset
                    .checked_add(1)
                    .ok_or(IggyDlqDuplicateScanError::OffsetOverflow)?;

                if remaining == 0 || received_count < requested_count {
                    break;
                }
            }
        }

        summarize_dlq_duplicates(observations).map_err(Into::into)
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum IggyDlqDuplicateScanError {
    #[error("external Iggy DLQ duplicate scan request is invalid")]
    InvalidRequest,
    #[error("external Iggy DLQ duplicate polling failed")]
    PollFailed,
    #[error("external Iggy DLQ duplicate poll response is invalid")]
    InvalidBrokerResponse,
    #[error("external Iggy DLQ duplicate offset overflow")]
    OffsetOverflow,
    #[error(transparent)]
    Inspection(#[from] DlqDuplicateInspectionError),
}

impl IggyDlqDuplicateScanError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "iggy.dlq_duplicate.scan_invalid",
            Self::PollFailed => "iggy.dlq_duplicate.scan_failed",
            Self::InvalidBrokerResponse => "iggy.dlq_duplicate.scan_response_invalid",
            Self::OffsetOverflow => "iggy.dlq_duplicate.scan_offset_overflow",
            Self::Inspection(error) => error.stable_code(),
        }
    }
}

fn validate_partitions(partitions: &[u32]) -> Result<(), IggyDlqDuplicateScanError> {
    if partitions.is_empty() || partitions.len() > MAX_SCAN_PARTITIONS {
        return Err(IggyDlqDuplicateScanError::InvalidRequest);
    }
    let mut unique = BTreeSet::new();
    for &partition in partitions {
        if partition == 0 || !unique.insert(partition) {
            return Err(IggyDlqDuplicateScanError::InvalidRequest);
        }
    }
    Ok(())
}

fn validate_stream_name(stream_name: &str) -> Result<(), IggyDlqDuplicateScanError> {
    if stream_name.is_empty()
        || stream_name.trim() != stream_name
        || stream_name.len() > MAX_STREAM_NAME_BYTES
        || stream_name.chars().any(char::is_control)
    {
        return Err(IggyDlqDuplicateScanError::InvalidRequest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_request_requires_unique_positive_partitions() {
        let valid = IggyDlqDuplicateScanRequest::new(vec![1, 3], 42, 100, 25).unwrap();
        assert_eq!(valid.partitions(), &[1, 3]);
        assert_eq!(valid.start_offset(), 42);
        assert_eq!(valid.max_messages(), 100);
        assert_eq!(valid.batch_size(), 25);

        for partitions in [vec![], vec![0], vec![1, 1]] {
            assert!(matches!(
                IggyDlqDuplicateScanRequest::new(partitions, 0, 1, 1),
                Err(IggyDlqDuplicateScanError::InvalidRequest)
            ));
        }
    }

    #[test]
    fn bounded_request_rejects_unbounded_counts() {
        for (max_messages, batch_size) in [
            (0, 1),
            (MAX_SCAN_MESSAGES + 1, 1),
            (1, 0),
            (1, 2),
            (MAX_SCAN_MESSAGES, MAX_BATCH_MESSAGES + 1),
        ] {
            assert!(matches!(
                IggyDlqDuplicateScanRequest::new(vec![1], 0, max_messages, batch_size),
                Err(IggyDlqDuplicateScanError::InvalidRequest)
            ));
        }
    }

    #[test]
    fn stable_errors_do_not_expose_broker_coordinates() {
        assert_eq!(
            IggyDlqDuplicateScanError::InvalidRequest.stable_code(),
            "iggy.dlq_duplicate.scan_invalid"
        );
        assert_eq!(
            IggyDlqDuplicateScanError::PollFailed.stable_code(),
            "iggy.dlq_duplicate.scan_failed"
        );
        assert_eq!(
            IggyDlqDuplicateScanError::InvalidBrokerResponse.stable_code(),
            "iggy.dlq_duplicate.scan_response_invalid"
        );
        assert_eq!(
            IggyDlqDuplicateScanError::OffsetOverflow.stable_code(),
            "iggy.dlq_duplicate.scan_offset_overflow"
        );
    }
}
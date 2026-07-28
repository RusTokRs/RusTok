use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::index_dlq_receipt::SocialGraphIndexDlqIdentity;

const DLQ_MESSAGE_ID_DOMAIN: &[u8] = b"rustok.social_graph.index.dlq.message_id.v1";

/// Derives one stable RFC 9562 UUIDv8 for the immutable owner-side DLQ identity.
///
/// The broker identifier is an additional duplicate-suppression key. The durable
/// receipt remains authoritative because Iggy deduplication is deployment-owned,
/// bounded, and may be disabled or expire before a retry.
pub(crate) fn social_graph_index_dlq_broker_message_id(
    identity: &SocialGraphIndexDlqIdentity,
) -> Uuid {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, DLQ_MESSAGE_ID_DOMAIN);
    hash_part(&mut hasher, identity.tenant_id.as_bytes());
    hash_part(&mut hasher, identity.consumer_group.as_bytes());
    hash_part(&mut hasher, identity.event_id.as_bytes());
    hash_part(&mut hasher, identity.source_stream.as_bytes());
    hash_part(&mut hasher, identity.source_topic.as_bytes());
    hash_part(&mut hasher, &identity.source_partition.to_be_bytes());
    hash_part(&mut hasher, &identity.source_offset.to_be_bytes());
    hash_part(&mut hasher, &identity.payload);

    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // RFC 9562 variant plus custom version 8. The hash construction above is
    // the versioned application contract; this is not UUIDv5/SHA-1.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    hasher.update(length.to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(payload: Vec<u8>) -> SocialGraphIndexDlqIdentity {
        SocialGraphIndexDlqIdentity::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "rustok-social-graph-index",
            "rustok",
            "domain",
            1,
            42,
            payload,
        )
        .unwrap()
    }

    #[test]
    fn broker_message_id_is_stable_and_custom_versioned() {
        let first = social_graph_index_dlq_broker_message_id(&identity(vec![1, 2, 3]));
        let second = social_graph_index_dlq_broker_message_id(&identity(vec![1, 2, 3]));

        assert_eq!(first, second);
        assert_eq!(first.as_bytes()[6] >> 4, 8);
        assert_eq!(first.as_bytes()[8] & 0xc0, 0x80);
    }

    #[test]
    fn broker_message_id_changes_with_exact_payload_or_source_position() {
        let base = identity(vec![1, 2, 3]);
        let different_payload = identity(vec![1, 2, 4]);
        let mut different_offset = identity(vec![1, 2, 3]);
        different_offset.source_offset = 43;

        assert_ne!(
            social_graph_index_dlq_broker_message_id(&base),
            social_graph_index_dlq_broker_message_id(&different_payload)
        );
        assert_ne!(
            social_graph_index_dlq_broker_message_id(&base),
            social_graph_index_dlq_broker_message_id(&different_offset)
        );
    }
}

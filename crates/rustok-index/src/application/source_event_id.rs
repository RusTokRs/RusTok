use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::LocaleKey;

const MAX_SOURCE_EVENT_DOMAIN_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexSourceEventIdError {
    #[error("Index source event identity domain is invalid: {0}")]
    InvalidDomain(String),
    #[error("Index source event identity tenant cannot be nil")]
    NilTenantId,
    #[error("Index source event identity entity cannot be nil")]
    NilEntityId,
    #[error("Index source event identity source version must be positive")]
    ZeroSourceVersion,
}

/// Derives one deterministic non-nil UUID for a logical source mutation version.
///
/// Source adapters use this helper instead of inventing random replay delivery IDs. The identity is
/// stable across retries and changes when tenant, entity, locale, source version, or the
/// owner-selected versioned domain changes. The helper performs no I/O and does not expose the hash
/// bytes as a public persistence contract.
pub fn derive_index_source_event_id(
    domain: &str,
    tenant_id: Uuid,
    entity_id: Uuid,
    locale: Option<&LocaleKey>,
    source_version: u64,
) -> Result<Uuid, IndexSourceEventIdError> {
    validate_domain(domain)?;
    if tenant_id.is_nil() {
        return Err(IndexSourceEventIdError::NilTenantId);
    }
    if entity_id.is_nil() {
        return Err(IndexSourceEventIdError::NilEntityId);
    }
    if source_version == 0 {
        return Err(IndexSourceEventIdError::ZeroSourceVersion);
    }

    let mut hasher = Sha256::new();
    write_bytes(&mut hasher, b"rustok-index-source-event-id-v1");
    write_bytes(&mut hasher, domain.as_bytes());
    hasher.update(tenant_id.as_bytes());
    hasher.update(entity_id.as_bytes());
    match locale {
        Some(locale) => {
            hasher.update([1]);
            write_bytes(&mut hasher, locale.as_str().as_bytes());
        }
        None => hasher.update([0]),
    }
    hasher.update(source_version.to_be_bytes());

    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let event_id = Uuid::from_bytes(bytes);
    debug_assert!(!event_id.is_nil());
    Ok(event_id)
}

fn validate_domain(domain: &str) -> Result<(), IndexSourceEventIdError> {
    let valid = !domain.is_empty()
        && domain.len() <= MAX_SOURCE_EVENT_DOMAIN_BYTES
        && domain.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.')
        });
    if valid {
        Ok(())
    } else {
        Err(IndexSourceEventIdError::InvalidDomain(domain.to_owned()))
    }
}

fn write_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_event_identity_is_stable_and_scope_sensitive() {
        let tenant_id = Uuid::from_u128(1);
        let entity_id = Uuid::from_u128(2);
        let locale = LocaleKey::new("en-US").unwrap();
        let first = derive_index_source_event_id(
            "rustok-product.product-replay-v1",
            tenant_id,
            entity_id,
            Some(&locale),
            7,
        )
        .unwrap();
        let retry = derive_index_source_event_id(
            "rustok-product.product-replay-v1",
            tenant_id,
            entity_id,
            Some(&locale),
            7,
        )
        .unwrap();
        assert_eq!(first, retry);
        assert_ne!(
            first,
            derive_index_source_event_id(
                "rustok-product.product-replay-v1",
                tenant_id,
                entity_id,
                Some(&locale),
                8,
            )
            .unwrap()
        );
        assert_ne!(
            first,
            derive_index_source_event_id(
                "rustok-product.product-replay-v1",
                tenant_id,
                entity_id,
                None,
                7,
            )
            .unwrap()
        );
        assert!(!first.is_nil());
    }

    #[test]
    fn source_event_identity_rejects_invalid_scope() {
        assert!(matches!(
            derive_index_source_event_id("BAD DOMAIN", Uuid::from_u128(1), Uuid::from_u128(2), None, 1),
            Err(IndexSourceEventIdError::InvalidDomain(_))
        ));
        assert_eq!(
            derive_index_source_event_id("source.v1", Uuid::nil(), Uuid::from_u128(2), None, 1),
            Err(IndexSourceEventIdError::NilTenantId)
        );
        assert_eq!(
            derive_index_source_event_id("source.v1", Uuid::from_u128(1), Uuid::nil(), None, 1),
            Err(IndexSourceEventIdError::NilEntityId)
        );
        assert_eq!(
            derive_index_source_event_id("source.v1", Uuid::from_u128(1), Uuid::from_u128(2), None, 0),
            Err(IndexSourceEventIdError::ZeroSourceVersion)
        );
    }
}

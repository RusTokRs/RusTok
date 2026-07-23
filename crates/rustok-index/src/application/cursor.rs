use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{IndexValue, LocaleKey, SchemaFingerprint, SchemaRef};

const CURSOR_VERSION: u8 = 1;
const CHECKSUM_LEN: usize = 16;

/// Stable keyset cursor payload.
///
/// The schema fingerprint prevents a cursor from silently crossing a schema
/// contract change. Tenant and locale remain explicit so a cursor cannot be
/// reused in another scope by accident.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexCursor {
    pub tenant_id: Uuid,
    pub schema: SchemaRef,
    pub schema_fingerprint: SchemaFingerprint,
    pub locale: Option<LocaleKey>,
    pub order_values: Vec<IndexValue>,
    pub entity_id: Uuid,
}

#[derive(Debug, Error)]
pub enum CursorCodecError {
    #[error("cursor encoding is invalid: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("cursor payload is too short")]
    TooShort,

    #[error("unsupported cursor version: {0}")]
    UnsupportedVersion(u8),

    #[error("cursor checksum is invalid")]
    InvalidChecksum,

    #[error("cursor payload serialization failed: {0}")]
    Postcard(#[from] postcard::Error),
}

pub struct CursorCodec;

impl CursorCodec {
    pub fn encode(cursor: &IndexCursor) -> Result<String, CursorCodecError> {
        let payload = postcard::to_stdvec(cursor)?;
        let checksum = Sha256::digest(&payload);
        let mut envelope = Vec::with_capacity(1 + payload.len() + CHECKSUM_LEN);
        envelope.push(CURSOR_VERSION);
        envelope.extend_from_slice(&payload);
        envelope.extend_from_slice(&checksum[..CHECKSUM_LEN]);
        Ok(URL_SAFE_NO_PAD.encode(envelope))
    }

    pub fn decode(encoded: &str) -> Result<IndexCursor, CursorCodecError> {
        let envelope = URL_SAFE_NO_PAD.decode(encoded)?;
        if envelope.len() < 1 + CHECKSUM_LEN {
            return Err(CursorCodecError::TooShort);
        }

        let version = envelope[0];
        if version != CURSOR_VERSION {
            return Err(CursorCodecError::UnsupportedVersion(version));
        }

        let payload_end = envelope.len() - CHECKSUM_LEN;
        let payload = &envelope[1..payload_end];
        let checksum = &envelope[payload_end..];
        let expected = Sha256::digest(payload);
        if checksum != &expected[..CHECKSUM_LEN] {
            return Err(CursorCodecError::InvalidChecksum);
        }

        Ok(postcard::from_bytes(payload)?)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValueType,
        LocaleMode, ModuleName, SchemaVersion,
    };

    fn schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("rustok-product").unwrap(),
                entity: EntityName::new("product").unwrap(),
                version: SchemaVersion::INITIAL,
            },
            locale_mode: LocaleMode::Required,
            fields: vec![IndexField {
                name: FieldName::new("id").unwrap(),
                value_type: IndexValueType::Uuid,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            }],
            links: Vec::new(),
        }
    }

    proptest! {
        #[test]
        fn cursor_round_trip_preserves_scope_and_order(
            tenant in any::<u128>(),
            entity in any::<u128>(),
            values in prop::collection::vec(any::<i64>(), 0..8),
            use_locale in any::<bool>(),
        ) {
            let schema = schema();
            let cursor = IndexCursor {
                tenant_id: Uuid::from_u128(tenant),
                schema: schema.reference.clone(),
                schema_fingerprint: schema.fingerprint().unwrap(),
                locale: use_locale.then(|| LocaleKey::new("en-US").unwrap()),
                order_values: values.into_iter().map(IndexValue::Integer).collect(),
                entity_id: Uuid::from_u128(entity),
            };

            let encoded = CursorCodec::encode(&cursor).unwrap();
            prop_assert_eq!(CursorCodec::decode(&encoded).unwrap(), cursor);
        }
    }

    #[test]
    fn rejects_corrupted_cursor() {
        let schema = schema();
        let cursor = IndexCursor {
            tenant_id: Uuid::new_v4(),
            schema: schema.reference.clone(),
            schema_fingerprint: schema.fingerprint().unwrap(),
            locale: Some(LocaleKey::new("en-US").unwrap()),
            order_values: vec![IndexValue::Integer(42)],
            entity_id: Uuid::new_v4(),
        };
        let mut encoded = CursorCodec::encode(&cursor).unwrap().into_bytes();
        let last = encoded.last_mut().unwrap();
        *last = if *last == b'A' { b'B' } else { b'A' };
        let encoded = String::from_utf8(encoded).unwrap();

        assert!(CursorCodec::decode(&encoded).is_err());
    }
}

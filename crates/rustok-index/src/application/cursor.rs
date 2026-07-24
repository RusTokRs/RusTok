use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{IndexQuery, IndexValue, LocaleKey, SchemaFingerprint, SchemaRef};

use super::{SchemaRegistry, SchemaRegistryError};

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

#[derive(Debug, Error)]
pub enum CursorValidationError {
    #[error(transparent)]
    Codec(#[from] CursorCodecError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
    #[error("cursor tenant does not match query tenant")]
    TenantMismatch,
    #[error("cursor schema does not match query schema")]
    SchemaMismatch,
    #[error("cursor schema fingerprint is stale")]
    SchemaFingerprintMismatch,
    #[error("cursor locale does not match query locale")]
    LocaleMismatch,
    #[error("cursor contains {actual} order values but query defines {expected} order expressions")]
    OrderArityMismatch { expected: usize, actual: usize },
    #[error("cursor entity id must not be nil")]
    NilEntityId,
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

    pub fn decode_for_query(
        encoded: &str,
        query: &IndexQuery,
        registry: &SchemaRegistry,
    ) -> Result<IndexCursor, CursorValidationError> {
        let cursor = Self::decode(encoded)?;
        if cursor.tenant_id != query.scope.tenant_id {
            return Err(CursorValidationError::TenantMismatch);
        }
        if cursor.schema != query.schema {
            return Err(CursorValidationError::SchemaMismatch);
        }
        if cursor.locale != query.scope.locale {
            return Err(CursorValidationError::LocaleMismatch);
        }
        if cursor.entity_id.is_nil() {
            return Err(CursorValidationError::NilEntityId);
        }

        let registered = registry
            .get(&query.schema)
            .ok_or_else(|| SchemaRegistryError::SchemaNotFound(query.schema.clone()))?;
        if cursor.schema_fingerprint != registered.fingerprint {
            return Err(CursorValidationError::SchemaFingerprintMismatch);
        }
        if cursor.order_values.len() != query.order_by.len() {
            return Err(CursorValidationError::OrderArityMismatch {
                expected: query.order_by.len(),
                actual: cursor.order_values.len(),
            });
        }

        Ok(cursor)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, FieldPath, IndexField, IndexQueryScope,
        IndexSchema, IndexValueType, LocaleMode, ModuleName, OrderDirection, OrderExpr, Pagination,
        SchemaVersion,
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

    fn registry_with_schema(schema: &IndexSchema) -> SchemaRegistry {
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        registry
    }

    fn query(schema: &IndexSchema, tenant_id: Uuid) -> IndexQuery {
        IndexQuery {
            scope: IndexQueryScope {
                tenant_id,
                locale: Some(LocaleKey::new("en-US").unwrap()),
            },
            schema: schema.reference.clone(),
            fields: vec![FieldPath::new(FieldName::new("id").unwrap())],
            filter: None,
            order_by: vec![OrderExpr {
                field: FieldPath::new(FieldName::new("id").unwrap()),
                direction: OrderDirection::Asc,
            }],
            pagination: Pagination::Cursor {
                first: 20,
                after: None,
            },
            include_exact_count: false,
        }
    }

    proptest! {
        #[test]
        fn cursor_round_trip_preserves_scope_and_order(
            tenant in 1u128..u128::MAX,
            entity in 1u128..u128::MAX,
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
    fn validates_cursor_against_query_scope() {
        let schema = schema();
        let registry = registry_with_schema(&schema);
        let tenant_id = Uuid::new_v4();
        let query = query(&schema, tenant_id);
        let cursor = IndexCursor {
            tenant_id,
            schema: schema.reference.clone(),
            schema_fingerprint: schema.fingerprint().unwrap(),
            locale: query.scope.locale.clone(),
            order_values: vec![IndexValue::Uuid(Uuid::new_v4())],
            entity_id: Uuid::new_v4(),
        };
        let encoded = CursorCodec::encode(&cursor).unwrap();

        assert_eq!(
            CursorCodec::decode_for_query(&encoded, &query, &registry).unwrap(),
            cursor
        );
    }

    #[test]
    fn rejects_cross_tenant_cursor() {
        let schema = schema();
        let registry = registry_with_schema(&schema);
        let query = query(&schema, Uuid::new_v4());
        let cursor = IndexCursor {
            tenant_id: Uuid::new_v4(),
            schema: schema.reference.clone(),
            schema_fingerprint: schema.fingerprint().unwrap(),
            locale: query.scope.locale.clone(),
            order_values: vec![IndexValue::Uuid(Uuid::new_v4())],
            entity_id: Uuid::new_v4(),
        };
        let encoded = CursorCodec::encode(&cursor).unwrap();

        assert!(matches!(
            CursorCodec::decode_for_query(&encoded, &query, &registry),
            Err(CursorValidationError::TenantMismatch)
        ));
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

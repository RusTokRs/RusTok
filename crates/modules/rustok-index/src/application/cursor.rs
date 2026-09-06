use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{
    FieldPath, FilterExpr, IndexQuery, IndexValue, IndexValueType, LocaleKey, OrderExpr,
    SchemaFingerprint, SchemaRef,
};

use super::{QueryValidationError, SchemaRegistry, SchemaRegistryError};

const CURSOR_VERSION: u8 = 1;
const SCOPED_CURSOR_VERSION: u8 = 2;
const CHECKSUM_LEN: usize = 16;

/// Stable keyset cursor payload.
///
/// Tenant, schema, locale, order values, and entity identity remain explicit.
/// Production continuation tokens must be encoded with `encode_for_query`, which
/// adds a separate filter/order query fingerprint around this payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexCursor {
    pub tenant_id: Uuid,
    pub schema: SchemaRef,
    pub schema_fingerprint: SchemaFingerprint,
    pub locale: Option<LocaleKey>,
    pub order_values: Vec<IndexValue>,
    pub entity_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ScopedCursorEnvelope {
    query_fingerprint: [u8; 32],
    cursor: IndexCursor,
}

#[derive(Serialize)]
struct CursorQueryIdentity<'a> {
    tenant_id: Uuid,
    schema: &'a SchemaRef,
    locale: &'a Option<LocaleKey>,
    filter: &'a Option<FilterExpr>,
    order_by: &'a [OrderExpr],
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
    Query(#[from] QueryValidationError),
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
    #[error("cursor query fingerprint does not match filter/order semantics")]
    QueryFingerprintMismatch,
    #[error("cursor contains {actual} order values but query defines {expected} order expressions")]
    OrderArityMismatch { expected: usize, actual: usize },
    #[error("cursor order field contract is missing at position {index}")]
    OrderFieldContractMissing { index: usize },
    #[error("cursor order value at position {index} has type {actual:?}, expected {expected:?}")]
    OrderValueTypeMismatch {
        index: usize,
        expected: IndexValueType,
        actual: Option<IndexValueType>,
    },
    #[error("cursor entity id must not be nil")]
    NilEntityId,
}

pub struct CursorCodec;

impl CursorCodec {
    /// Encode the raw cursor payload for storage/round-trip tests.
    ///
    /// Query continuation APIs must use `encode_for_query` instead.
    pub fn encode(cursor: &IndexCursor) -> Result<String, CursorCodecError> {
        encode_payload(CURSOR_VERSION, cursor)
    }

    pub fn decode(encoded: &str) -> Result<IndexCursor, CursorCodecError> {
        decode_payload(encoded, CURSOR_VERSION)
    }

    /// Encode a production continuation token bound to filter and order semantics.
    pub fn encode_for_query(
        cursor: &IndexCursor,
        query: &IndexQuery,
        registry: &SchemaRegistry,
    ) -> Result<String, CursorValidationError> {
        validate_cursor(cursor, query, registry)?;
        let envelope = ScopedCursorEnvelope {
            query_fingerprint: query_fingerprint(query)?,
            cursor: cursor.clone(),
        };
        Ok(encode_payload(SCOPED_CURSOR_VERSION, &envelope)?)
    }

    /// Decode the legacy raw envelope and validate scope/type fields.
    ///
    /// This remains for the test-only reference engine. SQL continuation paths
    /// require `decode_scoped_for_query`.
    pub fn decode_for_query(
        encoded: &str,
        query: &IndexQuery,
        registry: &SchemaRegistry,
    ) -> Result<IndexCursor, CursorValidationError> {
        let cursor = Self::decode(encoded)?;
        validate_cursor(&cursor, query, registry)?;
        Ok(cursor)
    }

    /// Decode a query-scoped continuation token for PostgreSQL keyset compilation.
    pub fn decode_scoped_for_query(
        encoded: &str,
        query: &IndexQuery,
        registry: &SchemaRegistry,
    ) -> Result<IndexCursor, CursorValidationError> {
        let envelope: ScopedCursorEnvelope = decode_payload(encoded, SCOPED_CURSOR_VERSION)?;
        if envelope.query_fingerprint != query_fingerprint(query)? {
            return Err(CursorValidationError::QueryFingerprintMismatch);
        }
        validate_cursor(&envelope.cursor, query, registry)?;
        Ok(envelope.cursor)
    }
}

fn encode_payload<T: Serialize>(version: u8, value: &T) -> Result<String, CursorCodecError> {
    let payload = postcard::to_stdvec(value)?;
    let checksum = Sha256::digest(&payload);
    let mut envelope = Vec::with_capacity(1 + payload.len() + CHECKSUM_LEN);
    envelope.push(version);
    envelope.extend_from_slice(&payload);
    envelope.extend_from_slice(&checksum[..CHECKSUM_LEN]);
    Ok(URL_SAFE_NO_PAD.encode(envelope))
}

fn decode_payload<T: DeserializeOwned>(
    encoded: &str,
    expected_version: u8,
) -> Result<T, CursorCodecError> {
    let envelope = URL_SAFE_NO_PAD.decode(encoded)?;
    if envelope.len() < 1 + CHECKSUM_LEN {
        return Err(CursorCodecError::TooShort);
    }

    let version = envelope[0];
    if version != expected_version {
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

fn query_fingerprint(query: &IndexQuery) -> Result<[u8; 32], CursorCodecError> {
    let identity = CursorQueryIdentity {
        tenant_id: query.scope.tenant_id,
        schema: &query.schema,
        locale: &query.scope.locale,
        filter: &query.filter,
        order_by: &query.order_by,
    };
    let bytes = postcard::to_stdvec(&identity)?;
    let mut hasher = Sha256::new();
    hasher.update(b"rustok-index-cursor-query-v1");
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

fn validate_cursor(
    cursor: &IndexCursor,
    query: &IndexQuery,
    registry: &SchemaRegistry,
) -> Result<(), CursorValidationError> {
    registry.validate_query(query)?;
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

    for (index, (order, value)) in query.order_by.iter().zip(&cursor.order_values).enumerate() {
        if matches!(value, IndexValue::Null) {
            continue;
        }
        let expected = resolve_order_value_type(registry, &query.schema, &order.field)
            .ok_or(CursorValidationError::OrderFieldContractMissing { index })?;
        let actual = value.value_type();
        if actual != Some(expected) {
            return Err(CursorValidationError::OrderValueTypeMismatch {
                index,
                expected,
                actual,
            });
        }
    }

    Ok(())
}

fn resolve_order_value_type(
    registry: &SchemaRegistry,
    root: &SchemaRef,
    path: &FieldPath,
) -> Option<IndexValueType> {
    let mut registered = registry.get(root)?;
    for link_name in path.links() {
        let link = registered
            .schema
            .links
            .iter()
            .find(|link| link.name == *link_name)?;
        registered = registry.get(&link.target_schema)?;
    }
    registered
        .schema
        .fields
        .iter()
        .find(|field| field.name == *path.field())
        .map(|field| field.value_type)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, FieldPath, IndexField, IndexQueryScope,
        IndexSchema, LocaleMode, ModuleName, OrderDirection, Pagination, SchemaVersion,
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
    fn validates_scoped_cursor_against_query() {
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
        let encoded = CursorCodec::encode_for_query(&cursor, &query, &registry).unwrap();

        assert_eq!(
            CursorCodec::decode_scoped_for_query(&encoded, &query, &registry).unwrap(),
            cursor
        );
    }

    #[test]
    fn rejects_cursor_reuse_across_order_semantics() {
        let schema = schema();
        let registry = registry_with_schema(&schema);
        let tenant_id = Uuid::new_v4();
        let original = query(&schema, tenant_id);
        let cursor = IndexCursor {
            tenant_id,
            schema: schema.reference.clone(),
            schema_fingerprint: schema.fingerprint().unwrap(),
            locale: original.scope.locale.clone(),
            order_values: vec![IndexValue::Uuid(Uuid::new_v4())],
            entity_id: Uuid::new_v4(),
        };
        let encoded = CursorCodec::encode_for_query(&cursor, &original, &registry).unwrap();
        let mut changed = original.clone();
        changed.order_by[0].direction = OrderDirection::Desc;

        assert!(matches!(
            CursorCodec::decode_scoped_for_query(&encoded, &changed, &registry),
            Err(CursorValidationError::QueryFingerprintMismatch)
        ));
    }

    #[test]
    fn rejects_wrong_cursor_order_value_type_during_scoped_encoding() {
        let schema = schema();
        let registry = registry_with_schema(&schema);
        let tenant_id = Uuid::new_v4();
        let query = query(&schema, tenant_id);
        let cursor = IndexCursor {
            tenant_id,
            schema: schema.reference.clone(),
            schema_fingerprint: schema.fingerprint().unwrap(),
            locale: query.scope.locale.clone(),
            order_values: vec![IndexValue::Integer(7)],
            entity_id: Uuid::new_v4(),
        };

        assert!(matches!(
            CursorCodec::encode_for_query(&cursor, &query, &registry),
            Err(CursorValidationError::OrderValueTypeMismatch {
                index: 0,
                expected: IndexValueType::Uuid,
                actual: Some(IndexValueType::Integer),
            })
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

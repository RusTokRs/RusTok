use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{
    FieldPath, FilterExpr, IndexValue, IndexValueType, LocaleKey, LocalizedEntityQuery,
    OrderDirection, OrderExpr, SchemaFingerprint, SchemaRef,
};

use super::{LocalizedEntityQueryValidationError, SchemaRegistry, SchemaRegistryError};

const LOCALIZED_SCOPED_CURSOR_VERSION: u8 = 3;
const CHECKSUM_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedIndexCursor {
    pub tenant_id: Uuid,
    pub schema: SchemaRef,
    pub schema_fingerprint: SchemaFingerprint,
    pub requested_locale: LocaleKey,
    pub fallback_locale: Option<LocaleKey>,
    pub order_values: Vec<IndexValue>,
    pub entity_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LocalizedScopedCursorEnvelope {
    query_fingerprint: [u8; 32],
    cursor: LocalizedIndexCursor,
}

#[derive(Serialize)]
struct LocalizedCursorQueryIdentity<'a> {
    mode: &'static str,
    tenant_id: Uuid,
    schema: &'a SchemaRef,
    requested_locale: Option<&'a LocaleKey>,
    fallback_locale: Option<&'a LocaleKey>,
    filter: &'a Option<FilterExpr>,
    any_locale_filter: &'a Option<FilterExpr>,
    localized_projection_fields: Vec<&'a FieldPath>,
    order_by: &'a [OrderExpr],
    identity_order_direction: OrderDirection,
}

#[derive(Debug, Error)]
pub enum LocalizedCursorCodecError {
    #[error("localized cursor encoding is invalid: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("localized cursor payload is too short")]
    TooShort,
    #[error("unsupported localized cursor version: {0}")]
    UnsupportedVersion(u8),
    #[error("localized cursor checksum is invalid")]
    InvalidChecksum,
    #[error("localized cursor payload serialization failed: {0}")]
    Postcard(#[from] postcard::Error),
}

#[derive(Debug, Error)]
pub enum LocalizedCursorValidationError {
    #[error(transparent)]
    Codec(#[from] LocalizedCursorCodecError),
    #[error(transparent)]
    Query(#[from] LocalizedEntityQueryValidationError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
    #[error("localized cursor tenant does not match query tenant")]
    TenantMismatch,
    #[error("localized cursor schema does not match query schema")]
    SchemaMismatch,
    #[error("localized cursor schema fingerprint is stale")]
    SchemaFingerprintMismatch,
    #[error("localized cursor requested locale does not match query")]
    RequestedLocaleMismatch,
    #[error("localized cursor fallback locale does not match canonical query fallback")]
    FallbackLocaleMismatch,
    #[error(
        "localized cursor query fingerprint does not match fold/filter/projection/order semantics"
    )]
    QueryFingerprintMismatch,
    #[error(
        "localized cursor contains {actual} order values but query defines {expected} order expressions"
    )]
    OrderArityMismatch { expected: usize, actual: usize },
    #[error("localized cursor order field contract is missing at position {index}")]
    OrderFieldContractMissing { index: usize },
    #[error(
        "localized cursor order value at position {index} has type {actual:?}, expected {expected:?}"
    )]
    OrderValueTypeMismatch {
        index: usize,
        expected: IndexValueType,
        actual: Option<IndexValueType>,
    },
    #[error("localized cursor entity id must not be nil")]
    NilEntityId,
}

pub struct LocalizedCursorCodec;

impl LocalizedCursorCodec {
    pub fn encode_for_query(
        cursor: &LocalizedIndexCursor,
        query: &LocalizedEntityQuery,
        registry: &SchemaRegistry,
    ) -> Result<String, LocalizedCursorValidationError> {
        validate_cursor(cursor, query, registry)?;
        let envelope = LocalizedScopedCursorEnvelope {
            query_fingerprint: query_fingerprint(query)?,
            cursor: cursor.clone(),
        };
        Ok(encode_payload(LOCALIZED_SCOPED_CURSOR_VERSION, &envelope)?)
    }

    pub fn decode_scoped_for_query(
        encoded: &str,
        query: &LocalizedEntityQuery,
        registry: &SchemaRegistry,
    ) -> Result<LocalizedIndexCursor, LocalizedCursorValidationError> {
        let envelope: LocalizedScopedCursorEnvelope =
            decode_payload(encoded, LOCALIZED_SCOPED_CURSOR_VERSION)?;
        if envelope.query_fingerprint != query_fingerprint(query)? {
            return Err(LocalizedCursorValidationError::QueryFingerprintMismatch);
        }
        validate_cursor(&envelope.cursor, query, registry)?;
        Ok(envelope.cursor)
    }
}

fn encode_payload<T: Serialize>(
    version: u8,
    value: &T,
) -> Result<String, LocalizedCursorCodecError> {
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
) -> Result<T, LocalizedCursorCodecError> {
    let envelope = URL_SAFE_NO_PAD.decode(encoded)?;
    if envelope.len() < 1 + CHECKSUM_LEN {
        return Err(LocalizedCursorCodecError::TooShort);
    }
    let version = envelope[0];
    if version != expected_version {
        return Err(LocalizedCursorCodecError::UnsupportedVersion(version));
    }
    let payload_end = envelope.len() - CHECKSUM_LEN;
    let payload = &envelope[1..payload_end];
    let checksum = &envelope[payload_end..];
    let expected = Sha256::digest(payload);
    if checksum != &expected[..CHECKSUM_LEN] {
        return Err(LocalizedCursorCodecError::InvalidChecksum);
    }
    Ok(postcard::from_bytes(payload)?)
}

fn query_fingerprint(query: &LocalizedEntityQuery) -> Result<[u8; 32], LocalizedCursorCodecError> {
    let mut localized_projection_fields =
        query.localized_projection_fields.iter().collect::<Vec<_>>();
    localized_projection_fields.sort();
    let identity = LocalizedCursorQueryIdentity {
        mode: "localized_entity_fold_v1",
        tenant_id: query.query.scope.tenant_id,
        schema: &query.query.schema,
        requested_locale: query.requested_locale(),
        fallback_locale: query.canonical_fallback_locale(),
        filter: &query.query.filter,
        any_locale_filter: &query.any_locale_filter,
        localized_projection_fields,
        order_by: &query.query.order_by,
        identity_order_direction: query.identity_order_direction,
    };
    let bytes = postcard::to_stdvec(&identity)?;
    let mut hasher = Sha256::new();
    hasher.update(b"rustok-index-localized-cursor-query-v1");
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

fn validate_cursor(
    cursor: &LocalizedIndexCursor,
    query: &LocalizedEntityQuery,
    registry: &SchemaRegistry,
) -> Result<(), LocalizedCursorValidationError> {
    registry.validate_localized_entity_query(query)?;
    let requested_locale = query
        .requested_locale()
        .expect("validated localized queries always carry a requested locale");
    if cursor.tenant_id != query.query.scope.tenant_id {
        return Err(LocalizedCursorValidationError::TenantMismatch);
    }
    if cursor.schema != query.query.schema {
        return Err(LocalizedCursorValidationError::SchemaMismatch);
    }
    if &cursor.requested_locale != requested_locale {
        return Err(LocalizedCursorValidationError::RequestedLocaleMismatch);
    }
    if cursor.fallback_locale.as_ref() != query.canonical_fallback_locale() {
        return Err(LocalizedCursorValidationError::FallbackLocaleMismatch);
    }
    if cursor.entity_id.is_nil() {
        return Err(LocalizedCursorValidationError::NilEntityId);
    }

    let registered = registry
        .get(&query.query.schema)
        .ok_or_else(|| SchemaRegistryError::SchemaNotFound(query.query.schema.clone()))?;
    if cursor.schema_fingerprint != registered.fingerprint {
        return Err(LocalizedCursorValidationError::SchemaFingerprintMismatch);
    }
    if cursor.order_values.len() != query.query.order_by.len() {
        return Err(LocalizedCursorValidationError::OrderArityMismatch {
            expected: query.query.order_by.len(),
            actual: cursor.order_values.len(),
        });
    }

    for (index, (order, value)) in query
        .query
        .order_by
        .iter()
        .zip(&cursor.order_values)
        .enumerate()
    {
        if matches!(value, IndexValue::Null) {
            continue;
        }
        let expected = resolve_order_value_type(registry, &query.query.schema, &order.field)
            .ok_or(LocalizedCursorValidationError::OrderFieldContractMissing { index })?;
        let actual = value.value_type();
        if actual != Some(expected) {
            return Err(LocalizedCursorValidationError::OrderValueTypeMismatch {
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
    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexQuery, IndexQueryScope,
        IndexSchema, LocaleMode, ModuleName, Pagination, SchemaVersion,
    };
    use crate::{CursorCodec, IndexCursor};

    fn schema() -> IndexSchema {
        IndexSchema {
            reference: SchemaRef {
                module: ModuleName::new("rustok-product").unwrap(),
                entity: EntityName::new("product").unwrap(),
                version: SchemaVersion::INITIAL,
            },
            locale_mode: LocaleMode::Required,
            fields: vec![
                IndexField {
                    name: FieldName::new("id").unwrap(),
                    value_type: IndexValueType::Uuid,
                    cardinality: FieldCardinality::One,
                    nullable: false,
                    selectable: true,
                    filterable: true,
                    sortable: true,
                },
                IndexField {
                    name: FieldName::new("title").unwrap(),
                    value_type: IndexValueType::String,
                    cardinality: FieldCardinality::One,
                    nullable: false,
                    selectable: true,
                    filterable: true,
                    sortable: false,
                },
            ],
            links: Vec::new(),
        }
    }

    fn query(schema: &IndexSchema, fallback: Option<&str>) -> LocalizedEntityQuery {
        LocalizedEntityQuery::new(
            IndexQuery {
                scope: IndexQueryScope {
                    tenant_id: Uuid::new_v4(),
                    locale: Some(LocaleKey::new("en-US").unwrap()),
                },
                schema: schema.reference.clone(),
                fields: vec![
                    FieldPath::new(FieldName::new("id").unwrap()),
                    FieldPath::new(FieldName::new("title").unwrap()),
                ],
                filter: None,
                order_by: vec![OrderExpr {
                    field: FieldPath::new(FieldName::new("id").unwrap()),
                    direction: OrderDirection::Asc,
                }],
                pagination: Pagination::Cursor {
                    first: 20,
                    after: None,
                },
                include_exact_count: true,
            },
            fallback.map(|value| LocaleKey::new(value).unwrap()),
            None,
        )
        .with_localized_projection_fields([FieldPath::new(FieldName::new("title").unwrap())])
    }

    fn cursor(query: &LocalizedEntityQuery, schema: &IndexSchema) -> LocalizedIndexCursor {
        LocalizedIndexCursor {
            tenant_id: query.query.scope.tenant_id,
            schema: schema.reference.clone(),
            schema_fingerprint: schema.fingerprint().unwrap(),
            requested_locale: LocaleKey::new("en-US").unwrap(),
            fallback_locale: query.canonical_fallback_locale().cloned(),
            order_values: vec![IndexValue::Uuid(Uuid::new_v4())],
            entity_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn localized_cursor_is_bound_to_fallback_and_has_separate_wire_version() {
        let schema = schema();
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let original = query(&schema, Some("en"));
        let cursor = cursor(&original, &schema);
        let encoded =
            LocalizedCursorCodec::encode_for_query(&cursor, &original, &registry).unwrap();
        assert_eq!(
            LocalizedCursorCodec::decode_scoped_for_query(&encoded, &original, &registry).unwrap(),
            cursor
        );

        let standard = IndexCursor {
            tenant_id: original.query.scope.tenant_id,
            schema: schema.reference.clone(),
            schema_fingerprint: schema.fingerprint().unwrap(),
            locale: original.query.scope.locale.clone(),
            order_values: vec![IndexValue::Uuid(Uuid::new_v4())],
            entity_id: Uuid::new_v4(),
        };
        let standard_encoded =
            CursorCodec::encode_for_query(&standard, &original.query, &registry).unwrap();
        assert!(matches!(
            LocalizedCursorCodec::decode_scoped_for_query(&standard_encoded, &original, &registry),
            Err(LocalizedCursorValidationError::Codec(
                LocalizedCursorCodecError::UnsupportedVersion(2)
            ))
        ));
    }

    #[test]
    fn localized_cursor_cannot_cross_fallback_projection_or_identity_order_semantics() {
        let schema = schema();
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone()).unwrap();
        let original = query(&schema, Some("en"));
        let cursor = cursor(&original, &schema);
        let encoded =
            LocalizedCursorCodec::encode_for_query(&cursor, &original, &registry).unwrap();

        let mut changed_fallback = original.clone();
        changed_fallback.fallback_locale = Some(LocaleKey::new("fr").unwrap());
        assert!(matches!(
            LocalizedCursorCodec::decode_scoped_for_query(&encoded, &changed_fallback, &registry),
            Err(LocalizedCursorValidationError::QueryFingerprintMismatch)
        ));

        let mut changed_projection = original.clone();
        changed_projection.localized_projection_fields.clear();
        assert!(matches!(
            LocalizedCursorCodec::decode_scoped_for_query(&encoded, &changed_projection, &registry),
            Err(LocalizedCursorValidationError::QueryFingerprintMismatch)
        ));

        let changed_identity_order = original
            .clone()
            .with_identity_order_direction(OrderDirection::Desc);
        assert!(matches!(
            LocalizedCursorCodec::decode_scoped_for_query(
                &encoded,
                &changed_identity_order,
                &registry
            ),
            Err(LocalizedCursorValidationError::QueryFingerprintMismatch)
        ));
    }
}

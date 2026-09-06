use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use super::{CompiledPostgresCell, CompiledPostgresRow, PostgresQueryDecodeError, SchemaRegistry};
use crate::domain::{
    EntityName, FieldCardinality, FieldName, FieldPath, IndexField, IndexLink, IndexQuery,
    IndexQueryScope, IndexSchema, IndexValue, IndexValueType, LinkCardinality, LinkName, LocaleKey,
    LocaleMode, ModuleName, Pagination, SchemaRef, SchemaVersion,
};

fn schema_ref(entity: &str) -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new(entity).unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

fn field(name: &str, value_type: IndexValueType, nullable: bool) -> IndexField {
    IndexField {
        name: FieldName::new(name).unwrap(),
        value_type,
        cardinality: FieldCardinality::One,
        nullable,
        selectable: true,
        filterable: true,
        sortable: true,
    }
}

fn registry() -> SchemaRegistry {
    let variant = IndexSchema {
        reference: schema_ref("variant"),
        locale_mode: LocaleMode::Required,
        fields: vec![
            field("id", IndexValueType::Uuid, false),
            field("score", IndexValueType::Integer, true),
        ],
        links: Vec::new(),
    };
    let product = IndexSchema {
        reference: schema_ref("product"),
        locale_mode: LocaleMode::Required,
        fields: vec![field("id", IndexValueType::Uuid, false)],
        links: vec![IndexLink {
            name: LinkName::new("variants").unwrap(),
            source_fields: vec![FieldName::new("id").unwrap()],
            target_schema: variant.reference.clone(),
            target_fields: vec![FieldName::new("id").unwrap()],
            cardinality: LinkCardinality::Many,
        }],
    };
    let mut registry = SchemaRegistry::new();
    registry.register_batch([product, variant]).unwrap();
    registry
}

fn query() -> IndexQuery {
    IndexQuery {
        scope: IndexQueryScope {
            tenant_id: Uuid::from_u128(1),
            locale: Some(LocaleKey::new("en-US").unwrap()),
        },
        schema: schema_ref("product"),
        fields: vec![
            FieldPath::new(FieldName::new("id").unwrap()),
            FieldPath::linked(
                [LinkName::new("variants").unwrap()],
                FieldName::new("id").unwrap(),
            ),
            FieldPath::linked(
                [LinkName::new("variants").unwrap()],
                FieldName::new("score").unwrap(),
            ),
        ],
        filter: None,
        order_by: Vec::new(),
        pagination: Pagination::Cursor {
            first: 2,
            after: None,
        },
        include_exact_count: false,
    }
}

fn tagged(value: IndexValue) -> JsonValue {
    serde_json::to_value(value).unwrap()
}

fn row(root_id: Uuid, nested: JsonValue) -> CompiledPostgresRow {
    CompiledPostgresRow::from_values([
        (
            "__t0_entity_id".to_owned(),
            CompiledPostgresCell::Uuid(root_id),
        ),
        (
            "f0".to_owned(),
            CompiledPostgresCell::Json(tagged(IndexValue::Uuid(root_id))),
        ),
        ("__many_0".to_owned(), CompiledPostgresCell::Json(nested)),
    ])
}

#[test]
fn decodes_aligned_nested_identity_and_value_arrays() {
    let registry = registry();
    let query = query();
    let page_query = registry.compile_postgres_page_query(&query).unwrap();
    let root_id = Uuid::from_u128(100);
    let first = Uuid::from_u128(10);
    let second = Uuid::from_u128(20);
    let nested = json!([
        {
            "entity_ids": [first],
            "values": [tagged(IndexValue::Uuid(first)), tagged(IndexValue::Integer(10))]
        },
        {
            "entity_ids": [second],
            "values": [tagged(IndexValue::Uuid(second)), tagged(IndexValue::Integer(20))]
        }
    ]);

    let page = registry
        .decode_postgres_query_page(&query, &page_query, vec![row(root_id, nested)], None)
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].fields.len(), 1);
    assert_eq!(page.items[0].nested_relations.len(), 1);
    let projection = &page.items[0].nested_relations[0];
    assert_eq!(projection.path, vec![LinkName::new("variants").unwrap()]);
    assert_eq!(projection.items.len(), 2);
    assert_eq!(projection.items[0].relations[0].entity_id, Some(first));
    assert_eq!(projection.items[0].fields[1].value, IndexValue::Integer(10));
    assert_eq!(projection.items[1].relations[0].entity_id, Some(second));
}

#[test]
fn rejects_nested_identity_and_field_arity_drift() {
    let registry = registry();
    let query = query();
    let page_query = registry.compile_postgres_page_query(&query).unwrap();
    let root_id = Uuid::from_u128(100);
    let variant = Uuid::from_u128(10);

    let identity_error = row(
        root_id,
        json!([{
            "entity_ids": [],
            "values": [tagged(IndexValue::Uuid(variant)), tagged(IndexValue::Integer(10))]
        }]),
    );
    assert!(matches!(
        registry.decode_postgres_query_page(&query, &page_query, vec![identity_error], None),
        Err(PostgresQueryDecodeError::NestedIdentityArity {
            expected: 1,
            actual: 0,
            ..
        })
    ));

    let field_error = row(
        root_id,
        json!([{
            "entity_ids": [variant],
            "values": [tagged(IndexValue::Uuid(variant))]
        }]),
    );
    assert!(matches!(
        registry.decode_postgres_query_page(&query, &page_query, vec![field_error], None),
        Err(PostgresQueryDecodeError::NestedFieldArity {
            expected: 2,
            actual: 1,
            ..
        })
    ));
}

#[test]
fn rejects_nil_and_duplicate_nested_identity_chains() {
    let registry = registry();
    let query = query();
    let page_query = registry.compile_postgres_page_query(&query).unwrap();
    let root_id = Uuid::from_u128(100);
    let variant = Uuid::from_u128(10);

    let nil_identity = row(
        root_id,
        json!([{
            "entity_ids": [Uuid::nil()],
            "values": [tagged(IndexValue::Uuid(variant)), tagged(IndexValue::Integer(10))]
        }]),
    );
    assert!(matches!(
        registry.decode_postgres_query_page(&query, &page_query, vec![nil_identity], None),
        Err(PostgresQueryDecodeError::NilNestedIdentity { .. })
    ));

    let duplicate = json!([
        {
            "entity_ids": [variant],
            "values": [tagged(IndexValue::Uuid(variant)), tagged(IndexValue::Integer(10))]
        },
        {
            "entity_ids": [variant],
            "values": [tagged(IndexValue::Uuid(variant)), tagged(IndexValue::Integer(20))]
        }
    ]);
    assert!(matches!(
        registry.decode_postgres_query_page(
            &query,
            &page_query,
            vec![row(root_id, duplicate)],
            None
        ),
        Err(PostgresQueryDecodeError::DuplicateNestedIdentity { .. })
    ));
}

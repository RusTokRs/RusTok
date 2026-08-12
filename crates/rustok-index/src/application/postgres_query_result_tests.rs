use uuid::Uuid;

use super::{
    CompiledPostgresCell, CompiledPostgresPageQuery, CompiledPostgresRow, CompiledQueryColumn,
    CursorCodec, IndexCursor, PostgresBindValue, PostgresQueryDecodeError, SchemaRegistry,
};
use crate::domain::{
    EntityName, FieldCardinality, FieldName, FieldPath, IndexField, IndexLink, IndexQuery,
    IndexQueryScope, IndexSchema, IndexValue, IndexValueType, LinkCardinality, LinkName, LocaleKey,
    LocaleMode, ModuleName, OrderDirection, OrderExpr, Pagination, SchemaRef, SchemaVersion,
};

fn schema_ref(entity: &str) -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new(entity).unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

fn field(name: &str, value_type: IndexValueType, nullable: bool, sortable: bool) -> IndexField {
    IndexField {
        name: FieldName::new(name).unwrap(),
        value_type,
        cardinality: FieldCardinality::One,
        nullable,
        selectable: true,
        filterable: true,
        sortable,
    }
}

fn registry() -> SchemaRegistry {
    let channel = IndexSchema {
        reference: schema_ref("sales_channel"),
        locale_mode: LocaleMode::None,
        fields: vec![field("id", IndexValueType::Uuid, false, true)],
        links: Vec::new(),
    };
    let product = IndexSchema {
        reference: schema_ref("product"),
        locale_mode: LocaleMode::Required,
        fields: vec![
            field("id", IndexValueType::Uuid, false, true),
            field("score", IndexValueType::Integer, true, true),
            field("sales_channel_id", IndexValueType::Uuid, false, false),
        ],
        links: vec![IndexLink {
            name: LinkName::new("sales_channel").unwrap(),
            source_fields: vec![FieldName::new("sales_channel_id").unwrap()],
            target_schema: channel.reference.clone(),
            target_fields: vec![FieldName::new("id").unwrap()],
            cardinality: LinkCardinality::One,
        }],
    };

    let mut registry = SchemaRegistry::new();
    registry.register_batch([product, channel]).unwrap();
    registry
}

fn path(name: &str) -> FieldPath {
    FieldPath::new(FieldName::new(name).unwrap())
}

fn linked_id() -> FieldPath {
    FieldPath::linked(
        [LinkName::new("sales_channel").unwrap()],
        FieldName::new("id").unwrap(),
    )
}

fn query(tenant_id: Uuid) -> IndexQuery {
    IndexQuery {
        scope: IndexQueryScope {
            tenant_id,
            locale: Some(LocaleKey::new("en-US").unwrap()),
        },
        schema: schema_ref("product"),
        fields: vec![path("id"), path("score"), linked_id()],
        filter: None,
        order_by: vec![OrderExpr {
            field: path("score"),
            direction: OrderDirection::Asc,
        }],
        pagination: Pagination::Cursor {
            first: 2,
            after: None,
        },
        include_exact_count: true,
    }
}

fn compiled_row(
    page_query: &CompiledPostgresPageQuery,
    root_entity_id: Uuid,
    linked_entity_id: Option<Uuid>,
    projected: &[IndexValue],
    order_values: &[IndexValue],
) -> CompiledPostgresRow {
    let mut row = CompiledPostgresRow::new();
    let mut projected = projected.iter();
    let mut order_values = order_values.iter();
    for column in &page_query.compiled().columns {
        match column {
            CompiledQueryColumn::EntityId {
                output_alias,
                relation_alias,
            } => {
                let value = if relation_alias.as_str() == "t0" {
                    CompiledPostgresCell::Uuid(root_entity_id)
                } else {
                    linked_entity_id.map_or(CompiledPostgresCell::Null, CompiledPostgresCell::Uuid)
                };
                let _ = row.insert(output_alias.clone(), value);
            }
            CompiledQueryColumn::Field { output_alias, .. } => {
                let value = projected.next().expect("projection fixture arity");
                let _ = row.insert(
                    output_alias.clone(),
                    CompiledPostgresCell::Json(serde_json::to_value(value).unwrap()),
                );
            }
            CompiledQueryColumn::OrderValue { output_alias, .. } => {
                let value = order_values.next().expect("order fixture arity");
                let _ = row.insert(
                    output_alias.clone(),
                    CompiledPostgresCell::Json(serde_json::to_value(value).unwrap()),
                );
            }
        }
    }
    assert!(projected.next().is_none());
    assert!(order_values.next().is_none());
    row
}

fn exact_count_row(value: i64) -> CompiledPostgresRow {
    CompiledPostgresRow::from_values([(
        "__exact_count".to_owned(),
        CompiledPostgresCell::Integer(value),
    )])
}

#[test]
fn page_compilation_adds_exactly_one_lookahead_row() {
    let registry = registry();
    let query = query(Uuid::new_v4());
    let raw = registry.compile_postgres_query(&query).unwrap();
    let page = registry.compile_postgres_page_query(&query).unwrap();

    assert_eq!(raw.binds.last(), Some(&PostgresBindValue::Integer(2)));
    assert_eq!(
        page.compiled().binds.last(),
        Some(&PostgresBindValue::Integer(3))
    );
    assert_eq!(page.requested_page_size(), 2);
    assert_eq!(page.compiled().sql, raw.sql);
}

#[test]
fn offset_page_compilation_preserves_offset_and_adds_lookahead() {
    let registry = registry();
    let mut query = query(Uuid::new_v4());
    query.pagination = Pagination::Offset {
        limit: 2,
        offset: 5,
    };
    let page = registry.compile_postgres_page_query(&query).unwrap();
    let binds = &page.compiled().binds;

    assert_eq!(
        binds.get(binds.len() - 2),
        Some(&PostgresBindValue::Integer(3))
    );
    assert_eq!(binds.last(), Some(&PostgresBindValue::Integer(5)));
}

#[test]
fn decodes_projection_relations_exact_count_and_next_cursor() {
    let registry = registry();
    let tenant_id = Uuid::new_v4();
    let query = query(tenant_id);
    let page_query = registry.compile_postgres_page_query(&query).unwrap();
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    let third_id = Uuid::new_v4();
    let first_channel = Uuid::new_v4();
    let third_channel = Uuid::new_v4();
    let rows = vec![
        compiled_row(
            &page_query,
            first_id,
            Some(first_channel),
            &[
                IndexValue::Uuid(first_id),
                IndexValue::Integer(10),
                IndexValue::Uuid(first_channel),
            ],
            &[IndexValue::Integer(10)],
        ),
        compiled_row(
            &page_query,
            second_id,
            None,
            &[
                IndexValue::Uuid(second_id),
                IndexValue::Integer(20),
                IndexValue::Null,
            ],
            &[IndexValue::Integer(20)],
        ),
        compiled_row(
            &page_query,
            third_id,
            Some(third_channel),
            &[
                IndexValue::Uuid(third_id),
                IndexValue::Integer(30),
                IndexValue::Uuid(third_channel),
            ],
            &[IndexValue::Integer(30)],
        ),
    ];

    let page = registry
        .decode_postgres_query_page(&query, &page_query, rows, Some(exact_count_row(3)))
        .unwrap();

    assert_eq!(page.items.len(), 2);
    assert_eq!(page.exact_count, Some(3));
    assert!(page.has_more);
    assert_eq!(page.items[0].entity_id, first_id);
    assert_eq!(page.items[0].relations[0].entity_id, Some(first_channel));
    assert_eq!(page.items[1].relations[0].entity_id, None);
    assert_eq!(page.items[1].fields[2].value, IndexValue::Null);

    let cursor = CursorCodec::decode_scoped_for_query(
        page.next_cursor.as_deref().expect("lookahead cursor"),
        &query,
        &registry,
    )
    .unwrap();
    assert_eq!(cursor.entity_id, second_id);
    assert_eq!(cursor.order_values, vec![IndexValue::Integer(20)]);
}

#[test]
fn omits_next_cursor_when_no_lookahead_row_exists() {
    let registry = registry();
    let query = query(Uuid::new_v4());
    let page_query = registry.compile_postgres_page_query(&query).unwrap();
    let entity_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let row = compiled_row(
        &page_query,
        entity_id,
        Some(channel_id),
        &[
            IndexValue::Uuid(entity_id),
            IndexValue::Integer(10),
            IndexValue::Uuid(channel_id),
        ],
        &[IndexValue::Integer(10)],
    );

    let page = registry
        .decode_postgres_query_page(&query, &page_query, vec![row], Some(exact_count_row(1)))
        .unwrap();

    assert!(!page.has_more);
    assert!(page.next_cursor.is_none());
}

#[test]
fn rejects_page_compiled_for_different_query_semantics() {
    let registry = registry();
    let tenant_id = Uuid::new_v4();
    let query = query(tenant_id);
    let mut changed = query.clone();
    changed.order_by[0].direction = OrderDirection::Desc;
    let page_query = registry.compile_postgres_page_query(&changed).unwrap();

    assert!(matches!(
        registry.decode_postgres_query_page(
            &query,
            &page_query,
            Vec::new(),
            Some(exact_count_row(0)),
        ),
        Err(PostgresQueryDecodeError::PlanFingerprintMismatch { .. })
    ));
}

#[test]
fn rejects_invalid_tagged_field_contract() {
    let registry = registry();
    let query = query(Uuid::new_v4());
    let page_query = registry.compile_postgres_page_query(&query).unwrap();
    let entity_id = Uuid::new_v4();
    let channel_id = Uuid::new_v4();
    let row = compiled_row(
        &page_query,
        entity_id,
        Some(channel_id),
        &[
            IndexValue::Uuid(entity_id),
            IndexValue::String("wrong-type".to_owned()),
            IndexValue::Uuid(channel_id),
        ],
        &[IndexValue::Integer(10)],
    );

    assert!(matches!(
        registry.decode_postgres_query_page(
            &query,
            &page_query,
            vec![row],
            Some(exact_count_row(1)),
        ),
        Err(PostgresQueryDecodeError::InvalidFieldValue { path: field_path })
            if field_path == path("score")
    ));
}

#[test]
fn scoped_cursor_fixture_remains_query_bound() {
    let registry = registry();
    let query = query(Uuid::new_v4());
    let cursor = IndexCursor {
        tenant_id: query.scope.tenant_id,
        schema: query.schema.clone(),
        schema_fingerprint: registry.get(&query.schema).unwrap().fingerprint,
        locale: query.scope.locale.clone(),
        order_values: vec![IndexValue::Integer(10)],
        entity_id: Uuid::new_v4(),
    };

    assert!(CursorCodec::encode_for_query(&cursor, &query, &registry).is_ok());
}

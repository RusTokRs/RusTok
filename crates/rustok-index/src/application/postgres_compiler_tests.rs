use uuid::Uuid;

use super::{
    CompiledQueryColumn, CursorCodec, CursorValidationError, IndexCursor, PostgresBindValue,
    PostgresQueryBuildError, PostgresQueryCompileError, SchemaRegistry,
};
use crate::domain::{
    EntityName, FieldCardinality, FieldName, FieldPath, FilterExpr, IndexField, IndexLink,
    IndexQuery, IndexQueryScope, IndexSchema, IndexValue, IndexValueType, LinkCardinality,
    LinkName, LocaleKey, LocaleMode, ModuleName, OrderDirection, OrderExpr, Pagination, SchemaRef,
    SchemaVersion,
};

fn schema_ref(entity: &str) -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new(entity).unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

fn field(
    name: &str,
    value_type: IndexValueType,
    cardinality: FieldCardinality,
    nullable: bool,
    sortable: bool,
) -> IndexField {
    IndexField {
        name: FieldName::new(name).unwrap(),
        value_type,
        cardinality,
        nullable,
        selectable: true,
        filterable: true,
        sortable,
    }
}

fn registry(link_cardinality: LinkCardinality) -> SchemaRegistry {
    let channel = IndexSchema {
        reference: schema_ref("sales_channel"),
        locale_mode: LocaleMode::None,
        fields: vec![field(
            "id",
            IndexValueType::Uuid,
            FieldCardinality::One,
            false,
            true,
        )],
        links: Vec::new(),
    };
    let product = IndexSchema {
        reference: schema_ref("product"),
        locale_mode: LocaleMode::Required,
        fields: vec![
            field(
                "id",
                IndexValueType::Uuid,
                FieldCardinality::One,
                false,
                true,
            ),
            field(
                "score",
                IndexValueType::Integer,
                FieldCardinality::One,
                true,
                true,
            ),
            field(
                "title",
                IndexValueType::String,
                FieldCardinality::One,
                true,
                true,
            ),
            field(
                "tags",
                IndexValueType::String,
                FieldCardinality::Many,
                false,
                false,
            ),
            field(
                "sales_channel_id",
                IndexValueType::Uuid,
                FieldCardinality::One,
                false,
                false,
            ),
        ],
        links: vec![IndexLink {
            name: LinkName::new("sales_channel").unwrap(),
            source_fields: vec![FieldName::new("sales_channel_id").unwrap()],
            target_schema: channel.reference.clone(),
            target_fields: vec![FieldName::new("id").unwrap()],
            cardinality: link_cardinality,
        }],
    };

    let mut registry = SchemaRegistry::new();
    registry.register_batch([product, channel]).unwrap();
    registry
}

fn many_registry() -> SchemaRegistry {
    let channel = IndexSchema {
        reference: schema_ref("sales_channel"),
        locale_mode: LocaleMode::None,
        fields: vec![field(
            "id",
            IndexValueType::Uuid,
            FieldCardinality::One,
            false,
            true,
        )],
        links: Vec::new(),
    };
    let variant = IndexSchema {
        reference: schema_ref("variant"),
        locale_mode: LocaleMode::Required,
        fields: vec![
            field(
                "id",
                IndexValueType::Uuid,
                FieldCardinality::One,
                false,
                true,
            ),
            field(
                "score",
                IndexValueType::Integer,
                FieldCardinality::One,
                true,
                true,
            ),
            field(
                "title",
                IndexValueType::String,
                FieldCardinality::One,
                true,
                true,
            ),
            field(
                "tags",
                IndexValueType::String,
                FieldCardinality::Many,
                false,
                false,
            ),
            field(
                "sales_channel_id",
                IndexValueType::Uuid,
                FieldCardinality::One,
                false,
                false,
            ),
        ],
        links: vec![IndexLink {
            name: LinkName::new("sales_channel").unwrap(),
            source_fields: vec![FieldName::new("sales_channel_id").unwrap()],
            target_schema: channel.reference.clone(),
            target_fields: vec![FieldName::new("id").unwrap()],
            cardinality: LinkCardinality::One,
        }],
    };
    let product = IndexSchema {
        reference: schema_ref("product"),
        locale_mode: LocaleMode::Required,
        fields: vec![field(
            "id",
            IndexValueType::Uuid,
            FieldCardinality::One,
            false,
            true,
        )],
        links: vec![IndexLink {
            name: LinkName::new("variants").unwrap(),
            source_fields: vec![FieldName::new("id").unwrap()],
            target_schema: variant.reference.clone(),
            target_fields: vec![FieldName::new("id").unwrap()],
            cardinality: LinkCardinality::Many,
        }],
    };

    let mut registry = SchemaRegistry::new();
    registry
        .register_batch([product, variant, channel])
        .unwrap();
    registry
}

fn path(name: &str) -> FieldPath {
    FieldPath::new(FieldName::new(name).unwrap())
}

fn linked_path(links: &[&str], field: &str) -> FieldPath {
    FieldPath::linked(
        links
            .iter()
            .map(|link| LinkName::new(*link).unwrap())
            .collect::<Vec<_>>(),
        FieldName::new(field).unwrap(),
    )
}

fn root_query(tenant_id: Uuid) -> IndexQuery {
    IndexQuery {
        scope: IndexQueryScope {
            tenant_id,
            locale: Some(LocaleKey::new("en-US").unwrap()),
        },
        schema: schema_ref("product"),
        fields: vec![path("id")],
        filter: None,
        order_by: Vec::new(),
        pagination: Pagination::Cursor {
            first: 20,
            after: None,
        },
        include_exact_count: false,
    }
}

#[test]
fn compiles_root_projection_with_bound_scope_and_limit() {
    let registry = registry(LinkCardinality::One);
    let tenant_id = Uuid::new_v4();
    let plan = registry.plan_query(&root_query(tenant_id)).unwrap();
    let compiled = plan.compile_postgres().unwrap();

    assert!(compiled.sql.starts_with(
        "SELECT \"t0\".entity_id AS \"__t0_entity_id\", jsonb_extract_path(\"t0\".payload, $6::text) AS \"f0\" FROM index_entities AS \"t0\""
    ));
    assert!(compiled.sql.contains("\"t0\".tenant_id = $1"));
    assert!(compiled.sql.contains("\"t0\".locale_key = $5"));
    assert!(compiled.sql.ends_with("ORDER BY \"t0\".entity_id ASC LIMIT $7"));
    assert!(!compiled.sql.contains(&tenant_id.to_string()));
    assert_eq!(
        compiled.binds,
        vec![
            PostgresBindValue::Uuid(tenant_id),
            PostgresBindValue::Text("rustok-product".to_owned()),
            PostgresBindValue::Text("product".to_owned()),
            PostgresBindValue::Integer(1),
            PostgresBindValue::Text("en-US".to_owned()),
            PostgresBindValue::Text("id".to_owned()),
            PostgresBindValue::Integer(20),
        ]
    );
    assert_eq!(compiled.columns.len(), 2);
    assert!(matches!(
        &compiled.columns[0],
        CompiledQueryColumn::EntityId { relation_alias, .. } if relation_alias == "t0"
    ));
    assert!(compiled.exact_count.is_none());
    assert_eq!(compiled.plan_fingerprint, plan.fingerprint().unwrap());
}

#[test]
fn compiles_one_link_projection_without_interpolating_contract_values() {
    let registry = registry(LinkCardinality::One);
    let tenant_id = Uuid::new_v4();
    let mut query = root_query(tenant_id);
    query.fields.insert(
        0,
        FieldPath::linked(
            [LinkName::new("sales_channel").unwrap()],
            FieldName::new("id").unwrap(),
        ),
    );

    let compiled = registry.compile_postgres_query(&query).unwrap();

    assert!(compiled.sql.contains("LEFT JOIN index_links AS \"l1\""));
    assert!(compiled
        .sql
        .contains("\"l1\".source_version = \"t0\".source_version"));
    assert!(compiled.sql.contains("LEFT JOIN index_entities AS \"t1\""));
    assert!(compiled.sql.contains("\"t1\".is_deleted = FALSE"));
    assert!(!compiled.sql.contains("sales_channel"));
    assert_eq!(
        &compiled.binds[5..9],
        &[
            PostgresBindValue::Text("sales_channel".to_owned()),
            PostgresBindValue::Text("rustok-product".to_owned()),
            PostgresBindValue::Text("sales_channel".to_owned()),
            PostgresBindValue::Integer(1),
        ]
    );
    assert_eq!(compiled.columns.len(), 4);
    assert!(matches!(
        &compiled.columns[1],
        CompiledQueryColumn::EntityId { relation_alias, .. } if relation_alias == "t1"
    ));
}

#[test]
fn compiles_typed_filters_order_exact_count_and_bounded_offset() {
    let registry = registry(LinkCardinality::One);
    let tenant_id = Uuid::new_v4();
    let expected_id = Uuid::new_v4();
    let mut query = root_query(tenant_id);
    query.fields.push(path("score"));
    query.filter = Some(FilterExpr::And(vec![
        FilterExpr::Eq(path("id"), IndexValue::Uuid(expected_id)),
        FilterExpr::Gt(path("score"), IndexValue::Integer(10)),
        FilterExpr::Contains(path("tags"), IndexValue::String("featured".to_owned())),
        FilterExpr::IsNull(path("title"), false),
        FilterExpr::Not(Box::new(FilterExpr::Ne(
            path("title"),
            IndexValue::String("blocked".to_owned()),
        ))),
    ]));
    query.order_by = vec![
        OrderExpr {
            field: path("score"),
            direction: OrderDirection::Desc,
        },
        OrderExpr {
            field: path("title"),
            direction: OrderDirection::Asc,
        },
    ];
    query.pagination = Pagination::Offset {
        limit: 25,
        offset: 50,
    };
    query.include_exact_count = true;

    let compiled = registry.compile_postgres_query(&query).unwrap();

    assert!(compiled.sql.contains("::uuid"));
    assert!(compiled.sql.contains("::bigint"));
    assert!(compiled.sql.contains("COLLATE \"C\""));
    assert!(compiled.sql.contains(" @> "));
    assert!(compiled.sql.contains("COALESCE("));
    assert!(compiled.sql.contains(" DESC NULLS FIRST"));
    assert!(compiled.sql.contains(" ASC NULLS LAST"));
    assert!(compiled.sql.contains("\"t0\".entity_id ASC"));
    assert!(compiled.sql.contains(" LIMIT $"));
    assert!(compiled.sql.contains(" OFFSET $"));
    assert!(compiled
        .binds
        .contains(&PostgresBindValue::Uuid(expected_id)));
    assert!(compiled
        .binds
        .contains(&PostgresBindValue::Integer(10)));
    assert!(compiled
        .binds
        .iter()
        .any(|value| matches!(value, PostgresBindValue::Json(_))));
    assert_eq!(
        compiled
            .columns
            .iter()
            .filter(|column| matches!(column, CompiledQueryColumn::OrderValue { .. }))
            .count(),
        2
    );

    let count = compiled.exact_count.expect("exact count must be compiled");
    assert!(count
        .sql
        .starts_with("SELECT COUNT(*)::bigint AS \"__exact_count\" FROM index_entities"));
    assert!(count.sql.contains("COALESCE("));
    assert!(!count.sql.contains("ORDER BY"));
    assert!(!count.sql.contains("LIMIT"));
    assert!(!count.sql.contains("OFFSET"));
    assert!(count.binds.contains(&PostgresBindValue::Uuid(expected_id)));
}

#[test]
fn compiles_validated_lexicographic_keyset_with_entity_tie_breaker() {
    let registry = registry(LinkCardinality::One);
    let tenant_id = Uuid::new_v4();
    let cursor_entity_id = Uuid::new_v4();
    let mut query = root_query(tenant_id);
    query.order_by = vec![
        OrderExpr {
            field: path("score"),
            direction: OrderDirection::Asc,
        },
        OrderExpr {
            field: path("title"),
            direction: OrderDirection::Desc,
        },
    ];
    let fingerprint = registry.get(&query.schema).unwrap().fingerprint;
    let cursor = IndexCursor {
        tenant_id,
        schema: query.schema.clone(),
        schema_fingerprint: fingerprint,
        locale: query.scope.locale.clone(),
        order_values: vec![IndexValue::Integer(42), IndexValue::Null],
        entity_id: cursor_entity_id,
    };
    let encoded = CursorCodec::encode_for_query(&cursor, &query, &registry).unwrap();
    query.pagination = Pagination::Cursor {
        first: 20,
        after: Some(encoded),
    };

    let compiled = registry.compile_postgres_query(&query).unwrap();

    assert!(compiled.sql.contains(" OR "));
    assert!(compiled.sql.contains("FALSE"));
    assert!(compiled.sql.contains(".entity_id > $"));
    assert!(compiled.sql.contains(" ASC NULLS LAST"));
    assert!(compiled.sql.contains(" DESC NULLS FIRST"));
    assert!(compiled
        .binds
        .contains(&PostgresBindValue::Integer(42)));
    assert!(compiled
        .binds
        .contains(&PostgresBindValue::Uuid(cursor_entity_id)));
    assert_eq!(
        compiled
            .columns
            .iter()
            .filter(|column| matches!(column, CompiledQueryColumn::OrderValue { .. }))
            .count(),
        2
    );

    let plan = registry.plan_query(&query).unwrap();
    assert!(matches!(
        plan.compile_postgres(),
        Err(PostgresQueryCompileError::CursorContextRequired)
    ));
}

#[test]
fn rejects_cursor_reuse_across_query_semantics_before_sql_compilation() {
    let registry = registry(LinkCardinality::One);
    let tenant_id = Uuid::new_v4();
    let mut original = root_query(tenant_id);
    original.order_by = vec![OrderExpr {
        field: path("score"),
        direction: OrderDirection::Asc,
    }];
    let cursor = IndexCursor {
        tenant_id,
        schema: original.schema.clone(),
        schema_fingerprint: registry.get(&original.schema).unwrap().fingerprint,
        locale: original.scope.locale.clone(),
        order_values: vec![IndexValue::Integer(7)],
        entity_id: Uuid::new_v4(),
    };
    let encoded = CursorCodec::encode_for_query(&cursor, &original, &registry).unwrap();
    let mut changed = original.clone();
    changed.order_by[0].direction = OrderDirection::Desc;
    changed.pagination = Pagination::Cursor {
        first: 20,
        after: Some(encoded),
    };

    assert!(matches!(
        registry.compile_postgres_query(&changed),
        Err(PostgresQueryBuildError::Cursor(
            CursorValidationError::QueryFingerprintMismatch
        ))
    ));
}

#[test]
fn compiles_nested_many_link_filter_as_correlated_exists_without_outer_join() {
    let registry = many_registry();
    let tenant_id = Uuid::new_v4();
    let expected_channel = Uuid::new_v4();
    let mut query = root_query(tenant_id);
    query.filter = Some(FilterExpr::Eq(
        linked_path(&["variants", "sales_channel"], "id"),
        IndexValue::Uuid(expected_channel),
    ));
    query.include_exact_count = true;

    let compiled = registry.compile_postgres_query(&query).unwrap();

    assert!(compiled
        .sql
        .contains("EXISTS (SELECT 1 FROM index_links AS \"mx_l1\""));
    assert!(compiled
        .sql
        .contains("EXISTS (SELECT 1 FROM index_links AS \"mx_l2\""));
    assert!(!compiled.sql.contains("LEFT JOIN index_links AS \"l1\""));
    assert!(!compiled.sql.contains("__t1_entity_id"));
    assert!(!compiled.sql.contains("variants"));
    assert!(!compiled.sql.contains("sales_channel"));
    assert_eq!(compiled.columns.len(), 2);
    assert!(compiled
        .binds
        .contains(&PostgresBindValue::Text("variants".to_owned())));
    assert!(compiled
        .binds
        .contains(&PostgresBindValue::Text("sales_channel".to_owned())));
    assert!(compiled
        .binds
        .contains(&PostgresBindValue::Uuid(expected_channel)));

    let count = compiled.exact_count.expect("many filter count");
    assert!(count.sql.contains("EXISTS (SELECT 1 FROM index_links"));
    assert!(!count.sql.contains("LEFT JOIN index_links AS \"l1\""));
    assert!(!count.sql.contains("ORDER BY"));
    assert!(!count.sql.contains("LIMIT"));
}

#[test]
fn compiles_many_link_ne_and_is_null_with_reference_totality() {
    let registry = many_registry();
    let mut query = root_query(Uuid::new_v4());
    query.filter = Some(FilterExpr::And(vec![
        FilterExpr::Ne(
            linked_path(&["variants"], "score"),
            IndexValue::Integer(7),
        ),
        FilterExpr::IsNull(linked_path(&["variants"], "title"), true),
        FilterExpr::Contains(
            linked_path(&["variants"], "tags"),
            IndexValue::String("featured".to_owned()),
        ),
    ]));

    let compiled = registry.compile_postgres_query(&query).unwrap();

    assert!(compiled.sql.matches("EXISTS (SELECT 1 FROM index_links").count() >= 4);
    assert!(compiled.sql.contains("AND NOT (EXISTS"));
    assert!(compiled.sql.contains("IS NULL OR"));
    assert!(compiled.sql.contains("NOT (EXISTS"));
    assert!(compiled.sql.contains(" @> "));
    assert!(!compiled.sql.contains("LEFT JOIN index_links AS \"l1\""));
}

#[test]
fn rejects_many_link_projection_until_nested_aggregation_exists() {
    let registry = registry(LinkCardinality::Many);
    let mut query = root_query(Uuid::new_v4());
    let many_id = FieldPath::linked(
        [LinkName::new("sales_channel").unwrap()],
        FieldName::new("id").unwrap(),
    );
    query.fields = vec![many_id.clone()];

    assert!(matches!(
        registry.compile_postgres_query(&query),
        Err(PostgresQueryBuildError::Compile(
            PostgresQueryCompileError::ManyLinkProjectionPending(path)
        )) if path == many_id
    ));
}

#[test]
fn rejects_tampered_many_traversal_metadata() {
    let registry = many_registry();
    let mut query = root_query(Uuid::new_v4());
    query.filter = Some(FilterExpr::Eq(
        linked_path(&["variants"], "id"),
        IndexValue::Uuid(Uuid::new_v4()),
    ));
    let mut plan = registry.plan_query(&query).unwrap();
    plan.joins[0].traverses_many = false;

    assert!(matches!(
        plan.compile_postgres(),
        Err(PostgresQueryCompileError::ManyTraversalMismatch(_))
    ));
}

#[test]
fn rejects_tampered_path_alias_mapping() {
    let registry = registry(LinkCardinality::One);
    let mut plan = registry
        .plan_query(&root_query(Uuid::new_v4()))
        .unwrap();
    plan.path_aliases.insert(Vec::new(), "t9".to_owned());

    assert!(matches!(
        plan.compile_postgres(),
        Err(PostgresQueryCompileError::AliasMappingMismatch)
    ));
}

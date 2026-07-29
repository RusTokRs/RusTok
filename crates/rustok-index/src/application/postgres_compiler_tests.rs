use uuid::Uuid;

use super::{
    CompiledQueryColumn, PostgresBindValue, PostgresQueryCompileError, SchemaRegistry,
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

fn field(name: &str, sortable: bool) -> IndexField {
    IndexField {
        name: FieldName::new(name).unwrap(),
        value_type: IndexValueType::Uuid,
        cardinality: FieldCardinality::One,
        nullable: false,
        selectable: true,
        filterable: true,
        sortable,
    }
}

fn registry(link_cardinality: LinkCardinality) -> SchemaRegistry {
    let channel = IndexSchema {
        reference: schema_ref("sales_channel"),
        locale_mode: LocaleMode::None,
        fields: vec![field("id", true)],
        links: Vec::new(),
    };
    let product = IndexSchema {
        reference: schema_ref("product"),
        locale_mode: LocaleMode::Required,
        fields: vec![field("id", true), field("sales_channel_id", false)],
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

fn root_query(tenant_id: Uuid) -> IndexQuery {
    IndexQuery {
        scope: IndexQueryScope {
            tenant_id,
            locale: Some(LocaleKey::new("en-US").unwrap()),
        },
        schema: schema_ref("product"),
        fields: vec![FieldPath::new(FieldName::new("id").unwrap())],
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

    let compiled = registry
        .plan_query(&query)
        .unwrap()
        .compile_postgres()
        .unwrap();

    assert!(compiled.sql.contains("LEFT JOIN index_links AS \"l1\""));
    assert!(compiled.sql.contains("\"l1\".source_version = \"t0\".source_version"));
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
fn rejects_semantics_reserved_for_follow_up_compiler_slices() {
    let registry = registry(LinkCardinality::One);
    let mut query = root_query(Uuid::new_v4());
    query.filter = Some(FilterExpr::Eq(
        FieldPath::new(FieldName::new("id").unwrap()),
        IndexValue::Uuid(Uuid::new_v4()),
    ));
    assert!(matches!(
        registry.plan_query(&query).unwrap().compile_postgres(),
        Err(PostgresQueryCompileError::FilterPending)
    ));

    query.filter = None;
    query.order_by = vec![OrderExpr {
        field: FieldPath::new(FieldName::new("id").unwrap()),
        direction: OrderDirection::Asc,
    }];
    assert!(matches!(
        registry.plan_query(&query).unwrap().compile_postgres(),
        Err(PostgresQueryCompileError::OrderingPending)
    ));

    query.order_by.clear();
    query.include_exact_count = true;
    assert!(matches!(
        registry.plan_query(&query).unwrap().compile_postgres(),
        Err(PostgresQueryCompileError::ExactCountPending)
    ));

    query.include_exact_count = false;
    query.pagination = Pagination::Cursor {
        first: 20,
        after: Some("opaque-cursor".to_owned()),
    };
    assert!(matches!(
        registry.plan_query(&query).unwrap().compile_postgres(),
        Err(PostgresQueryCompileError::CursorContinuationPending)
    ));

    query.pagination = Pagination::Offset {
        limit: 20,
        offset: 0,
    };
    assert!(matches!(
        registry.plan_query(&query).unwrap().compile_postgres(),
        Err(PostgresQueryCompileError::OffsetPaginationPending)
    ));
}

#[test]
fn rejects_many_link_projection_before_sql_is_emitted() {
    let registry = registry(LinkCardinality::Many);
    let mut query = root_query(Uuid::new_v4());
    query.fields = vec![FieldPath::linked(
        [LinkName::new("sales_channel").unwrap()],
        FieldName::new("id").unwrap(),
    )];

    assert!(matches!(
        registry.plan_query(&query).unwrap().compile_postgres(),
        Err(PostgresQueryCompileError::ManyLinkProjectionPending)
    ));
}

#[test]
fn rejects_tampered_path_alias_mapping() {
    let registry = registry(LinkCardinality::One);
    let mut plan = registry
        .plan_query(&root_query(Uuid::new_v4()))
        .unwrap();
    plan.projection[0].relation_alias = "t9".to_owned();

    assert!(matches!(
        plan.compile_postgres(),
        Err(PostgresQueryCompileError::AliasMappingMismatch)
    ));
}

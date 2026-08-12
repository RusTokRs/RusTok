use uuid::Uuid;

use super::{
    AggregateOrderValidationError, PostgresQueryCompileError, QueryPlanError, SchemaRegistry,
};
use crate::domain::{
    EntityName, FieldCardinality, FieldName, FieldPath, IndexField, IndexLink, IndexQuery,
    IndexQueryScope, IndexSchema, IndexValueType, LinkCardinality, LinkName, LocaleKey, LocaleMode,
    ModuleName, OrderDirection, OrderExpr, Pagination, SchemaRef, SchemaVersion,
};

fn reference(entity: &str) -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("test").unwrap(),
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

fn registry(score_type: IndexValueType) -> SchemaRegistry {
    let child = IndexSchema {
        reference: reference("child"),
        locale_mode: LocaleMode::Required,
        fields: vec![
            field("root_id", IndexValueType::Uuid, false, false),
            field("score", score_type, false, true),
        ],
        links: Vec::new(),
    };
    let root = IndexSchema {
        reference: reference("root"),
        locale_mode: LocaleMode::Required,
        fields: vec![field("id", IndexValueType::Uuid, false, true)],
        links: vec![IndexLink {
            name: LinkName::new("children").unwrap(),
            source_fields: vec![FieldName::new("id").unwrap()],
            target_schema: child.reference.clone(),
            target_fields: vec![FieldName::new("root_id").unwrap()],
            cardinality: LinkCardinality::Many,
        }],
    };
    let mut registry = SchemaRegistry::new();
    registry.register_batch([root, child]).unwrap();
    registry
}

fn linked_score() -> FieldPath {
    FieldPath::linked(
        [LinkName::new("children").unwrap()],
        FieldName::new("score").unwrap(),
    )
}

fn query(direction: OrderDirection) -> IndexQuery {
    IndexQuery {
        scope: IndexQueryScope {
            tenant_id: Uuid::new_v4(),
            locale: Some(LocaleKey::new("en-US").unwrap()),
        },
        schema: reference("root"),
        fields: vec![FieldPath::new(FieldName::new("id").unwrap())],
        filter: None,
        order_by: vec![OrderExpr {
            field: linked_score(),
            direction,
        }],
        pagination: Pagination::Offset {
            limit: 20,
            offset: 0,
        },
        include_exact_count: false,
    }
}

#[test]
fn min_asc_compiles_correlated_tagged_order_value() {
    let registry = registry(IndexValueType::Integer);
    let query = query(OrderDirection::MinAsc);
    let plan = registry.plan_query(&query).unwrap();
    assert!(plan.order_by[0].field.traverses_many);
    assert!(plan.order_by[0].field.nullable);

    let compiled = registry.compile_postgres_query(&query).unwrap();
    assert!(compiled.sql.contains("SELECT MIN("));
    assert!(compiled.sql.contains("FROM index_links AS \"mo_l1\""));
    assert!(
        compiled
            .sql
            .contains("jsonb_build_object('type', 'integer'")
    );
    assert!(compiled.sql.contains("ASC NULLS LAST"));
    assert!(compiled.sql.contains("\"t0\".entity_id ASC"));
    assert!(!compiled.sql.contains(" LEFT JOIN index_links AS \"l1\""));
}

#[test]
fn max_desc_compiles_explicit_null_policy() {
    let registry = registry(IndexValueType::String);
    let compiled = registry
        .compile_postgres_query(&query(OrderDirection::MaxDesc))
        .unwrap();
    assert!(compiled.sql.contains("SELECT MAX("));
    assert!(compiled.sql.contains("jsonb_build_object('type', 'string'"));
    assert!(compiled.sql.contains("DESC NULLS FIRST"));
}

#[test]
fn decimal_aggregate_uses_numeric_order_and_exact_string_wire() {
    let registry = registry(IndexValueType::Decimal);
    let compiled = registry
        .compile_postgres_query(&query(OrderDirection::MaxDesc))
        .unwrap();

    assert!(compiled.sql.contains("SELECT MAX("));
    assert!(compiled.sql.contains(")::numeric"));
    assert!(
        compiled
            .sql
            .contains("jsonb_build_object('type', 'decimal', 'value', to_jsonb(((SELECT MAX(")
    );
    assert!(compiled.sql.contains(")::text)) END AS \"__order_0\""));
    assert!(compiled.sql.contains("DESC NULLS FIRST"));
}

#[test]
fn aggregate_cursor_and_uuid_modes_fail_closed() {
    let integer = registry(IndexValueType::Integer);
    let mut cursor_query = query(OrderDirection::MinAsc);
    cursor_query.pagination = Pagination::Cursor {
        first: 20,
        after: None,
    };
    assert!(matches!(
        integer.plan_query(&cursor_query),
        Err(QueryPlanError::AggregateValidation(
            AggregateOrderValidationError::AggregateRequiresOffsetPagination
        ))
    ));

    let uuid = registry(IndexValueType::Uuid);
    assert!(matches!(
        uuid.plan_query(&query(OrderDirection::MaxAsc)),
        Err(QueryPlanError::AggregateValidation(
            AggregateOrderValidationError::AggregateRequiresOrderedScalar(_)
        ))
    ));
}

#[test]
fn forged_plans_remain_fail_closed() {
    let registry = registry(IndexValueType::Integer);

    let mut aggregate_cursor = registry.plan_query(&query(OrderDirection::MinAsc)).unwrap();
    aggregate_cursor.pagination = Pagination::Cursor {
        first: 20,
        after: None,
    };
    assert!(matches!(
        aggregate_cursor.compile_postgres(),
        Err(PostgresQueryCompileError::AggregateOrderingRequiresOffsetPagination)
    ));

    let mut ambiguous = registry.plan_query(&query(OrderDirection::MinAsc)).unwrap();
    ambiguous.order_by[0].direction = OrderDirection::Asc;
    ambiguous.order_by[0].field.nullable = false;
    assert!(matches!(
        ambiguous.compile_postgres(),
        Err(PostgresQueryCompileError::ManyLinkOrderingPending(_))
    ));

    let mut root_query = query(OrderDirection::MinAsc);
    root_query.order_by = vec![OrderExpr {
        field: FieldPath::new(FieldName::new("id").unwrap()),
        direction: OrderDirection::Asc,
    }];
    let mut singular = registry.plan_query(&root_query).unwrap();
    singular.order_by[0].direction = OrderDirection::MinAsc;
    singular.order_by[0].field.nullable = true;
    assert!(matches!(
        singular.compile_postgres(),
        Err(PostgresQueryCompileError::AggregateOrderingWithoutManyLink(
            _
        ))
    ));
}

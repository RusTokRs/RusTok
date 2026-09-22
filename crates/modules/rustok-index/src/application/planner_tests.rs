use uuid::Uuid;

use super::{QueryPlanError, SchemaRegistry};
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

fn registry() -> SchemaRegistry {
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
            cardinality: LinkCardinality::One,
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
        fields: vec![field("id", true)],
        links: Vec::new(),
    };
    let variant = IndexSchema {
        reference: schema_ref("variant"),
        locale_mode: LocaleMode::Required,
        fields: vec![field("id", true), field("sales_channel_id", false)],
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
        fields: vec![field("id", true)],
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

fn query() -> IndexQuery {
    let linked_id = FieldPath::linked(
        [LinkName::new("sales_channel").unwrap()],
        FieldName::new("id").unwrap(),
    );
    IndexQuery {
        scope: IndexQueryScope {
            tenant_id: Uuid::new_v4(),
            locale: Some(LocaleKey::new("en-US").unwrap()),
        },
        schema: schema_ref("product"),
        fields: vec![
            linked_id.clone(),
            FieldPath::new(FieldName::new("id").unwrap()),
        ],
        filter: Some(FilterExpr::Eq(
            linked_id.clone(),
            IndexValue::Uuid(Uuid::new_v4()),
        )),
        order_by: vec![OrderExpr {
            field: linked_id,
            direction: OrderDirection::Asc,
        }],
        pagination: Pagination::Cursor {
            first: 20,
            after: None,
        },
        include_exact_count: true,
    }
}

#[test]
fn aliases_do_not_depend_on_reference_encounter_order() {
    let registry = registry();
    let first = query();
    let mut second = first.clone();
    second.fields.reverse();

    let first = registry.plan_query(&first).unwrap();
    let second = registry.plan_query(&second).unwrap();

    assert_eq!(first.path_aliases, second.path_aliases);
    assert_eq!(first.joins, second.joins);
    assert_eq!(first.referenced_fields, second.referenced_fields);
    assert_eq!(first.root_alias, "t0");
    assert_eq!(first.joins[0].alias, "t1");
    assert!(!first.joins[0].traverses_many);
    assert_eq!(first.order_by[0].field.relation_alias, "t1");
    assert_eq!(first.order_by[0].field.value_type, IndexValueType::Uuid);
    assert_eq!(first.order_by[0].field.cardinality, FieldCardinality::One);
    assert!(!first.order_by[0].field.nullable);
    assert!(!first.order_by[0].field.traverses_many);
}

#[test]
fn many_traversal_propagates_through_descendant_joins_and_fields() {
    let registry = many_registry();
    let channel_id = FieldPath::linked(
        [
            LinkName::new("variants").unwrap(),
            LinkName::new("sales_channel").unwrap(),
        ],
        FieldName::new("id").unwrap(),
    );
    let query = IndexQuery {
        scope: IndexQueryScope {
            tenant_id: Uuid::new_v4(),
            locale: Some(LocaleKey::new("en-US").unwrap()),
        },
        schema: schema_ref("product"),
        fields: vec![FieldPath::new(FieldName::new("id").unwrap())],
        filter: Some(FilterExpr::Eq(
            channel_id.clone(),
            IndexValue::Uuid(Uuid::new_v4()),
        )),
        order_by: Vec::new(),
        pagination: Pagination::Cursor {
            first: 20,
            after: None,
        },
        include_exact_count: true,
    };

    let plan = registry.plan_query(&query).unwrap();

    assert_eq!(plan.joins.len(), 2);
    assert!(plan.joins.iter().all(|join| join.traverses_many));
    assert!(plan.field(&channel_id).unwrap().traverses_many);
    assert_eq!(plan.outer_joins().count(), 0);
}

#[test]
fn validation_precedes_plan_construction() {
    let registry = registry();
    let mut query = query();
    query.fields = vec![FieldPath::new(FieldName::new("missing").unwrap())];

    assert!(matches!(
        registry.plan_query(&query),
        Err(QueryPlanError::Validation(_))
    ));
}

#[test]
fn fingerprint_changes_with_order_semantics() {
    let registry = registry();
    let first = query();
    let mut second = first.clone();
    second.order_by[0].direction = OrderDirection::Desc;

    let first = registry.plan_query(&first).unwrap().fingerprint().unwrap();
    let second = registry.plan_query(&second).unwrap().fingerprint().unwrap();

    assert_ne!(first, second);
    assert_eq!(first.to_hex().len(), 64);
}

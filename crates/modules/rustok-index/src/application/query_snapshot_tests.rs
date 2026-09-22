use uuid::Uuid;

use super::{
    CompiledManyRelationColumn, CompiledQueryColumn, ExecutableQueryPlan, PostgresBindValue,
    SchemaRegistry,
};
use crate::domain::{
    EntityName, FieldCardinality, FieldName, FieldPath, IndexField, IndexLink, IndexQuery,
    IndexQueryScope, IndexSchema, IndexValueType, LinkCardinality, LinkName, LocaleKey, LocaleMode,
    ModuleName, Pagination, SchemaRef, SchemaVersion,
};

const PLAN_SNAPSHOT: &str = include_str!("snapshots/m4_many_projection.plan.snap");
const SQL_SNAPSHOT: &str = include_str!("snapshots/m4_many_projection.sql");
const COMPILED_SNAPSHOT: &str = include_str!("snapshots/m4_many_projection.compiled.snap");

fn schema_ref(entity: &str) -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new(entity).unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

fn field(name: &str) -> IndexField {
    IndexField {
        name: FieldName::new(name).unwrap(),
        value_type: IndexValueType::Uuid,
        cardinality: FieldCardinality::One,
        nullable: false,
        selectable: true,
        filterable: true,
        sortable: true,
    }
}

fn registry() -> SchemaRegistry {
    let variant = IndexSchema {
        reference: schema_ref("variant"),
        locale_mode: LocaleMode::Required,
        fields: vec![field("id")],
        links: Vec::new(),
    };
    let product = IndexSchema {
        reference: schema_ref("product"),
        locale_mode: LocaleMode::Required,
        fields: vec![field("id")],
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

#[test]
fn retained_v4_plan_and_sql_snapshots_are_stable() {
    let registry = registry();
    let query = query();
    let plan = registry.plan_query(&query).unwrap();
    let compiled = registry.compile_postgres_query(&query).unwrap();

    assert_eq!(render_plan(&plan), PLAN_SNAPSHOT);
    assert_eq!(format!("{}\n", compiled.sql), SQL_SNAPSHOT);
    assert_eq!(
        render_compiled(&compiled.binds, &compiled.columns, &compiled.many_relations),
        COMPILED_SNAPSHOT
    );
    assert!(compiled.exact_count.is_none());
}

fn render_plan(plan: &ExecutableQueryPlan) -> String {
    let mut lines = vec![
        format!("root={}", plan.root_schema),
        format!("root_alias={}", plan.root_alias),
    ];
    for (path, alias) in &plan.path_aliases {
        lines.push(format!("alias:{}={alias}", render_links(path)));
    }
    for join in &plan.joins {
        lines.push(format!(
            "join:{}|{}->{}|{}|{}|traverses_many={}",
            render_links(&join.path),
            join.source_alias,
            join.alias,
            join.target_schema,
            render_link_cardinality(join.cardinality),
            join.traverses_many,
        ));
    }
    for field in &plan.projection {
        lines.push(format!(
            "projection:{}|{}|{}|{}|nullable={}|traverses_many={}",
            render_field_path(&field.path),
            field.relation_alias,
            render_value_type(field.value_type),
            render_field_cardinality(field.cardinality),
            field.nullable,
            field.traverses_many,
        ));
    }
    for projection in &plan.many_projections {
        lines.push(format!(
            "many:{}|identities={}|fields={}",
            render_links(&projection.path),
            projection
                .identity_paths
                .iter()
                .map(|path| render_links(path))
                .collect::<Vec<_>>()
                .join(","),
            projection
                .fields
                .iter()
                .map(|field| render_field_path(&field.path))
                .collect::<Vec<_>>()
                .join(","),
        ));
    }
    lines.push("order=<none>".to_owned());
    lines.push("pagination=cursor:first=2:after=false".to_owned());
    lines.push("exact_count=false".to_owned());
    format!("{}\n", lines.join("\n"))
}

fn render_compiled(
    binds: &[PostgresBindValue],
    columns: &[CompiledQueryColumn],
    many_relations: &[CompiledManyRelationColumn],
) -> String {
    let mut lines = Vec::new();
    for (index, bind) in binds.iter().enumerate() {
        lines.push(format!(
            "bind:{}={}",
            index + 1,
            serde_json::to_string(bind).unwrap(),
        ));
    }
    for column in columns {
        match column {
            CompiledQueryColumn::EntityId {
                output_alias,
                relation_alias,
            } => lines.push(format!("column:entity_id|{output_alias}|{relation_alias}")),
            CompiledQueryColumn::Field {
                output_alias,
                field,
            } => lines.push(format!(
                "column:field|{output_alias}|{}|{}",
                render_field_path(&field.path),
                field.relation_alias,
            )),
            CompiledQueryColumn::OrderValue { .. } => {
                panic!("canonical snapshot query has no explicit ordering")
            }
        }
    }
    for column in many_relations {
        lines.push(format!(
            "many_column:{}|path={}|identities={}|fields={}",
            column.output_alias,
            render_links(&column.projection.path),
            column
                .projection
                .identity_paths
                .iter()
                .map(|path| render_links(path))
                .collect::<Vec<_>>()
                .join(","),
            column
                .projection
                .fields
                .iter()
                .map(|field| render_field_path(&field.path))
                .collect::<Vec<_>>()
                .join(","),
        ));
    }
    format!("{}\n", lines.join("\n"))
}

fn render_links(path: &[LinkName]) -> String {
    if path.is_empty() {
        "<root>".to_owned()
    } else {
        path.iter()
            .map(|link| link.as_str())
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn render_field_path(path: &FieldPath) -> String {
    if path.links().is_empty() {
        path.field().as_str().to_owned()
    } else {
        format!("{}.{}", render_links(path.links()), path.field().as_str())
    }
}

fn render_value_type(value_type: IndexValueType) -> &'static str {
    match value_type {
        IndexValueType::Boolean => "boolean",
        IndexValueType::Integer => "integer",
        IndexValueType::Decimal => "decimal",
        IndexValueType::String => "string",
        IndexValueType::Uuid => "uuid",
        IndexValueType::Timestamp => "timestamp",
    }
}

fn render_field_cardinality(cardinality: FieldCardinality) -> &'static str {
    match cardinality {
        FieldCardinality::One => "one",
        FieldCardinality::Many => "many",
    }
}

fn render_link_cardinality(cardinality: LinkCardinality) -> &'static str {
    match cardinality {
        LinkCardinality::One => "one",
        LinkCardinality::Many => "many",
    }
}

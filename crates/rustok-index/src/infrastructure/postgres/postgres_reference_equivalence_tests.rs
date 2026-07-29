use std::{
    cmp::Ordering,
    collections::BTreeMap,
    sync::Arc,
};

use rustok_core::MigrationSource;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::{
    CursorCodec, EntityKey, EntityName, ExecutableQueryPlan, FieldCardinality, FieldName,
    FieldPath, FilterExpr, IndexField, IndexLink, IndexLinkValue, IndexModule, IndexMutation,
    IndexNestedRelationItem, IndexNestedRelationProjection, IndexProjectedValue, IndexQuery,
    IndexQueryItem, IndexQueryPage, IndexQueryPort, IndexQueryScope, IndexRecord,
    IndexRelationIdentity, IndexSchema, IndexValue, IndexValueType, LinkCardinality,
    LinkedEntityKey, LinkName, LocaleKey, LocaleMode, ModuleName, MutationDelivery,
    OrderDirection, OrderExpr, Pagination, PlannedField, PostgresIndexQueryPort,
    PostgresMutationStore, PostgresSchemaRegistrationStore, SchemaRef, SchemaRegistry,
    SchemaVersion,
};

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const TENANT: Uuid = Uuid::from_u128(0x11111111111111111111111111111111);
const LOCALE: &str = "en-US";

struct PostgresTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresTestDb {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping rustok-index PostgreSQL/reference equivalence"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_index_equivalence_{}",
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
            .await?;
        db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
            .await?;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id) VALUES ($1)",
            vec![TENANT.into()],
        ))
        .await?;

        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await?;
        }

        Ok(Some(Self {
            control,
            db,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

fn postgres_database_url() -> Option<String> {
    std::env::var(DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

struct Fixture {
    registry: Arc<SchemaRegistry>,
    records: Vec<IndexRecord>,
    product: SchemaRef,
    channel: SchemaRef,
    variant: SchemaRef,
}

impl Fixture {
    fn new() -> Self {
        let channel = schema_ref("channel");
        let variant = schema_ref("variant");
        let product = schema_ref("product");

        let channel_schema = IndexSchema {
            reference: channel.clone(),
            locale_mode: LocaleMode::None,
            fields: vec![
                field("id", IndexValueType::Uuid, FieldCardinality::One, false, true),
                field("name", IndexValueType::String, FieldCardinality::One, false, true),
            ],
            links: Vec::new(),
        };
        let variant_schema = IndexSchema {
            reference: variant.clone(),
            locale_mode: LocaleMode::Required,
            fields: vec![
                field("id", IndexValueType::Uuid, FieldCardinality::One, false, true),
                field("score", IndexValueType::Integer, FieldCardinality::One, false, true),
                field("title", IndexValueType::String, FieldCardinality::One, true, true),
                field("tags", IndexValueType::String, FieldCardinality::Many, false, false),
            ],
            links: Vec::new(),
        };
        let product_schema = IndexSchema {
            reference: product.clone(),
            locale_mode: LocaleMode::Required,
            fields: vec![
                field("id", IndexValueType::Uuid, FieldCardinality::One, false, true),
                field("score", IndexValueType::Integer, FieldCardinality::One, false, true),
                field("title", IndexValueType::String, FieldCardinality::One, true, true),
                field(
                    "channel_id",
                    IndexValueType::Uuid,
                    FieldCardinality::One,
                    false,
                    false,
                ),
            ],
            links: vec![
                IndexLink {
                    name: link("channel"),
                    source_fields: vec![field_name("channel_id")],
                    target_schema: channel.clone(),
                    target_fields: vec![field_name("id")],
                    cardinality: LinkCardinality::One,
                },
                IndexLink {
                    name: link("variants"),
                    source_fields: vec![field_name("id")],
                    target_schema: variant.clone(),
                    target_fields: vec![field_name("id")],
                    cardinality: LinkCardinality::Many,
                },
            ],
        };

        let mut registry = SchemaRegistry::new();
        registry
            .register_batch([
                product_schema,
                channel_schema,
                variant_schema,
            ])
            .expect("equivalence schemas should register");

        let c1 = Uuid::from_u128(1);
        let c2 = Uuid::from_u128(2);
        let v1 = Uuid::from_u128(11);
        let v2 = Uuid::from_u128(12);
        let v3 = Uuid::from_u128(13);
        let v4 = Uuid::from_u128(14);
        let p1 = Uuid::from_u128(101);
        let p2 = Uuid::from_u128(102);
        let p3 = Uuid::from_u128(103);
        let p4 = Uuid::from_u128(104);

        let records = vec![
            channel_record(&channel, c1, "retail"),
            channel_record(&channel, c2, "wholesale"),
            variant_record(&variant, v1, 7, IndexValue::String("blue".to_owned()), &["featured", "summer"]),
            variant_record(&variant, v2, 3, IndexValue::Null, &["clearance"]),
            variant_record(&variant, v3, 11, IndexValue::String("blocked".to_owned()), &["clearance"]),
            variant_record(&variant, v4, 9, IndexValue::String("green".to_owned()), &["featured"]),
            product_record(&product, &channel, &variant, p1, 10, IndexValue::String("Alpha".to_owned()), c1, &[v1, v2]),
            product_record(&product, &channel, &variant, p2, 20, IndexValue::String("Beta".to_owned()), c2, &[v3]),
            product_record(&product, &channel, &variant, p3, 15, IndexValue::Null, c1, &[v4]),
            product_record(&product, &channel, &variant, p4, 5, IndexValue::String("Delta".to_owned()), c2, &[]),
        ];

        Self {
            registry: Arc::new(registry),
            records,
            product,
            channel,
            variant,
        }
    }

    fn schemas(&self) -> Vec<IndexSchema> {
        [self.product.clone(), self.channel.clone(), self.variant.clone()]
            .into_iter()
            .map(|reference| {
                self.registry
                    .get(&reference)
                    .expect("fixture schema should remain registered")
                    .schema
                    .clone()
            })
            .collect()
    }
}

fn schema_ref(entity: &str) -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("catalog").unwrap(),
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
        name: field_name(name),
        value_type,
        cardinality,
        nullable,
        selectable: true,
        filterable: true,
        sortable,
    }
}

fn field_name(value: &str) -> FieldName {
    FieldName::new(value).unwrap()
}

fn link(value: &str) -> LinkName {
    LinkName::new(value).unwrap()
}

fn root_path(field: &str) -> FieldPath {
    FieldPath::new(field_name(field))
}

fn linked_path(links: &[&str], field: &str) -> FieldPath {
    FieldPath::linked(links.iter().map(|value| link(value)), field_name(field))
}

fn locale() -> LocaleKey {
    LocaleKey::new(LOCALE).unwrap()
}

fn channel_record(schema: &SchemaRef, entity_id: Uuid, name: &str) -> IndexRecord {
    IndexRecord {
        key: EntityKey {
            tenant_id: TENANT,
            schema: schema.clone(),
            entity_id,
            locale: None,
        },
        source_version: 1,
        fields: BTreeMap::from([
            (field_name("id"), IndexValue::Uuid(entity_id)),
            (field_name("name"), IndexValue::String(name.to_owned())),
        ]),
        links: Vec::new(),
    }
}

fn variant_record(
    schema: &SchemaRef,
    entity_id: Uuid,
    score: i64,
    title: IndexValue,
    tags: &[&str],
) -> IndexRecord {
    IndexRecord {
        key: EntityKey {
            tenant_id: TENANT,
            schema: schema.clone(),
            entity_id,
            locale: Some(locale()),
        },
        source_version: 1,
        fields: BTreeMap::from([
            (field_name("id"), IndexValue::Uuid(entity_id)),
            (field_name("score"), IndexValue::Integer(score)),
            (field_name("title"), title),
            (
                field_name("tags"),
                IndexValue::List(
                    tags.iter()
                        .map(|value| IndexValue::String((*value).to_owned()))
                        .collect(),
                ),
            ),
        ]),
        links: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn product_record(
    product: &SchemaRef,
    channel: &SchemaRef,
    variant: &SchemaRef,
    entity_id: Uuid,
    score: i64,
    title: IndexValue,
    channel_id: Uuid,
    variant_ids: &[Uuid],
) -> IndexRecord {
    IndexRecord {
        key: EntityKey {
            tenant_id: TENANT,
            schema: product.clone(),
            entity_id,
            locale: Some(locale()),
        },
        source_version: 1,
        fields: BTreeMap::from([
            (field_name("id"), IndexValue::Uuid(entity_id)),
            (field_name("score"), IndexValue::Integer(score)),
            (field_name("title"), title),
            (field_name("channel_id"), IndexValue::Uuid(channel_id)),
        ]),
        links: vec![
            IndexLinkValue {
                name: link("channel"),
                targets: vec![LinkedEntityKey {
                    schema: channel.clone(),
                    entity_id: channel_id,
                    locale: None,
                }],
            },
            IndexLinkValue {
                name: link("variants"),
                targets: variant_ids
                    .iter()
                    .map(|entity_id| LinkedEntityKey {
                        schema: variant.clone(),
                        entity_id: *entity_id,
                        locale: Some(locale()),
                    })
                    .collect(),
            },
        ],
    }
}

struct ReferenceFixture<'a> {
    registry: &'a SchemaRegistry,
    records: BTreeMap<EntityKey, IndexRecord>,
}

impl<'a> ReferenceFixture<'a> {
    fn new(registry: &'a SchemaRegistry, records: &[IndexRecord]) -> Self {
        Self {
            registry,
            records: records
                .iter()
                .cloned()
                .map(|record| (record.key.clone(), record))
                .collect(),
        }
    }

    fn page(&self, query: &IndexQuery) -> IndexQueryPage {
        self.registry
            .validate_query(query)
            .expect("equivalence query should validate");
        let plan = self
            .registry
            .plan_query(query)
            .expect("equivalence query should plan");

        let mut records = self
            .records
            .values()
            .filter(|record| record.key.schema == query.schema)
            .filter(|record| record.key.tenant_id == query.scope.tenant_id)
            .filter(|record| record.key.locale == query.scope.locale)
            .filter(|record| {
                query
                    .filter
                    .as_ref()
                    .is_none_or(|filter| self.matches_filter(record, filter))
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| self.compare_records(left, right, query));
        let exact_count = query.include_exact_count.then_some(records.len() as u64);

        if let Pagination::Cursor {
            after: Some(encoded),
            ..
        } = &query.pagination
        {
            let cursor = CursorCodec::decode_scoped_for_query(encoded, query, self.registry)
                .expect("production cursor should decode in reference fixture");
            records.retain(|record| self.compare_record_to_cursor(record, &cursor, query).is_gt());
        }

        let page_size = match query.pagination {
            Pagination::Cursor { first, .. } => first as usize,
            Pagination::Offset { limit, .. } => limit as usize,
        };
        let offset = match query.pagination {
            Pagination::Cursor { .. } => 0,
            Pagination::Offset { offset, .. } => offset as usize,
        };
        let mut window = records
            .into_iter()
            .skip(offset)
            .take(page_size + 1)
            .collect::<Vec<_>>();
        let has_more = window.len() > page_size;
        window.truncate(page_size);

        let items = window
            .iter()
            .map(|record| self.project_item(&plan, record))
            .collect::<Vec<_>>();
        let next_cursor = if has_more && matches!(query.pagination, Pagination::Cursor { .. }) {
            window.last().map(|record| {
                CursorCodec::encode_for_query(&self.cursor_for(record, query), query, self.registry)
                    .expect("reference cursor should encode")
            })
        } else {
            None
        };

        IndexQueryPage {
            items,
            exact_count,
            has_more,
            next_cursor,
        }
    }

    fn project_item(&self, plan: &ExecutableQueryPlan, root: &IndexRecord) -> IndexQueryItem {
        let relations = plan
            .outer_joins()
            .filter(|join| {
                plan.projection.iter().any(|field| {
                    !field.traverses_many && field.path.links().starts_with(&join.path)
                })
            })
            .map(|join| IndexRelationIdentity {
                path: join.path.clone(),
                entity_id: self
                    .relation_chains(root, &join.path)
                    .first()
                    .and_then(|chain| chain.last())
                    .map(|record| record.key.entity_id),
            })
            .collect();
        let fields = plan
            .outer_projection()
            .map(|field| IndexProjectedValue {
                path: field.path.clone(),
                value: self.projected_value(root, field),
            })
            .collect();
        let nested_relations = plan
            .many_projections
            .iter()
            .map(|projection| {
                let items = self
                    .relation_chains(root, &projection.path)
                    .into_iter()
                    .map(|chain| {
                        let terminal = chain
                            .last()
                            .expect("many projection path should have a terminal record");
                        IndexNestedRelationItem {
                            relations: projection
                                .identity_paths
                                .iter()
                                .cloned()
                                .zip(chain.iter())
                                .map(|(path, record)| IndexRelationIdentity {
                                    path,
                                    entity_id: Some(record.key.entity_id),
                                })
                                .collect(),
                            fields: projection
                                .fields
                                .iter()
                                .map(|field| IndexProjectedValue {
                                    path: field.path.clone(),
                                    value: terminal
                                        .fields
                                        .get(field.path.field())
                                        .cloned()
                                        .unwrap_or(IndexValue::Null),
                                })
                                .collect(),
                        }
                    })
                    .collect();
                IndexNestedRelationProjection {
                    path: projection.path.clone(),
                    items,
                }
            })
            .collect();

        IndexQueryItem {
            entity_id: root.key.entity_id,
            relations,
            fields,
            nested_relations,
        }
    }

    fn projected_value(&self, root: &IndexRecord, field: &PlannedField) -> IndexValue {
        if field.path.links().is_empty() {
            return root
                .fields
                .get(field.path.field())
                .cloned()
                .unwrap_or(IndexValue::Null);
        }
        self.relation_chains(root, field.path.links())
            .first()
            .and_then(|chain| chain.last())
            .and_then(|record| record.fields.get(field.path.field()))
            .cloned()
            .unwrap_or(IndexValue::Null)
    }

    fn relation_chains<'b>(
        &'b self,
        root: &'b IndexRecord,
        path: &[LinkName],
    ) -> Vec<Vec<&'b IndexRecord>> {
        let mut chains = vec![(root, Vec::<&IndexRecord>::new())];
        for link_name in path {
            let mut next = Vec::new();
            for (source, chain) in chains {
                let Some(link) = source.links.iter().find(|link| link.name == *link_name) else {
                    continue;
                };
                for target in &link.targets {
                    let key = EntityKey {
                        tenant_id: source.key.tenant_id,
                        schema: target.schema.clone(),
                        entity_id: target.entity_id,
                        locale: target.locale.clone(),
                    };
                    if let Some(record) = self.records.get(&key) {
                        let mut child_chain = chain.clone();
                        child_chain.push(record);
                        next.push((record, child_chain));
                    }
                }
            }
            chains = next;
        }
        chains.into_iter().map(|(_, chain)| chain).collect()
    }

    fn values_for_path<'b>(
        &'b self,
        root: &'b IndexRecord,
        path: &FieldPath,
    ) -> Vec<&'b IndexValue> {
        if path.links().is_empty() {
            return root.fields.get(path.field()).into_iter().collect();
        }
        self.relation_chains(root, path.links())
            .into_iter()
            .filter_map(|chain| chain.last().and_then(|record| record.fields.get(path.field())))
            .collect()
    }

    fn matches_filter(&self, record: &IndexRecord, filter: &FilterExpr) -> bool {
        match filter {
            FilterExpr::And(children) => children
                .iter()
                .all(|child| self.matches_filter(record, child)),
            FilterExpr::Or(children) => children
                .iter()
                .any(|child| self.matches_filter(record, child)),
            FilterExpr::Not(child) => !self.matches_filter(record, child),
            FilterExpr::Eq(path, expected) => self
                .values_for_path(record, path)
                .into_iter()
                .any(|value| value == expected),
            FilterExpr::Ne(path, expected) => {
                let values = self.values_for_path(record, path);
                !values.is_empty()
                    && values
                        .into_iter()
                        .all(|value| !matches!(value, IndexValue::Null) && value != expected)
            }
            FilterExpr::In(path, expected) => self
                .values_for_path(record, path)
                .into_iter()
                .any(|value| expected.contains(value)),
            FilterExpr::Gt(path, expected) => {
                self.matches_ordered(record, path, expected, Ordering::is_gt)
            }
            FilterExpr::Gte(path, expected) => self.matches_ordered(
                record,
                path,
                expected,
                |ordering| ordering.is_gt() || ordering.is_eq(),
            ),
            FilterExpr::Lt(path, expected) => {
                self.matches_ordered(record, path, expected, Ordering::is_lt)
            }
            FilterExpr::Lte(path, expected) => self.matches_ordered(
                record,
                path,
                expected,
                |ordering| ordering.is_lt() || ordering.is_eq(),
            ),
            FilterExpr::Contains(path, expected) => self
                .values_for_path(record, path)
                .into_iter()
                .any(|value| match value {
                    IndexValue::List(values) => values.contains(expected),
                    _ => false,
                }),
            FilterExpr::IsNull(path, expected_null) => {
                let values = self.values_for_path(record, path);
                let is_null = values.is_empty()
                    || values
                        .into_iter()
                        .all(|value| matches!(value, IndexValue::Null));
                is_null == *expected_null
            }
        }
    }

    fn matches_ordered(
        &self,
        record: &IndexRecord,
        path: &FieldPath,
        expected: &IndexValue,
        predicate: impl Fn(Ordering) -> bool,
    ) -> bool {
        self.values_for_path(record, path)
            .into_iter()
            .filter_map(|value| compare_values(value, expected))
            .any(predicate)
    }

    fn compare_records(
        &self,
        left: &IndexRecord,
        right: &IndexRecord,
        query: &IndexQuery,
    ) -> Ordering {
        for order in &query.order_by {
            let comparison = compare_optional_values(
                self.values_for_path(left, &order.field).into_iter().next(),
                self.values_for_path(right, &order.field).into_iter().next(),
            );
            if comparison != Ordering::Equal {
                return apply_direction(comparison, order.direction);
            }
        }
        left.key.entity_id.cmp(&right.key.entity_id)
    }

    fn compare_record_to_cursor(
        &self,
        record: &IndexRecord,
        cursor: &crate::IndexCursor,
        query: &IndexQuery,
    ) -> Ordering {
        for (order, cursor_value) in query.order_by.iter().zip(&cursor.order_values) {
            let comparison = compare_optional_to_cursor(
                self.values_for_path(record, &order.field).into_iter().next(),
                cursor_value,
            );
            if comparison != Ordering::Equal {
                return apply_direction(comparison, order.direction);
            }
        }
        record.key.entity_id.cmp(&cursor.entity_id)
    }

    fn cursor_for(&self, record: &IndexRecord, query: &IndexQuery) -> crate::IndexCursor {
        let schema = self
            .registry
            .get(&query.schema)
            .expect("validated query schema should remain registered");
        crate::IndexCursor {
            tenant_id: query.scope.tenant_id,
            schema: query.schema.clone(),
            schema_fingerprint: schema.fingerprint,
            locale: query.scope.locale.clone(),
            order_values: query
                .order_by
                .iter()
                .map(|order| {
                    self.values_for_path(record, &order.field)
                        .into_iter()
                        .next()
                        .cloned()
                        .unwrap_or(IndexValue::Null)
                })
                .collect(),
            entity_id: record.key.entity_id,
        }
    }
}

fn apply_direction(ordering: Ordering, direction: OrderDirection) -> Ordering {
    match direction {
        OrderDirection::Asc => ordering,
        OrderDirection::Desc => ordering.reverse(),
    }
}

fn compare_optional_values(left: Option<&IndexValue>, right: Option<&IndexValue>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_values(left, right).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_optional_to_cursor(
    record_value: Option<&IndexValue>,
    cursor_value: &IndexValue,
) -> Ordering {
    match (record_value, cursor_value) {
        (None, IndexValue::Null) | (Some(IndexValue::Null), IndexValue::Null) => Ordering::Equal,
        (None, _) | (Some(IndexValue::Null), _) => Ordering::Greater,
        (Some(_), IndexValue::Null) => Ordering::Less,
        (Some(record_value), cursor_value) => {
            compare_values(record_value, cursor_value).unwrap_or(Ordering::Equal)
        }
    }
}

fn compare_values(left: &IndexValue, right: &IndexValue) -> Option<Ordering> {
    match (left, right) {
        (IndexValue::Boolean(left), IndexValue::Boolean(right)) => Some(left.cmp(right)),
        (IndexValue::Integer(left), IndexValue::Integer(right)) => Some(left.cmp(right)),
        (IndexValue::Decimal(left), IndexValue::Decimal(right)) => Some(left.cmp(right)),
        (IndexValue::String(left), IndexValue::String(right)) => Some(left.cmp(right)),
        (IndexValue::Uuid(left), IndexValue::Uuid(right)) => Some(left.cmp(right)),
        (IndexValue::Timestamp(left), IndexValue::Timestamp(right)) => Some(left.cmp(right)),
        _ => None,
    }
}

fn query(fields: Vec<FieldPath>, filter: Option<FilterExpr>, order_by: Vec<OrderExpr>) -> IndexQuery {
    IndexQuery {
        scope: IndexQueryScope {
            tenant_id: TENANT,
            locale: Some(locale()),
        },
        schema: schema_ref("product"),
        fields,
        filter,
        order_by,
        pagination: Pagination::Cursor {
            first: 10,
            after: None,
        },
        include_exact_count: true,
    }
}

async fn assert_equivalent(
    port: &PostgresIndexQueryPort,
    reference: &ReferenceFixture<'_>,
    query: IndexQuery,
) -> TestResult<IndexQueryPage> {
    let expected = reference.page(&query);
    let actual = port.execute_query(query).await?;
    assert_eq!(actual, expected);
    Ok(actual)
}

#[tokio::test]
async fn postgres_query_port_matches_reference_fixture() -> TestResult<()> {
    let Some(test_db) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let fixture = Fixture::new();
    let registration = PostgresSchemaRegistrationStore::new(test_db.db.clone());
    for schema in fixture.schemas() {
        registration.register(TENANT, &schema).await?;
    }

    let mutation_store = PostgresMutationStore::new(test_db.db.clone());
    for (index, record) in fixture.records.iter().cloned().enumerate() {
        let mutation = IndexMutation::Upsert {
            event_id: Uuid::from_u128(1000 + index as u128),
            record,
        };
        let delivery = MutationDelivery::new(
            "m4-postgres-reference-equivalence",
            format!("record-{index}"),
            mutation,
        )?;
        mutation_store.apply(&fixture.registry, &delivery).await?;
    }

    let port = PostgresIndexQueryPort::new(test_db.db.clone(), Arc::clone(&fixture.registry));
    let reference = ReferenceFixture::new(&fixture.registry, &fixture.records);

    let mut first_page_query = query(
        vec![
            root_path("id"),
            root_path("score"),
            linked_path(&["channel"], "name"),
        ],
        Some(FilterExpr::And(vec![
            FilterExpr::Gte(root_path("score"), IndexValue::Integer(10)),
            FilterExpr::IsNull(root_path("title"), false),
        ])),
        vec![OrderExpr {
            field: root_path("score"),
            direction: OrderDirection::Desc,
        }],
    );
    first_page_query.pagination = Pagination::Cursor {
        first: 1,
        after: None,
    };
    let first_page = assert_equivalent(&port, &reference, first_page_query.clone()).await?;
    let mut second_page_query = first_page_query;
    second_page_query.pagination = Pagination::Cursor {
        first: 1,
        after: first_page.next_cursor.clone(),
    };
    assert_equivalent(&port, &reference, second_page_query).await?;

    assert_equivalent(
        &port,
        &reference,
        query(
            vec![root_path("id"), linked_path(&["channel"], "name")],
            Some(FilterExpr::Eq(
                linked_path(&["channel"], "name"),
                IndexValue::String("retail".to_owned()),
            )),
            Vec::new(),
        ),
    )
    .await?;

    assert_equivalent(
        &port,
        &reference,
        query(
            vec![
                root_path("id"),
                linked_path(&["variants"], "id"),
                linked_path(&["variants"], "score"),
                linked_path(&["variants"], "tags"),
            ],
            Some(FilterExpr::And(vec![
                FilterExpr::Gte(
                    linked_path(&["variants"], "score"),
                    IndexValue::Integer(7),
                ),
                FilterExpr::Contains(
                    linked_path(&["variants"], "tags"),
                    IndexValue::String("featured".to_owned()),
                ),
                FilterExpr::Ne(
                    linked_path(&["variants"], "title"),
                    IndexValue::String("blocked".to_owned()),
                ),
            ])),
            Vec::new(),
        ),
    )
    .await?;

    assert_equivalent(
        &port,
        &reference,
        query(
            vec![root_path("id")],
            Some(FilterExpr::IsNull(
                linked_path(&["variants"], "title"),
                true,
            )),
            Vec::new(),
        ),
    )
    .await?;

    let mut offset_query = query(
        vec![root_path("id"), root_path("score")],
        None,
        vec![OrderExpr {
            field: root_path("score"),
            direction: OrderDirection::Asc,
        }],
    );
    offset_query.pagination = Pagination::Offset {
        limit: 2,
        offset: 1,
    };
    assert_equivalent(&port, &reference, offset_query).await?;

    test_db.cleanup().await?;
    Ok(())
}

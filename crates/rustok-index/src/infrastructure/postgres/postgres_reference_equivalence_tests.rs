mod reference_fixture;

use std::{collections::BTreeMap, sync::Arc};

use reference_fixture::ReferenceFixture;
use rustok_core::MigrationSource;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::{
    EntityKey, EntityName, FieldCardinality, FieldName, FieldPath, FilterExpr, IndexField,
    IndexLink, IndexLinkValue, IndexModule, IndexMutation, IndexQuery, IndexQueryPage,
    IndexQueryPort, IndexQueryScope, IndexRecord, IndexSchema, IndexValue, IndexValueType,
    LinkCardinality, LinkName, LinkedEntityKey, LocaleKey, LocaleMode, ModuleName,
    MutationDelivery, OrderDirection, OrderExpr, Pagination, PostgresIndexQueryPort,
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
        let schema_name = format!("rustok_index_equivalence_{}", Uuid::new_v4().simple());
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
    schemas: Vec<IndexSchema>,
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
                field(
                    "id",
                    IndexValueType::Uuid,
                    FieldCardinality::One,
                    false,
                    true,
                ),
                field(
                    "name",
                    IndexValueType::String,
                    FieldCardinality::One,
                    false,
                    true,
                ),
            ],
            links: Vec::new(),
        };
        let variant_schema = IndexSchema {
            reference: variant.clone(),
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
                    false,
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
            ],
            links: Vec::new(),
        };
        let product_schema = IndexSchema {
            reference: product.clone(),
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
                    false,
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

        let schemas = vec![
            product_schema.clone(),
            channel_schema.clone(),
            variant_schema.clone(),
        ];
        let mut registry = SchemaRegistry::new();
        registry
            .register_batch(schemas.clone())
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
            variant_record(
                &variant,
                v1,
                7,
                IndexValue::String("blue".to_owned()),
                &["featured", "summer"],
            ),
            variant_record(&variant, v2, 3, IndexValue::Null, &["clearance"]),
            variant_record(
                &variant,
                v3,
                11,
                IndexValue::String("blocked".to_owned()),
                &["clearance"],
            ),
            variant_record(
                &variant,
                v4,
                9,
                IndexValue::String("green".to_owned()),
                &["featured"],
            ),
            product_record(
                &product,
                &channel,
                &variant,
                p1,
                10,
                IndexValue::String("Alpha".to_owned()),
                c1,
                &[v1, v2],
            ),
            product_record(
                &product,
                &channel,
                &variant,
                p2,
                20,
                IndexValue::String("Beta".to_owned()),
                c2,
                &[v3],
            ),
            product_record(
                &product,
                &channel,
                &variant,
                p3,
                15,
                IndexValue::Null,
                c1,
                &[v4],
            ),
            product_record(
                &product,
                &channel,
                &variant,
                p4,
                5,
                IndexValue::String("Delta".to_owned()),
                c2,
                &[],
            ),
        ];

        Self {
            registry: Arc::new(registry),
            records,
            schemas,
        }
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

fn query(
    fields: Vec<FieldPath>,
    filter: Option<FilterExpr>,
    order_by: Vec<OrderExpr>,
) -> IndexQuery {
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
    for schema in &fixture.schemas {
        registration.register(TENANT, schema).await?;
    }

    let mutation_store = PostgresMutationStore::new(test_db.db.clone());
    for (index, record) in fixture.records.iter().cloned().enumerate() {
        let delivery = MutationDelivery::new(
            "m4-postgres-reference-equivalence",
            format!("record-{index}"),
            IndexMutation::Upsert {
                event_id: Uuid::from_u128(1000 + index as u128),
                record,
            },
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
        after: first_page.next_cursor,
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
                FilterExpr::Gte(linked_path(&["variants"], "score"), IndexValue::Integer(7)),
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

#![cfg(feature = "mod-product")]

use std::{env, error::Error};

use rustok_core::MigrationSource;
use rustok_index::IndexModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value,
};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_PRODUCT_STOREFRONT_COLLATION_DATABASE_URL";
const TENANT_ID: Uuid = Uuid::from_u128(0x7100);
const PRODUCT_NEEDLE_UPPER: Uuid = Uuid::from_u128(0x7201);
const PRODUCT_NEEDLE_LOWER: Uuid = Uuid::from_u128(0x7202);
const PRODUCT_CAFE_NFC: Uuid = Uuid::from_u128(0x7203);
const PRODUCT_CAFE_NFD: Uuid = Uuid::from_u128(0x7204);
const PRODUCT_UNDERSCORE: Uuid = Uuid::from_u128(0x7205);
const PRODUCT_UNDERSCORE_WILDCARD: Uuid = Uuid::from_u128(0x7206);
const PRODUCT_PERCENT: Uuid = Uuid::from_u128(0x7207);
const PRODUCT_PERCENT_WILDCARD: Uuid = Uuid::from_u128(0x7208);
const PRODUCT_STRASSE_SHARP_S: Uuid = Uuid::from_u128(0x7209);
const PRODUCT_STRASSE_ASCII: Uuid = Uuid::from_u128(0x7210);

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct ProductMigrator;

#[async_trait::async_trait]
impl MigratorTrait for ProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_product::migrations::migrations()
    }
}

struct TestDatabase {
    control: DatabaseConnection,
    query: DatabaseConnection,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Product Storefront collation packet"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_product_storefront_collation_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let query = scoped_connection(
            &database_url,
            &schema_name,
            &format!("product_storefront_collation_{suffix}"),
        )
        .await?;
        create_product_migration_prerequisites(&query).await?;
        ProductMigrator::up(&query, None).await?;
        // Keep Index migrations in the retained packet so the database shape is compatible with the
        // same distribution evidence environment used by the owner-vs-shadow packets. The collation
        // probe itself intentionally compares the two exact LIKE expressions on the owner column.
        let manager = SchemaManager::new(&query);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await?;
        }
        seed_titles(&query).await?;

        Ok(Some(Self {
            control,
            query,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.query.close().await?;
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        self.control.close().await?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct LikeCase {
    label: &'static str,
    search: &'static str,
    expected_c_ids: &'static [Uuid],
}

const CASES: &[LikeCase] = &[
    LikeCase {
        label: "ASCII case-sensitive upper",
        search: "Needle",
        expected_c_ids: &[PRODUCT_NEEDLE_UPPER],
    },
    LikeCase {
        label: "ASCII case-sensitive lower",
        search: "needle",
        expected_c_ids: &[PRODUCT_NEEDLE_LOWER],
    },
    LikeCase {
        label: "Unicode NFC remains byte-distinct",
        search: "café",
        expected_c_ids: &[PRODUCT_CAFE_NFC],
    },
    LikeCase {
        label: "Unicode NFD remains byte-distinct",
        search: "cafe\u{301}",
        expected_c_ids: &[PRODUCT_CAFE_NFD],
    },
    LikeCase {
        label: "underscore wildcard",
        search: "A_B",
        expected_c_ids: &[PRODUCT_UNDERSCORE, PRODUCT_UNDERSCORE_WILDCARD],
    },
    LikeCase {
        label: "escaped underscore literal",
        search: r"A\_B",
        expected_c_ids: &[PRODUCT_UNDERSCORE],
    },
    LikeCase {
        label: "percent wildcard",
        search: "100%",
        expected_c_ids: &[PRODUCT_PERCENT, PRODUCT_PERCENT_WILDCARD],
    },
    LikeCase {
        label: "escaped percent literal",
        search: r"100\%",
        expected_c_ids: &[PRODUCT_PERCENT],
    },
    LikeCase {
        label: "sharp-s remains distinct from ASCII SS",
        search: "straße",
        expected_c_ids: &[PRODUCT_STRASSE_SHARP_S],
    },
    LikeCase {
        label: "ASCII SS remains distinct from sharp-s",
        search: "STRASSE",
        expected_c_ids: &[PRODUCT_STRASSE_ASCII],
    },
];

#[tokio::test]
async fn product_storefront_default_like_matches_index_c_collation_matrix() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_collation_matrix(&database.query).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_collation_matrix(db: &DatabaseConnection) -> TestResult<()> {
    let lc_collate = current_lc_collate(db).await?;
    for case in CASES {
        let owner_ids = title_like_ids(db, case.search, false).await?;
        let index_c_ids = title_like_ids(db, case.search, true).await?;
        assert_eq!(
            index_c_ids, case.expected_c_ids,
            "retained C-collation expectation changed for {}",
            case.label
        );
        if owner_ids != index_c_ids {
            return Err(std::io::Error::other(format!(
                "Product Storefront title LIKE collation mismatch for {} under lc_collate={lc_collate:?}: owner-default={owner_ids:?}, index-C={index_c_ids:?}",
                case.label,
            ))
            .into());
        }
    }
    Ok(())
}

async fn title_like_ids(
    db: &DatabaseConnection,
    search: &str,
    explicit_c_collation: bool,
) -> TestResult<Vec<Uuid>> {
    let pattern = format!("%{search}%");
    let sql = if explicit_c_collation {
        r#"
SELECT translation.product_id
FROM product_translations translation
WHERE translation.tenant_id = $1
  AND (translation.title COLLATE "C") LIKE $2 ESCAPE E'\\'
ORDER BY translation.product_id ASC
"#
    } else {
        // Mirrors Product owner `product_title_search_condition`: no explicit collation and no explicit
        // ESCAPE clause. PostgreSQL's default LIKE escape remains backslash.
        r#"
SELECT translation.product_id
FROM product_translations translation
WHERE translation.tenant_id = $1
  AND translation.title LIKE $2
ORDER BY translation.product_id ASC
"#
    };
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![TENANT_ID.into(), pattern.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| row.try_get("", "product_id").map_err(Into::into))
        .collect()
}

async fn current_lc_collate(db: &DatabaseConnection) -> TestResult<String> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT current_setting('lc_collate') AS lc_collate",
            Vec::<Value>::new(),
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("PostgreSQL lc_collate probe returned no row"))?;
    Ok(row.try_get("", "lc_collate")?)
}

async fn create_product_migration_prerequisites(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(
        r#"
CREATE TABLE tenants (
    id UUID PRIMARY KEY
);
CREATE TABLE taxonomy_terms (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    UNIQUE (tenant_id, id)
);
CREATE TABLE channel_index_identity_generations (
    tenant_id UUID PRIMARY KEY,
    generation BIGINT NOT NULL CHECK (generation > 0)
);
"#,
    )
    .await?;
    let manager = SchemaManager::new(db);
    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    Ok(())
}

async fn seed_titles(db: &DatabaseConnection) -> TestResult<()> {
    let products = [
        (PRODUCT_NEEDLE_UPPER, "Needle ASCII", "needle-upper"),
        (PRODUCT_NEEDLE_LOWER, "needle ascii", "needle-lower"),
        (PRODUCT_CAFE_NFC, "café", "cafe-nfc"),
        (PRODUCT_CAFE_NFD, "cafe\u{301}", "cafe-nfd"),
        (PRODUCT_UNDERSCORE, "A_B", "underscore-literal"),
        (PRODUCT_UNDERSCORE_WILDCARD, "AxB", "underscore-wildcard"),
        (PRODUCT_PERCENT, "100% real", "percent-literal"),
        (
            PRODUCT_PERCENT_WILDCARD,
            "100 percent real",
            "percent-wildcard",
        ),
        (PRODUCT_STRASSE_SHARP_S, "straße", "strasse-sharp-s"),
        (PRODUCT_STRASSE_ASCII, "STRASSE", "strasse-ascii"),
    ];

    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{TENANT_ID}')"))
        .await?;
    for (offset, (product_id, title, handle)) in products.into_iter().enumerate() {
        let translation_id = Uuid::from_u128(0x7300_u128 + offset as u128 + 1);
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO products (id, tenant_id) VALUES ($1, $2)",
            vec![product_id.into(), TENANT_ID.into()],
        ))
        .await?;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES ($1, $2, $3, 'en', $4, $5)",
            vec![
                translation_id.into(),
                product_id.into(),
                TENANT_ID.into(),
                title.to_owned().into(),
                handle.to_owned().into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
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

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
    application_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    db.execute_unprepared(&format!("SET application_name TO '{application_name}'"))
        .await?;
    Ok(db)
}

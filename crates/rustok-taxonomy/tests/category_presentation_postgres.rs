use std::{env, error::Error as StdError, time::Duration};

use rustok_core::{SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, SetTaxonomyCategoryPresentationInput, TaxonomyError,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_TAXONOMY_TEST_DATABASE_URL";

type TestResult<T> = Result<T, Box<dyn StdError + Send + Sync>>;

#[tokio::test]
async fn postgres_category_presentation_guard_and_cas_fail_closed() -> TestResult<()> {
    let Some(database_url) = postgres_database_url() else {
        eprintln!(
            "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Taxonomy Category presentation PostgreSQL evidence"
        );
        return Ok(());
    };

    let db = connect(&database_url).await?;
    ensure_category_presentation_schema(&db).await?;
    let tenant_id = Uuid::new_v4();
    let category_id = create_term(
        &db,
        tenant_id,
        TaxonomyTermKind::Category,
        "Presentation Category",
    )
    .await?;
    let tag_id = create_term(&db, tenant_id, TaxonomyTermKind::Tag, "Presentation Tag").await?;

    let tag_insert = db
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "INSERT INTO taxonomy_category_presentations (tenant_id, term_id, icon_key, revision) VALUES ('{tenant_id}'::uuid, '{tag_id}'::uuid, 'tag', 1)"
            ),
        ))
        .await
        .expect_err("PostgreSQL guard must reject Tag presentation rows");
    assert!(
        tag_insert.to_string().contains("not a Category"),
        "Tag rejection must come from the Category presentation storage guard: {tag_insert}"
    );

    let zero_revision = db
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "INSERT INTO taxonomy_category_presentations (tenant_id, term_id, icon_key, revision) VALUES ('{tenant_id}'::uuid, '{category_id}'::uuid, 'message-square', 0)"
            ),
        ))
        .await
        .expect_err("PostgreSQL guard must reject non-positive revisions");
    assert!(
        zero_revision
            .to_string()
            .contains("revision must be positive"),
        "non-positive revision rejection must come from the storage guard: {zero_revision}"
    );

    db.execute_raw(Statement::from_string(
        DbBackend::Postgres,
        format!(
            "INSERT INTO taxonomy_category_presentations (tenant_id, term_id, icon_key, color, revision) VALUES ('{tenant_id}'::uuid, '{category_id}'::uuid, 'message-square', '#112233', 1)"
        ),
    ))
    .await?;

    let skipped_revision = db
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "UPDATE taxonomy_category_presentations SET color = '#445566', revision = 3 WHERE tenant_id = '{tenant_id}'::uuid AND term_id = '{category_id}'::uuid"
            ),
        ))
        .await
        .expect_err("PostgreSQL guard must reject skipped revisions");
    assert!(
        skipped_revision
            .to_string()
            .contains("advance revision by exactly one"),
        "skipped revision rejection must come from the storage guard: {skipped_revision}"
    );

    let service_a = TaxonomyService::new(connect(&database_url).await?);
    let service_b = TaxonomyService::new(connect(&database_url).await?);
    let left = service_a.set_category_presentation(
        tenant_id,
        admin(),
        category_id,
        SetTaxonomyCategoryPresentationInput {
            icon_key: Some("message-square".to_string()),
            color: Some("#aabbcc".to_string()),
            image_media_id: None,
            cover_media_id: None,
            expected_revision: Some(1),
        },
        None,
    );
    let right = service_b.set_category_presentation(
        tenant_id,
        admin(),
        category_id,
        SetTaxonomyCategoryPresentationInput {
            icon_key: Some("message-square".to_string()),
            color: Some("#ddeeff".to_string()),
            image_media_id: None,
            cover_media_id: None,
            expected_revision: Some(1),
        },
        None,
    );
    let (left, right) = tokio::join!(left, right);
    let success_count = usize::from(left.is_ok()) + usize::from(right.is_ok());
    assert_eq!(
        success_count, 1,
        "same-revision Category presentation writers must admit exactly one commit; left={left:?} right={right:?}"
    );
    let loser = left
        .as_ref()
        .err()
        .or_else(|| right.as_ref().err())
        .expect("one same-revision writer must lose");
    assert!(
        matches!(loser, TaxonomyError::Conflict(_)),
        "losing presentation writer must fail with Taxonomy conflict: {loser}"
    );

    let current = TaxonomyService::new(db)
        .get_category_presentation(tenant_id, admin(), category_id)
        .await?;
    assert_eq!(current.revision, 2);
    assert!(
        matches!(current.color.as_deref(), Some("#aabbcc") | Some("#ddeeff")),
        "winning presentation value must be retained: {current:?}"
    );

    Ok(())
}

fn postgres_database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .ok()
        .filter(|value| value.starts_with("postgres://") || value.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_string());
    options
        .max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10));
    Ok(Database::connect(options).await?)
}

async fn ensure_category_presentation_schema(db: &DatabaseConnection) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('taxonomy_category_presentations') IS NOT NULL AS present"
                .to_string(),
        ))
        .await?
        .ok_or("PostgreSQL schema probe returned no row")?;
    let present: bool = row.try_get("", "present")?;
    if !present {
        return Err(
            "canonical server migrations did not create taxonomy_category_presentations".into(),
        );
    }
    Ok(())
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_term(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    kind: TaxonomyTermKind,
    name: &str,
) -> TestResult<Uuid> {
    let service = TaxonomyService::new(db.clone());
    Ok(service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: name.to_string(),
                slug: None,
                canonical_key: Some(format!("presentation-{}", Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await?)
}

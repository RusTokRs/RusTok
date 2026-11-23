mod support;

use std::{path::Path, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::MigrationSource;
use rustok_migrations::{
    Migrator, apply_installer_owner_schema, apply_installer_remaining_schema,
    execution::{ExecutionError, ExecutionPhase, StepRunner},
    verification::{DatabaseSnapshot, compare, observe_database, validate_journal},
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_product::{
    ProductCatalogReadPort, ProductCatalogReadRuntime, ProductProjectionRequest,
    PublishedProductsRequest, VariantProductProjectionRequest,
};
use rustok_test_utils::{
    assert_postgres_column_contract as assert_column_contract,
    assert_postgres_column_missing as assert_column_missing,
    assert_postgres_table_missing as assert_table_missing, assert_postgres_url,
    assert_valid_postgres_database_name as assert_valid_database_name, connect_postgres,
    create_postgres_database as create_database,
    drop_postgres_database_if_exists as drop_database_if_exists,
    postgres_database_url as database_url_from_admin_url, unique_postgres_database_name,
};
use sea_orm_migration::{
    MigrationTrait, SchemaManager,
    prelude::MigratorTrait,
    sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait},
};
use support::backfill_fixtures::{
    BackfillFixture, apply_setup as apply_backfill_setup,
    assert_results as assert_backfill_results, load_contract_fixtures,
    load_from_environment as load_backfill_fixtures, validate_pending as validate_backfill_targets,
};

#[tokio::test]
#[ignore = "requires PostgreSQL admin access; run scripts/verify/verify-migration-smoke.sh"]
async fn postgres_zero_migration_smoke_applies_from_empty_database() {
    if let Err(error) = run_postgres_zero_migration_smoke().await {
        panic!("PostgreSQL migration smoke failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RUSTOK_MIGRATION_SMOKE_ADMIN_URL"]
async fn postgres_installer_schema_preserves_the_owner_boundary()
-> Result<(), Box<dyn std::error::Error>> {
    const FENCING: &str = "m20261118_000002_add_installer_attempt_fencing";
    let admin_url = smoke_admin_url()?;
    let name = unique_postgres_database_name("rustok_installer_schema");
    let url = database_url_from_admin_url(&admin_url, &name);
    let admin = connect_postgres(&admin_url).await?;
    create_database(&admin, &name).await?;
    let result = async {
        let db = connect_postgres(&url).await?;
        let owner_plan = [
            "m20250101_000001_create_tenants",
            "m20260403_000061_create_registry_publish_tables",
            "m20260426_000001_create_install_sessions",
            "m20260722_000034_static_promotions",
            FENCING,
        ]
        .map(str::to_owned);
        for _ in 0..2 {
            apply_installer_owner_schema(&db).await?;
            assert_migration_journal(&db, &owner_plan).await?;
        }
        let manager = SchemaManager::new(&db);
        if !manager
            .has_table("module_static_distribution_release_state")
            .await?
            || !manager
                .has_column("install_sessions", "attempt_generation")
                .await?
            || !manager
                .has_column("install_step_receipts", "attempt_generation")
                .await?
        {
            return Err("installer import prerequisites are missing".into());
        }
        assert_table_missing(&db, "users").await?;
        assert_table_missing(&db, "blog_posts").await?;

        apply_installer_remaining_schema(&db).await?;
        let complete_plan = Migrator::inventory()?;
        assert_migration_journal(&db, complete_plan.migrations()).await?;
        if !manager.has_table("blog_posts").await? {
            return Err("remaining installer schema did not include Blog".into());
        }

        let runner = StepRunner::connect(&url, Migrator::migrations()).await?;
        let blocked = apply_installer_owner_schema(&db).await;
        runner.close().await?;
        if !blocked.is_err_and(|error| error.to_string().contains("migration writer")) {
            return Err("installer bootstrap must respect the shared migration lock".into());
        }

        // Exercise the production rollback for fencing while retaining the other
        // compiled journal entries that an existing database already applied.
        let transaction = db.begin().await?;
        let fencing = Migrator::migrations()
            .into_iter()
            .find(|migration| migration.name() == FENCING)
            .ok_or("fencing migration missing")?;
        fencing.down(&SchemaManager::new(&transaction)).await?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM seaql_migrations WHERE version = $1",
                [FENCING.into()],
            ))
            .await?;
        transaction.commit().await?;

        db.execute_unprepared(
            "ALTER TABLE install_step_receipts ADD COLUMN attempt_generation bigint",
        )
        .await?;
        if apply_installer_owner_schema(&db).await.is_ok()
            || manager
                .has_column("install_sessions", "attempt_generation")
                .await?
        {
            return Err(
                "failed attempt fencing must roll back both installer schema changes".into(),
            );
        }
        db.execute_unprepared("ALTER TABLE install_step_receipts DROP COLUMN attempt_generation")
            .await?;
        apply_installer_owner_schema(&db).await?;
        assert_migration_journal(&db, complete_plan.migrations()).await?;
        if !manager
            .has_column("install_sessions", "attempt_generation")
            .await?
            || !manager
                .has_column("install_step_receipts", "attempt_generation")
                .await?
        {
            return Err("installer retry did not restore fencing columns".into());
        }
        db.execute_unprepared(
            "INSERT INTO seaql_migrations VALUES ('unknown_installer_fixture', 0)",
        )
        .await?;
        if !apply_installer_owner_schema(&db)
            .await
            .is_err_and(|error| error.to_string().contains("unknown_installer_fixture"))
        {
            return Err("a complete bootstrap must still reject unknown journal identities".into());
        }
        db.close().await?;
        Ok(())
    }
    .await;
    finish_fixture(&admin, &name, result).await
}

async fn assert_migration_journal(
    db: &DatabaseConnection,
    plan: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let versions = Migrator::get_migration_models(db)
        .await?
        .into_iter()
        .map(|row| row.version)
        .collect::<Vec<_>>();
    Ok(validate_journal(&versions, plan)?)
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RUSTOK_MIGRATION_SMOKE_ADMIN_URL"]
async fn postgres_blog_rollback_preserves_translation_data()
-> Result<(), Box<dyn std::error::Error>> {
    const WIDTH: &str = "m20261030_000001_widen_blog_translation_columns";
    const INDEXES: &str = "m20261030_000002_add_blog_list_indexes";
    let inventory = Migrator::inventory()?;
    let migrations = inventory.migrations();
    let width = migrations
        .iter()
        .position(|name| name == WIDTH)
        .ok_or("Blog width step missing")?;
    if migrations.get(width + 1).map(String::as_str) != Some(INDEXES) {
        return Err("expected the Blog index step after the width step".into());
    }

    let admin_url = smoke_admin_url()?;
    let name = unique_postgres_database_name("rustok_blog_rollback");
    let url = database_url_from_admin_url(&admin_url, &name);
    let admin = connect_postgres(&admin_url).await?;
    create_database(&admin, &name).await?;
    let result = async {
        let prefix = ExecutionPhase {
            expected_prefix: Vec::new(),
            steps: migrations[..width + 2].to_vec(),
        };
        apply_verified_phase_steps(&url, &prefix, &prefix.steps, &[]).await?;
        let db = connect_postgres(&url).await?;
        db.execute_unprepared(
            "INSERT INTO tenants (id, name, slug) VALUES \
                ('ce020000-0000-0000-0000-000000000001', 'Blog rollback fixture', 'blog-rollback-fixture'); \
             INSERT INTO blog_posts (id, tenant_id, author_id, slug) VALUES \
                ('ce020000-0000-0000-0000-000000000002', 'ce020000-0000-0000-0000-000000000001', \
                 'ce020000-0000-0000-0000-000000000003', 'rollback-fixture'); \
             INSERT INTO blog_post_translations (id, post_id, locale, title, seo_description, body) VALUES \
                ('ce020000-0000-0000-0000-000000000004', 'ce020000-0000-0000-0000-000000000002', \
                 'zh-Hant-TW-variant1', repeat('T', 512), repeat('D', 1000), 'fixture body')",
        ).await?;

        Migrator::down(&db, Some(1)).await?;
        assert_migration_journal(&db, &migrations[..width + 1]).await?;
        assert_blog_rollback_state(&db, "32,1000,512|512|1000|zh-Hant-TW-variant1|fixture body|false|true").await?;

        if Migrator::down(&db, Some(1)).await.is_ok() {
            return Err("Blog rollback accepted a title wider than the old column".into());
        }
        assert_migration_journal(&db, &migrations[..width + 1]).await?;
        assert_blog_rollback_state(&db, "32,1000,512|512|1000|zh-Hant-TW-variant1|fixture body|false|true").await?;

        db.execute_unprepared(
            "UPDATE blog_post_translations SET title = repeat('T', 255), seo_description = repeat('D', 500) \
             WHERE id = 'ce020000-0000-0000-0000-000000000004'",
        ).await?;
        Migrator::down(&db, Some(1)).await?;
        assert_migration_journal(&db, &migrations[..width]).await?;
        assert_blog_rollback_state(&db, "32,500,255|255|500|zh-Hant-TW-variant1|fixture body|false|true").await?;

        let phase = ExecutionPhase {
            expected_prefix: migrations[..width].to_vec(),
            steps: migrations[width..width + 2].to_vec(),
        };
        apply_verified_phase_steps(&url, &phase, &phase.steps, &[]).await?;
        assert_migration_journal(&db, &migrations[..width + 2]).await?;
        assert_blog_rollback_state(&db, "32,1000,512|255|500|zh-Hant-TW-variant1|fixture body|true|false").await?;
        Ok(db.close().await?)
    }
    .await;
    finish_fixture(&admin, &name, result).await
}

async fn assert_blog_rollback_state(
    db: &DatabaseConnection,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT concat_ws('|', \
            (SELECT string_agg(character_maximum_length::text, ',' ORDER BY column_name) \
             FROM information_schema.columns WHERE table_schema = 'public' \
               AND table_name = 'blog_post_translations' AND column_name IN ('locale', 'seo_description', 'title')), \
            char_length(title), char_length(seo_description), locale, body, \
            (to_regclass('public.idx_blog_posts_public_keyset') IS NOT NULL)::text, \
            (to_regclass('public.idx_blog_preview_tokens_token') IS NOT NULL)::text) AS state \
         FROM blog_post_translations WHERE id = 'ce020000-0000-0000-0000-000000000004'",
    )).await?.ok_or("missing Blog rollback translation")?;
    let actual: String = row.try_get("", "state")?;
    if actual != expected {
        return Err(
            format!("Blog rollback state differs: expected {expected}, got {actual}").into(),
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RUSTOK_MIGRATION_SMOKE_ADMIN_URL"]
async fn postgres_backfill_assertion_stops_at_its_boundary()
-> Result<(), Box<dyn std::error::Error>> {
    let admin_url = smoke_admin_url()?;
    let name = unique_postgres_database_name("rustok_backfill_boundary");
    let url = database_url_from_admin_url(&admin_url, &name);
    let admin = connect_postgres(&admin_url).await?;
    create_database(&admin, &name).await?;
    let result = async {
        let target = "m20260726_000001_enforce_tenant_locale_policy";
        let fixture = load_contract_fixtures(&[target])?
            .pop()
            .ok_or("missing tenant locale backfill fixture")?;
        let existing_locales = BackfillFixture {
            id: "tenant-existing-locales".into(),
            migration: target.into(),
            setup_sql: r#"
                INSERT INTO tenants (id, name, slug, default_locale)
                VALUES ('33000000-0000-0000-0000-000000000002', 'Localized tenant', 'localized-tenant-fixture', 'fr');
                INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale)
                VALUES
                    ('33000000-0000-0000-0000-000000000011', '33000000-0000-0000-0000-000000000002', 'fr', 'French', 'Francais', true, true, NULL),
                    ('33000000-0000-0000-0000-000000000012', '33000000-0000-0000-0000-000000000002', 'fr-CA', 'Canadian French', 'Francais canadien', false, false, 'fr');
            "#.into(),
            assertion_sql: r#"
                SELECT (COUNT(*) = 2 AND COUNT(*) FILTER (WHERE policy_revision = 0) = 2
                    AND COUNT(*) FILTER (WHERE id = '33000000-0000-0000-0000-000000000011'
                        AND locale = 'fr' AND is_default AND is_enabled AND fallback_locale IS NULL) = 1
                    AND COUNT(*) FILTER (WHERE id = '33000000-0000-0000-0000-000000000012'
                        AND locale = 'fr-CA' AND NOT is_default AND NOT is_enabled AND fallback_locale = 'fr') = 1
                ) AS passed
                FROM tenant_locales WHERE tenant_id = '33000000-0000-0000-0000-000000000002';
            "#.into(),
        };
        let plan = Migrator::migrations();
        let index = plan
            .iter()
            .position(|migration| migration.name() == target)
            .ok_or("fixture target missing from composed plan")?;
        let following = plan
            .get(index + 1)
            .ok_or("fixture needs a following step")?
            .name()
            .to_owned();
        let mut runner = StepRunner::connect(&url, plan).await?;
        let prefix = async {
            while runner.pending_migrations()?.next() != Some(target) {
                if runner.apply_next().await?.is_none() {
                    return Err("fixture boundary was not reached".into());
                }
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        }
        .await;
        runner.close().await?;
        prefix?;

        let mut rejected = existing_locales.clone();
        rejected.assertion_sql = "SELECT false AS passed".into();
        let error = apply_migrations_and_assert_schema(&url, true, false, &[fixture.clone(), rejected])
            .await
            .err()
            .ok_or("a failing second boundary assertion was accepted")?;
        if !error
            .to_string()
            .contains(&format!("{} failed after migration {target}", existing_locales.id))
        {
            return Err(format!("unexpected boundary failure: {error}").into());
        }
        let fixtures = [fixture, existing_locales];
        let db = connect_postgres(&url).await?;
        let check = async {
            if !migration_is_applied(&db, target).await?
                || migration_is_applied(&db, &following).await?
            {
                return Err("executor advanced beyond the failing fixture boundary".into());
            }
            assert_backfill_results(&db, &fixtures).await?;
            // A completed target cannot silently count as pending fixture coverage.
            if apply_migrations_and_assert_schema(&url, true, false, &fixtures)
                .await
                .is_ok()
            {
                return Err("already-applied fixture target was accepted".into());
            }
            apply_migrations_and_assert_schema(&url, true, false, &[]).await?;
            assert_backfill_results(&db, &fixtures).await?;
            Ok::<_, Box<dyn std::error::Error>>(())
        }
        .await;
        db.close().await?;
        check
    }
    .await;
    finish_fixture(&admin, &name, result).await
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL, RUSTOK_MIGRATION_SMOKE_ADMIN_URL and RUSTOK_MIGRATION_VERIFY_TEST_PG_DUMP"]
async fn postgres_seeded_module_phase_matches_after_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    let admin_url = smoke_admin_url()?;
    let pg_dump = std::env::var("RUSTOK_MIGRATION_VERIFY_TEST_PG_DUMP")?;
    let pg_dump = Path::new(&pg_dump);
    let steps = [
        "m20261027_000081_transition_intervention_required_state",
        "m20261027_000082_gc_candidate_claims",
        "m20261027_000083_artifact_permission_registration_status",
        "m20261027_000084_module_operation_result_snapshot",
        ARTIFACT_UI_BACKFILL,
    ];
    let fixtures = load_contract_fixtures(&[steps[0], steps[2], steps[4]])?;
    let plan = Migrator::inventory()?;
    let start = plan
        .migrations()
        .iter()
        .position(|name| name == steps[0])
        .ok_or("missing first module phase migration")?;
    let phase = ExecutionPhase {
        expected_prefix: plan.migrations()[..start].to_vec(),
        steps: steps.map(str::to_owned).to_vec(),
    };
    let completed: Vec<_> = phase
        .expected_prefix
        .iter()
        .chain(&phase.steps)
        .cloned()
        .collect();
    let admin = connect_postgres(&admin_url).await?;
    let predecessor = unique_postgres_database_name("rustok_module_phase_predecessor");
    let reference = unique_postgres_database_name("rustok_module_phase_reference");
    let resumed = unique_postgres_database_name("rustok_module_phase_resumed");
    let mut created = Vec::new();

    let result = async {
        create_database(&admin, &predecessor).await?;
        created.push(predecessor.as_str());
        let predecessor_url = database_url_from_admin_url(&admin_url, &predecessor);
        let prefix = ExecutionPhase {
            expected_prefix: Vec::new(),
            steps: phase.expected_prefix.clone(),
        };
        apply_verified_phase_steps(&predecessor_url, &prefix, &prefix.steps, &[]).await?;
        let db = connect_postgres(&predecessor_url).await?;
        let seeded = apply_backfill_setup(&db, &fixtures).await;
        db.close().await?;
        seeded?;

        // Copy the same committed rows, clocks, sequences and journal into both
        // fixtures after closing every connection to the predecessor.
        for name in [&reference, &resumed] {
            assert_valid_database_name(name);
            admin
                .execute_unprepared(&format!(
                    "CREATE DATABASE \"{name}\" TEMPLATE \"{predecessor}\""
                ))
                .await?;
            created.push(name.as_str());
        }
        let reference_url = database_url_from_admin_url(&admin_url, &reference);
        let resumed_url = database_url_from_admin_url(&admin_url, &resumed);
        apply_verified_phase_steps(&reference_url, &phase, &phase.steps, &fixtures).await?;
        let (final_step, preceding_steps) = phase.steps.split_last().ok_or("empty phase")?;
        for step in preceding_steps {
            apply_verified_phase_steps(&resumed_url, &phase, std::slice::from_ref(step), &fixtures)
                .await?;
        }

        assert_failed_ui_backfill_rolls_back(
            &resumed_url,
            pg_dump,
            &phase,
            &completed[..completed.len() - 1],
        )
        .await?;
        apply_verified_phase_steps(
            &resumed_url,
            &phase,
            std::slice::from_ref(final_step),
            &fixtures,
        )
        .await?;

        let original = observe_backfill_fixture(&reference_url, pg_dump).await?;
        let recovered = observe_backfill_fixture(&resumed_url, pg_dump).await?;
        let comparison = compare(&original, &recovered, &completed, &completed)?;
        if !comparison.equivalent || comparison.reference_digest != comparison.candidate_digest {
            return Err(format!(
                "module phase recovery diverged: {:?}",
                comparison.differences
            )
            .into());
        }
        Ok(())
    }
    .await;

    let mut result = result;
    for name in created.into_iter().rev() {
        result = finish_fixture(&admin, name, result).await;
    }
    result
}

const ARTIFACT_UI_BACKFILL: &str = "m20261027_000085_artifact_ui_resources";

async fn assert_failed_ui_backfill_rolls_back(
    url: &str,
    pg_dump: &Path,
    phase: &ExecutionPhase,
    applied_prefix: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    // Fail the journal INSERT after the real migration creates and populates the
    // projection, without changing the migration or its input data.
    let db = connect_postgres(url).await?;
    db.execute_unprepared(&format!(
        "ALTER TABLE seaql_migrations ADD CONSTRAINT fixture_reject_ui_backfill \
         CHECK (version <> '{ARTIFACT_UI_BACKFILL}')"
    ))
    .await?;
    let before = observe_database(&db, url, pg_dump).await?;
    validate_journal(&before.migration_versions, applied_prefix)?;

    let mut runner =
        StepRunner::connect_phase(url, Migrator::inventory()?.into_migrations(), phase.clone())
            .await?;
    let result = runner.apply_next().await;
    let retry = runner.apply_next().await;
    runner.close().await?;
    match result {
        Err(ExecutionError::Migration { name, source })
            if name == ARTIFACT_UI_BACKFILL
                && source.to_string().contains("fixture_reject_ui_backfill") => {}
        result => {
            return Err(format!(
                "expected journal rejection for {ARTIFACT_UI_BACKFILL}, got {result:?}"
            )
            .into());
        }
    }
    if !matches!(retry, Err(ExecutionError::Interrupted)) {
        return Err(format!("failed runner allowed another step: {retry:?}").into());
    }

    assert_table_missing(&db, "module_artifact_ui_resources").await?;
    let after = observe_database(&db, url, pg_dump).await?;
    let rollback = compare(&before, &after, applied_prefix, applied_prefix)?;
    if !rollback.equivalent || rollback.reference_digest != rollback.candidate_digest {
        return Err(format!(
            "failed UI backfill changed the predecessor: {:?}",
            rollback.differences
        )
        .into());
    }
    db.execute_unprepared(
        "ALTER TABLE seaql_migrations DROP CONSTRAINT fixture_reject_ui_backfill",
    )
    .await?;
    Ok(db.close().await?)
}

async fn apply_verified_phase_steps(
    url: &str,
    phase: &ExecutionPhase,
    steps: &[String],
    fixtures: &[BackfillFixture],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut runner =
        StepRunner::connect_phase(url, Migrator::inventory()?.into_migrations(), phase.clone())
            .await?;
    let result = async {
        for expected in steps {
            let applied = runner.apply_next().await?;
            if applied.as_deref() != Some(expected) {
                return Err(format!("expected phase step {expected}, applied {applied:?}").into());
            }
            for fixture in fixtures
                .iter()
                .filter(|fixture| &fixture.migration == expected)
            {
                assert_backfill_results(runner.database(), std::slice::from_ref(fixture)).await?;
            }
        }
        if steps.last() == phase.steps.last() && runner.apply_next().await?.is_some() {
            return Err("execution advanced past the requested phase".into());
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    }
    .await;
    let close = runner.close().await;
    result?;
    Ok(close?)
}

async fn observe_backfill_fixture(
    url: &str,
    pg_dump: &Path,
) -> Result<DatabaseSnapshot, Box<dyn std::error::Error>> {
    let db = connect_postgres(url).await?;
    let snapshot = observe_database(&db, url, pg_dump).await;
    db.close().await?;
    Ok(snapshot?)
}

struct ProductMigrator;

#[async_trait::async_trait]
impl MigratorTrait for ProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_product::migrations::migrations()
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn product_postgres_migrations_apply_and_replay_latest() {
    if let Err(error) = run_product_postgres_migration_lifecycle().await {
        panic!("Product PostgreSQL migration lifecycle failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn product_postgres_constraints_reject_invalid_and_racing_writes() {
    if let Err(error) = run_product_postgres_constraint_checks().await {
        panic!("Product PostgreSQL constraint checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn product_tenant_integrity_migration_rejects_dirty_data_and_maps_inventory() {
    if let Err(error) = run_product_dirty_data_migration_checks().await {
        panic!("Product dirty-data migration checks failed: {error}");
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL admin access"]
async fn product_catalog_read_port_executes_against_postgres() {
    if let Err(error) = run_product_catalog_read_port_checks().await {
        panic!("Product catalog read-port PostgreSQL checks failed: {error}");
    }
}

async fn run_product_postgres_migration_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let admin_url = smoke_admin_url()?;

    let database_name = format!("{}_product", default_database_name());
    assert_valid_database_name(&database_name);
    let target_url = database_url_from_admin_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("admin database must be reachable: {error}"))?;

    create_database(&admin, &database_name).await?;

    let lifecycle_result = async {
        let db = connect_postgres(&target_url).await?;
        create_product_migration_prerequisites(&db).await?;

        ProductMigrator::up(&db, None)
            .await
            .map_err(|error| format!("initial product migration up failed: {error}"))?;
        assert_product_migration_schema(&db).await?;

        let latest = ProductMigrator::migrations()
            .pop()
            .ok_or("Product migration plan must not be empty")?;
        ProductMigrator::down(&db, Some(1))
            .await
            .map_err(|error| format!("latest Product migration down failed: {error}"))?;
        let pending = ProductMigrator::get_pending_migrations(&db).await?;
        if pending.len() != 1 || pending[0].name() != latest.name() {
            return Err(
                format!("Product rollback must leave only {} pending", latest.name()).into(),
            );
        }

        ProductMigrator::up(&db, Some(1))
            .await
            .map_err(|error| format!("second product migration up failed: {error}"))?;
        assert_product_migration_schema(&db).await
    }
    .await;

    finish_fixture(&admin, &database_name, lifecycle_result).await
}

async fn run_product_postgres_constraint_checks() -> Result<(), Box<dyn std::error::Error>> {
    let admin_url = smoke_admin_url()?;

    let database_name = format!("{}_product_constraints", default_database_name());
    assert_valid_database_name(&database_name);
    let target_url = database_url_from_admin_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("admin database must be reachable: {error}"))?;

    create_database(&admin, &database_name).await?;

    let constraint_result = async {
        let db = connect_postgres(&target_url).await?;
        create_product_migration_prerequisites(&db).await?;
        // The category-tree SQL below exercises the retained predecessor. Taxonomy owns
        // the live hierarchy after Product's category cutover.
        let tree_boundary = ProductMigrator::migrations()
            .iter()
            .position(|migration| {
                migration.name() == "m20260725_000002_enforce_catalog_category_tree_invariants"
            })
            .ok_or("missing Product category-tree predecessor")?;
        ProductMigrator::up(&db, Some(u32::try_from(tree_boundary + 1)?))
            .await
            .map_err(|error| format!("product predecessor migration up failed: {error}"))?;
        seed_product_constraint_fixtures(&db).await?;

        assert_exactly_one_concurrent_write_succeeds(
            &db,
            r#"
INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle)
VALUES (
    '00000000-0000-0000-0000-000000000111',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000001',
    'en',
    'First racing product',
    'shared-handle'
)
"#,
            r#"
INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle)
VALUES (
    '00000000-0000-0000-0000-000000000112',
    '00000000-0000-0000-0000-000000000102',
    '00000000-0000-0000-0000-000000000001',
    'en',
    'Second racing product',
    'shared-handle'
)
"#,
            "uq_product_translations_tenant_locale_handle",
        )
        .await?;

        assert_exactly_one_concurrent_write_succeeds(
            &db,
            r#"
INSERT INTO product_variants (id, product_id, tenant_id, sku)
VALUES (
    '00000000-0000-0000-0000-000000000121',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000001',
    'SHARED-SKU'
)
"#,
            r#"
INSERT INTO product_variants (id, product_id, tenant_id, sku)
VALUES (
    '00000000-0000-0000-0000-000000000122',
    '00000000-0000-0000-0000-000000000102',
    '00000000-0000-0000-0000-000000000001',
    'SHARED-SKU'
)
"#,
            "uq_product_variants_tenant_sku",
        )
        .await?;

        assert_statement_rejected(
            &db,
            r#"
INSERT INTO product_tags (product_id, term_id, tenant_id)
VALUES (
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000201',
    '00000000-0000-0000-0000-000000000001'
)
"#,
            "fk_product_tags_term_tenant",
        )
        .await?;

        assert_statement_rejected(
            &db,
            r#"
INSERT INTO product_attribute_values (
    id, tenant_id, product_id, attribute_id, value_text, value_integer
)
VALUES (
    '00000000-0000-0000-0000-000000000401',
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000301',
    'not an integer',
    42
)
"#,
            "stored product attribute value does not match attribute type integer",
        )
        .await?;

        assert_statement_rejected(
            &db,
            r#"
INSERT INTO product_categories (tenant_id, product_id, category_id, assignment_kind)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000501',
    'primary'
)
"#,
            "chk_product_categories_no_primary_assignment",
        )
        .await?;

        assert_statement_rejected(
            &db,
            r#"
INSERT INTO catalog_categories (id, tenant_id, code, slug, path)
VALUES (
    '00000000-0000-0000-0000-000000000502',
    '00000000-0000-0000-0000-000000000001',
    'duplicate-root',
    'catalog',
    '/duplicate-root'
)
"#,
            "uq_catalog_categories_tenant_root_slug",
        )
        .await?;

        assert_category_cycle_rejected(&db).await?;
        assert_category_closure_drift_rejected(&db).await?;

        db.close().await?;
        Ok(())
    }
    .await;

    finish_fixture(&admin, &database_name, constraint_result).await
}

async fn run_product_dirty_data_migration_checks() -> Result<(), Box<dyn std::error::Error>> {
    let admin_url = smoke_admin_url()?;

    let database_name = format!("{}_product_dirty_data", default_database_name());
    assert_valid_database_name(&database_name);
    let target_url = database_url_from_admin_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("admin database must be reachable: {error}"))?;

    create_database(&admin, &database_name).await?;

    let migration_result = async {
        let db = connect_postgres(&target_url).await?;
        create_product_migration_prerequisites(&db).await?;

        // Apply through m20260711_000001_product_status_enum, immediately before
        // m20260711_000002_enforce_product_tenant_integrity.
        ProductMigrator::up(&db, Some(15))
            .await
            .map_err(|error| format!("product migration prefix failed: {error}"))?;
        seed_product_dirty_data_fixtures(&db).await?;

        assert_next_product_migration_rejected(&db, "uq_product_translations_tenant_locale_handle")
            .await?;
        db.execute_unprepared(
            "DELETE FROM product_translations \
             WHERE id = '00000000-0000-0000-0000-000000000112'",
        )
        .await?;

        assert_next_product_migration_rejected(&db, "uq_product_variants_tenant_sku").await?;
        db.execute_unprepared(
            "DELETE FROM product_variants \
             WHERE id = '00000000-0000-0000-0000-000000000122'",
        )
        .await?;

        assert_next_product_migration_rejected(&db, "uq_catalog_categories_tenant_root_slug")
            .await?;
        db.execute_unprepared(
            "DELETE FROM catalog_categories \
             WHERE id = '00000000-0000-0000-0000-000000000502'",
        )
        .await?;

        ProductMigrator::up(&db, Some(1))
            .await
            .map_err(|error| format!("cleaned tenant-integrity migration failed: {error}"))?;
        assert_legacy_inventory_mapping(&db).await?;
        db.close().await?;
        Ok(())
    }
    .await;

    finish_fixture(&admin, &database_name, migration_result).await
}

async fn run_product_catalog_read_port_checks() -> Result<(), Box<dyn std::error::Error>> {
    let admin_url = smoke_admin_url()?;

    let database_name = format!("{}_product_read_port", default_database_name());
    assert_valid_database_name(&database_name);
    let target_url = database_url_from_admin_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("admin database must be reachable: {error}"))?;

    create_database(&admin, &database_name).await?;

    let port_result = async {
        let db = connect_postgres(&target_url).await?;
        create_product_migration_prerequisites(&db).await?;
        ProductMigrator::up(&db, None)
            .await
            .map_err(|error| format!("product migration up failed: {error}"))?;
        create_product_read_owner_tables(&db).await?;
        seed_product_read_port_fixtures(&db).await?;

        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
        let service = ProductCatalogReadRuntime::in_process_with_pricing(
            db.clone(),
            event_bus,
            Some(rustok_pricing::in_process_product_pricing_port(db.clone())),
        )
        .read_port();
        assert_product_read_projection(service.as_ref()).await?;
        assert_variant_read_projection(service.as_ref()).await?;
        assert_published_product_listing(service.as_ref()).await?;
        assert_product_read_tenant_isolation(service.as_ref()).await?;

        db.close().await?;
        Ok(())
    }
    .await;

    finish_fixture(&admin, &database_name, port_result).await
}

fn product_read_context(tenant_id: &str, correlation_id: &str) -> PortContext {
    PortContext::new(
        tenant_id,
        PortActor::service("product-postgres-read-port-test"),
        "ru",
        correlation_id,
    )
    .with_deadline(Duration::from_secs(5))
}

async fn assert_product_read_projection(
    service: &dyn ProductCatalogReadPort,
) -> Result<(), Box<dyn std::error::Error>> {
    let product = service
        .read_product_projection(
            product_read_context(
                "00000000-0000-0000-0000-000000000001",
                "product-read-projection",
            ),
            ProductProjectionRequest {
                product_id: "00000000-0000-0000-0000-000000000101".parse()?,
                locale: Some("ru".to_owned()),
                fallback_locale: Some("en".to_owned()),
            },
        )
        .await
        .map_err(|error| {
            format!(
                "product projection failed: {}: {}",
                error.code, error.message
            )
        })?;

    if product.id.to_string() != "00000000-0000-0000-0000-000000000101"
        || product.tenant_id.to_string() != "00000000-0000-0000-0000-000000000001"
        || product.variants.len() != 1
    {
        return Err(
            "product read projection did not preserve owner identity and variant data".into(),
        );
    }
    let variant = &product.variants[0];
    if variant.inventory_quantity != 8 || variant.prices.len() != 1 {
        return Err(format!(
            "product read projection expected inventory 8 and one price, got inventory {} and {} prices",
            variant.inventory_quantity,
            variant.prices.len()
        )
        .into());
    }
    Ok(())
}

async fn assert_variant_read_projection(
    service: &dyn ProductCatalogReadPort,
) -> Result<(), Box<dyn std::error::Error>> {
    let product = service
        .read_variant_product_projection(
            product_read_context(
                "00000000-0000-0000-0000-000000000001",
                "variant-read-projection",
            ),
            VariantProductProjectionRequest {
                variant_id: "00000000-0000-0000-0000-000000000121".parse()?,
                locale: None,
                fallback_locale: Some("en".to_owned()),
            },
        )
        .await
        .map_err(|error| {
            format!(
                "variant-first product projection failed: {}: {}",
                error.code, error.message
            )
        })?;
    if product.id.to_string() != "00000000-0000-0000-0000-000000000101" {
        return Err("variant-first projection resolved the wrong owner product".into());
    }
    Ok(())
}

async fn assert_published_product_listing(
    service: &dyn ProductCatalogReadPort,
) -> Result<(), Box<dyn std::error::Error>> {
    let first_page = service
        .list_published_products(
            product_read_context(
                "00000000-0000-0000-0000-000000000001",
                "published-products-page-one",
            ),
            PublishedProductsRequest {
                locale: Some("ru".to_owned()),
                fallback_locale: Some("en".to_owned()),
                public_channel_slug: Some("mobile".to_owned()),
                page: 1,
                per_page: 1,
            },
        )
        .await
        .map_err(|error| {
            format!(
                "published product page one failed: {}: {}",
                error.code, error.message
            )
        })?;
    if first_page.total != 2
        || first_page.items.len() != 1
        || !first_page.has_next
        || first_page.items[0].title != "Mobile product"
    {
        return Err(format!(
            "published list page one mismatch: total={}, items={}, has_next={}, title={:?}",
            first_page.total,
            first_page.items.len(),
            first_page.has_next,
            first_page.items.first().map(|item| item.title.as_str())
        )
        .into());
    }

    let second_page = service
        .list_published_products(
            product_read_context(
                "00000000-0000-0000-0000-000000000001",
                "published-products-page-two",
            ),
            PublishedProductsRequest {
                locale: Some("ru".to_owned()),
                fallback_locale: Some("en".to_owned()),
                public_channel_slug: Some("mobile".to_owned()),
                page: 2,
                per_page: 1,
            },
        )
        .await
        .map_err(|error| {
            format!(
                "published product page two failed: {}: {}",
                error.code, error.message
            )
        })?;
    if second_page.total != 2
        || second_page.items.len() != 1
        || second_page.has_next
        || second_page.items[0].title != "Global product"
    {
        return Err(
            "published list page two did not preserve pagination and locale fallback".into(),
        );
    }

    let web_only = service
        .list_published_products(
            product_read_context(
                "00000000-0000-0000-0000-000000000001",
                "published-products-web",
            ),
            PublishedProductsRequest {
                locale: None,
                fallback_locale: Some("en".to_owned()),
                public_channel_slug: Some("web".to_owned()),
                page: 1,
                per_page: 48,
            },
        )
        .await
        .map_err(|error| {
            format!(
                "published web product list failed: {}: {}",
                error.code, error.message
            )
        })?;
    if web_only.total != 1
        || web_only.items.len() != 1
        || web_only.items[0].title != "Global product"
    {
        return Err("published list did not enforce channel visibility in PostgreSQL".into());
    }
    Ok(())
}

async fn assert_product_read_tenant_isolation(
    service: &dyn ProductCatalogReadPort,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = service
        .read_product_projection(
            product_read_context(
                "00000000-0000-0000-0000-000000000002",
                "cross-tenant-product-read",
            ),
            ProductProjectionRequest {
                product_id: "00000000-0000-0000-0000-000000000101".parse()?,
                locale: None,
                fallback_locale: Some("en".to_owned()),
            },
        )
        .await
        .expect_err("cross-tenant product projection must be rejected");
    if error.kind != PortErrorKind::NotFound || error.code != "product.product_not_found" {
        return Err(format!(
            "cross-tenant read must map to product.product_not_found, got {}",
            error.code
        )
        .into());
    }
    Ok(())
}

async fn create_product_read_owner_tables(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
CREATE TABLE prices (
    id UUID PRIMARY KEY,
    variant_id UUID NOT NULL,
    price_list_id UUID,
    channel_id UUID,
    channel_slug VARCHAR(255),
    currency_code VARCHAR(3) NOT NULL,
    region_id UUID,
    amount_decimal NUMERIC(20, 6) NOT NULL,
    compare_at_amount_decimal NUMERIC(20, 6),
    amount BIGINT,
    compare_at_amount BIGINT,
    min_quantity INTEGER,
    max_quantity INTEGER
);

CREATE TABLE inventory_items (
    id UUID PRIMARY KEY,
    variant_id UUID NOT NULL,
    sku VARCHAR(100),
    requires_shipping BOOLEAN NOT NULL,
    metadata JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE inventory_levels (
    id UUID PRIMARY KEY,
    inventory_item_id UUID NOT NULL,
    location_id UUID NOT NULL,
    stocked_quantity INTEGER NOT NULL,
    reserved_quantity INTEGER NOT NULL,
    incoming_quantity INTEGER NOT NULL,
    low_stock_threshold INTEGER,
    updated_at TIMESTAMPTZ NOT NULL
);
"#,
    )
    .await?;
    Ok(())
}

async fn seed_product_read_port_fixtures(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
INSERT INTO tenants (id) VALUES
    ('00000000-0000-0000-0000-000000000001'),
    ('00000000-0000-0000-0000-000000000002');

INSERT INTO products (
    id, tenant_id, status, metadata, created_at, updated_at, published_at
) VALUES
    (
        '00000000-0000-0000-0000-000000000101',
        '00000000-0000-0000-0000-000000000001',
        'active',
        '{}'::jsonb,
        now() - interval '3 hours',
        now() - interval '3 hours',
        now() - interval '2 hours'
    ),
    (
        '00000000-0000-0000-0000-000000000102',
        '00000000-0000-0000-0000-000000000001',
        'active',
        '{"channel_visibility":{"allowed_channel_slugs":["mobile"]}}'::jsonb,
        now() - interval '2 hours',
        now() - interval '2 hours',
        now() - interval '1 hour'
    ),
    (
        '00000000-0000-0000-0000-000000000103',
        '00000000-0000-0000-0000-000000000001',
        'draft',
        '{}'::jsonb,
        now(),
        now(),
        NULL
    ),
    (
        '00000000-0000-0000-0000-000000000104',
        '00000000-0000-0000-0000-000000000002',
        'active',
        '{}'::jsonb,
        now(),
        now(),
        now()
    );

INSERT INTO product_translations (
    id, product_id, tenant_id, locale, title, handle
) VALUES
    (
        '00000000-0000-0000-0000-000000000111',
        '00000000-0000-0000-0000-000000000101',
        '00000000-0000-0000-0000-000000000001',
        'en',
        'Global product',
        'global-product'
    ),
    (
        '00000000-0000-0000-0000-000000000112',
        '00000000-0000-0000-0000-000000000102',
        '00000000-0000-0000-0000-000000000001',
        'en',
        'Mobile product',
        'mobile-product'
    );

INSERT INTO product_variants (id, product_id, tenant_id, sku)
VALUES (
    '00000000-0000-0000-0000-000000000121',
    '00000000-0000-0000-0000-000000000101',
    '00000000-0000-0000-0000-000000000001',
    'GLOBAL-SKU'
);

INSERT INTO prices (
    id, variant_id, currency_code, amount_decimal, amount
) VALUES (
    '00000000-0000-0000-0000-000000000131',
    '00000000-0000-0000-0000-000000000121',
    'USD',
    19.99,
    1999
);

INSERT INTO inventory_items (
    id, variant_id, sku, requires_shipping, metadata, created_at, updated_at
) VALUES (
    '00000000-0000-0000-0000-000000000141',
    '00000000-0000-0000-0000-000000000121',
    'GLOBAL-SKU',
    TRUE,
    '{}'::jsonb,
    now(),
    now()
);

INSERT INTO inventory_levels (
    id, inventory_item_id, location_id, stocked_quantity, reserved_quantity,
    incoming_quantity, updated_at
) VALUES (
    '00000000-0000-0000-0000-000000000151',
    '00000000-0000-0000-0000-000000000141',
    '00000000-0000-0000-0000-000000000161',
    10,
    2,
    0,
    now()
);
"#,
    )
    .await?;
    Ok(())
}

async fn seed_product_dirty_data_fixtures(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
INSERT INTO tenants (id)
VALUES ('00000000-0000-0000-0000-000000000001');

INSERT INTO products (id, tenant_id) VALUES
    (
        '00000000-0000-0000-0000-000000000101',
        '00000000-0000-0000-0000-000000000001'
    ),
    (
        '00000000-0000-0000-0000-000000000102',
        '00000000-0000-0000-0000-000000000001'
    );

INSERT INTO product_translations (id, product_id, locale, title, handle) VALUES
    (
        '00000000-0000-0000-0000-000000000111',
        '00000000-0000-0000-0000-000000000101',
        'en',
        'First legacy product',
        'duplicate-handle'
    ),
    (
        '00000000-0000-0000-0000-000000000112',
        '00000000-0000-0000-0000-000000000102',
        'en',
        'Second legacy product',
        'duplicate-handle'
    );

INSERT INTO product_variants (
    id, product_id, tenant_id, sku, manage_inventory, allow_backorder, variant_rank
) VALUES
    (
        '00000000-0000-0000-0000-000000000121',
        '00000000-0000-0000-0000-000000000101',
        '00000000-0000-0000-0000-000000000001',
        'DUPLICATE-SKU',
        TRUE,
        TRUE,
        7
    ),
    (
        '00000000-0000-0000-0000-000000000122',
        '00000000-0000-0000-0000-000000000102',
        '00000000-0000-0000-0000-000000000001',
        'DUPLICATE-SKU',
        FALSE,
        FALSE,
        3
    );

INSERT INTO catalog_categories (id, tenant_id, code, slug, path) VALUES
    (
        '00000000-0000-0000-0000-000000000501',
        '00000000-0000-0000-0000-000000000001',
        'first-root',
        'duplicate-root',
        '/first-root'
    ),
    (
        '00000000-0000-0000-0000-000000000502',
        '00000000-0000-0000-0000-000000000001',
        'second-root',
        'duplicate-root',
        '/second-root'
    );
"#,
    )
    .await?;
    Ok(())
}

async fn assert_next_product_migration_rejected(
    db: &DatabaseConnection,
    rejected_by: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = ProductMigrator::up(db, Some(1))
        .await
        .expect_err("dirty product data must block the tenant-integrity migration");
    let message = error.to_string();
    if !message.contains(rejected_by) {
        return Err(format!(
            "dirty-data migration must be rejected by {rejected_by}, got: {message}"
        )
        .into());
    }
    Ok(())
}

async fn assert_legacy_inventory_mapping(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            r#"
SELECT inventory_management, inventory_policy, position
FROM product_variants
WHERE id = '00000000-0000-0000-0000-000000000121'
"#,
        ))
        .await?
        .ok_or("mapped legacy product variant must exist")?;
    let inventory_management: String = row.try_get("", "inventory_management")?;
    let inventory_policy: String = row.try_get("", "inventory_policy")?;
    let position: i32 = row.try_get("", "position")?;
    if inventory_management != "manual" || inventory_policy != "continue" || position != 7 {
        return Err(format!(
            "legacy inventory mapping mismatch: management={inventory_management}, \
             policy={inventory_policy}, position={position}"
        )
        .into());
    }

    for column in ["manage_inventory", "allow_backorder", "variant_rank"] {
        assert_column_missing(db, "product_variants", column).await?;
    }
    Ok(())
}

async fn seed_product_constraint_fixtures(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
INSERT INTO tenants (id) VALUES
    ('00000000-0000-0000-0000-000000000001'),
    ('00000000-0000-0000-0000-000000000002');

INSERT INTO products (id, tenant_id) VALUES
    (
        '00000000-0000-0000-0000-000000000101',
        '00000000-0000-0000-0000-000000000001'
    ),
    (
        '00000000-0000-0000-0000-000000000102',
        '00000000-0000-0000-0000-000000000001'
    );

INSERT INTO taxonomy_terms (id, tenant_id, kind, scope_type, canonical_key)
VALUES (
    '00000000-0000-0000-0000-000000000201',
    '00000000-0000-0000-0000-000000000002',
    'tag', 'global', 'other-tenant-tag'
);

INSERT INTO product_attributes (id, tenant_id, code, value_type)
VALUES (
    '00000000-0000-0000-0000-000000000301',
    '00000000-0000-0000-0000-000000000001',
    'stock_count',
    'integer'
);

INSERT INTO catalog_categories (id, tenant_id, parent_id, code, slug, path, level) VALUES
    (
        '00000000-0000-0000-0000-000000000501',
        '00000000-0000-0000-0000-000000000001',
        NULL,
        'catalog',
        'catalog',
        '/catalog',
        0
    ),
    (
        '00000000-0000-0000-0000-000000000503',
        '00000000-0000-0000-0000-000000000001',
        '00000000-0000-0000-0000-000000000501',
        'catalog-child',
        'child',
        '/catalog/child',
        1
    );

INSERT INTO catalog_category_closure (tenant_id, ancestor_id, descendant_id, depth) VALUES
    (
        '00000000-0000-0000-0000-000000000001',
        '00000000-0000-0000-0000-000000000501',
        '00000000-0000-0000-0000-000000000501',
        0
    ),
    (
        '00000000-0000-0000-0000-000000000001',
        '00000000-0000-0000-0000-000000000503',
        '00000000-0000-0000-0000-000000000503',
        0
    ),
    (
        '00000000-0000-0000-0000-000000000001',
        '00000000-0000-0000-0000-000000000501',
        '00000000-0000-0000-0000-000000000503',
        1
    );
"#,
    )
    .await?;
    Ok(())
}

async fn assert_category_cycle_rejected(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let transaction = db.begin().await?;
    transaction
        .execute_unprepared(
            r#"
UPDATE catalog_categories
SET parent_id = '00000000-0000-0000-0000-000000000503'
WHERE id = '00000000-0000-0000-0000-000000000501'
"#,
        )
        .await?;
    let error = transaction
        .commit()
        .await
        .expect_err("a catalog category cycle must be rejected at commit");
    let message = error.to_string();
    if !message.contains("catalog category tree contains a cycle") {
        return Err(
            format!("category cycle rejection returned an unexpected error: {message}").into(),
        );
    }
    Ok(())
}

async fn assert_category_closure_drift_rejected(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let transaction = db.begin().await?;
    transaction
        .execute_unprepared(
            r#"
DELETE FROM catalog_category_closure
WHERE tenant_id = '00000000-0000-0000-0000-000000000001'
  AND ancestor_id = '00000000-0000-0000-0000-000000000501'
  AND descendant_id = '00000000-0000-0000-0000-000000000503'
"#,
        )
        .await?;
    let error = transaction
        .commit()
        .await
        .expect_err("catalog category closure drift must be rejected at commit");
    let message = error.to_string();
    if !message.contains("catalog category closure is not the canonical parent-tree projection") {
        return Err(format!(
            "category closure-drift rejection returned an unexpected error: {message}"
        )
        .into());
    }
    Ok(())
}

async fn assert_exactly_one_concurrent_write_succeeds(
    db: &DatabaseConnection,
    first_sql: &str,
    second_sql: &str,
    rejected_by: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let first_connection = db.clone();
    let second_connection = db.clone();
    let (first, second) = tokio::join!(
        first_connection.execute_unprepared(first_sql),
        second_connection.execute_unprepared(second_sql)
    );

    match (first, second) {
        (Ok(_), Err(error)) | (Err(error), Ok(_)) => {
            let message = error.to_string();
            if !message.contains(rejected_by) {
                return Err(format!(
                    "concurrent write must be rejected by {rejected_by}, got: {message}"
                )
                .into());
            }
            Ok(())
        }
        (Ok(_), Ok(_)) => Err(format!(
            "exactly one concurrent write guarded by {rejected_by} must fail, but both succeeded"
        )
        .into()),
        (Err(first_error), Err(second_error)) => Err(format!(
            "exactly one concurrent write guarded by {rejected_by} must succeed; \
             first error: {first_error}; second error: {second_error}"
        )
        .into()),
    }
}

async fn assert_statement_rejected(
    db: &DatabaseConnection,
    sql: &str,
    rejected_by: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = db
        .execute_unprepared(sql)
        .await
        .expect_err("invalid product persistence write must be rejected");
    let message = error.to_string();
    if !message.contains(rejected_by) {
        return Err(
            format!("invalid write must be rejected by {rejected_by}, got: {message}").into(),
        );
    }
    Ok(())
}

async fn create_product_migration_prerequisites(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    db.execute_unprepared(
        r#"
CREATE TABLE tenants (
    id UUID PRIMARY KEY
);
"#,
    )
    .await?;
    let manager = SchemaManager::new(db);
    for migration in rustok_outbox::OutboxModule
        .migrations()
        .into_iter()
        .chain(rustok_taxonomy::TaxonomyModule.migrations())
    {
        migration.up(&manager).await?;
    }
    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    Ok(())
}

async fn assert_product_migration_schema(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    for table in [
        "products",
        "product_translations",
        "product_variants",
        "product_tags",
        "catalog_categories",
        "product_attributes",
        "product_attribute_values",
    ] {
        assert_table_exists(db, table).await?;
    }
    for constraint in [
        "fk_product_translations_product_tenant",
        "fk_product_tags_product_tenant",
        "fk_product_tags_term_tenant",
        "chk_product_attribute_values_one_scalar",
    ] {
        assert_constraint_exists(db, constraint).await?;
    }
    for index in [
        "uq_product_translations_tenant_locale_handle",
        "uq_product_variants_tenant_sku",
        "idx_products_storefront_published",
        "idx_products_channel_visibility_jsonb",
    ] {
        assert_index_exists(db, index).await?;
    }
    assert_trigger_exists(db, "taxonomy_category_hierarchy_guard").await?;
    assert_trigger_exists(db, "trg_products_index_graph_projection_update").await?;
    assert_trigger_does_not_exist(db, "trg_catalog_categories_validate_tree").await?;
    assert_table_missing(db, "catalog_category_closure").await?;
    for (table, columns) in [
        (
            "products",
            &[
                "is_gift_card",
                "discountable",
                "weight",
                "length",
                "height",
                "width",
                "hs_code",
                "origin_country",
                "mid_code",
                "external_id",
                "deleted_at",
            ][..],
        ),
        ("product_translations", &["subtitle", "material"][..]),
        ("product_options", &["name", "values"][..]),
        ("product_images", &["url", "metadata", "created_at"][..]),
        (
            "product_variants",
            &[
                "length",
                "height",
                "width",
                "hs_code",
                "origin_country",
                "mid_code",
                "metadata",
                "deleted_at",
            ][..],
        ),
    ] {
        for column in columns {
            assert_column_missing(db, table, column).await?;
        }
    }
    assert_column_contract(db, "product_images", "media_id", "uuid", false).await?;
    assert_column_contract(db, "product_variants", "weight", "numeric", true).await?;
    Ok(())
}

async fn run_postgres_zero_migration_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let admin_url = smoke_admin_url()?;

    let database_name =
        std::env::var("RUSTOK_MIGRATION_SMOKE_DB_NAME").unwrap_or_else(|_| default_database_name());
    assert_valid_database_name(&database_name);

    let target_url = database_url_from_admin_url(&admin_url, &database_name);
    let keep_database = env_binary_flag("RUSTOK_MIGRATION_SMOKE_KEEP_DB")?;
    let incremental = env_binary_flag("RUSTOK_MIGRATION_SMOKE_INCREMENTAL")?;
    let reuse_database = env_binary_flag("RUSTOK_MIGRATION_SMOKE_REUSE_DB")?;
    let rollback_latest = env_binary_flag("RUSTOK_MIGRATION_SMOKE_ROLLBACK_LATEST")?;
    let backfill_fixtures = load_backfill_fixtures()?;
    if !backfill_fixtures.is_empty() && !reuse_database {
        return Err("backfill fixtures require RUSTOK_MIGRATION_SMOKE_REUSE_DB=1".into());
    }

    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("admin database must be reachable: {error}"))?;

    if reuse_database {
        connect_postgres(&target_url)
            .await
            .map_err(|error| {
                format!(
                    "reused migration smoke database {database_name} must already exist and be reachable: {error}"
                )
            })?;
    } else {
        // A name collision must fail before the fixture can affect an existing database.
        create_database(&admin, &database_name)
            .await
            .map_err(|error| {
                format!("fresh migration smoke database {database_name} must be created: {error}")
            })?;
    }

    let smoke_result = apply_migrations_and_assert_schema(
        &target_url,
        incremental,
        rollback_latest,
        &backfill_fixtures,
    )
    .await;

    if keep_database {
        eprintln!("Keeping migration smoke database '{database_name}'");
        return smoke_result;
    }

    finish_fixture(&admin, &database_name, smoke_result).await
}

fn smoke_admin_url() -> Result<String, Box<dyn std::error::Error>> {
    let url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL").map_err(
        |_| "RUSTOK_MIGRATION_SMOKE_ADMIN_URL must name an isolated PostgreSQL instance",
    )?;
    assert_postgres_url(&url);
    Ok(url)
}

async fn finish_fixture(
    admin: &DatabaseConnection,
    name: &str,
    result: Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cleanup = drop_database_if_exists(admin, name).await;
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; database cleanup: {cleanup}").into()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
        (Ok(()), Ok(())) => Ok(()),
    }
}

async fn apply_migrations_and_assert_schema(
    target_url: &str,
    incremental: bool,
    rollback_latest: bool,
    backfill_fixtures: &[BackfillFixture],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut runner = Some(StepRunner::connect(target_url, Migrator::migrations()).await?);
    let result = async {
        let initial = runner.as_ref().ok_or("missing initial migration session")?;
        let pending: std::collections::HashSet<_> =
            initial.pending_migrations()?.map(str::to_owned).collect();
        validate_backfill_targets(backfill_fixtures, &pending)?;
        // Seed the predecessor once. Check each fixture immediately at its
        // target boundary and again after the complete range.
        apply_backfill_setup(initial.database(), backfill_fixtures).await?;
        loop {
            let current = runner.as_mut().ok_or("missing migration session")?;
            let Some(name) = current.apply_next().await? else {
                break;
            };
            for fixture in backfill_fixtures
                .iter()
                .filter(|fixture| fixture.migration == name)
            {
                assert_backfill_results(current.database(), std::slice::from_ref(fixture)).await?;
            }
            if incremental {
                runner
                    .take()
                    .ok_or("missing migration session before restart")?
                    .close()
                    .await?;
                runner = Some(StepRunner::connect(target_url, Migrator::migrations()).await?);
            }
        }
        let current = runner.as_ref().ok_or("missing final migration session")?;
        assert_no_pending_migrations(current.database(), "initial migration apply").await?;
        assert_schema_contract(current.database()).await?;
        assert_backfill_results(current.database(), backfill_fixtures).await?;
        if rollback_latest {
            rollback_latest_and_reapply(current.database()).await?;
            assert_schema_contract(current.database()).await?;
            assert_backfill_results(current.database(), backfill_fixtures).await?;
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    }
    .await;
    let close = match runner {
        Some(runner) => runner.close().await,
        None => Ok(()),
    };
    match (result, close) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; session cleanup: {cleanup}").into()),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error.into()),
        _ => Ok(()),
    }
}

async fn rollback_latest_and_reapply(
    db: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let applied = Migrator::get_applied_migrations_read_only(db).await?;
    let latest = applied
        .last()
        .ok_or("rollback requires an applied migration")?;
    Migrator::down(db, Some(1))
        .await
        .map_err(|error| format!("latest migration must support one-step rollback: {error}"))?;

    let pending = Migrator::get_pending_migrations(db)
        .await
        .map_err(|error| format!("pending list after rollback must be readable: {error}"))?;
    let names = pending
        .iter()
        .map(|migration| migration.name())
        .collect::<Vec<_>>();
    let rolled_back_name = latest.name();
    if names != [rolled_back_name] {
        return Err(format!(
            "one-step rollback must expose exactly one pending migration ({rolled_back_name}), found {names:?}"
        )
        .into());
    }

    Migrator::up(db, Some(1)).await.map_err(|error| {
        format!("rolled-back migration {rolled_back_name} must reapply successfully: {error}")
    })?;
    assert_no_pending_migrations(db, "rollback reapply").await?;
    eprintln!(
        "Rehearsed rollback and reapply for migration {rolled_back_name} with {} migrations applied",
        applied.len()
    );
    Ok(())
}

async fn assert_no_pending_migrations(
    db: &DatabaseConnection,
    phase: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pending = Migrator::get_pending_migrations(db)
        .await
        .map_err(|error| {
            format!("pending migration list during {phase} must be readable: {error}")
        })?;
    if !pending.is_empty() {
        let pending_names = pending
            .iter()
            .map(|migration| migration.name().to_string())
            .collect::<Vec<_>>();
        return Err(format!("{phase} must leave no pending migrations: {pending_names:?}").into());
    }
    Ok(())
}

async fn assert_schema_contract(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    for table in [
        "tenants",
        "users",
        "product_variants",
        "prices",
        "inventory_items",
        "channels",
        "oauth_apps",
        "oauth_app_translations",
        "blog_post_tags",
        "forum_topic_tags",
        "taxonomy_terms",
    ] {
        assert_table_exists(db, table).await?;
    }

    assert_table_does_not_exist(db, "o_auth_app_translations").await?;

    for constraint in [
        "uq_product_translations_tenant_id",
        "fk_product_translations_product_tenant",
        "fk_product_tags_product_tenant",
        "fk_product_tags_term_tenant",
        "chk_product_categories_no_primary_assignment",
        "chk_product_attribute_values_one_scalar",
        "chk_product_variant_attribute_values_one_scalar",
    ] {
        assert_constraint_exists(db, constraint).await?;
    }
    for index in [
        "uq_product_translations_tenant_locale_handle",
        "uq_product_variants_tenant_sku",
        "idx_products_storefront_published",
        "idx_products_channel_visibility_jsonb",
    ] {
        assert_index_exists(db, index).await?;
    }
    assert_trigger_exists(db, "trg_products_normalize_channel_visibility").await?;

    if migration_is_applied(
        db,
        "m20260916_000032_clean_product_category_canonical_taxonomy",
    )
    .await?
    {
        assert_index_does_not_exist(db, "uq_catalog_categories_tenant_root_slug").await?;
        assert_trigger_does_not_exist(db, "trg_catalog_categories_validate_tree").await?;
    } else {
        assert_index_exists(db, "uq_catalog_categories_tenant_root_slug").await?;
        assert_trigger_exists(db, "trg_catalog_categories_validate_tree").await?;
    }

    if migration_is_applied(
        db,
        "m20260829_000020_retire_product_category_closure_storage",
    )
    .await?
    {
        assert_table_does_not_exist(db, "catalog_category_closure").await?;
    } else {
        assert_table_exists(db, "catalog_category_closure").await?;
        assert_trigger_exists(db, "trg_catalog_category_closure_validate_tree").await?;
    }

    Ok(())
}

async fn migration_is_applied(
    db: &DatabaseConnection,
    migration: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT EXISTS (SELECT 1 FROM seaql_migrations WHERE version = $1) AS exists",
            [migration.to_owned().into()],
        ))
        .await
        .map_err(|error| format!("migration presence query for {migration} must succeed: {error}"))?
        .ok_or_else(|| format!("migration presence query for {migration} returned no row"))?;
    let applied: bool = row.try_get("", "exists").map_err(|error| {
        format!("migration presence result for {migration} must decode: {error}")
    })?;
    Ok(applied)
}

async fn assert_table_exists(
    db: &DatabaseConnection,
    table: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1) IS NOT NULL AS exists",
            [format!("public.{table}").into()],
        ))
        .await
        .map_err(|error| format!("table existence query for {table} must succeed: {error}"))?
        .ok_or_else(|| format!("table existence query for {table} returned no row"))?;
    let exists: bool = row
        .try_get("", "exists")
        .map_err(|error| format!("table existence result for {table} must decode: {error}"))?;
    if !exists {
        return Err(format!("expected table {table} to exist after migrations").into());
    }
    Ok(())
}

async fn assert_table_does_not_exist(
    db: &DatabaseConnection,
    table: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1) IS NOT NULL AS exists",
            [format!("public.{table}").into()],
        ))
        .await
        .map_err(|error| format!("table absence query for {table} must succeed: {error}"))?
        .ok_or_else(|| format!("table absence query for {table} returned no row"))?;
    let exists: bool = row
        .try_get("", "exists")
        .map_err(|error| format!("table absence result for {table} must decode: {error}"))?;
    if exists {
        return Err(format!("unexpected table {table} exists after migrations").into());
    }
    Ok(())
}

async fn assert_constraint_exists(
    db: &DatabaseConnection,
    constraint: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_catalog_object_exists(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = $1) AS exists",
        constraint,
        "constraint",
    )
    .await
}

async fn assert_index_exists(
    db: &DatabaseConnection,
    index: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_catalog_object_exists(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_class WHERE relkind = 'i' AND relname = $1) AS exists",
        index,
        "index",
    )
    .await
}

async fn assert_trigger_exists(
    db: &DatabaseConnection,
    trigger: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_catalog_object_exists(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = $1 AND NOT tgisinternal) AS exists",
        trigger,
        "trigger",
    )
    .await
}

async fn assert_catalog_object_exists(
    db: &DatabaseConnection,
    query: &str,
    name: &str,
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            query,
            [name.to_owned().into()],
        ))
        .await?
        .ok_or_else(|| format!("{kind} existence query returned no row"))?;
    let exists: bool = row.try_get("", "exists")?;
    if !exists {
        return Err(format!("expected {kind} `{name}` after migrations").into());
    }
    Ok(())
}

async fn assert_index_does_not_exist(
    db: &DatabaseConnection,
    index: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_catalog_object_does_not_exist(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_class WHERE relkind = 'i' AND relname = $1) AS exists",
        index,
        "index",
    )
    .await
}

async fn assert_trigger_does_not_exist(
    db: &DatabaseConnection,
    trigger: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_catalog_object_does_not_exist(
        db,
        "SELECT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = $1 AND NOT tgisinternal) AS exists",
        trigger,
        "trigger",
    )
    .await
}

async fn assert_catalog_object_does_not_exist(
    db: &DatabaseConnection,
    query: &str,
    name: &str,
    kind: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            query,
            [name.to_owned().into()],
        ))
        .await?
        .ok_or_else(|| format!("{kind} existence query returned no row"))?;
    let exists: bool = row.try_get("", "exists")?;
    if exists {
        return Err(format!("unexpected {kind} `{name}` exists after migrations").into());
    }
    Ok(())
}

fn env_binary_flag(name: &str) -> Result<bool, Box<dyn std::error::Error>> {
    parse_binary_flag(name, std::env::var(name).ok().as_deref())
}

fn parse_binary_flag(name: &str, value: Option<&str>) -> Result<bool, Box<dyn std::error::Error>> {
    match value.unwrap_or("0") {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(format!("{name} must be 0 or 1, got {other:?}").into()),
    }
}

fn default_database_name() -> String {
    unique_postgres_database_name("rustok_migration_smoke")
}

#[cfg(test)]
mod tests {
    use super::parse_binary_flag;

    #[test]
    fn binary_flag_defaults_to_false_when_missing() {
        assert!(
            !parse_binary_flag("RUSTOK_MIGRATION_SMOKE_INCREMENTAL", None)
                .expect("missing flag should default to false")
        );
    }

    #[test]
    fn binary_flag_accepts_zero_and_one_only() {
        assert!(
            !parse_binary_flag("RUSTOK_MIGRATION_SMOKE_INCREMENTAL", Some("0"))
                .expect("0 should be accepted")
        );
        assert!(
            parse_binary_flag("RUSTOK_MIGRATION_SMOKE_ROLLBACK_LATEST", Some("1"))
                .expect("1 should be accepted")
        );
        assert!(
            parse_binary_flag("RUSTOK_MIGRATION_SMOKE_REUSE_DB", Some("true"))
                .expect_err("non-binary values should be rejected")
                .to_string()
                .contains("must be 0 or 1")
        );
    }
}

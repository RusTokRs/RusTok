use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;

use chrono::Utc;
use rustok_core::{UserRole, UserStatus};
use rustok_migrations::Migrator;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_server::models::_entities::{roles, user_roles};
use rustok_server::models::{tenants, users};
use rustok_server::services::rbac_service::RbacService;
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};
use sea_orm_migration::MigratorTrait;
use tokio::sync::Barrier;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const RBAC_POSTGRES_ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn concurrent_role_replacement_serializes_one_target_and_advances_two_generations() {
    with_rbac_postgres_database("rustok_rbac_role_replace", |db_a, db_b| async move {
        let tenant_id = insert_tenant(&db_a, "role-replacement").await?;
        let target_user_id = insert_user(&db_a, tenant_id, "role-target@example.com").await?;
        RbacRoleAssignmentDbWriter::new(db_a.clone())
            .assign_role_permissions(tenant_id, target_user_id, UserRole::Customer)
            .await?;

        let generation_before = rustok_rbac::read_permission_invalidation_generation(&db_a).await?;
        let assertion_db = db_a.clone();
        let task_db_a = db_a.clone();
        let task_db_b = db_b.clone();
        let barrier = Arc::new(Barrier::new(2));
        let barrier_a = Arc::clone(&barrier);
        let barrier_b = Arc::clone(&barrier);
        let task_a = tokio::spawn(async move {
            barrier_a.wait().await;
            RbacService::replace_user_role_committed(
                &task_db_a,
                &target_user_id,
                &tenant_id,
                UserRole::Admin,
            )
            .await
        });
        let task_b = tokio::spawn(async move {
            barrier_b.wait().await;
            RbacService::replace_user_role_committed(
                &task_db_b,
                &target_user_id,
                &tenant_id,
                UserRole::Manager,
            )
            .await
        });

        task_a.await??;
        task_b.await??;

        let assignments = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(target_user_id))
            .all(&assertion_db)
            .await?;
        if assignments.len() != 1 {
            return Err(format!(
                "concurrent replacement left {} role assignments, expected exactly one",
                assignments.len()
            )
            .into());
        }

        let final_role = roles::Entity::find_by_id(assignments[0].role_id)
            .filter(roles::Column::TenantId.eq(tenant_id))
            .one(&assertion_db)
            .await?
            .ok_or("final role assignment points to a missing tenant role")?;
        let accepted_roles =
            HashSet::from([UserRole::Admin.to_string(), UserRole::Manager.to_string()]);
        if !accepted_roles.contains(&final_role.slug) {
            return Err(format!(
                "concurrent replacement produced unexpected role {}",
                final_role.slug
            )
            .into());
        }

        let generation_after =
            rustok_rbac::read_permission_invalidation_generation(&assertion_db).await?;
        if generation_after != generation_before + 2 {
            return Err(format!(
                "two serialized role changes advanced generation from {generation_before} to \
                 {generation_after}, expected {}",
                generation_before + 2
            )
            .into());
        }
        Ok(())
    })
    .await
    .unwrap_or_else(|error| panic!("RBAC concurrent role replacement evidence failed: {error}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn concurrent_super_admin_demotions_preserve_one_active_super_admin() {
    with_rbac_postgres_database("rustok_rbac_super_admin", |db_a, db_b| async move {
        let tenant_id = insert_tenant(&db_a, "super-admin-continuity").await?;
        let first_user_id = insert_user(&db_a, tenant_id, "super-a@example.com").await?;
        let second_user_id = insert_user(&db_a, tenant_id, "super-b@example.com").await?;
        let writer = RbacRoleAssignmentDbWriter::new(db_a.clone());
        writer
            .assign_role_permissions(tenant_id, first_user_id, UserRole::SuperAdmin)
            .await?;
        writer
            .assign_role_permissions(tenant_id, second_user_id, UserRole::SuperAdmin)
            .await?;

        let generation_before = rustok_rbac::read_permission_invalidation_generation(&db_a).await?;
        let assertion_db = db_a.clone();
        let task_db_a = db_a.clone();
        let task_db_b = db_b.clone();
        let barrier = Arc::new(Barrier::new(2));
        let barrier_a = Arc::clone(&barrier);
        let barrier_b = Arc::clone(&barrier);
        let first = tokio::spawn(async move {
            barrier_a.wait().await;
            RbacService::replace_user_role_committed(
                &task_db_a,
                &first_user_id,
                &tenant_id,
                UserRole::Admin,
            )
            .await
        });
        let second = tokio::spawn(async move {
            barrier_b.wait().await;
            RbacService::replace_user_role_committed(
                &task_db_b,
                &second_user_id,
                &tenant_id,
                UserRole::Admin,
            )
            .await
        });

        let outcomes = [first.await?, second.await?];
        let success_count = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
        if success_count != 1 {
            return Err(format!(
                "concurrent super-admin demotions produced {success_count} successes, expected one"
            )
            .into());
        }
        let rejection = outcomes
            .iter()
            .find_map(|outcome| outcome.as_ref().err())
            .ok_or("one concurrent super-admin demotion must be rejected")?;
        if !rejection
            .to_string()
            .contains("cannot demote the last active super administrator")
        {
            return Err(format!("unexpected continuity rejection: {rejection}").into());
        }

        let super_admin_role = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(tenant_id))
            .filter(roles::Column::Slug.eq(UserRole::SuperAdmin.to_string()))
            .one(&assertion_db)
            .await?
            .ok_or("canonical super-admin role is missing")?;
        let remaining_super_admins = user_roles::Entity::find()
            .filter(user_roles::Column::RoleId.eq(super_admin_role.id))
            .all(&assertion_db)
            .await?;
        if remaining_super_admins.len() != 1 {
            return Err(format!(
                "continuity guard retained {} super-admin assignments, expected one",
                remaining_super_admins.len()
            )
            .into());
        }

        let generation_after =
            rustok_rbac::read_permission_invalidation_generation(&assertion_db).await?;
        if generation_after != generation_before + 1 {
            return Err(format!(
                "one committed demotion advanced generation from {generation_before} to \
                 {generation_after}, expected {}",
                generation_before + 1
            )
            .into());
        }
        Ok(())
    })
    .await
    .unwrap_or_else(|error| panic!("RBAC super-admin serialization evidence failed: {error}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn concurrent_generation_reservations_are_unique_contiguous_and_committed() {
    with_rbac_postgres_database("rustok_rbac_generation", |db_a, db_b| async move {
        let generation_before = rustok_rbac::read_permission_invalidation_generation(&db_a).await?;
        let barrier = Arc::new(Barrier::new(8));
        let mut tasks = Vec::new();
        for index in 0..8 {
            let db = if index % 2 == 0 {
                db_a.clone()
            } else {
                db_b.clone()
            };
            let barrier = Arc::clone(&barrier);
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                let transaction = db.begin().await?;
                let generation =
                    rustok_rbac::reserve_permission_invalidation_generation(&transaction).await?;
                transaction.commit().await?;
                Ok::<u64, Box<dyn std::error::Error + Send + Sync>>(generation)
            }));
        }

        let mut generations = Vec::with_capacity(tasks.len());
        for task in tasks {
            generations.push(task.await??);
        }
        generations.sort_unstable();
        let expected = (generation_before + 1..=generation_before + 8).collect::<Vec<_>>();
        if generations != expected {
            return Err(format!(
                "concurrent generation reservations returned {generations:?}, expected {expected:?}"
            )
            .into());
        }

        let generation_after = rustok_rbac::read_permission_invalidation_generation(&db_a).await?;
        if generation_after != generation_before + 8 {
            return Err(format!(
                "committed generation ended at {generation_after}, expected {}",
                generation_before + 8
            )
            .into());
        }
        Ok(())
    })
    .await
    .unwrap_or_else(|error| panic!("RBAC generation allocation evidence failed: {error}"));
}

async fn with_rbac_postgres_database<T, F, Fut>(prefix: &str, test: F) -> TestResult<T>
where
    F: FnOnce(DatabaseConnection, DatabaseConnection) -> Fut,
    Fut: Future<Output = TestResult<T>>,
{
    let admin_url = std::env::var(RBAC_POSTGRES_ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name(prefix);
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("PostgreSQL admin database must be reachable: {error}"))?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let test_result = async {
        let db_a = connect_postgres(&target_url).await?;
        Migrator::up(&db_a, None).await?;
        let db_b = connect_postgres(&target_url).await?;
        let result = test(db_a.clone(), db_b.clone()).await;
        db_a.close().await?;
        db_b.close().await?;
        result
    }
    .await;

    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    test_result
}

async fn insert_tenant(db: &DatabaseConnection, suffix: &str) -> TestResult<Uuid> {
    let tenant_id = Uuid::new_v4();
    tenants::Entity::insert(tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set(format!("RBAC PostgreSQL {suffix}")),
        slug: Set(format!("rbac-pg-{tenant_id}")),
        domain: Set(None),
        settings: Set(serde_json::json!({})),
        default_locale: Set("en".to_string()),
        is_active: Set(true),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    })
    .exec(db)
    .await?;
    Ok(tenant_id)
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, email: &str) -> TestResult<Uuid> {
    let user_id = Uuid::new_v4();
    users::Entity::insert(users::ActiveModel {
        id: Set(user_id),
        tenant_id: Set(tenant_id),
        email: Set(email.to_string()),
        password_hash: Set("hash".to_string()),
        name: Set(None),
        status: Set(UserStatus::Active),
        email_verified_at: Set(None),
        last_login_at: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    })
    .exec(db)
    .await?;
    Ok(user_id)
}

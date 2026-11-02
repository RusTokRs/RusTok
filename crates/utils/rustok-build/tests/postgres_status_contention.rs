//! PostgreSQL regression test for the terminal-status compare-and-swap.
//!
//! Run against a disposable PostgreSQL instance with permission to create a
//! schema: `RUSTOK_BUILD_TEST_POSTGRES_URL=... cargo test -p rustok-build
//! --test postgres_status_contention -- --ignored --nocapture`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rustok_build::{
    BuildEvent, BuildEventPublisher, BuildService, BuildStatus, DeploymentProfile, build,
};
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseBackend,
    DatabaseConnection, EntityTrait, IntoActiveModel, Schema, Set, Statement, TransactionTrait,
};
use tokio::sync::Barrier;
use uuid::Uuid;

const TEST_URL: &str = "RUSTOK_BUILD_TEST_POSTGRES_URL";
const WAIT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Default)]
struct RecordingPublisher(Mutex<Vec<BuildEvent>>);

#[async_trait::async_trait]
impl BuildEventPublisher for RecordingPublisher {
    async fn publish(&self, event: BuildEvent) -> anyhow::Result<()> {
        self.0.lock().expect("event lock").push(event);
        Ok(())
    }
}

fn schema_url(base: &str, schema: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    format!("{base}{separator}options=-csearch_path%3D{schema}%2Cpublic")
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("PostgreSQL connection")
}

async fn set_application_name(db: &DatabaseConnection, name: &str) {
    // Names are assembled solely from fixed ASCII and a generated UUID.
    db.execute_unprepared(&format!("SET application_name = '{name}'"))
        .await
        .expect("set PostgreSQL application name");
}

async fn wait_until_both_updates_blocked(admin: &DatabaseConnection, first: &str, second: &str) {
    tokio::time::timeout(WAIT_TIMEOUT, async {
        loop {
            let row = admin
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT COUNT(*) AS waiting FROM pg_stat_activity \
                     WHERE application_name IN ($1, $2) AND wait_event_type = 'Lock' \
                     AND query LIKE 'UPDATE%builds%'",
                    [first.into(), second.into()],
                ))
                .await
                .expect("inspect contending sessions")
                .expect("count row");
            let waiting: i64 = row.try_get("", "waiting").expect("waiting count");
            if waiting == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("both status writes must reach the held row lock");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires RUSTOK_BUILD_TEST_POSTGRES_URL and schema-creation permission"]
async fn competing_terminal_transitions_do_not_overwrite_each_other() {
    let base_url = std::env::var(TEST_URL).expect("test PostgreSQL URL is required");
    let suffix = Uuid::new_v4().simple().to_string();
    let schema_name = format!("build_cas_{suffix}");
    let admin = connect(&base_url).await;
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema_name}"))
        .await
        .expect("create isolated schema");

    let url = schema_url(&base_url, &schema_name);
    let fixture = connect(&url).await;
    let schema = Schema::new(DatabaseBackend::Postgres);
    fixture
        .execute_raw(
            DatabaseBackend::Postgres.build(&schema.create_table_from_entity(build::Entity)),
        )
        .await
        .expect("create builds table");

    let mut model = build::Model::new(
        "platform_state:1".to_string(),
        "test-hash".to_string(),
        1,
        serde_json::json!({"modules": {}}),
        "test-operator".to_string(),
        DeploymentProfile::HeadlessApi,
    )
    .into_active_model();
    model.status = Set(BuildStatus::Running);
    let build = model.insert(&fixture).await.expect("insert running build");

    let first = connect(&url).await;
    let second = connect(&url).await;
    let first_name = format!("build_cas_a_{suffix}");
    let second_name = format!("build_cas_b_{suffix}");
    set_application_name(&first, &first_name).await;
    set_application_name(&second, &second_name).await;

    // Holding the row lock lets both service transactions read Running, then
    // block in their UPDATE. The later UPDATE must recheck its predicate after
    // the winner commits, not overwrite the winning terminal status.
    let lock = fixture.begin().await.expect("begin row-lock transaction");
    lock.query_one_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id FROM builds WHERE id = $1 FOR UPDATE",
        [build.id.into()],
    ))
    .await
    .expect("lock build row")
    .expect("build row exists");

    let publisher = Arc::new(RecordingPublisher::default());
    let barrier = Arc::new(Barrier::new(3));
    let build_id = build.id;
    let success_task = {
        let publisher = publisher.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            let service = BuildService::with_event_publisher(first, publisher);
            barrier.wait().await;
            service
                .update_build_status(build_id, BuildStatus::Success, None, Some(100))
                .await
        })
    };
    let failure_task = {
        let publisher = publisher.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            let service = BuildService::with_event_publisher(second, publisher);
            barrier.wait().await;
            service
                .fail_build(build_id, "test failure".to_string())
                .await
        })
    };
    barrier.wait().await;
    wait_until_both_updates_blocked(&admin, &first_name, &second_name).await;
    lock.commit().await.expect("release row lock");

    let (success, failure) = tokio::time::timeout(WAIT_TIMEOUT, async {
        tokio::join!(success_task, failure_task)
    })
    .await
    .expect("contending updates complete");
    let success = success.expect("success task did not panic");
    let failure = failure.expect("failure task did not panic");
    assert_ne!(
        success.is_ok(),
        failure.is_ok(),
        "exactly one terminal writer wins"
    );
    let events = publisher.0.lock().expect("event lock");
    assert_eq!(events.len(), 1, "only the winning transition publishes");
    if success.is_ok() {
        assert!(
            matches!(&events[0], BuildEvent::BuildCompleted { build_id: id } if *id == build_id)
        );
    } else {
        assert!(
            matches!(&events[0], BuildEvent::BuildFailed { build_id: id, error, .. } if *id == build_id && error == "test failure")
        );
    }
    drop(events);

    let stored = build::Entity::find_by_id(build_id)
        .one(&fixture)
        .await
        .expect("read build")
        .expect("build exists");
    if success.is_ok() {
        assert_eq!(stored.status, BuildStatus::Success);
        assert_eq!(stored.progress, 100);
        assert_eq!(stored.error_message, None);
    } else {
        assert_eq!(stored.status, BuildStatus::Failed);
        assert_eq!(stored.error_message.as_deref(), Some("test failure"));
    }
    assert!(stored.finished_at.is_some());

    drop(fixture);
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema_name} CASCADE"))
        .await
        .expect("drop isolated schema");
}

//! ORM relation regressions. Production migrations already use composite policy
//! FKs; entity-generated consumer fixtures must preserve them with FK checks ON.
use rustok_forum as forum_under_test;
#[path = "support/audience_schema.rs"]
mod audience_schema;
use audience_schema::fixture;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::collections::BTreeSet;
use uuid::Uuid;

async fn verify_relations(prefix: &str, column: &str) {
    let (db, tenant, category, topic) = fixture().await;
    let subject = if column == "category_id" {
        category
    } else {
        topic
    };
    db.execute_raw(Statement::from_sql_and_values(DatabaseBackend::Sqlite,
        format!("INSERT INTO {prefix}_policies (tenant_id, {column}, minimum_trust_level, updated_at) VALUES ($1, $2, 1, CURRENT_TIMESTAMP)"),
        [tenant.into(), subject.into()],
    )).await.unwrap();
    for (suffix, extra_columns, extra_values) in [
        ("roles", "role", vec![sea_orm::Value::from("customer")]),
        (
            "channels",
            "channel_slug",
            vec![sea_orm::Value::from("members")],
        ),
        (
            "groups",
            "group_id",
            vec![sea_orm::Value::from(Uuid::new_v4())],
        ),
        (
            "users",
            "user_id, effect",
            vec![
                sea_orm::Value::from(Uuid::new_v4()),
                sea_orm::Value::from("allow"),
            ],
        ),
    ] {
        let table = format!("{prefix}_{suffix}");
        let foreign_keys = db
            .query_all_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("PRAGMA foreign_key_list({table})"),
            ))
            .await
            .unwrap();
        let columns: BTreeSet<(String, String)> = foreign_keys
            .into_iter()
            .map(|row| {
                assert_eq!(
                    row.try_get::<String>("", "table").unwrap(),
                    format!("{prefix}_policies")
                );
                (
                    row.try_get("", "from").unwrap(),
                    row.try_get("", "to").unwrap(),
                )
            })
            .collect();
        assert_eq!(
            columns,
            BTreeSet::from([
                ("tenant_id".into(), "tenant_id".into()),
                (column.into(), column.into())
            ]),
            "{table}"
        );
        let placeholders = (3..3 + extra_values.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {table} (tenant_id, {column}, {extra_columns}) VALUES ($1, $2, {placeholders})"
        );
        let mut values = vec![tenant.into(), subject.into()];
        values.extend(extra_values.clone());
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql.clone(),
            values,
        ))
        .await
        .unwrap();
        let mut foreign_values = vec![Uuid::new_v4().into(), subject.into()];
        foreign_values.extend(extra_values);
        let error = db
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                foreign_values,
            ))
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("FOREIGN KEY constraint failed"),
            "{table}: {error}"
        );
    }
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        format!("DELETE FROM {prefix}_policies WHERE tenant_id = $1 AND {column} = $2"),
        [tenant.into(), subject.into()],
    ))
    .await
    .unwrap();
    for suffix in ["roles", "channels", "groups", "users"] {
        let count: i64 = db
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("SELECT COUNT(*) FROM {prefix}_{suffix}"),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get_by_index(0)
            .unwrap();
        assert_eq!(count, 0, "clearing {prefix} must cascade through {suffix}");
    }
}

macro_rules! schema_test {
    ($name:ident, $prefix:literal, $column:literal) => {
        #[tokio::test]
        async fn $name() {
            verify_relations($prefix, $column).await;
        }
    };
}
schema_test!(
    category_visibility_relations_are_tenant_scoped,
    "forum_category_audience",
    "category_id"
);
schema_test!(
    topic_creation_relations_are_tenant_scoped,
    "forum_category_topic_create_audience",
    "category_id"
);
schema_test!(
    category_reply_creation_relations_are_tenant_scoped,
    "forum_category_reply_create_audience",
    "category_id"
);
schema_test!(
    moderation_relations_are_tenant_scoped,
    "forum_category_moderation_audience",
    "category_id"
);
schema_test!(
    topic_visibility_relations_are_tenant_scoped,
    "forum_topic_audience",
    "topic_id"
);
schema_test!(
    topic_reply_creation_relations_are_tenant_scoped,
    "forum_topic_reply_create_audience",
    "topic_id"
);

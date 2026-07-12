use std::collections::{HashMap, HashSet};

use sea_orm::sea_query::{Alias, Expr, OnConflict, Query};
use sea_orm::{ConnectionTrait, QueryResult, Statement};
use sea_orm_migration::prelude::*;
use serde_json::{Map, Value};
use uuid::Uuid;

use rustok_api::{normalize_locale_tag, PLATFORM_FALLBACK_LOCALE};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Clone, Copy)]
struct DonorSpec {
    entity_type: &'static str,
    donor_table: &'static str,
    definition_table: &'static str,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let tenant_locales = load_tenant_default_locales(connection).await?;

        for spec in donor_specs() {
            backfill_donor(connection, spec, &tenant_locales).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let tenant_locales = load_tenant_default_locales(connection).await?;

        for spec in donor_specs() {
            restore_donor(connection, spec, &tenant_locales).await?;
        }

        Ok(())
    }
}

fn donor_specs() -> [DonorSpec; 4] {
    [
        DonorSpec {
            entity_type: "user",
            donor_table: "users",
            definition_table: "user_field_definitions",
        },
        DonorSpec {
            entity_type: "product",
            donor_table: "products",
            definition_table: "product_field_definitions",
        },
        DonorSpec {
            entity_type: "order",
            donor_table: "orders",
            definition_table: "order_field_definitions",
        },
        DonorSpec {
            entity_type: "topic",
            donor_table: "forum_topics",
            definition_table: "topic_field_definitions",
        },
    ]
}

async fn load_tenant_default_locales<C>(connection: &C) -> Result<HashMap<Uuid, String>, DbErr>
where
    C: ConnectionTrait,
{
    let rows = connection
        .query_all(Statement::from_string(
            connection.get_database_backend(),
            "SELECT id, default_locale FROM tenants".to_string(),
        ))
        .await?;

    let mut locales = HashMap::new();
    for row in rows {
        let tenant_id: Uuid = row.try_get("", "id")?;
        let locale: String = row.try_get("", "default_locale")?;
        locales.insert(
            tenant_id,
            normalize_locale_tag(&locale).unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string()),
        );
    }

    Ok(locales)
}

async fn backfill_donor<C>(
    connection: &C,
    spec: DonorSpec,
    tenant_locales: &HashMap<Uuid, String>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let localized_keys_by_tenant = load_localized_keys(connection, spec).await?;
    if localized_keys_by_tenant.is_empty() {
        return Ok(());
    }

    let rows = load_donor_rows(connection, spec.donor_table).await?;

    for row in rows {
        let entity_id: Uuid = row.try_get("", "id")?;
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        let metadata: Value = row.try_get("", "metadata")?;

        let Some(localized_keys) = localized_keys_by_tenant.get(&tenant_id) else {
            continue;
        };

        let (shared, localized) = split_metadata(&metadata, localized_keys);
        if localized.is_empty() {
            continue;
        }

        let locale = extract_locale(&metadata, tenant_locales.get(&tenant_id));

        for (field_key, field_value) in localized {
            let mut insert = Query::insert();
            insert
                .into_table(Alias::new("flex_attached_localized_values"))
                .columns([
                    Alias::new("id"),
                    Alias::new("tenant_id"),
                    Alias::new("entity_type"),
                    Alias::new("entity_id"),
                    Alias::new("field_key"),
                    Alias::new("locale"),
                    Alias::new("value"),
                ])
                .values_panic([
                    Uuid::new_v4().into(),
                    tenant_id.into(),
                    spec.entity_type.into(),
                    entity_id.into(),
                    field_key.into(),
                    locale.clone().into(),
                    field_value.into(),
                ])
                .on_conflict(
                    OnConflict::columns([
                        Alias::new("tenant_id"),
                        Alias::new("entity_type"),
                        Alias::new("entity_id"),
                        Alias::new("field_key"),
                        Alias::new("locale"),
                    ])
                    .update_column(Alias::new("value"))
                    .to_owned(),
                );

            connection
                .execute(connection.get_database_backend().build(&insert))
                .await?;
        }

        persist_shared_metadata(
            connection,
            spec.donor_table,
            entity_id,
            Value::Object(shared),
        )
        .await?;
    }

    Ok(())
}

async fn restore_donor<C>(
    connection: &C,
    spec: DonorSpec,
    tenant_locales: &HashMap<Uuid, String>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let donor_rows = load_donor_rows(connection, spec.donor_table).await?;

    for row in donor_rows {
        let entity_id: Uuid = row.try_get("", "id")?;
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        let metadata: Value = row.try_get("", "metadata")?;
        let target_locale = tenant_locales
            .get(&tenant_id)
            .cloned()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

        let localized = load_attached_localized_for_locale(
            connection,
            spec,
            tenant_id,
            entity_id,
            &target_locale,
        )
        .await?;
        if localized.is_empty() {
            continue;
        }

        let mut merged = metadata.as_object().cloned().unwrap_or_default();
        for (key, value) in localized {
            merged.insert(key, value);
        }

        persist_shared_metadata(
            connection,
            spec.donor_table,
            entity_id,
            Value::Object(merged),
        )
        .await?;
    }

    Ok(())
}

async fn load_localized_keys<C>(
    connection: &C,
    spec: DonorSpec,
) -> Result<HashMap<Uuid, HashSet<String>>, DbErr>
where
    C: ConnectionTrait,
{
    let statement = Statement::from_string(
        connection.get_database_backend(),
        format!(
            "SELECT tenant_id, field_key FROM {} WHERE is_active IS TRUE AND is_localized IS TRUE",
            spec.definition_table
        ),
    );
    let rows = connection.query_all(statement).await?;

    let mut keys_by_tenant: HashMap<Uuid, HashSet<String>> = HashMap::new();
    for row in rows {
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        let field_key: String = row.try_get("", "field_key")?;
        keys_by_tenant
            .entry(tenant_id)
            .or_default()
            .insert(field_key);
    }

    Ok(keys_by_tenant)
}

async fn load_donor_rows<C>(connection: &C, donor_table: &str) -> Result<Vec<QueryResult>, DbErr>
where
    C: ConnectionTrait,
{
    connection
        .query_all(Statement::from_string(
            connection.get_database_backend(),
            format!("SELECT id, tenant_id, metadata FROM {donor_table}"),
        ))
        .await
}

async fn load_attached_localized_for_locale<C>(
    connection: &C,
    spec: DonorSpec,
    tenant_id: Uuid,
    entity_id: Uuid,
    locale: &str,
) -> Result<Map<String, Value>, DbErr>
where
    C: ConnectionTrait,
{
    let mut select = Query::select();
    select
        .column(Alias::new("field_key"))
        .column(Alias::new("value"))
        .from(Alias::new("flex_attached_localized_values"))
        .and_where(Expr::col(Alias::new("tenant_id")).eq(tenant_id))
        .and_where(Expr::col(Alias::new("entity_type")).eq(spec.entity_type))
        .and_where(Expr::col(Alias::new("entity_id")).eq(entity_id))
        .and_where(Expr::col(Alias::new("locale")).eq(locale.to_string()));

    let rows = connection
        .query_all(connection.get_database_backend().build(&select))
        .await?;

    let mut values = Map::new();
    for row in rows {
        let field_key: String = row.try_get("", "field_key")?;
        let value: Value = row.try_get("", "value")?;
        values.insert(field_key, value);
    }

    Ok(values)
}

async fn persist_shared_metadata<C>(
    connection: &C,
    donor_table: &str,
    entity_id: Uuid,
    metadata: Value,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let mut update = Query::update();
    update
        .table(Alias::new(donor_table))
        .value(Alias::new("metadata"), metadata)
        .and_where(Expr::col(Alias::new("id")).eq(entity_id));

    connection
        .execute(connection.get_database_backend().build(&update))
        .await?;

    Ok(())
}

fn split_metadata(
    metadata: &Value,
    localized_keys: &HashSet<String>,
) -> (Map<String, Value>, Map<String, Value>) {
    let mut shared = Map::new();
    let mut localized = Map::new();

    for (key, value) in metadata.as_object().cloned().unwrap_or_default() {
        if localized_keys.contains(&key) {
            localized.insert(key, value);
        } else {
            shared.insert(key, value);
        }
    }

    (shared, localized)
}

fn extract_locale(metadata: &Value, tenant_default_locale: Option<&String>) -> String {
    metadata
        .get("locale")
        .and_then(Value::as_str)
        .or_else(|| metadata.get("locale_code").and_then(Value::as_str))
        .or_else(|| {
            metadata
                .get("cart_context")
                .and_then(Value::as_object)
                .and_then(|object| object.get("locale"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            metadata
                .get("context")
                .and_then(Value::as_object)
                .and_then(|object| object.get("locale"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            metadata
                .get("store_context")
                .and_then(Value::as_object)
                .and_then(|object| object.get("locale"))
                .and_then(Value::as_str)
        })
        .and_then(normalize_locale_tag)
        .or_else(|| tenant_default_locale.cloned())
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
}

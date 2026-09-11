//! Flex-owned governance policy for standalone localized fields.
//!
//! Standalone policy is keyed by `(tenant_id, schema_id, field_key)` and deliberately lives
//! outside `flex_schemas.fields_config`. Classification / AI-export admission are governance
//! metadata, not translated content, so changing policy must never manufacture content revisions
//! or Translation change-journal rows. Missing policy is always fail-closed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::field_schema::is_valid_field_key;
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::FlexDataClassification;

pub const FLEX_STANDALONE_FIELD_POLICIES_TABLE: &str = "flex_standalone_field_policies";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexStandaloneFieldPolicy {
    pub classification: FlexDataClassification,
    pub ai_export_allowed: bool,
}

impl Default for FlexStandaloneFieldPolicy {
    fn default() -> Self {
        Self {
            classification: FlexDataClassification::TenantPrivate,
            ai_export_allowed: false,
        }
    }
}

impl FlexStandaloneFieldPolicy {
    pub fn validate(self) -> FlexStandaloneFieldPolicyResult<()> {
        if self.ai_export_allowed
            && matches!(
                self.classification,
                FlexDataClassification::Secret | FlexDataClassification::ImmutableTransaction
            )
        {
            return Err(FlexStandaloneFieldPolicyError::Invalid(
                "AI export cannot be enabled for secret or immutable-transaction standalone fields"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

/// Effective standalone-field policy together with provenance.
///
/// `explicit=false` means no policy row exists and the returned policy is the fail-closed default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlexStandaloneFieldPolicyResolution {
    pub policy: FlexStandaloneFieldPolicy,
    pub explicit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlexStandaloneFieldPolicyError {
    Invalid(String),
    Storage(String),
}

impl fmt::Display for FlexStandaloneFieldPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) | Self::Storage(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for FlexStandaloneFieldPolicyError {}

pub type FlexStandaloneFieldPolicyResult<T> = Result<T, FlexStandaloneFieldPolicyError>;

mod policy_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "flex_standalone_field_policies")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub tenant_id: Uuid,
        #[sea_orm(primary_key, auto_increment = false)]
        pub schema_id: Uuid,
        #[sea_orm(primary_key, auto_increment = false)]
        pub field_key: String,
        pub classification: FlexDataClassification,
        pub ai_export_allowed: bool,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

fn validate_scope(tenant_id: Uuid, schema_id: Uuid) -> FlexStandaloneFieldPolicyResult<()> {
    if tenant_id.is_nil() {
        return Err(FlexStandaloneFieldPolicyError::Invalid(
            "standalone field policy tenant_id must not be the nil UUID".to_string(),
        ));
    }
    if schema_id.is_nil() {
        return Err(FlexStandaloneFieldPolicyError::Invalid(
            "standalone field policy schema_id must not be the nil UUID".to_string(),
        ));
    }
    Ok(())
}

fn validate_field_key(field_key: &str) -> FlexStandaloneFieldPolicyResult<()> {
    if !is_valid_field_key(field_key) {
        return Err(FlexStandaloneFieldPolicyError::Invalid(format!(
            "standalone field policy field_key is invalid: {field_key}"
        )));
    }
    Ok(())
}

/// Resolve policy plus provenance for a bounded standalone-field set in one caller-owned database
/// snapshot.
///
/// Every requested field is returned. A missing row resolves to `TenantPrivate` with AI export
/// disabled and `explicit=false`; explicit rows are validated before they can influence a consumer.
pub async fn resolve_standalone_field_policy_resolutions<C>(
    db: &C,
    tenant_id: Uuid,
    schema_id: Uuid,
    field_keys: &[String],
) -> FlexStandaloneFieldPolicyResult<BTreeMap<String, FlexStandaloneFieldPolicyResolution>>
where
    C: ConnectionTrait,
{
    validate_scope(tenant_id, schema_id)?;
    let mut unique = BTreeSet::new();
    for field_key in field_keys {
        validate_field_key(field_key)?;
        unique.insert(field_key.clone());
    }
    if unique.is_empty() {
        return Ok(BTreeMap::new());
    }

    let mut resolved = unique
        .iter()
        .cloned()
        .map(|field_key| (field_key, FlexStandaloneFieldPolicyResolution::default()))
        .collect::<BTreeMap<_, _>>();
    let rows = policy_entity::Entity::find()
        .filter(policy_entity::Column::TenantId.eq(tenant_id))
        .filter(policy_entity::Column::SchemaId.eq(schema_id))
        .filter(policy_entity::Column::FieldKey.is_in(unique.iter().cloned()))
        .all(db)
        .await
        .map_err(|error| FlexStandaloneFieldPolicyError::Storage(error.to_string()))?;
    for row in rows {
        let policy = FlexStandaloneFieldPolicy {
            classification: row.classification,
            ai_export_allowed: row.ai_export_allowed,
        };
        policy.validate()?;
        resolved.insert(
            row.field_key,
            FlexStandaloneFieldPolicyResolution {
                policy,
                explicit: true,
            },
        );
    }
    Ok(resolved)
}

/// Resolve effective policy for a bounded standalone-field set in one caller-owned database snapshot.
pub async fn resolve_standalone_field_policies<C>(
    db: &C,
    tenant_id: Uuid,
    schema_id: Uuid,
    field_keys: &[String],
) -> FlexStandaloneFieldPolicyResult<BTreeMap<String, FlexStandaloneFieldPolicy>>
where
    C: ConnectionTrait,
{
    Ok(
        resolve_standalone_field_policy_resolutions(db, tenant_id, schema_id, field_keys)
            .await?
            .into_iter()
            .map(|(field_key, resolution)| (field_key, resolution.policy))
            .collect(),
    )
}

/// Atomically create or replace one explicit policy row.
pub async fn upsert_standalone_field_policy<C>(
    db: &C,
    tenant_id: Uuid,
    schema_id: Uuid,
    field_key: &str,
    policy: FlexStandaloneFieldPolicy,
) -> FlexStandaloneFieldPolicyResult<()>
where
    C: ConnectionTrait,
{
    validate_scope(tenant_id, schema_id)?;
    validate_field_key(field_key)?;
    policy.validate()?;

    let now = Utc::now().fixed_offset();
    let model = policy_entity::ActiveModel {
        tenant_id: Set(tenant_id),
        schema_id: Set(schema_id),
        field_key: Set(field_key.to_string()),
        classification: Set(policy.classification),
        ai_export_allowed: Set(policy.ai_export_allowed),
        created_at: Set(now),
        updated_at: Set(now),
    };
    policy_entity::Entity::insert(model)
        .on_conflict(
            OnConflict::columns([
                policy_entity::Column::TenantId,
                policy_entity::Column::SchemaId,
                policy_entity::Column::FieldKey,
            ])
            .update_columns([
                policy_entity::Column::Classification,
                policy_entity::Column::AiExportAllowed,
                policy_entity::Column::UpdatedAt,
            ])
            .to_owned(),
        )
        .exec(db)
        .await
        .map_err(|error| FlexStandaloneFieldPolicyError::Storage(error.to_string()))?;
    Ok(())
}

/// Remove an explicit policy row. Resolution immediately falls back to the fail-closed default.
pub async fn delete_standalone_field_policy<C>(
    db: &C,
    tenant_id: Uuid,
    schema_id: Uuid,
    field_key: &str,
) -> FlexStandaloneFieldPolicyResult<bool>
where
    C: ConnectionTrait,
{
    validate_scope(tenant_id, schema_id)?;
    validate_field_key(field_key)?;
    let result = policy_entity::Entity::delete_many()
        .filter(policy_entity::Column::TenantId.eq(tenant_id))
        .filter(policy_entity::Column::SchemaId.eq(schema_id))
        .filter(policy_entity::Column::FieldKey.eq(field_key))
        .exec(db)
        .await
        .map_err(|error| FlexStandaloneFieldPolicyError::Storage(error.to_string()))?;
    Ok(result.rows_affected > 0)
}

#[async_trait]
pub trait FlexStandaloneFieldPolicyResolver: Send + Sync {
    async fn resolve(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        field_keys: &[String],
    ) -> FlexStandaloneFieldPolicyResult<BTreeMap<String, FlexStandaloneFieldPolicy>>;
}

/// Reusable database-backed resolver for Translation and future AI/export consumers.
#[derive(Clone)]
pub struct FlexStandaloneFieldPolicyStore {
    db: DatabaseConnection,
}

impl FlexStandaloneFieldPolicyStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn upsert(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        field_key: &str,
        policy: FlexStandaloneFieldPolicy,
    ) -> FlexStandaloneFieldPolicyResult<()> {
        upsert_standalone_field_policy(&self.db, tenant_id, schema_id, field_key, policy).await
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        field_key: &str,
    ) -> FlexStandaloneFieldPolicyResult<bool> {
        delete_standalone_field_policy(&self.db, tenant_id, schema_id, field_key).await
    }
}

#[async_trait]
impl FlexStandaloneFieldPolicyResolver for FlexStandaloneFieldPolicyStore {
    async fn resolve(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        field_keys: &[String],
    ) -> FlexStandaloneFieldPolicyResult<BTreeMap<String, FlexStandaloneFieldPolicy>> {
        resolve_standalone_field_policies(&self.db, tenant_id, schema_id, field_keys).await
    }
}

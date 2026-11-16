//! Flex-owned governance policy for attached dynamic fields.
//!
//! Policy is intentionally separate from donor field-definition storage. Historical donors may
//! keep owner-specific definition tables while sharing the same classification / AI-export
//! contract keyed by `(tenant_id, entity_type, field_key)`. Missing policy is always fail-closed.

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::field_schema::is_valid_field_key;
use rustok_translation_targets::TranslationDataClassification;
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    sea_query::OnConflict,
};
use uuid::Uuid;

use crate::is_valid_flex_entity_type;

pub const FLEX_ATTACHED_FIELD_POLICIES_TABLE: &str = "flex_attached_field_policies";

pub use flex_ports::{
    FlexAttachedFieldPolicy, FlexAttachedFieldPolicyError, FlexAttachedFieldPolicyResolution,
    FlexAttachedFieldPolicyResult, FlexDataClassification,
};

pub(crate) fn translation_data_classification(
    value: FlexDataClassification,
) -> TranslationDataClassification {
    match value {
        FlexDataClassification::Public => TranslationDataClassification::Public,
        FlexDataClassification::TenantPrivate => TranslationDataClassification::TenantPrivate,
        FlexDataClassification::Personal => TranslationDataClassification::Personal,
        FlexDataClassification::Sensitive => TranslationDataClassification::Sensitive,
        FlexDataClassification::Secret => TranslationDataClassification::Secret,
        FlexDataClassification::ImmutableTransaction => {
            TranslationDataClassification::ImmutableTransaction
        }
    }
}

mod policy_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "flex_attached_field_policies")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub tenant_id: Uuid,
        #[sea_orm(primary_key, auto_increment = false)]
        pub entity_type: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub field_key: String,
        pub classification: String,
        pub ai_export_allowed: bool,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

fn validate_scope(tenant_id: Uuid, entity_type: &str) -> FlexAttachedFieldPolicyResult<()> {
    if tenant_id.is_nil() {
        return Err(FlexAttachedFieldPolicyError::Invalid(
            "attached field policy tenant_id must not be the nil UUID".to_string(),
        ));
    }
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexAttachedFieldPolicyError::Invalid(format!(
            "attached field policy entity_type is invalid: {entity_type}"
        )));
    }
    Ok(())
}

fn validate_field_key(field_key: &str) -> FlexAttachedFieldPolicyResult<()> {
    if !is_valid_field_key(field_key) {
        return Err(FlexAttachedFieldPolicyError::Invalid(format!(
            "attached field policy field_key is invalid: {field_key}"
        )));
    }
    Ok(())
}

/// Resolve policy plus provenance for a bounded attached-field set in one caller-owned database
/// snapshot.
///
/// Every requested field is returned. A missing row resolves to `TenantPrivate` with AI export
/// disabled and `explicit=false`; explicit rows are validated before they can influence a consumer.
pub async fn resolve_attached_field_policy_resolutions<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    field_keys: &[String],
) -> FlexAttachedFieldPolicyResult<BTreeMap<String, FlexAttachedFieldPolicyResolution>>
where
    C: ConnectionTrait,
{
    validate_scope(tenant_id, entity_type)?;
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
        .map(|field_key| (field_key, FlexAttachedFieldPolicyResolution::default()))
        .collect::<BTreeMap<_, _>>();
    let rows = policy_entity::Entity::find()
        .filter(policy_entity::Column::TenantId.eq(tenant_id))
        .filter(policy_entity::Column::EntityType.eq(entity_type))
        .filter(policy_entity::Column::FieldKey.is_in(unique.iter().cloned()))
        .all(db)
        .await
        .map_err(|error| FlexAttachedFieldPolicyError::Storage(error.to_string()))?;
    for row in rows {
        let policy = FlexAttachedFieldPolicy {
            classification: row.classification.parse().map_err(|_| {
                FlexAttachedFieldPolicyError::Storage(
                    "Invalid stored attached field classification".into(),
                )
            })?,
            ai_export_allowed: row.ai_export_allowed,
        };
        policy.validate()?;
        resolved.insert(
            row.field_key,
            FlexAttachedFieldPolicyResolution {
                policy,
                explicit: true,
            },
        );
    }
    Ok(resolved)
}

/// Resolve effective policy for a bounded attached-field set in one caller-owned database snapshot.
pub async fn resolve_attached_field_policies<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    field_keys: &[String],
) -> FlexAttachedFieldPolicyResult<BTreeMap<String, FlexAttachedFieldPolicy>>
where
    C: ConnectionTrait,
{
    Ok(
        resolve_attached_field_policy_resolutions(db, tenant_id, entity_type, field_keys)
            .await?
            .into_iter()
            .map(|(field_key, resolution)| (field_key, resolution.policy))
            .collect(),
    )
}

/// Atomically create or replace one explicit policy row.
pub async fn upsert_attached_field_policy<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    field_key: &str,
    policy: FlexAttachedFieldPolicy,
) -> FlexAttachedFieldPolicyResult<()>
where
    C: ConnectionTrait,
{
    validate_scope(tenant_id, entity_type)?;
    validate_field_key(field_key)?;
    policy.validate()?;

    let now = Utc::now().fixed_offset();
    let model = policy_entity::ActiveModel {
        tenant_id: Set(tenant_id),
        entity_type: Set(entity_type.to_string()),
        field_key: Set(field_key.to_string()),
        classification: Set(policy.classification.as_str().to_owned()),
        ai_export_allowed: Set(policy.ai_export_allowed),
        created_at: Set(now),
        updated_at: Set(now),
    };
    policy_entity::Entity::insert(model)
        .on_conflict(
            OnConflict::columns([
                policy_entity::Column::TenantId,
                policy_entity::Column::EntityType,
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
        .map_err(|error| FlexAttachedFieldPolicyError::Storage(error.to_string()))?;
    Ok(())
}

/// Remove an explicit policy row. Resolution immediately falls back to the fail-closed default.
pub async fn delete_attached_field_policy<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    field_key: &str,
) -> FlexAttachedFieldPolicyResult<bool>
where
    C: ConnectionTrait,
{
    validate_scope(tenant_id, entity_type)?;
    validate_field_key(field_key)?;
    let result = policy_entity::Entity::delete_many()
        .filter(policy_entity::Column::TenantId.eq(tenant_id))
        .filter(policy_entity::Column::EntityType.eq(entity_type))
        .filter(policy_entity::Column::FieldKey.eq(field_key))
        .exec(db)
        .await
        .map_err(|error| FlexAttachedFieldPolicyError::Storage(error.to_string()))?;
    Ok(result.rows_affected > 0)
}

#[async_trait]
pub trait FlexAttachedFieldPolicyResolver: Send + Sync {
    async fn resolve(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        field_keys: &[String],
    ) -> FlexAttachedFieldPolicyResult<BTreeMap<String, FlexAttachedFieldPolicy>>;
}

/// Reusable database-backed resolver for Translation and future AI/export consumers.
#[derive(Clone)]
pub struct FlexAttachedFieldPolicyStore {
    db: DatabaseConnection,
}

impl FlexAttachedFieldPolicyStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn upsert(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        field_key: &str,
        policy: FlexAttachedFieldPolicy,
    ) -> FlexAttachedFieldPolicyResult<()> {
        upsert_attached_field_policy(&self.db, tenant_id, entity_type, field_key, policy).await
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        field_key: &str,
    ) -> FlexAttachedFieldPolicyResult<bool> {
        delete_attached_field_policy(&self.db, tenant_id, entity_type, field_key).await
    }
}

#[async_trait]
impl FlexAttachedFieldPolicyResolver for FlexAttachedFieldPolicyStore {
    async fn resolve(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        field_keys: &[String],
    ) -> FlexAttachedFieldPolicyResult<BTreeMap<String, FlexAttachedFieldPolicy>> {
        resolve_attached_field_policies(&self.db, tenant_id, entity_type, field_keys).await
    }
}

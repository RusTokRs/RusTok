use std::{collections::BTreeSet, sync::Arc};

use rustok_api::{Action, PortCallPolicy, PortContext, Resource, TenantLocale};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::TransactionalEventBus;
use rustok_tenant::TenantLocalePolicyPort;
use rustok_translation_targets::{
    FieldKey, OpaqueRevision, TranslationDataClassification, TranslationResourceIdentity,
    TranslationResourceSnapshot, TranslationStrategy, TranslationTargetRegistry,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ProposalOrigin, ProposalRecord, ProposalValue, SaveProposalInput, TranslationError,
    TranslationResult, TranslationWorkflowService,
    entities::{job, job_item},
};

pub const INTERCHANGE_SCHEMA_VERSION: u16 = 1;
const MAX_EXPORT_ITEMS: u16 = 200;
const MAX_FIELDS_PER_ITEM: usize = 200;
const MAX_FIELD_VALUE_BYTES: usize = 32 * 1024;
const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportTranslationJobInput {
    pub job_id: Uuid,
    pub max_items: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportTranslationItemInput {
    pub schema_version: u16,
    pub job_id: Uuid,
    pub item_id: Uuid,
    pub identity: TranslationResourceIdentity,
    pub source_digest: String,
    pub values: Vec<ProposalValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationInterchangeDocument {
    pub schema_version: u16,
    pub job_id: Uuid,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub items: Vec<TranslationInterchangeItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationInterchangeItem {
    pub item_id: Uuid,
    #[serde(with = "interchange_identity")]
    pub identity: TranslationResourceIdentity,
    pub source_digest: String,
    pub source_revision: OpaqueRevision,
    pub target_revision: Option<OpaqueRevision>,
    pub fields: Vec<TranslationInterchangeField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationInterchangeField {
    pub key: FieldKey,
    pub source_value: String,
    pub exact_target_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed_value: Option<String>,
    pub source_hash: String,
    pub required: bool,
    pub max_characters: Option<u32>,
    pub protected_tokens: Vec<String>,
}

/// The artifact wire format is a camel-case public document even though the
/// reusable target identity uses Rust field names in its own serialization.
/// Keeping the conversion here prevents the target SPI's internal serde shape
/// from leaking into the Translation interchange contract.
mod interchange_identity {
    use rustok_translation_targets::{
        OwnerSlug, ResourceId, ResourceKind, TranslationResourceIdentity,
    };
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct WireIdentity {
        owner_slug: OwnerSlug,
        resource_kind: ResourceKind,
        resource_id: ResourceId,
        subresource_id: Option<ResourceId>,
    }

    impl From<&TranslationResourceIdentity> for WireIdentity {
        fn from(value: &TranslationResourceIdentity) -> Self {
            Self {
                owner_slug: value.owner_slug.clone(),
                resource_kind: value.resource_kind.clone(),
                resource_id: value.resource_id.clone(),
                subresource_id: value.subresource_id.clone(),
            }
        }
    }

    impl From<WireIdentity> for TranslationResourceIdentity {
        fn from(value: WireIdentity) -> Self {
            Self {
                owner_slug: value.owner_slug,
                resource_kind: value.resource_kind,
                resource_id: value.resource_id,
                subresource_id: value.subresource_id,
            }
        }
    }

    pub fn serialize<S>(
        value: &TranslationResourceIdentity,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        WireIdentity::from(value).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<TranslationResourceIdentity, D::Error>
    where
        D: Deserializer<'de>,
    {
        WireIdentity::deserialize(deserializer).map(Into::into)
    }
}

pub struct TranslationInterchangeService {
    database: DatabaseConnection,
    workflow: TranslationWorkflowService,
}

impl TranslationInterchangeService {
    pub fn new(
        database: DatabaseConnection,
        providers: Arc<TranslationTargetRegistry>,
        tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
        event_bus: TransactionalEventBus,
    ) -> Self {
        Self {
            database: database.clone(),
            workflow: TranslationWorkflowService::new(
                database,
                providers,
                tenant_locale_policies,
                event_bus,
            ),
        }
    }

    pub async fn export_job(
        &self,
        context: PortContext,
        input: ExportTranslationJobInput,
    ) -> TranslationResult<TranslationInterchangeDocument> {
        let tenant_id = authorize(&context, Action::Export, PortCallPolicy::read())?;
        if input.max_items == 0 || input.max_items > MAX_EXPORT_ITEMS {
            return Err(TranslationError::InvalidRequest(format!(
                "translation export max_items must be between 1 and {MAX_EXPORT_ITEMS}"
            )));
        }
        let job = job::Entity::find_by_id(input.job_id)
            .filter(job::Column::TenantId.eq(tenant_id))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::JobNotFound)?;
        let source_locale = TenantLocale::new(&job.source_locale)
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        let target_locale = TenantLocale::new(&job.target_locale)
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        let rows = job_item::Entity::find()
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::JobId.eq(job.id))
            .order_by_asc(job_item::Column::CreatedAt)
            .order_by_asc(job_item::Column::Id)
            .limit(u64::from(input.max_items) + 1)
            .all(&self.database)
            .await?;
        if rows.len() > usize::from(input.max_items) {
            return Err(TranslationError::InvalidRequest(
                "translation export exceeds the requested bounded item count".to_string(),
            ));
        }

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let snapshot: TranslationResourceSnapshot =
                serde_json::from_value(row.source_snapshot.clone())?;
            validate_snapshot_binding(&row, &snapshot, &source_locale, &target_locale)?;
            let fields = snapshot
                .fields
                .iter()
                .filter(|field| export_safe(field))
                .map(|field| TranslationInterchangeField {
                    key: field.descriptor.key.clone(),
                    source_value: field.source_value.clone(),
                    exact_target_value: field.exact_target_value.clone(),
                    proposed_value: None,
                    source_hash: field.source_hash.clone(),
                    required: field.descriptor.required,
                    max_characters: field.descriptor.max_characters,
                    protected_tokens: field.protected_tokens.clone(),
                })
                .collect::<Vec<_>>();
            validate_export_fields(&fields)?;
            items.push(TranslationInterchangeItem {
                item_id: row.id,
                identity: snapshot.summary.identity,
                source_digest: row.source_digest,
                source_revision: snapshot.source_revision,
                target_revision: snapshot.target_revision,
                fields,
            });
        }
        let document = TranslationInterchangeDocument {
            schema_version: INTERCHANGE_SCHEMA_VERSION,
            job_id: job.id,
            source_locale,
            target_locale,
            items,
        };
        if serde_json::to_vec(&document)?.len() > MAX_DOCUMENT_BYTES {
            return Err(TranslationError::InvalidRequest(
                "translation export exceeds the document byte bound".to_string(),
            ));
        }
        Ok(document)
    }

    pub async fn import_item(
        &self,
        context: PortContext,
        input: ImportTranslationItemInput,
    ) -> TranslationResult<ProposalRecord> {
        validate_import_input(&input)?;
        let tenant_id = authorize(&context, Action::Import, PortCallPolicy::write())?;
        let row = job_item::Entity::find_by_id(input.item_id)
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::JobId.eq(input.job_id))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::ItemNotFound)?;
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(row.source_snapshot.clone())?;
        if snapshot.summary.identity != input.identity || row.source_digest != input.source_digest {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let allowed = snapshot
            .fields
            .iter()
            .filter(|field| export_safe(field))
            .map(|field| field.descriptor.key.as_str())
            .collect::<BTreeSet<_>>();
        if input
            .values
            .iter()
            .any(|value| !allowed.contains(value.key.as_str()))
        {
            return Err(TranslationError::InvalidRequest(
                "translation import contains a field that was not eligible for interchange"
                    .to_string(),
            ));
        }
        self.workflow
            .save_proposal(
                context,
                SaveProposalInput {
                    item_id: input.item_id,
                    origin: ProposalOrigin::Import,
                    values: input.values,
                },
            )
            .await
    }
}

fn validate_import_input(input: &ImportTranslationItemInput) -> TranslationResult<()> {
    if input.schema_version != INTERCHANGE_SCHEMA_VERSION {
        return Err(TranslationError::InvalidRequest(
            "translation interchange schema version is unsupported".to_string(),
        ));
    }
    if input.values.is_empty() || input.values.len() > MAX_FIELDS_PER_ITEM {
        return Err(TranslationError::InvalidRequest(format!(
            "translation import must contain between 1 and {MAX_FIELDS_PER_ITEM} fields"
        )));
    }
    let mut keys = BTreeSet::new();
    for value in &input.values {
        if !keys.insert(value.key.as_str()) {
            return Err(TranslationError::InvalidRequest(
                "translation import contains a duplicate field".to_string(),
            ));
        }
        if value.value.len() > MAX_FIELD_VALUE_BYTES {
            return Err(TranslationError::InvalidRequest(
                "translation import field exceeds the byte bound".to_string(),
            ));
        }
    }
    if serde_json::to_vec(input)?.len() > MAX_DOCUMENT_BYTES {
        return Err(TranslationError::InvalidRequest(
            "translation import exceeds the document byte bound".to_string(),
        ));
    }
    Ok(())
}

fn validate_export_fields(fields: &[TranslationInterchangeField]) -> TranslationResult<()> {
    if fields.len() > MAX_FIELDS_PER_ITEM
        || fields.iter().any(|field| {
            field.source_value.len() > MAX_FIELD_VALUE_BYTES
                || field
                    .exact_target_value
                    .as_ref()
                    .is_some_and(|value| value.len() > MAX_FIELD_VALUE_BYTES)
        })
    {
        return Err(TranslationError::InvalidRequest(
            "translation export field bounds were exceeded".to_string(),
        ));
    }
    Ok(())
}

fn export_safe(field: &rustok_translation_targets::TranslationFieldSnapshot) -> bool {
    !matches!(field.descriptor.strategy, TranslationStrategy::Excluded)
        && matches!(
            field.descriptor.classification,
            TranslationDataClassification::Public | TranslationDataClassification::TenantPrivate
        )
}

fn validate_snapshot_binding(
    row: &job_item::Model,
    snapshot: &TranslationResourceSnapshot,
    source_locale: &TenantLocale,
    target_locale: &TenantLocale,
) -> TranslationResult<()> {
    snapshot
        .validate()
        .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
    if &snapshot.source_locale != source_locale
        || &snapshot.target_locale != target_locale
        || snapshot.summary.identity.owner_slug.as_str() != row.owner_slug
        || snapshot.summary.identity.resource_kind.as_str() != row.resource_kind
        || snapshot.summary.identity.resource_id.as_str() != row.resource_id
    {
        return Err(TranslationError::WorkflowRevisionConflict);
    }
    Ok(())
}

fn authorize(
    context: &PortContext,
    action: Action,
    policy: PortCallPolicy,
) -> TranslationResult<Uuid> {
    context.require_policy(policy)?;
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Translations, action) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

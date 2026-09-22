use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Action, PortContext, PortError, Resource, TenantLocale, sha256_digest};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::TransactionalEventBus;
use rustok_seo_targets::{SeoTargetRegistry, SeoTargetSlug};
use rustok_translation_targets::provider_support::{
    contract_validation_error, field_hash, merged_patch_values, normalize_optional_target_value,
    parse_resource_lifecycle, read_request_from_patch, validate_patch_against_snapshot,
    validation_to_port_error,
};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, OpaqueRevision, OwnerSlug,
    ReadTranslationResourceRequest, ResourceId, ResourceKind, TranslationApplicationReceipt,
    TranslationDataClassification, TranslationFieldDescriptor, TranslationFieldSnapshot,
    TranslationPatchIssue, TranslationPatchIssueSeverity, TranslationPatchRequest,
    TranslationPatchValidation, TranslationResourceIdentity, TranslationResourceLifecycle,
    TranslationResourcePage, TranslationResourceSnapshot, TranslationResourceSummary,
    TranslationStrategy, TranslationTargetCapability, TranslationTargetChange,
    TranslationTargetChangePage, TranslationTargetChangesRequest, TranslationTargetProgressFacts,
    TranslationTargetProgressRequest, TranslationTargetProvider,
    TranslationTargetProviderDescriptor, TranslationTargetRegistryError, TranslationValueProfile,
    register_translation_target_provider, validate_translation_apply_context,
    validate_translation_read_context,
};
use sea_orm::{
    AccessMode, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, FromQueryResult, IsolationLevel,
    QueryFilter, QueryOrder, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::SeoError;
use crate::entities as seo_meta;
use crate::entities::meta_translation;

use super::events::SeoMetaUpsertedEventInput;
use super::{SeoService, trimmed_option};

const OWNER_SLUG: &str = "seo";
const RESOURCE_KIND: &str = "seo_copy";
const RESOURCE_STATE_TABLE: &str = "seo_translation_resource_state";
const LOCALE_STATE_TABLE: &str = "seo_translation_locale_state";
const CHANGE_JOURNAL_TABLE: &str = "seo_translation_change_journal";
const APPLY_RECEIPTS_TABLE: &str = "seo_translation_apply_receipts";
const PROGRESS_STABILITY_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct SeoTranslationTargetProvider {
    db: DatabaseConnection,
    service: SeoService,
}

impl SeoTranslationTargetProvider {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            service: SeoService::new(
                db.clone(),
                event_bus,
                Arc::new(SeoTargetRegistry::default()),
            ),
            db,
        }
    }

    fn descriptor_value() -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new(OWNER_SLUG)
                .expect("static SEO owner slug must satisfy Translation target contract"),
            resource_kind: ResourceKind::new(RESOURCE_KIND)
                .expect("static SEO resource kind must satisfy Translation target contract"),
            display_name: "SEO explicit copy".to_string(),
            capabilities: BTreeSet::from([
                TranslationTargetCapability::ListResources,
                TranslationTargetCapability::ReadExactResource,
                TranslationTargetCapability::AggregateProgress,
                TranslationTargetCapability::ValidatePatch,
                TranslationTargetCapability::ApplyPatch,
                TranslationTargetCapability::ChangeCursor,
            ]),
            read_permission_floor: BTreeSet::from(["seo:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["seo:update".to_string()]),
        }
    }

    async fn list_resources_in(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        request: &ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        let after = request
            .cursor
            .as_ref()
            .map(parse_resource_cursor)
            .transpose()?;
        let rows = list_source_rows(
            txn,
            tenant_id,
            request.source_locale.as_str(),
            after.as_ref(),
            request.limit.saturating_add(1),
        )
        .await?;
        let has_more = rows.len() > usize::from(request.limit);
        let visible = rows
            .into_iter()
            .take(usize::from(request.limit))
            .collect::<Vec<_>>();

        let mut resources = Vec::with_capacity(visible.len());
        for row in &visible {
            let exact_locales = exact_locales_in(txn, row.meta_id).await?;
            resources.push(TranslationResourceSummary {
                identity: resource_identity(&row.target_kind, row.target_id)?,
                display_label: row
                    .title
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{}/{}", row.target_kind, row.target_id)),
                lifecycle: TranslationResourceLifecycle::Active,
                resource_revision: seo_resource_revision(required_positive_revision(
                    row.resource_revision,
                    "resource",
                )?)?,
                exact_locales,
            });
        }

        let next_cursor = if has_more {
            visible
                .last()
                .map(|row| {
                    OpaqueCursor::new(format!("{}:{}", row.target_kind, row.target_id))
                        .map_err(|error| contract_validation_error(error.to_string()))
                })
                .transpose()?
        } else {
            None
        };

        Ok(TranslationResourcePage {
            resources,
            next_cursor,
        })
    }

    async fn read_snapshot(
        &self,
        tenant_id: Uuid,
        request: &ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        ensure_postgres(&self.db)?;
        let txn = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(database_error)?;
        let snapshot = load_snapshot_in(&txn, tenant_id, request).await?;
        txn.commit().await.map_err(database_error)?;
        Ok(snapshot)
    }

    async fn apply_patch_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_user_id: Uuid,
        idempotency_key: &str,
        request_fingerprint: &str,
        request: &TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        let (target_kind, target_id) = parse_resource_identity(&request.identity)?;
        advisory_lock(txn, tenant_id, idempotency_key).await?;

        if let Some(receipt) = load_receipt(txn, tenant_id, idempotency_key).await? {
            if receipt.actor_user_id != actor_user_id
                || receipt.target_kind != target_kind.as_str()
                || receipt.target_id != target_id
                || receipt.request_fingerprint != request_fingerprint
            {
                return Err(PortError::conflict(
                    "seo.translation_idempotency_conflict",
                    "SEO Translation idempotency key is already bound to a different request",
                ));
            }
            return receipt_to_application(receipt, request);
        }

        let meta = lock_meta(txn, tenant_id, target_kind.as_str(), target_id).await?;
        let read_request = read_request_from_patch(request);
        let before = load_snapshot_in(txn, tenant_id, &read_request).await?;
        let validation = validate_patch_for_snapshot(request, &before);
        if !validation.accepted {
            return Err(validation_to_port_error(&validation));
        }

        let target_copy = merged_target_copy(request, &before);
        let existing_target = meta_translation::Entity::find()
            .filter(meta_translation::Column::MetaId.eq(meta.id))
            .filter(meta_translation::Column::Locale.eq(request.target_locale.as_str()))
            .one(txn)
            .await
            .map_err(database_error)?;
        let before_copy = existing_target
            .as_ref()
            .map(SeoCopy::from_model)
            .unwrap_or_default();
        let changed = before_copy != target_copy;

        if changed {
            if let Some(existing) = existing_target {
                let mut active: meta_translation::ActiveModel = existing.into();
                active.title = Set(target_copy.title.clone());
                active.description = Set(target_copy.description.clone());
                active.keywords = Set(target_copy.keywords.clone());
                active.og_title = Set(target_copy.og_title.clone());
                active.og_description = Set(target_copy.og_description.clone());
                active.update(txn).await.map_err(database_error)?;
            } else {
                meta_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    meta_id: Set(meta.id),
                    locale: Set(request.target_locale.as_str().to_string()),
                    title: Set(target_copy.title.clone()),
                    description: Set(target_copy.description.clone()),
                    keywords: Set(target_copy.keywords.clone()),
                    og_title: Set(target_copy.og_title.clone()),
                    og_description: Set(target_copy.og_description.clone()),
                    og_image: Set(None),
                }
                .insert(txn)
                .await
                .map_err(database_error)?;
            }

            let transition_ref = format!("translation:{}", request.proposal_id.trim());
            self.service
                .publish_seo_meta_upserted_event_in_tx(
                    txn,
                    SeoMetaUpsertedEventInput {
                        tenant_id,
                        target_kind: target_kind.as_str(),
                        target_id,
                        locale: request.target_locale.as_str(),
                        source: "translation",
                        transition_ref: Some(transition_ref.as_str()),
                    },
                )
                .await
                .map_err(seo_error_to_port_error)?;
        }

        let after = load_snapshot_in(txn, tenant_id, &read_request).await?;
        verify_target_copy(&target_copy, &after)?;
        let target_revision = after.target_revision.clone().ok_or_else(|| {
            PortError::invariant_violation(
                "seo.translation_target_revision_missing",
                "SEO exact target copy is missing its owner revision",
            )
        })?;
        let operation_id = Uuid::new_v4();
        insert_receipt(
            txn,
            ReceiptInsert {
                tenant_id,
                idempotency_key,
                actor_user_id,
                target_kind: target_kind.as_str(),
                target_id,
                request_fingerprint,
                operation_id,
                resource_revision: after.summary.resource_revision.as_str(),
                source_revision: after.source_revision.as_str(),
                target_revision: target_revision.as_str(),
            },
        )
        .await?;

        Ok(TranslationApplicationReceipt {
            provider_receipt_id: operation_id.to_string(),
            resource_revision: after.summary.resource_revision,
            target_revision,
            applied_field_keys: request
                .fields
                .iter()
                .map(|field| field.key.clone())
                .collect(),
        })
    }
}

#[async_trait]
impl TranslationTargetProvider for SeoTranslationTargetProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        Self::descriptor_value()
    }

    async fn list_resources(
        &self,
        context: PortContext,
        request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        ensure_postgres(&self.db)?;
        let tenant_id = tenant_id(&context)?;

        let txn = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(database_error)?;
        let page = self.list_resources_in(&txn, tenant_id, &request).await?;
        txn.commit().await.map_err(database_error)?;
        Ok(page)
    }

    async fn read_resource(
        &self,
        context: PortContext,
        request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        let tenant_id = tenant_id(&context)?;
        let snapshot = self.read_snapshot(tenant_id, &request).await?;
        snapshot
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        Ok(snapshot)
    }

    async fn validate_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        let tenant_id = tenant_id(&context)?;
        let snapshot = self
            .read_snapshot(tenant_id, &read_request_from_patch(&request))
            .await?;
        let validation = validate_patch_for_snapshot(&request, &snapshot);
        validation
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        Ok(validation)
    }

    async fn apply_patch(
        &self,
        context: PortContext,
        request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        validate_translation_apply_context(&context)?;
        let security = authorize(&context, Action::Update)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        ensure_postgres(&self.db)?;
        let tenant_id = tenant_id(&context)?;
        let actor_user_id = security.user_id.ok_or_else(|| {
            PortError::forbidden(
                "seo.translation_user_actor_required",
                "SEO Translation apply requires an authenticated user actor",
            )
        })?;
        let idempotency_key = context.idempotency_key.as_deref().ok_or_else(|| {
            PortError::validation(
                "port.idempotency_key_required",
                "write port calls require a non-empty idempotency key",
            )
        })?;
        let request_fingerprint = request_fingerprint(&request)?;

        let txn = self
            .db
            .begin_with_config(Some(IsolationLevel::Serializable), None)
            .await
            .map_err(database_error)?;
        let receipt = self
            .apply_patch_in_tx(
                &txn,
                tenant_id,
                actor_user_id,
                idempotency_key,
                &request_fingerprint,
                &request,
            )
            .await?;
        txn.commit().await.map_err(database_error)?;
        Ok(receipt)
    }

    async fn read_progress(
        &self,
        context: PortContext,
        request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        ensure_postgres(&self.db)?;
        let tenant_id = tenant_id(&context)?;

        for _ in 0..PROGRESS_STABILITY_ATTEMPTS {
            let before = latest_change_seq(&self.db, tenant_id).await?;
            let rows = progress_rows(
                &self.db,
                tenant_id,
                request.source_locale.as_str(),
                request.target_locale.as_str(),
            )
            .await?;
            let after = latest_change_seq(&self.db, tenant_id).await?;
            if before == after {
                let facts = progress_facts(rows, after)?;
                facts
                    .validate()
                    .map_err(|error| contract_validation_error(error.to_string()))?;
                return Ok(facts);
            }
        }

        Err(PortError::conflict(
            "seo.translation_progress_unstable",
            "SEO Translation progress changed while it was being aggregated",
        ))
    }

    async fn read_changes(
        &self,
        context: PortContext,
        request: TranslationTargetChangesRequest,
    ) -> Result<TranslationTargetChangePage, PortError> {
        validate_translation_read_context(&context)?;
        authorize(&context, Action::Read)?;
        request
            .validate()
            .map_err(|error| contract_validation_error(error.to_string()))?;
        ensure_postgres(&self.db)?;
        let tenant_id = tenant_id(&context)?;
        let after = request
            .after
            .as_ref()
            .map(parse_change_cursor)
            .transpose()?
            .unwrap_or(0);
        let through = latest_change_seq(&self.db, tenant_id).await?.unwrap_or(0);
        if through <= after {
            return Ok(TranslationTargetChangePage {
                changes: Vec::new(),
                next_cursor: None,
            });
        }

        let rows = change_rows(
            &self.db,
            tenant_id,
            after,
            through,
            request.limit.saturating_add(1),
        )
        .await?;
        let has_more = rows.len() > usize::from(request.limit);
        let visible = rows
            .into_iter()
            .take(usize::from(request.limit))
            .collect::<Vec<_>>();
        let mut changes = Vec::with_capacity(visible.len());
        for row in &visible {
            changes.push(TranslationTargetChange {
                identity: resource_identity(&row.target_kind, row.target_id)?,
                resource_revision: OpaqueRevision::new(row.resource_revision.clone()).map_err(
                    |error| {
                        PortError::invariant_violation(
                            "seo.translation_change_revision_invalid",
                            error.to_string(),
                        )
                    },
                )?,
                lifecycle: parse_resource_lifecycle(&row.lifecycle)?,
            });
        }
        let next_cursor = if has_more {
            visible
                .last()
                .map(|row| OpaqueCursor::new(row.change_seq.to_string()))
                .transpose()
                .map_err(|error| contract_validation_error(error.to_string()))?
        } else {
            None
        };
        Ok(TranslationTargetChangePage {
            changes,
            next_cursor,
        })
    }
}

pub fn register_seo_translation_target_provider(
    extensions: &mut rustok_core::ModuleRuntimeExtensions,
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Result<(), TranslationTargetRegistryError> {
    register_translation_target_provider(
        extensions,
        SeoTranslationTargetProvider::new(db, event_bus),
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SeoCopy {
    title: Option<String>,
    description: Option<String>,
    keywords: Option<String>,
    og_title: Option<String>,
    og_description: Option<String>,
}

impl SeoCopy {
    fn from_model(model: &meta_translation::Model) -> Self {
        Self {
            title: trimmed_option(model.title.clone()),
            description: trimmed_option(model.description.clone()),
            keywords: trimmed_option(model.keywords.clone()),
            og_title: trimmed_option(model.og_title.clone()),
            og_description: trimmed_option(model.og_description.clone()),
        }
    }

    fn has_any(&self) -> bool {
        self.title.is_some()
            || self.description.is_some()
            || self.keywords.is_some()
            || self.og_title.is_some()
            || self.og_description.is_some()
    }

    fn values(&self) -> [(&'static str, &Option<String>); 5] {
        [
            ("title", &self.title),
            ("description", &self.description),
            ("keywords", &self.keywords),
            ("og_title", &self.og_title),
            ("og_description", &self.og_description),
        ]
    }
}

#[derive(Debug, FromQueryResult)]
struct ListSourceRow {
    meta_id: Uuid,
    target_kind: String,
    target_id: Uuid,
    title: Option<String>,
    resource_revision: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct RevisionRow {
    revision: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct HighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ProgressRow {
    source_title: Option<String>,
    source_description: Option<String>,
    source_keywords: Option<String>,
    source_og_title: Option<String>,
    source_og_description: Option<String>,
    target_title: Option<String>,
    target_description: Option<String>,
    target_keywords: Option<String>,
    target_og_title: Option<String>,
    target_og_description: Option<String>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    target_kind: String,
    target_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[derive(Debug, FromQueryResult)]
struct ReceiptRow {
    actor_user_id: Uuid,
    target_kind: String,
    target_id: Uuid,
    request_fingerprint: String,
    operation_id: Uuid,
    resource_revision: String,
    source_revision: String,
    target_revision: String,
}

struct ReceiptInsert<'a> {
    tenant_id: Uuid,
    idempotency_key: &'a str,
    actor_user_id: Uuid,
    target_kind: &'a str,
    target_id: Uuid,
    request_fingerprint: &'a str,
    operation_id: Uuid,
    resource_revision: &'a str,
    source_revision: &'a str,
    target_revision: &'a str,
}

async fn load_snapshot_in<C>(
    db: &C,
    tenant_id: Uuid,
    request: &ReadTranslationResourceRequest,
) -> Result<TranslationResourceSnapshot, PortError>
where
    C: ConnectionTrait,
{
    if request.source_locale == request.target_locale {
        return Err(PortError::validation(
            "seo.translation_locale_pair_invalid",
            "SEO Translation source and target locales must differ",
        ));
    }
    let (target_kind, target_id) = parse_resource_identity(&request.identity)?;
    let meta = seo_meta::Entity::find()
        .filter(seo_meta::Column::TenantId.eq(tenant_id))
        .filter(seo_meta::Column::TargetType.eq(target_kind.as_str()))
        .filter(seo_meta::Column::TargetId.eq(target_id))
        .one(db)
        .await
        .map_err(database_error)?
        .ok_or_else(resource_not_found)?;

    let translations = meta_translation::Entity::find()
        .filter(meta_translation::Column::MetaId.eq(meta.id))
        .order_by_asc(meta_translation::Column::Locale)
        .all(db)
        .await
        .map_err(database_error)?;
    let source = translations
        .iter()
        .find(|translation| translation.locale == request.source_locale.as_str())
        .filter(|translation| SeoCopy::from_model(translation).has_any())
        .ok_or_else(resource_not_found)?;
    let target = translations
        .iter()
        .find(|translation| translation.locale == request.target_locale.as_str())
        .filter(|translation| SeoCopy::from_model(translation).has_any());

    let resource_revision =
        load_resource_revision(db, tenant_id, target_kind.as_str(), target_id).await?;
    let source_revision = load_locale_revision(
        db,
        tenant_id,
        target_kind.as_str(),
        target_id,
        request.source_locale.as_str(),
    )
    .await?
    .ok_or_else(|| {
        PortError::invariant_violation(
            "seo.translation_source_revision_missing",
            "SEO exact source copy is missing its owner revision",
        )
    })?;
    let target_revision = if target.is_some() {
        Some(
            load_locale_revision(
                db,
                tenant_id,
                target_kind.as_str(),
                target_id,
                request.target_locale.as_str(),
            )
            .await?
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "seo.translation_target_revision_missing",
                    "SEO exact target copy is missing its owner revision",
                )
            })?,
        )
    } else {
        None
    };

    let exact_locales = translations
        .iter()
        .filter(|translation| SeoCopy::from_model(translation).has_any())
        .map(|translation| {
            TenantLocale::new(translation.locale.clone()).map_err(|error| {
                PortError::invariant_violation("seo.translation_locale_invalid", error.to_string())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let snapshot = TranslationResourceSnapshot {
        summary: TranslationResourceSummary {
            identity: resource_identity(target_kind.as_str(), target_id)?,
            display_label: source
                .title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}/{}", target_kind, target_id)),
            lifecycle: TranslationResourceLifecycle::Active,
            resource_revision: seo_resource_revision(resource_revision)?,
            exact_locales,
        },
        source_locale: request.source_locale.clone(),
        target_locale: request.target_locale.clone(),
        rendered_fallback_locale: None,
        source_revision: seo_locale_revision(source_revision)?,
        target_revision: target_revision.map(seo_locale_revision).transpose()?,
        fields: translation_fields(source, target),
    };
    snapshot
        .validate()
        .map_err(|error| contract_validation_error(error.to_string()))?;
    Ok(snapshot)
}

fn translation_fields(
    source: &meta_translation::Model,
    target: Option<&meta_translation::Model>,
) -> Vec<TranslationFieldSnapshot> {
    [
        (
            "title",
            source.title.as_deref(),
            target.and_then(|row| row.title.as_deref()),
        ),
        (
            "description",
            source.description.as_deref(),
            target.and_then(|row| row.description.as_deref()),
        ),
        (
            "keywords",
            source.keywords.as_deref(),
            target.and_then(|row| row.keywords.as_deref()),
        ),
        (
            "og_title",
            source.og_title.as_deref(),
            target.and_then(|row| row.og_title.as_deref()),
        ),
        (
            "og_description",
            source.og_description.as_deref(),
            target.and_then(|row| row.og_description.as_deref()),
        ),
    ]
    .into_iter()
    .map(|(key, source_value, target_value)| {
        let source_value = source_value.unwrap_or_default().to_string();
        TranslationFieldSnapshot {
            descriptor: TranslationFieldDescriptor {
                key: FieldKey::new(key)
                    .expect("static SEO field key must satisfy Translation target contract"),
                profile: TranslationValueProfile::SeoText,
                strategy: TranslationStrategy::Translate,
                classification: TranslationDataClassification::Public,
                required: false,
                ai_export_allowed: false,
                max_characters: None,
                preserves_whitespace: false,
            },
            source_hash: field_hash(&source_value),
            source_value,
            exact_target_value: target_value.map(str::to_string),
            protected_tokens: Vec::new(),
        }
    })
    .collect()
}

fn validate_patch_for_snapshot(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> TranslationPatchValidation {
    let mut validation = validate_patch_against_snapshot(request, snapshot);
    if validation.accepted && !merged_target_copy(request, snapshot).has_any() {
        validation.accepted = false;
        validation.issues.push(TranslationPatchIssue {
            field: None,
            severity: TranslationPatchIssueSeverity::Error,
            code: "seo_target_copy_empty".to_string(),
            message: "SEO Translation apply must retain at least one exact target copy field"
                .to_string(),
        });
    }
    validation
}

fn merged_target_copy(
    request: &TranslationPatchRequest,
    snapshot: &TranslationResourceSnapshot,
) -> SeoCopy {
    let mut merged = merged_patch_values(request, snapshot);
    SeoCopy {
        title: take_optional(&mut merged, "title"),
        description: take_optional(&mut merged, "description"),
        keywords: take_optional(&mut merged, "keywords"),
        og_title: take_optional(&mut merged, "og_title"),
        og_description: take_optional(&mut merged, "og_description"),
    }
}

async fn list_source_rows<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    after: Option<&(SeoTargetSlug, Uuid)>,
    limit: u16,
) -> Result<Vec<ListSourceRow>, PortError>
where
    C: ConnectionTrait,
{
    let (sql, values) = if let Some((after_kind, after_id)) = after {
        (
            format!(
                r#"
SELECT m.id AS meta_id,
       m.target_type AS target_kind,
       m.target_id,
       t.title,
       s.revision AS resource_revision
FROM meta m
JOIN meta_translations t
  ON t.meta_id = m.id
 AND t.locale = $2
LEFT JOIN {RESOURCE_STATE_TABLE} s
  ON s.tenant_id = m.tenant_id
 AND s.target_kind = m.target_type
 AND s.target_id = m.target_id
WHERE m.tenant_id = $1
  AND (
    NULLIF(BTRIM(t.title), '') IS NOT NULL OR
    NULLIF(BTRIM(t.description), '') IS NOT NULL OR
    NULLIF(BTRIM(t.keywords), '') IS NOT NULL OR
    NULLIF(BTRIM(t.og_title), '') IS NOT NULL OR
    NULLIF(BTRIM(t.og_description), '') IS NOT NULL
  )
  AND (m.target_type > $3 OR (m.target_type = $3 AND m.target_id > $4))
ORDER BY m.target_type ASC, m.target_id ASC
LIMIT $5
"#
            ),
            vec![
                tenant_id.into(),
                source_locale.to_string().into(),
                after_kind.as_str().to_string().into(),
                (*after_id).into(),
                i64::from(limit).into(),
            ],
        )
    } else {
        (
            format!(
                r#"
SELECT m.id AS meta_id,
       m.target_type AS target_kind,
       m.target_id,
       t.title,
       s.revision AS resource_revision
FROM meta m
JOIN meta_translations t
  ON t.meta_id = m.id
 AND t.locale = $2
LEFT JOIN {RESOURCE_STATE_TABLE} s
  ON s.tenant_id = m.tenant_id
 AND s.target_kind = m.target_type
 AND s.target_id = m.target_id
WHERE m.tenant_id = $1
  AND (
    NULLIF(BTRIM(t.title), '') IS NOT NULL OR
    NULLIF(BTRIM(t.description), '') IS NOT NULL OR
    NULLIF(BTRIM(t.keywords), '') IS NOT NULL OR
    NULLIF(BTRIM(t.og_title), '') IS NOT NULL OR
    NULLIF(BTRIM(t.og_description), '') IS NOT NULL
  )
ORDER BY m.target_type ASC, m.target_id ASC
LIMIT $3
"#
            ),
            vec![
                tenant_id.into(),
                source_locale.to_string().into(),
                i64::from(limit).into(),
            ],
        )
    };
    ListSourceRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        sql,
        values,
    ))
    .all(db)
    .await
    .map_err(database_error)
}

async fn exact_locales_in<C>(db: &C, meta_id: Uuid) -> Result<Vec<TenantLocale>, PortError>
where
    C: ConnectionTrait,
{
    meta_translation::Entity::find()
        .filter(meta_translation::Column::MetaId.eq(meta_id))
        .order_by_asc(meta_translation::Column::Locale)
        .all(db)
        .await
        .map_err(database_error)?
        .into_iter()
        .filter(|row| SeoCopy::from_model(row).has_any())
        .map(|row| {
            TenantLocale::new(row.locale).map_err(|error| {
                PortError::invariant_violation("seo.translation_locale_invalid", error.to_string())
            })
        })
        .collect()
}

async fn load_resource_revision<C>(
    db: &C,
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
) -> Result<i64, PortError>
where
    C: ConnectionTrait,
{
    let row = RevisionRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "SELECT revision FROM {RESOURCE_STATE_TABLE} WHERE tenant_id = $1 AND target_kind = $2 AND target_id = $3"
        ),
        vec![
            tenant_id.into(),
            target_kind.to_string().into(),
            target_id.into(),
        ],
    ))
    .one(db)
    .await
    .map_err(database_error)?
    .ok_or_else(|| {
        PortError::invariant_violation(
            "seo.translation_resource_revision_missing",
            "SEO translation resource is missing its owner revision",
        )
    })?;
    required_positive_revision(row.revision, "resource")
}

async fn load_locale_revision<C>(
    db: &C,
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
    locale: &str,
) -> Result<Option<i64>, PortError>
where
    C: ConnectionTrait,
{
    let row = RevisionRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "SELECT revision FROM {LOCALE_STATE_TABLE} WHERE tenant_id = $1 AND target_kind = $2 AND target_id = $3 AND locale = $4"
        ),
        vec![
            tenant_id.into(),
            target_kind.to_string().into(),
            target_id.into(),
            locale.to_string().into(),
        ],
    ))
    .one(db)
    .await
    .map_err(database_error)?;
    row.map(|row| required_positive_revision(row.revision, "locale"))
        .transpose()
}

async fn lock_meta<C>(
    db: &C,
    tenant_id: Uuid,
    target_kind: &str,
    target_id: Uuid,
) -> Result<seo_meta::Model, PortError>
where
    C: ConnectionTrait,
{
    db.query_one_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id FROM meta WHERE tenant_id = $1 AND target_type = $2 AND target_id = $3 FOR UPDATE",
        vec![
            tenant_id.into(),
            target_kind.to_string().into(),
            target_id.into(),
        ],
    ))
    .await
    .map_err(database_error)?
    .ok_or_else(resource_not_found)?;

    seo_meta::Entity::find()
        .filter(seo_meta::Column::TenantId.eq(tenant_id))
        .filter(seo_meta::Column::TargetType.eq(target_kind))
        .filter(seo_meta::Column::TargetId.eq(target_id))
        .one(db)
        .await
        .map_err(database_error)?
        .ok_or_else(resource_not_found)
}

async fn advisory_lock<C>(db: &C, tenant_id: Uuid, idempotency_key: &str) -> Result<(), PortError>
where
    C: ConnectionTrait,
{
    let key = format!("seo-translation:{tenant_id}:{idempotency_key}");
    db.query_one_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
        vec![key.into()],
    ))
    .await
    .map_err(database_error)?
    .ok_or_else(|| {
        PortError::invariant_violation(
            "seo.translation_idempotency_lock_failed",
            "SEO Translation idempotency lock returned no row",
        )
    })?;
    Ok(())
}

async fn load_receipt<C>(
    db: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> Result<Option<ReceiptRow>, PortError>
where
    C: ConnectionTrait,
{
    ReceiptRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            r#"
SELECT actor_user_id,
       target_kind,
       target_id,
       request_fingerprint,
       operation_id,
       resource_revision,
       source_revision,
       target_revision
FROM {APPLY_RECEIPTS_TABLE}
WHERE tenant_id = $1 AND idempotency_key = $2
"#
        ),
        vec![tenant_id.into(), idempotency_key.to_string().into()],
    ))
    .one(db)
    .await
    .map_err(database_error)
}

async fn insert_receipt<C>(db: &C, receipt: ReceiptInsert<'_>) -> Result<(), PortError>
where
    C: ConnectionTrait,
{
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            r#"
INSERT INTO {APPLY_RECEIPTS_TABLE} (
  tenant_id,
  idempotency_key,
  actor_user_id,
  target_kind,
  target_id,
  request_fingerprint,
  operation_id,
  resource_revision,
  source_revision,
  target_revision
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
"#
        ),
        vec![
            receipt.tenant_id.into(),
            receipt.idempotency_key.to_string().into(),
            receipt.actor_user_id.into(),
            receipt.target_kind.to_string().into(),
            receipt.target_id.into(),
            receipt.request_fingerprint.to_string().into(),
            receipt.operation_id.into(),
            receipt.resource_revision.to_string().into(),
            receipt.source_revision.to_string().into(),
            receipt.target_revision.to_string().into(),
        ],
    ))
    .await
    .map_err(database_error)?;
    Ok(())
}

fn receipt_to_application(
    receipt: ReceiptRow,
    request: &TranslationPatchRequest,
) -> Result<TranslationApplicationReceipt, PortError> {
    let resource_revision = OpaqueRevision::new(receipt.resource_revision)
        .map_err(|error| receipt_invariant(error.to_string()))?;
    let _source_revision = OpaqueRevision::new(receipt.source_revision)
        .map_err(|error| receipt_invariant(error.to_string()))?;
    let target_revision = OpaqueRevision::new(receipt.target_revision)
        .map_err(|error| receipt_invariant(error.to_string()))?;
    Ok(TranslationApplicationReceipt {
        provider_receipt_id: receipt.operation_id.to_string(),
        resource_revision,
        target_revision,
        applied_field_keys: request
            .fields
            .iter()
            .map(|field| field.key.clone())
            .collect(),
    })
}

async fn latest_change_seq<C>(db: &C, tenant_id: Uuid) -> Result<Option<u64>, PortError>
where
    C: ConnectionTrait,
{
    let row = HighwaterRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "SELECT MAX(change_seq) AS highwater FROM {CHANGE_JOURNAL_TABLE} WHERE tenant_id = $1"
        ),
        vec![tenant_id.into()],
    ))
    .one(db)
    .await
    .map_err(database_error)?
    .ok_or_else(|| {
        PortError::invariant_violation(
            "seo.translation_change_highwater_missing",
            "SEO Translation change high-water query returned no row",
        )
    })?;
    row.highwater
        .map(|value| positive_u64(value, "change high-water"))
        .transpose()
}

async fn change_rows<C>(
    db: &C,
    tenant_id: Uuid,
    after: u64,
    through: u64,
    limit: u16,
) -> Result<Vec<ChangeRow>, PortError>
where
    C: ConnectionTrait,
{
    ChangeRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            r#"
SELECT change_seq, target_kind, target_id, resource_revision, lifecycle
FROM {CHANGE_JOURNAL_TABLE}
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#
        ),
        vec![
            tenant_id.into(),
            i64::try_from(after)
                .map_err(|_| invalid_sequence("after"))?
                .into(),
            i64::try_from(through)
                .map_err(|_| invalid_sequence("through"))?
                .into(),
            i64::from(limit).into(),
        ],
    ))
    .all(db)
    .await
    .map_err(database_error)
}

async fn progress_rows<C>(
    db: &C,
    tenant_id: Uuid,
    source_locale: &str,
    target_locale: &str,
) -> Result<Vec<ProgressRow>, PortError>
where
    C: ConnectionTrait,
{
    ProgressRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
SELECT source.title AS source_title,
       source.description AS source_description,
       source.keywords AS source_keywords,
       source.og_title AS source_og_title,
       source.og_description AS source_og_description,
       target.title AS target_title,
       target.description AS target_description,
       target.keywords AS target_keywords,
       target.og_title AS target_og_title,
       target.og_description AS target_og_description
FROM meta m
JOIN meta_translations source
  ON source.meta_id = m.id
 AND source.locale = $2
LEFT JOIN meta_translations target
  ON target.meta_id = m.id
 AND target.locale = $3
WHERE m.tenant_id = $1
  AND (
    NULLIF(BTRIM(source.title), '') IS NOT NULL OR
    NULLIF(BTRIM(source.description), '') IS NOT NULL OR
    NULLIF(BTRIM(source.keywords), '') IS NOT NULL OR
    NULLIF(BTRIM(source.og_title), '') IS NOT NULL OR
    NULLIF(BTRIM(source.og_description), '') IS NOT NULL
  )
ORDER BY m.target_type ASC, m.target_id ASC
"#,
        vec![
            tenant_id.into(),
            source_locale.to_string().into(),
            target_locale.to_string().into(),
        ],
    ))
    .all(db)
    .await
    .map_err(database_error)
}

fn progress_facts(
    rows: Vec<ProgressRow>,
    highwater: Option<u64>,
) -> Result<TranslationTargetProgressFacts, PortError> {
    let mut optional_units = 0_u64;
    let mut exact_optional_units = 0_u64;
    let mut complete_resources = 0_u64;

    for row in &rows {
        let pairs = [
            (&row.source_title, &row.target_title),
            (&row.source_description, &row.target_description),
            (&row.source_keywords, &row.target_keywords),
            (&row.source_og_title, &row.target_og_title),
            (&row.source_og_description, &row.target_og_description),
        ];
        let mut complete = true;
        for (source, target) in pairs {
            if has_text(source) {
                optional_units = optional_units.saturating_add(1);
                if has_text(target) {
                    exact_optional_units = exact_optional_units.saturating_add(1);
                } else {
                    complete = false;
                }
            }
        }
        if complete {
            complete_resources = complete_resources.saturating_add(1);
        }
    }

    Ok(TranslationTargetProgressFacts {
        required_units: 0,
        exact_required_units: 0,
        optional_units,
        exact_optional_units,
        resources: u64::try_from(rows.len()).map_err(|_| {
            PortError::invariant_violation(
                "seo.translation_progress_overflow",
                "SEO Translation resource count overflowed",
            )
        })?,
        complete_resources,
        owner_change_cursor: highwater
            .map(|value| OpaqueCursor::new(value.to_string()))
            .transpose()
            .map_err(|error| contract_validation_error(error.to_string()))?,
    })
}

fn verify_target_copy(
    expected: &SeoCopy,
    snapshot: &TranslationResourceSnapshot,
) -> Result<(), PortError> {
    let actual = snapshot
        .fields
        .iter()
        .map(|field| {
            (
                field.descriptor.key.as_str(),
                field
                    .exact_target_value
                    .clone()
                    .and_then(normalize_optional_target_value),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (key, value) in expected.values() {
        if actual.get(key) != Some(value) {
            return Err(PortError::invariant_violation(
                "seo.translation_apply_verification_failed",
                "SEO Translation owner write did not materialize the requested exact target copy",
            ));
        }
    }
    Ok(())
}

fn take_optional(values: &mut BTreeMap<String, Option<String>>, key: &str) -> Option<String> {
    values
        .remove(key)
        .flatten()
        .and_then(normalize_optional_target_value)
}

fn authorize(context: &PortContext, action: Action) -> Result<SecurityContext, PortError> {
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Seo, action) == PermissionScope::None {
        return Err(PortError::forbidden(
            "seo.translation_permission_denied",
            "SEO permission is required",
        ));
    }
    Ok(security)
}

fn tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "seo.translation_tenant_id_invalid",
            "SEO Translation tenant id must be a UUID",
        )
    })
}

fn resource_identity(
    target_kind: &str,
    target_id: Uuid,
) -> Result<TranslationResourceIdentity, PortError> {
    let target_kind = SeoTargetSlug::new(target_kind.to_string()).map_err(|error| {
        PortError::invariant_violation("seo.translation_target_kind_invalid", error.to_string())
    })?;
    Ok(TranslationResourceIdentity {
        owner_slug: OwnerSlug::new(OWNER_SLUG)
            .expect("static SEO owner slug must satisfy Translation target contract"),
        resource_kind: ResourceKind::new(RESOURCE_KIND)
            .expect("static SEO resource kind must satisfy Translation target contract"),
        resource_id: ResourceId::new(target_id.to_string())
            .map_err(|error| contract_validation_error(error.to_string()))?,
        subresource_id: Some(
            ResourceId::new(target_kind.as_str().to_string())
                .map_err(|error| contract_validation_error(error.to_string()))?,
        ),
    })
}

fn parse_resource_identity(
    identity: &TranslationResourceIdentity,
) -> Result<(SeoTargetSlug, Uuid), PortError> {
    if identity.owner_slug.as_str() != OWNER_SLUG
        || identity.resource_kind.as_str() != RESOURCE_KIND
    {
        return Err(PortError::validation(
            "seo.translation_identity_invalid",
            "SEO Translation provider received a foreign resource identity",
        ));
    }
    let target_kind = identity.subresource_id.as_ref().ok_or_else(|| {
        PortError::validation(
            "seo.translation_target_kind_missing",
            "SEO Translation resource identity requires target kind",
        )
    })?;
    let target_kind = SeoTargetSlug::new(target_kind.as_str().to_string()).map_err(|error| {
        PortError::validation("seo.translation_target_kind_invalid", error.to_string())
    })?;
    let target_id = Uuid::parse_str(identity.resource_id.as_str()).map_err(|_| {
        PortError::validation(
            "seo.translation_target_id_invalid",
            "SEO Translation resource id must be a UUID",
        )
    })?;
    Ok((target_kind, target_id))
}

fn parse_resource_cursor(cursor: &OpaqueCursor) -> Result<(SeoTargetSlug, Uuid), PortError> {
    let (kind, id) = cursor.as_str().rsplit_once(':').ok_or_else(|| {
        PortError::validation(
            "seo.translation_cursor_invalid",
            "SEO Translation resource cursor is invalid",
        )
    })?;
    let kind = SeoTargetSlug::new(kind.to_string()).map_err(|error| {
        PortError::validation("seo.translation_cursor_invalid", error.to_string())
    })?;
    let id = Uuid::parse_str(id).map_err(|_| {
        PortError::validation(
            "seo.translation_cursor_invalid",
            "SEO Translation resource cursor contains an invalid UUID",
        )
    })?;
    Ok((kind, id))
}

fn parse_change_cursor(cursor: &OpaqueCursor) -> Result<u64, PortError> {
    cursor
        .as_str()
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            PortError::validation(
                "seo.translation_change_cursor_invalid",
                "SEO Translation change cursor must be a positive sequence",
            )
        })
}

fn request_fingerprint(request: &TranslationPatchRequest) -> Result<String, PortError> {
    let payload = serde_json::to_vec(request).map_err(|error| {
        PortError::invariant_violation(
            "seo.translation_request_fingerprint_failed",
            error.to_string(),
        )
    })?;
    let digest = sha256_digest(&[payload.as_slice()]);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn seo_resource_revision(revision: i64) -> Result<OpaqueRevision, PortError> {
    opaque_owner_revision("seo", revision)
}

fn seo_locale_revision(revision: i64) -> Result<OpaqueRevision, PortError> {
    opaque_owner_revision("seo-locale", revision)
}

fn opaque_owner_revision(prefix: &str, revision: i64) -> Result<OpaqueRevision, PortError> {
    if revision <= 0 {
        return Err(PortError::invariant_violation(
            "seo.translation_revision_invalid",
            "SEO Translation owner revision must be positive",
        ));
    }
    OpaqueRevision::new(format!("{prefix}:{revision}")).map_err(|error| {
        PortError::invariant_violation("seo.translation_revision_invalid", error.to_string())
    })
}

fn required_positive_revision(revision: Option<i64>, field: &str) -> Result<i64, PortError> {
    let revision = revision.ok_or_else(|| {
        PortError::invariant_violation(
            "seo.translation_revision_missing",
            format!("SEO Translation {field} revision is missing"),
        )
    })?;
    if revision <= 0 {
        return Err(PortError::invariant_violation(
            "seo.translation_revision_invalid",
            format!("SEO Translation {field} revision must be positive"),
        ));
    }
    Ok(revision)
}

fn has_text(value: &Option<String>) -> bool {
    value
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn positive_u64(value: i64, field: &str) -> Result<u64, PortError> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid_sequence(field))
}

fn invalid_sequence(field: &str) -> PortError {
    PortError::invariant_violation(
        "seo.translation_change_sequence_invalid",
        format!("SEO Translation {field} sequence must be positive"),
    )
}

fn ensure_postgres(db: &DatabaseConnection) -> Result<(), PortError> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Err(PortError::unavailable(
            "seo.translation_postgres_required",
            "SEO Translation owner state requires PostgreSQL",
        ));
    }
    Ok(())
}

fn resource_not_found() -> PortError {
    PortError::not_found(
        "seo.translation_resource_not_found",
        "SEO Translation resource was not found",
    )
}

fn receipt_invariant(message: String) -> PortError {
    PortError::invariant_violation("seo.translation_receipt_invalid", message)
}

fn database_error(error: sea_orm::DbErr) -> PortError {
    PortError::unavailable(
        "seo.translation_owner_unavailable",
        format!("SEO Translation owner storage is unavailable: {error}"),
    )
}

fn seo_error_to_port_error(error: SeoError) -> PortError {
    match error {
        SeoError::Validation(message) => {
            PortError::validation("seo.translation_owner_validation", message)
        }
        SeoError::Configuration(message) => {
            PortError::invariant_violation("seo.translation_owner_configuration", message)
        }
        SeoError::NotFound => resource_not_found(),
        SeoError::PermissionDenied => PortError::forbidden(
            "seo.translation_permission_denied",
            "SEO permission is required",
        ),
        SeoError::Database(error) => database_error(error),
    }
}

use std::{collections::HashMap, future::Future};

use rustok_api::{PortError, TenantLocale};
use rustok_core::generate_id;
use rustok_events::DomainEvent;
use rustok_outbox::{TransactionalEventBus, idempotency};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{collection, collection_translation};
use crate::CommerceError;

pub const MAX_COLLECTION_TRANSLATION_RESOURCE_PAGE: u16 = 200;

const COLLECTION_COPY_OWNER: &str = "commerce";
const COLLECTION_COPY_RESOURCE_KIND: &str = "collection_copy";
const COLLECTION_COPY_RESOURCE_REVISION_NAMESPACE: &str =
    "rustok-commerce/collection-copy-resource/v1";
const COLLECTION_COPY_LOCALE_REVISION_NAMESPACE: &str =
    "rustok-commerce/collection-copy-locale/v1";

tokio::task_local! {
    static COLLECTION_TRANSLATION_OPERATION_LEASE: idempotency::Lease;
}

pub(crate) async fn with_collection_translation_operation_receipt<F, T>(
    lease: idempotency::Lease,
    future: F,
) -> T
where
    F: Future<Output = T>,
{
    COLLECTION_TRANSLATION_OPERATION_LEASE
        .scope(lease, future)
        .await
}

fn current_operation_lease() -> Option<idempotency::Lease> {
    COLLECTION_TRANSLATION_OPERATION_LEASE
        .try_with(|lease| *lease)
        .ok()
}

#[derive(Debug, Error)]
pub enum CollectionTranslationExactLocaleError {
    #[error(transparent)]
    Commerce(#[from] CommerceError),

    #[error("Collection not found: {0}")]
    CollectionNotFound(Uuid),

    #[error("Collection source locale not found: {locale} for collection {collection_id}")]
    SourceLocaleNotFound { collection_id: Uuid, locale: String },

    #[error("Collection target locale missing after apply: {locale} for collection {collection_id}")]
    TargetLocaleMissingAfterApply { collection_id: Uuid, locale: String },

    #[error("Collection translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },

    #[error("Collection translation owner receipt failed: {0}")]
    OperationReceipt(PortError),
}

impl From<sea_orm::DbErr> for CollectionTranslationExactLocaleError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Commerce(CommerceError::Database(error))
    }
}

pub type CollectionTranslationExactLocaleResult<T> =
    Result<T, CollectionTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionTranslationExactLocaleRecord {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

impl From<collection_translation::Model> for CollectionTranslationExactLocaleRecord {
    fn from(value: collection_translation::Model) -> Self {
        Self {
            locale: value.locale,
            title: value.title,
            handle: value.handle,
            description: value.description,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionTranslationExactLocaleSnapshot {
    pub collection_id: Uuid,
    pub collection_type: String,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: CollectionTranslationExactLocaleRecord,
    pub target: Option<CollectionTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionTranslationExactLocaleApplyReceipt {
    pub operation_id: Option<Uuid>,
    pub collection_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: CollectionTranslationExactLocaleRecord,
}

#[derive(Clone)]
pub struct CollectionTranslationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CollectionTranslationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn list_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> CollectionTranslationExactLocaleResult<(
        Vec<CollectionTranslationExactLocaleSnapshot>,
        Option<Uuid>,
    )> {
        validate_tenant_id(tenant_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 || limit > MAX_COLLECTION_TRANSLATION_RESOURCE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Collection translation resource page size must be between 1 and {MAX_COLLECTION_TRANSLATION_RESOURCE_PAGE}"
            ))
            .into());
        }

        let source_collection_ids = sea_orm::sea_query::Query::select()
            .column(collection_translation::Column::CollectionId)
            .from(collection_translation::Entity)
            .and_where(
                sea_orm::sea_query::Expr::col(collection_translation::Column::Locale)
                    .eq(source_locale.clone()),
            )
            .to_owned();
        let mut query = collection::Entity::find()
            .filter(collection::Column::TenantId.eq(tenant_id))
            .filter(collection::Column::DeletedAt.is_null())
            .filter(collection::Column::Id.in_subquery(source_collection_ids))
            .order_by_asc(collection::Column::Id);
        if let Some(after) = after {
            query = query.filter(collection::Column::Id.gt(after));
        }

        let mut collections = query.limit(u64::from(limit) + 1).all(&self.db).await?;
        let has_more = collections.len() > usize::from(limit);
        if has_more {
            collections.truncate(usize::from(limit));
        }
        let next_after = has_more
            .then(|| collections.last().map(|collection| collection.id))
            .flatten();
        if collections.is_empty() {
            return Ok((Vec::new(), None));
        }

        let collection_ids = collections
            .iter()
            .map(|collection| collection.id)
            .collect::<Vec<_>>();
        let mut translations = load_translations_for_collections(&self.db, &collection_ids)
            .await?
            .into_iter()
            .fold(
                HashMap::<Uuid, Vec<collection_translation::Model>>::new(),
                |mut grouped, translation| {
                    grouped
                        .entry(translation.collection_id)
                        .or_default()
                        .push(translation);
                    grouped
                },
            );

        let mut snapshots = Vec::with_capacity(collections.len());
        for collection in collections {
            let exact = translations.remove(&collection.id).ok_or_else(|| {
                CollectionTranslationExactLocaleError::SourceLocaleNotFound {
                    collection_id: collection.id,
                    locale: source_locale.clone(),
                }
            })?;
            snapshots.push(build_snapshot(
                collection,
                exact,
                source_locale.clone(),
                target_locale.clone(),
            )?);
        }
        Ok((snapshots, next_after))
    }

    pub async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> CollectionTranslationExactLocaleResult<CollectionTranslationExactLocaleSnapshot> {
        validate_tenant_id(tenant_id)?;
        validate_collection_id(collection_id)?;
        let source_locale = canonical_locale(source_locale)?;
        let target_locale = canonical_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let collection = load_active_collection(&self.db, tenant_id, collection_id).await?;
        let translations = load_collection_translations(&self.db, collection_id).await?;
        build_snapshot(collection, translations, source_locale, target_locale)
    }

    pub async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        collection_id: Uuid,
        request: CollectionTranslationExactLocaleApply,
    ) -> CollectionTranslationExactLocaleResult<CollectionTranslationExactLocaleApplyReceipt> {
        validate_tenant_id(tenant_id)?;
        validate_collection_id(collection_id)?;
        validate_copy(&request.title, &request.handle)?;
        let source_locale = canonical_locale(&request.source_locale)?;
        let target_locale = canonical_locale(&request.target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let txn = self.db.begin().await?;

        // Collection translations do not carry tenant_id. Lock every Collection
        // parent for this tenant in deterministic UUID order so concurrent owner
        // writes serialize before locale+handle collision checks.
        let tenant_collections = collection::Entity::find()
            .filter(collection::Column::TenantId.eq(tenant_id))
            .order_by_asc(collection::Column::Id)
            .lock_exclusive()
            .all(&txn)
            .await?;
        let collection = tenant_collections
            .into_iter()
            .find(|collection| collection.id == collection_id && collection.deleted_at.is_none())
            .ok_or(CollectionTranslationExactLocaleError::CollectionNotFound(
                collection_id,
            ))?;

        let translations = load_collection_translations(&txn, collection_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            CollectionTranslationExactLocaleError::SourceLocaleNotFound {
                collection_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);

        let current_resource_revision = resource_revision(&collection, &translations);
        let current_source_revision = locale_revision(source);
        let current_target_revision = target.map(locale_revision);
        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &current_resource_revision,
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &current_source_revision,
        )?;
        if request.expected_target_revision != current_target_revision {
            return Err(CollectionTranslationExactLocaleError::RevisionConflict {
                revision: "target",
            });
        }

        ensure_handle_available(
            &txn,
            tenant_id,
            collection_id,
            &target_locale,
            &request.handle,
        )
        .await?;

        let unchanged = target.is_some_and(|target| {
            target.title == request.title
                && target.handle == request.handle
                && target.description == request.description
        });
        if !unchanged {
            if let Some(existing) = target.cloned() {
                let mut active: collection_translation::ActiveModel = existing.into();
                active.title = Set(request.title.clone());
                active.handle = Set(request.handle.clone());
                active.description = Set(request.description.clone());
                active.update(&txn).await?;
            } else {
                collection_translation::ActiveModel {
                    id: Set(generate_id()),
                    collection_id: Set(collection_id),
                    locale: Set(target_locale.clone()),
                    title: Set(request.title.clone()),
                    handle: Set(request.handle.clone()),
                    description: Set(request.description.clone()),
                }
                .insert(&txn)
                .await?;
            }
        }

        let translations_after = load_collection_translations(&txn, collection_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale).ok_or_else(|| {
            CollectionTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                collection_id,
                locale: target_locale.clone(),
            }
        })?;
        let resource_revision = resource_revision(&collection, &translations_after);
        let target_revision = locale_revision(target_after);
        let target = CollectionTranslationExactLocaleRecord::from(target_after.clone());

        let operation_lease = current_operation_lease();
        let operation_id = operation_lease
            .map(|lease| lease.operation_id)
            .or_else(|| (!unchanged).then(generate_id));

        if !unchanged {
            let correlation_id = operation_id.expect("changed owner apply must have operation id");
            self.event_bus
                .publish_in_tx_with_envelope_id(
                    &txn,
                    tenant_id,
                    actor_user_id,
                    DomainEvent::TranslationTargetChanged {
                        owner_slug: COLLECTION_COPY_OWNER.to_owned(),
                        resource_kind: COLLECTION_COPY_RESOURCE_KIND.to_owned(),
                        resource_id: collection_id.to_string(),
                        changed_locale: target_locale,
                        resource_revision: resource_revision.clone(),
                        target_revision: target_revision.clone(),
                        operation: "apply_exact_locale".to_owned(),
                        correlation_id: correlation_id.to_string(),
                    },
                )
                .await
                .map_err(|error| CommerceError::Core(error))?;
        }

        let receipt = CollectionTranslationExactLocaleApplyReceipt {
            operation_id,
            collection_id,
            resource_revision,
            target_revision,
            target,
        };
        if let Some(lease) = operation_lease {
            idempotency::complete(&txn, lease, &receipt)
                .await
                .map_err(CollectionTranslationExactLocaleError::OperationReceipt)?;
        }

        txn.commit().await?;
        Ok(receipt)
    }
}

async fn load_active_collection<C>(
    db: &C,
    tenant_id: Uuid,
    collection_id: Uuid,
) -> CollectionTranslationExactLocaleResult<collection::Model>
where
    C: sea_orm::ConnectionTrait,
{
    collection::Entity::find_by_id(collection_id)
        .filter(collection::Column::TenantId.eq(tenant_id))
        .filter(collection::Column::DeletedAt.is_null())
        .one(db)
        .await?
        .ok_or(CollectionTranslationExactLocaleError::CollectionNotFound(
            collection_id,
        ))
}

async fn load_collection_translations<C>(
    db: &C,
    collection_id: Uuid,
) -> CollectionTranslationExactLocaleResult<Vec<collection_translation::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(collection_translation::Entity::find()
        .filter(collection_translation::Column::CollectionId.eq(collection_id))
        .order_by_asc(collection_translation::Column::Locale)
        .all(db)
        .await?)
}

async fn load_translations_for_collections<C>(
    db: &C,
    collection_ids: &[Uuid],
) -> CollectionTranslationExactLocaleResult<Vec<collection_translation::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    if collection_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(collection_translation::Entity::find()
        .filter(collection_translation::Column::CollectionId.is_in(collection_ids.to_vec()))
        .order_by_asc(collection_translation::Column::CollectionId)
        .order_by_asc(collection_translation::Column::Locale)
        .all(db)
        .await?)
}

async fn ensure_handle_available<C>(
    db: &C,
    tenant_id: Uuid,
    collection_id: Uuid,
    locale: &str,
    handle: &str,
) -> CollectionTranslationExactLocaleResult<()>
where
    C: sea_orm::ConnectionTrait,
{
    let tenant_collection_ids = collection::Entity::find()
        .select_only()
        .column(collection::Column::Id)
        .filter(collection::Column::TenantId.eq(tenant_id))
        .filter(collection::Column::DeletedAt.is_null())
        .into_tuple::<Uuid>()
        .all(db)
        .await?;
    let collision = collection_translation::Entity::find()
        .filter(collection_translation::Column::CollectionId.is_in(tenant_collection_ids))
        .filter(collection_translation::Column::CollectionId.ne(collection_id))
        .filter(collection_translation::Column::Locale.eq(locale))
        .filter(collection_translation::Column::Handle.eq(handle))
        .one(db)
        .await?;
    if collision.is_some() {
        return Err(CommerceError::DuplicateHandle {
            handle: handle.to_owned(),
            locale: locale.to_owned(),
        }
        .into());
    }
    Ok(())
}

fn build_snapshot(
    collection: collection::Model,
    translations: Vec<collection_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> CollectionTranslationExactLocaleResult<CollectionTranslationExactLocaleSnapshot> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(|| CollectionTranslationExactLocaleError::SourceLocaleNotFound {
            collection_id: collection.id,
            locale: source_locale.clone(),
        })?;
    let target = exact_locale_row(&translations, &target_locale).cloned();
    let resource_revision = resource_revision(&collection, &translations);
    let source_revision = locale_revision(&source);
    let target_revision = target.as_ref().map(locale_revision);
    let exact_locales = translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect();

    Ok(CollectionTranslationExactLocaleSnapshot {
        collection_id: collection.id,
        collection_type: collection.collection_type,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: CollectionTranslationExactLocaleRecord::from(source),
        target: target.map(CollectionTranslationExactLocaleRecord::from),
    })
}

fn exact_locale_row<'a>(
    translations: &'a [collection_translation::Model],
    locale: &str,
) -> Option<&'a collection_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn canonical_locale(locale: &str) -> CollectionTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn validate_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> CollectionTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(CommerceError::Validation(
            "Collection translation source and target locale must differ".to_owned(),
        )
        .into());
    }
    Ok(())
}

fn validate_tenant_id(tenant_id: Uuid) -> CollectionTranslationExactLocaleResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "Collection translation tenant_id must not be nil".to_owned(),
        )
        .into());
    }
    Ok(())
}

fn validate_collection_id(collection_id: Uuid) -> CollectionTranslationExactLocaleResult<()> {
    if collection_id.is_nil() {
        return Err(CommerceError::Validation(
            "Collection translation collection_id must not be nil".to_owned(),
        )
        .into());
    }
    Ok(())
}

fn validate_copy(title: &str, handle: &str) -> CollectionTranslationExactLocaleResult<()> {
    if title.trim().is_empty() || title.chars().count() > 255 {
        return Err(CommerceError::Validation(
            "Collection translation title must be nonblank and at most 255 characters".to_owned(),
        )
        .into());
    }
    if handle.trim().is_empty() || handle.chars().count() > 255 {
        return Err(CommerceError::Validation(
            "Collection translation handle must be nonblank and at most 255 characters".to_owned(),
        )
        .into());
    }
    Ok(())
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> CollectionTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(CollectionTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

fn resource_revision(
    collection: &collection::Model,
    translations: &[collection_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_text(&mut hasher, COLLECTION_COPY_RESOURCE_REVISION_NAMESPACE);
    digest_text(&mut hasher, &collection.id.to_string());
    digest_text(&mut hasher, &collection.tenant_id.to_string());
    digest_text(&mut hasher, &collection.collection_type);
    digest_optional_json(&mut hasher, collection.conditions.as_ref());
    digest_json(&mut hasher, &collection.metadata);

    let mut exact = translations.iter().collect::<Vec<_>>();
    exact.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in exact {
        digest_translation(&mut hasher, translation);
    }
    finish_revision(hasher)
}

fn locale_revision(translation: &collection_translation::Model) -> String {
    let mut hasher = Sha256::new();
    digest_text(&mut hasher, COLLECTION_COPY_LOCALE_REVISION_NAMESPACE);
    digest_translation(&mut hasher, translation);
    finish_revision(hasher)
}

fn digest_translation(hasher: &mut Sha256, translation: &collection_translation::Model) {
    digest_text(hasher, &translation.collection_id.to_string());
    digest_text(hasher, &translation.locale);
    digest_text(hasher, &translation.title);
    digest_text(hasher, &translation.handle);
    digest_optional_text(hasher, translation.description.as_deref());
}

fn digest_json(hasher: &mut Sha256, value: &serde_json::Value) {
    let bytes = serde_json::to_vec(value).expect("serde_json::Value serialization is infallible");
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn digest_optional_json(hasher: &mut Sha256, value: Option<&serde_json::Value>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            digest_json(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn digest_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn digest_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            digest_text(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn finish_revision(hasher: Sha256) -> String {
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

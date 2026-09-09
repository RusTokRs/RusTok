use async_trait::async_trait;
use flex::{
    FlexAttachedTranslationError, FlexAttachedTranslationExactProgress,
    FlexAttachedTranslationProgressOwnerPort, FlexAttachedTranslationResult,
    TAXONOMY_CATEGORY_ENTITY_TYPE, load_attached_translation_localized_values,
    load_attached_translation_resource_revisions, load_attached_translation_schema_in,
    validate_flex_attached_translation_locale_pair,
};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseBackend, DatabaseConnection, IsolationLevel,
    TransactionTrait,
};
use uuid::Uuid;

use super::flex_attached_translation_owner::build_snapshot_from_batch;

const PROGRESS_PAGE_SIZE: u16 = 200;

#[derive(Clone)]
pub struct ServerFlexTaxonomyCategoryTranslationProgressOwner {
    db: DatabaseConnection,
}

impl ServerFlexTaxonomyCategoryTranslationProgressOwner {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FlexAttachedTranslationProgressOwnerPort
    for ServerFlexTaxonomyCategoryTranslationProgressOwner
{
    fn entity_type(&self) -> &str {
        TAXONOMY_CATEGORY_ENTITY_TYPE
    }

    async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactProgress> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_flex_attached_translation_locale_pair(source_locale, target_locale)?;
        if self.db.get_database_backend() != DatabaseBackend::Postgres {
            return Err(FlexAttachedTranslationError::Invalid(
                "attached Translation aggregate progress requires PostgreSQL durable revision state"
                    .to_string(),
            ));
        }

        // Progress spans Taxonomy inventory pages plus Flex schema/value/revision state. Keep the
        // complete aggregation in one PostgreSQL repeatable-read snapshot so every observed leaf
        // and its `attached:N` resource revision describe the same owner state.
        let txn = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(database_error)?;

        let schema =
            load_attached_translation_schema_in(&txn, tenant_id, TAXONOMY_CATEGORY_ENTITY_TYPE)
                .await
                .map_err(flex_storage_error)?;
        let mut progress = FlexAttachedTranslationExactProgress::default();
        let mut after = None;

        loop {
            let page = rustok_taxonomy::list_category_owner_revisions_in(
                &txn,
                tenant_id,
                after,
                PROGRESS_PAGE_SIZE,
            )
            .await
            .map_err(taxonomy_inventory_error)?;
            if page.categories.is_empty() {
                break;
            }
            let ids = page
                .categories
                .iter()
                .map(|category| category.category_id)
                .collect::<Vec<_>>();
            let values = load_attached_translation_localized_values(
                &txn,
                tenant_id,
                TAXONOMY_CATEGORY_ENTITY_TYPE,
                &ids,
            )
            .await
            .map_err(flex_storage_error)?;
            let revisions = load_attached_translation_resource_revisions(
                &txn,
                tenant_id,
                TAXONOMY_CATEGORY_ENTITY_TYPE,
                &ids,
            )
            .await
            .map_err(flex_storage_error)?;

            for category in &page.categories {
                let snapshot = match build_snapshot_from_batch(
                    tenant_id,
                    category.category_id,
                    &schema,
                    &values,
                    &revisions,
                    source_locale,
                    target_locale,
                ) {
                    Ok(snapshot) => snapshot,
                    Err(FlexAttachedTranslationError::SourceLocaleNotFound { .. }) => continue,
                    Err(error) => return Err(error),
                };
                observe_snapshot(&mut progress, &snapshot)?;
            }

            let Some(next_after) = page.next_after else {
                break;
            };
            after = Some(next_after);
        }

        progress.validate()?;
        txn.commit().await.map_err(database_error)?;
        Ok(progress)
    }
}

fn observe_snapshot(
    progress: &mut FlexAttachedTranslationExactProgress,
    snapshot: &flex::FlexAttachedTranslationExactLocaleSnapshot,
) -> FlexAttachedTranslationResult<()> {
    checked_increment(&mut progress.resources, "resources")?;
    let mut complete = true;
    for leaf in &snapshot.leaves {
        let exact = leaf.target_value.is_some();
        if leaf.required {
            checked_increment(&mut progress.required_units, "required_units")?;
            if exact {
                checked_increment(&mut progress.exact_required_units, "exact_required_units")?;
            } else {
                complete = false;
            }
        } else {
            checked_increment(&mut progress.optional_units, "optional_units")?;
            if exact {
                checked_increment(&mut progress.exact_optional_units, "exact_optional_units")?;
            }
        }
    }
    if complete {
        checked_increment(&mut progress.complete_resources, "complete_resources")?;
    }
    Ok(())
}

fn checked_increment(value: &mut u64, label: &str) -> FlexAttachedTranslationResult<()> {
    *value = value.checked_add(1).ok_or_else(|| {
        FlexAttachedTranslationError::OwnerInvariant(format!(
            "attached Translation progress `{label}` overflowed u64"
        ))
    })?;
    Ok(())
}

fn validate_uuid(value: Uuid, label: &str) -> FlexAttachedTranslationResult<()> {
    if value.is_nil() {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "Flex attached translation {label} must not be the nil UUID"
        )));
    }
    Ok(())
}

fn taxonomy_inventory_error(error: rustok_taxonomy::TaxonomyError) -> FlexAttachedTranslationError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => {
            FlexAttachedTranslationError::Storage(error.to_string())
        }
        error => FlexAttachedTranslationError::OwnerInvariant(format!(
            "Taxonomy Category progress inventory violates attached owner contract: {error}"
        )),
    }
}

fn flex_storage_error(error: rustok_core::field_schema::FlexError) -> FlexAttachedTranslationError {
    FlexAttachedTranslationError::Storage(error.to_string())
}

fn database_error(error: sea_orm::DbErr) -> FlexAttachedTranslationError {
    FlexAttachedTranslationError::Storage(error.to_string())
}

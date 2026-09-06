use async_trait::async_trait;
use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, TransactionTrait};
use uuid::Uuid;

use crate::dto::{TaxonomyScopeType, TaxonomyTermKind};
use crate::entities::{taxonomy_term, taxonomy_term_translation};
use crate::error::{TaxonomyError, TaxonomyResult};
use crate::services::TaxonomyService;
use crate::translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx};

/// Host-supplied cleanup for capability-owned data attached to a canonical Category.
///
/// Taxonomy owns the Category hard-delete transaction, but it does not know or import the
/// capability that stores optional extension values. The host injects that cleanup here so
/// attached rows are removed before the owner row is deleted and the whole operation commits
/// atomically.
#[async_trait]
pub trait TaxonomyCategoryDeleteCleanupPort: Send + Sync {
    async fn cleanup_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> TaxonomyResult<()>;
}

impl TaxonomyService {
    /// Hard-delete one canonical Category together with host-owned attached capability data.
    ///
    /// Category transports must use this path rather than the generic term delete whenever the
    /// Category participates in attached capabilities such as Flex.
    pub async fn delete_category_with_cleanup(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        cleanup: &dyn TaxonomyCategoryDeleteCleanupPort,
    ) -> TaxonomyResult<()> {
        if matches!(
            security.get_scope(Resource::Taxonomy, Action::Delete),
            PermissionScope::None
        ) {
            return Err(TaxonomyError::forbidden("Permission denied"));
        }

        self.delete_category_with_cleanup_inner(tenant_id, category_id, None, cleanup)
            .await
    }

    /// Hard-delete a Category owned by a module that already authorized its consumer command.
    ///
    /// This is the delete counterpart to the transaction-bound module Category owner-sync APIs:
    /// the consumer remains the permission owner while Taxonomy validates the exact module scope
    /// and owns the canonical delete transaction. Host cleanup still runs before the canonical
    /// term is removed, so capability data and consumer cleanup can participate atomically.
    pub async fn delete_module_category_with_cleanup(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        module_scope: &str,
        cleanup: &dyn TaxonomyCategoryDeleteCleanupPort,
    ) -> TaxonomyResult<()> {
        let module_scope = module_scope.trim().to_ascii_lowercase();
        if module_scope.is_empty() {
            return Err(TaxonomyError::validation(
                "Category module-owner delete requires a non-empty module scope",
            ));
        }

        self.delete_category_with_cleanup_inner(
            tenant_id,
            category_id,
            Some(module_scope.as_str()),
            cleanup,
        )
        .await
    }

    async fn delete_category_with_cleanup_inner(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        module_scope: Option<&str>,
        cleanup: &dyn TaxonomyCategoryDeleteCleanupPort,
    ) -> TaxonomyResult<()> {
        let mut term_query = taxonomy_term::Entity::find_by_id(category_id)
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category));
        if let Some(module_scope) = module_scope {
            term_query = term_query
                .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
                .filter(taxonomy_term::Column::ScopeValue.eq(module_scope));
        }
        let term = term_query
            .one(self.database())
            .await?
            .ok_or(TaxonomyError::TermNotFound(category_id))?;
        let translations = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TermId.eq(category_id))
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .all(self.database())
            .await?;
        let deletion_translation = translations.iter().min_by(|left, right| {
            left.locale
                .cmp(&right.locale)
                .then_with(|| left.id.cmp(&right.id))
        });
        let locale = deletion_translation
            .map(|translation| translation.locale.as_str())
            .unwrap_or(PLATFORM_FALLBACK_LOCALE);
        let target_revision = deletion_translation
            .map(|translation| translation.revision)
            .unwrap_or_default();
        let resource_revision = term
            .revision
            .checked_add(1)
            .filter(|revision| term.revision > 0 && *revision > 0)
            .ok_or_else(|| {
                TaxonomyError::conflict(format!(
                    "taxonomy term {} has an invalid or exhausted resource revision",
                    term.id
                ))
            })?;

        let txn = self.database().begin().await?;
        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                term_id: category_id,
                locale,
                resource_revision,
                target_revision,
                operation: "delete",
            },
        )
        .await?;
        cleanup.cleanup_in_tx(&txn, tenant_id, category_id).await?;

        let mut delete_query = taxonomy_term::Entity::delete_many()
            .filter(taxonomy_term::Column::Id.eq(category_id))
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
            .filter(taxonomy_term::Column::Revision.eq(term.revision));
        if let Some(module_scope) = module_scope {
            delete_query = delete_query
                .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
                .filter(taxonomy_term::Column::ScopeValue.eq(module_scope));
        }
        let deleted = delete_query.exec(&txn).await?;
        if deleted.rows_affected != 1 {
            return Err(TaxonomyError::conflict(
                "taxonomy Category changed before deletion could commit",
            ));
        }

        txn.commit().await?;
        Ok(())
    }
}

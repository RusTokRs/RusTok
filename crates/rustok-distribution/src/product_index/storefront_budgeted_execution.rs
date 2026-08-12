use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::PortContext;
use rustok_index::IndexQueryPage;
use rustok_product::{
    ProductStorefrontTagHydration, StorefrontProductList, StorefrontProductListQuery,
};
use thiserror::Error;
use tokio::time::timeout;
use uuid::Uuid;

use super::{
    ProductStorefrontIndexPublicProjectionError, ProductStorefrontIndexServingBudgetDecision,
    ProductStorefrontIndexShadowComparison, ProductStorefrontIndexShadowExecutor,
    ProductStorefrontIndexShadowProjectionError, ProductStorefrontIndexTagHydrationError,
    project_product_storefront_index_page,
};

#[derive(Debug)]
pub(crate) struct ProductStorefrontIndexBudgetedExecution {
    /// Already-successful authoritative owner result. Budgeted projection can never replace it.
    pub(crate) authoritative: StorefrontProductList,
    pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexBudgetedProjectionError>,
    pub(crate) public_projected:
        Option<Result<IndexQueryPage, ProductStorefrontIndexPublicProjectionError>>,
    pub(crate) tag_hydration:
        Option<Result<ProductStorefrontTagHydration, ProductStorefrontIndexBudgetedTagHydrationError>>,
    pub(crate) comparison: Option<ProductStorefrontIndexShadowComparison>,
    pub(crate) index_execution_budget_ms: u64,
    pub(crate) tag_hydration_budget_ms: u64,
    pub(crate) safety_margin_ms: u64,
}

#[derive(Debug, Error)]
pub(crate) enum ProductStorefrontIndexBudgetedStartError {
    #[error("Product Storefront budgeted projection requires an eligible serving-budget decision: {0:?}")]
    BudgetNotEligible(ProductStorefrontIndexServingBudgetDecision),
}

#[derive(Debug, Error)]
pub(crate) enum ProductStorefrontIndexBudgetedProjectionError {
    #[error("Product Storefront projected Index phase exceeded its {budget_ms} ms budget")]
    TimedOut { budget_ms: u64 },
    #[error(transparent)]
    Projection(#[from] ProductStorefrontIndexShadowProjectionError),
}

#[derive(Debug, Error)]
pub(crate) enum ProductStorefrontIndexBudgetedTagHydrationError {
    #[error("Product Storefront tag hydration phase exceeded its {budget_ms} ms budget")]
    TimedOut { budget_ms: u64 },
    #[error(transparent)]
    Hydration(#[from] ProductStorefrontIndexTagHydrationError),
}

/// Post-owner projected phases consumed by the budgeted adapter.
///
/// Production uses `ProductStorefrontIndexShadowExecutor`. The trait exists so retained tests can exercise
/// the real budget/timeout wrapper deterministically without PostgreSQL or a synthetic shared Index runtime.
#[async_trait]
pub(crate) trait ProductStorefrontIndexProjectionPhases: Send + Sync {
    async fn execute_projected(
        &self,
        context: PortContext,
        fallback_locale: String,
        public_channel_slug: Option<String>,
        public_channel_id: Option<Uuid>,
        query: StorefrontProductListQuery,
    ) -> Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>;

    async fn hydrate_projected_tags(
        &self,
        context: PortContext,
        fallback_locale: String,
        projected: &IndexQueryPage,
    ) -> Result<ProductStorefrontTagHydration, ProductStorefrontIndexTagHydrationError>;
}

#[async_trait]
impl ProductStorefrontIndexProjectionPhases for ProductStorefrontIndexShadowExecutor {
    async fn execute_projected(
        &self,
        context: PortContext,
        fallback_locale: String,
        public_channel_slug: Option<String>,
        public_channel_id: Option<Uuid>,
        query: StorefrontProductListQuery,
    ) -> Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError> {
        ProductStorefrontIndexShadowExecutor::execute_projected(
            self,
            context,
            fallback_locale,
            public_channel_slug,
            public_channel_id,
            query,
        )
        .await
    }

    async fn hydrate_projected_tags(
        &self,
        context: PortContext,
        fallback_locale: String,
        projected: &IndexQueryPage,
    ) -> Result<ProductStorefrontTagHydration, ProductStorefrontIndexTagHydrationError> {
        ProductStorefrontIndexShadowExecutor::hydrate_projected_tags(
            self,
            context,
            fallback_locale,
            projected,
        )
        .await
    }
}

/// Non-serving executor for an already-authoritative Product Storefront page.
///
/// The caller must first produce the authoritative owner result and classify the post-owner remaining budget.
/// This adapter accepts only `Eligible` phase limits. It narrows Product port contexts to those phase budgets
/// and applies outer Tokio timeouts so local and external selected capabilities cannot create an unbounded tail.
/// Mounted Storefront does not call this adapter.
#[derive(Clone)]
pub(crate) struct ProductStorefrontIndexBudgetedProjectionExecutor {
    phases: Arc<dyn ProductStorefrontIndexProjectionPhases>,
}

impl ProductStorefrontIndexBudgetedProjectionExecutor {
    #[cfg(test)]
    pub(crate) fn from_phases(phases: Arc<dyn ProductStorefrontIndexProjectionPhases>) -> Self {
        Self { phases }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn execute_after_owner(
        &self,
        authoritative: StorefrontProductList,
        context: PortContext,
        fallback_locale: String,
        public_channel_slug: Option<String>,
        public_channel_id: Option<Uuid>,
        query: StorefrontProductListQuery,
        decision: ProductStorefrontIndexServingBudgetDecision,
    ) -> Result<ProductStorefrontIndexBudgetedExecution, ProductStorefrontIndexBudgetedStartError> {
        let (index_execution_budget_ms, tag_hydration_budget_ms, safety_margin_ms) = match decision {
            ProductStorefrontIndexServingBudgetDecision::Eligible {
                index_execution_ms,
                tag_hydration_ms,
                safety_margin_ms,
            } => (index_execution_ms, tag_hydration_ms, safety_margin_ms),
            other => return Err(ProductStorefrontIndexBudgetedStartError::BudgetNotEligible(other)),
        };

        let mut index_context = context.clone();
        index_context.deadline_ms = Some(index_execution_budget_ms);
        let projected = match timeout(
            Duration::from_millis(index_execution_budget_ms),
            self.phases.execute_projected(
                index_context,
                fallback_locale.clone(),
                public_channel_slug,
                public_channel_id,
                query,
            ),
        )
        .await
        {
            Ok(result) => result.map_err(ProductStorefrontIndexBudgetedProjectionError::Projection),
            Err(_) => Err(ProductStorefrontIndexBudgetedProjectionError::TimedOut {
                budget_ms: index_execution_budget_ms,
            }),
        };

        let public_projected = projected
            .as_ref()
            .ok()
            .cloned()
            .map(project_product_storefront_index_page);

        let tag_hydration = match projected.as_ref() {
            Ok(projected) => {
                let mut tag_context = context;
                tag_context.deadline_ms = Some(tag_hydration_budget_ms);
                Some(
                    match timeout(
                        Duration::from_millis(tag_hydration_budget_ms),
                        self.phases
                            .hydrate_projected_tags(tag_context, fallback_locale, projected),
                    )
                    .await
                    {
                        Ok(result) => result
                            .map_err(ProductStorefrontIndexBudgetedTagHydrationError::Hydration),
                        Err(_) => Err(ProductStorefrontIndexBudgetedTagHydrationError::TimedOut {
                            budget_ms: tag_hydration_budget_ms,
                        }),
                    },
                )
            }
            Err(_) => None,
        };

        let comparison = projected
            .as_ref()
            .ok()
            .map(|projected| compare_owner_and_projected(&authoritative, projected));

        Ok(ProductStorefrontIndexBudgetedExecution {
            authoritative,
            projected,
            public_projected,
            tag_hydration,
            comparison,
            index_execution_budget_ms,
            tag_hydration_budget_ms,
            safety_margin_ms,
        })
    }
}

fn compare_owner_and_projected(
    authoritative: &StorefrontProductList,
    projected: &IndexQueryPage,
) -> ProductStorefrontIndexShadowComparison {
    let authoritative_ids = authoritative
        .items
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let projected_ids = projected
        .items
        .iter()
        .map(|item| item.entity_id)
        .collect::<Vec<_>>();
    ProductStorefrontIndexShadowComparison {
        identities_match: authoritative_ids == projected_ids,
        exact_count_matches: projected.exact_count == Some(authoritative.total),
        has_more_matches: projected.has_more == authoritative.has_next,
    }
}

#[cfg(test)]
mod tests {
    use rustok_index::IndexValue;

    use super::*;

    #[test]
    fn noneligible_decision_is_representable_without_starting_projection() {
        let error = ProductStorefrontIndexBudgetedStartError::BudgetNotEligible(
            ProductStorefrontIndexServingBudgetDecision::OwnerNativeInsufficientBudget {
                required_ms: 140,
                remaining_ms: 139,
            },
        );
        assert!(error.to_string().contains("requires an eligible serving-budget decision"));
    }

    #[test]
    fn comparison_contract_still_uses_raw_identity_count_and_boundary() {
        let entity_id = Uuid::new_v4();
        let authoritative = StorefrontProductList {
            items: vec![rustok_product::StorefrontProductListItem {
                id: entity_id,
                status: rustok_product::entities::product::ProductStatus::Active,
                title: "Untitled product".to_owned(),
                handle: String::new(),
                seller_id: None,
                vendor: None,
                product_type: None,
                tags: Vec::new(),
                created_at: chrono::Utc::now(),
                published_at: None,
            }],
            total: 1,
            page: 1,
            per_page: 12,
            has_next: false,
        };
        let projected = IndexQueryPage {
            items: vec![rustok_index::IndexQueryItem {
                entity_id,
                relations: Vec::new(),
                fields: vec![rustok_index::IndexProjectedValue {
                    path: rustok_index::FieldPath::new(
                        rustok_index::FieldName::new("title").unwrap(),
                    ),
                    value: IndexValue::Null,
                }],
                nested_relations: Vec::new(),
            }],
            exact_count: Some(1),
            has_more: false,
            next_cursor: None,
        };
        assert!(compare_owner_and_projected(&authoritative, &projected).is_match());
    }
}

use rustok_api::PortContext;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProductStorefrontIndexServingBudget {
    index_execution_ms: u64,
    tag_hydration_ms: u64,
    safety_margin_ms: u64,
    required_ms: u64,
}

impl ProductStorefrontIndexServingBudget {
    pub(crate) fn new(
        index_execution_ms: u64,
        tag_hydration_ms: u64,
        safety_margin_ms: u64,
    ) -> Result<Self, ProductStorefrontIndexServingBudgetError> {
        if index_execution_ms == 0 {
            return Err(ProductStorefrontIndexServingBudgetError::ZeroIndexExecutionBudget);
        }
        if tag_hydration_ms == 0 {
            return Err(ProductStorefrontIndexServingBudgetError::ZeroTagHydrationBudget);
        }
        let required_ms = index_execution_ms
            .checked_add(tag_hydration_ms)
            .and_then(|value| value.checked_add(safety_margin_ms))
            .ok_or(ProductStorefrontIndexServingBudgetError::BudgetOverflow)?;
        Ok(Self {
            index_execution_ms,
            tag_hydration_ms,
            safety_margin_ms,
            required_ms,
        })
    }

    pub(crate) const fn index_execution_ms(self) -> u64 {
        self.index_execution_ms
    }

    pub(crate) const fn tag_hydration_ms(self) -> u64 {
        self.tag_hydration_ms
    }

    pub(crate) const fn safety_margin_ms(self) -> u64 {
        self.safety_margin_ms
    }

    pub(crate) const fn required_ms(self) -> u64 {
        self.required_ms
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProductStorefrontIndexServingBudgetObservation {
    /// Remaining request budget measured by the host at the post-owner handoff point.
    ///
    /// This must not be reconstructed from `PortContext.deadline_ms`: that field carries the original
    /// duration budget and does not automatically decrease while the authoritative owner call executes.
    pub(crate) remaining_ms: Option<u64>,
    pub(crate) tag_hydration_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductStorefrontIndexServingBudgetDecision {
    Eligible {
        index_execution_ms: u64,
        tag_hydration_ms: u64,
        safety_margin_ms: u64,
    },
    OwnerNativeMissingDeadline,
    OwnerNativeBudgetPolicyUnavailable,
    OwnerNativeRemainingBudgetUnavailable,
    OwnerNativeInvalidRemainingBudget {
        deadline_ms: u64,
        remaining_ms: u64,
    },
    OwnerNativeTagHydrationUnavailable,
    OwnerNativeInsufficientBudget {
        required_ms: u64,
        remaining_ms: u64,
    },
}

impl ProductStorefrontIndexServingBudgetDecision {
    pub(crate) const fn is_eligible(self) -> bool {
        matches!(self, Self::Eligible { .. })
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum ProductStorefrontIndexServingBudgetError {
    #[error("Product Storefront Index serving budget requires a positive Index execution phase")]
    ZeroIndexExecutionBudget,
    #[error("Product Storefront Index serving budget requires a positive Product tag hydration phase")]
    ZeroTagHydrationBudget,
    #[error("Product Storefront Index serving budget exceeds the supported millisecond range")]
    BudgetOverflow,
}

/// Classify whether a future serving router may spend the remaining request budget on Index plus owner
/// hydration after the authoritative Product owner call has already completed.
///
/// The caller must supply a host-measured `remaining_ms` observation at that exact handoff point. The policy
/// deliberately does not treat `PortContext.deadline_ms` as remaining time because it is only the original
/// duration budget. Missing/inconsistent timing information, missing tag capability, or insufficient budget
/// keeps the request owner-native rather than allowing an unbounded shadow/serving tail.
pub(crate) fn classify_product_storefront_index_serving_budget(
    context: &PortContext,
    budget: Option<ProductStorefrontIndexServingBudget>,
    observation: ProductStorefrontIndexServingBudgetObservation,
) -> ProductStorefrontIndexServingBudgetDecision {
    let Some(deadline_ms) = context.deadline_ms.filter(|deadline_ms| *deadline_ms > 0) else {
        return ProductStorefrontIndexServingBudgetDecision::OwnerNativeMissingDeadline;
    };
    let Some(budget) = budget else {
        return ProductStorefrontIndexServingBudgetDecision::OwnerNativeBudgetPolicyUnavailable;
    };
    let Some(remaining_ms) = observation.remaining_ms else {
        return ProductStorefrontIndexServingBudgetDecision::OwnerNativeRemainingBudgetUnavailable;
    };
    if remaining_ms > deadline_ms {
        return ProductStorefrontIndexServingBudgetDecision::OwnerNativeInvalidRemainingBudget {
            deadline_ms,
            remaining_ms,
        };
    }
    if !observation.tag_hydration_available {
        return ProductStorefrontIndexServingBudgetDecision::OwnerNativeTagHydrationUnavailable;
    }
    if remaining_ms < budget.required_ms() {
        return ProductStorefrontIndexServingBudgetDecision::OwnerNativeInsufficientBudget {
            required_ms: budget.required_ms(),
            remaining_ms,
        };
    }
    ProductStorefrontIndexServingBudgetDecision::Eligible {
        index_execution_ms: budget.index_execution_ms(),
        tag_hydration_ms: budget.tag_hydration_ms(),
        safety_margin_ms: budget.safety_margin_ms(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::PortActor;

    use super::*;

    fn context(deadline_ms: Option<u64>) -> PortContext {
        let context = PortContext::new("tenant", PortActor::system(), "en", "correlation");
        match deadline_ms {
            Some(deadline_ms) => context.with_deadline(Duration::from_millis(deadline_ms)),
            None => context,
        }
    }

    fn budget() -> ProductStorefrontIndexServingBudget {
        ProductStorefrontIndexServingBudget::new(80, 40, 20).unwrap()
    }

    #[test]
    fn requires_host_measured_remaining_budget_and_owner_tag_capability() {
        assert_eq!(
            classify_product_storefront_index_serving_budget(
                &context(None),
                Some(budget()),
                ProductStorefrontIndexServingBudgetObservation {
                    remaining_ms: Some(200),
                    tag_hydration_available: true,
                },
            ),
            ProductStorefrontIndexServingBudgetDecision::OwnerNativeMissingDeadline
        );
        assert_eq!(
            classify_product_storefront_index_serving_budget(
                &context(Some(300)),
                None,
                ProductStorefrontIndexServingBudgetObservation {
                    remaining_ms: Some(200),
                    tag_hydration_available: true,
                },
            ),
            ProductStorefrontIndexServingBudgetDecision::OwnerNativeBudgetPolicyUnavailable
        );
        assert_eq!(
            classify_product_storefront_index_serving_budget(
                &context(Some(300)),
                Some(budget()),
                ProductStorefrontIndexServingBudgetObservation {
                    remaining_ms: None,
                    tag_hydration_available: true,
                },
            ),
            ProductStorefrontIndexServingBudgetDecision::OwnerNativeRemainingBudgetUnavailable
        );
        assert_eq!(
            classify_product_storefront_index_serving_budget(
                &context(Some(300)),
                Some(budget()),
                ProductStorefrontIndexServingBudgetObservation {
                    remaining_ms: Some(301),
                    tag_hydration_available: true,
                },
            ),
            ProductStorefrontIndexServingBudgetDecision::OwnerNativeInvalidRemainingBudget {
                deadline_ms: 300,
                remaining_ms: 301,
            }
        );
        assert_eq!(
            classify_product_storefront_index_serving_budget(
                &context(Some(300)),
                Some(budget()),
                ProductStorefrontIndexServingBudgetObservation {
                    remaining_ms: Some(200),
                    tag_hydration_available: false,
                },
            ),
            ProductStorefrontIndexServingBudgetDecision::OwnerNativeTagHydrationUnavailable
        );
    }

    #[test]
    fn admits_only_when_remaining_budget_covers_all_bounded_phases() {
        assert_eq!(budget().required_ms(), 140);
        assert_eq!(
            classify_product_storefront_index_serving_budget(
                &context(Some(300)),
                Some(budget()),
                ProductStorefrontIndexServingBudgetObservation {
                    remaining_ms: Some(139),
                    tag_hydration_available: true,
                },
            ),
            ProductStorefrontIndexServingBudgetDecision::OwnerNativeInsufficientBudget {
                required_ms: 140,
                remaining_ms: 139,
            }
        );
        assert_eq!(
            classify_product_storefront_index_serving_budget(
                &context(Some(300)),
                Some(budget()),
                ProductStorefrontIndexServingBudgetObservation {
                    remaining_ms: Some(140),
                    tag_hydration_available: true,
                },
            ),
            ProductStorefrontIndexServingBudgetDecision::Eligible {
                index_execution_ms: 80,
                tag_hydration_ms: 40,
                safety_margin_ms: 20,
            }
        );
    }

    #[test]
    fn rejects_zero_phase_budgets_and_overflow() {
        assert_eq!(
            ProductStorefrontIndexServingBudget::new(0, 1, 0),
            Err(ProductStorefrontIndexServingBudgetError::ZeroIndexExecutionBudget)
        );
        assert_eq!(
            ProductStorefrontIndexServingBudget::new(1, 0, 0),
            Err(ProductStorefrontIndexServingBudgetError::ZeroTagHydrationBudget)
        );
        assert_eq!(
            ProductStorefrontIndexServingBudget::new(u64::MAX, 1, 0),
            Err(ProductStorefrontIndexServingBudgetError::BudgetOverflow)
        );
    }

    #[test]
    fn decision_is_eligible_distinguishes_eligible_variant() {
        let eligible = ProductStorefrontIndexServingBudgetDecision::Eligible {
            index_execution_ms: 10,
            tag_hydration_ms: 10,
            safety_margin_ms: 5,
        };
        assert!(eligible.is_eligible());
        assert!(!ProductStorefrontIndexServingBudgetDecision::OwnerNativeTagHydrationUnavailable.is_eligible());
    }
}

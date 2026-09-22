use rustok_events::ProductIndexRefreshEvent;

use super::{
    ProductIndexLocaleRefreshRecord, ProductIndexRefreshContract,
    ProductIndexRefreshContractTarget, ProductIndexRefreshEventFactory,
    ProductIndexVariantRefreshRecord,
};

impl ProductIndexRefreshContract for ProductIndexRefreshEvent {
    fn product_index_refresh_target(&self) -> ProductIndexRefreshContractTarget {
        match self {
            Self::LocaleRefreshRequested {
                product_id,
                locale,
                source_version,
            } => ProductIndexRefreshContractTarget::Locale {
                product_id: *product_id,
                locale: locale.clone(),
                source_version: *source_version,
            },
            Self::VariantRefreshRequested {
                product_id,
                variant_id,
                source_version,
            } => ProductIndexRefreshContractTarget::Variant {
                product_id: *product_id,
                variant_id: *variant_id,
                source_version: *source_version,
            },
        }
    }
}

/// Canonical factory from immutable Product refresh ledger records to the sealed wire family.
///
/// Envelope identity, tenant, actor and causation remain owned by the canonical writer. This
/// factory copies only the target facts that are compared back to the ledger before publication.
#[derive(Clone, Copy, Debug, Default)]
pub struct CanonicalProductIndexRefreshEventFactory;

impl ProductIndexRefreshEventFactory for CanonicalProductIndexRefreshEventFactory {
    type LocaleEvent = ProductIndexRefreshEvent;
    type VariantEvent = ProductIndexRefreshEvent;

    fn locale_event(&self, record: &ProductIndexLocaleRefreshRecord) -> Self::LocaleEvent {
        ProductIndexRefreshEvent::LocaleRefreshRequested {
            product_id: record.product_id(),
            locale: record.locale().to_owned(),
            source_version: record.source_version(),
        }
    }

    fn variant_event(&self, record: &ProductIndexVariantRefreshRecord) -> Self::VariantEvent {
        ProductIndexRefreshEvent::VariantRefreshRequested {
            product_id: record.product_id(),
            variant_id: record.variant_id(),
            source_version: record.source_version(),
        }
    }
}

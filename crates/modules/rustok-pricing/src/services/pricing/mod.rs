use sea_orm::DatabaseConnection;
use std::sync::Arc;

use rustok_api::{PortContext, PortError, PortErrorKind};
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_outbox::TransactionalEventBus;

pub mod commands;
pub mod projections;
pub mod queries;
pub mod resolution;
pub mod scope;
pub mod types;

pub use types::*;
pub(crate) use resolution::ResolutionError;

pub struct PricingService {
    pub(crate) db: DatabaseConnection,
    pub(crate) event_bus: TransactionalEventBus,
    pub(crate) product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime,
}

impl PricingService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        // This constructor is an explicit embedded baseline. Mounted reads install their
        // already selected Product runtime before use; failures never select this baseline.
        let product_catalog_read_runtime =
            rustok_product::ProductCatalogReadRuntime::in_process(db.clone(), event_bus.clone());
        Self {
            db,
            event_bus,
            product_catalog_read_runtime,
        }
    }

    pub fn with_product_catalog_read_runtime(
        mut self,
        runtime: rustok_product::ProductCatalogReadRuntime,
    ) -> Self {
        self.product_catalog_read_runtime = runtime;
        self
    }

    /// Current SKU mutation/preview helpers lock local Product variant rows. They cannot
    /// safely execute against a differently selected Product owner until that write protocol exists.
    pub(crate) fn require_embedded_variant_state(&self) -> Result<(), PortError> {
        if self.product_catalog_read_runtime.profile()
            != rustok_product::ProductCatalogReadProfile::EmbeddedNative
        {
            return Err(PortError::new(
                PortErrorKind::Unavailable,
                "pricing.product_write_capability_unavailable",
                "SKU price authoring is unavailable for the selected external Product owner",
                false,
            ));
        }
        Ok(())
    }

    pub(crate) fn require_embedded_variant_service_state(&self) -> CommerceResult<()> {
        self.require_embedded_variant_state().map_err(|_| {
            CommerceError::Validation(
                "SKU price authoring requires the embedded Product write protocol; no local fallback is allowed"
                    .to_owned(),
            )
        })
    }

    pub(crate) fn variant_identity_read_port(
        &self,
    ) -> Result<Arc<dyn rustok_product::ProductVariantIdentityReadPort>, PortError> {
        self.product_catalog_read_runtime
            .variant_identity_read_port()
            .ok_or_else(|| {
                PortError::new(
                    PortErrorKind::Unavailable,
                    "pricing.product_identity_capability_unavailable",
                    "The selected Product owner has not supplied SKU identity verification",
                    false,
                )
            })
    }

    pub(crate) async fn read_product_variant_identities(
        &self,
        context: &PortContext,
        request: &rustok_product::ProductVariantIdentitiesRequest,
    ) -> Result<Vec<rustok_product::ProductVariantIdentitySnapshot>, PortError> {
        let identities = self
            .variant_identity_read_port()?
            .read_variant_identities(context.clone(), request.clone())
            .await
            .map_err(projections::pricing_identity_port_error)?;

        rustok_product::validate_variant_identities_response(context, request, &identities)
            .map_err(|error| {
                PortError::new(
                    error.kind,
                    "pricing.variant_identity_invalid",
                    "The selected Product owner returned inconsistent SKU ownership",
                    error.retryable,
                )
            })?;

        Ok(identities)
    }

    pub(crate) fn channel_read_port(&self) -> Arc<dyn rustok_channel::ChannelReadPort> {
        rustok_channel::in_process_channel_read_port(self.db.clone())
    }

    pub(crate) fn region_read_port(&self) -> Arc<dyn rustok_region::RegionReadPort> {
        rustok_region::in_process_region_read_port(self.db.clone())
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn validate_discount_percent_bounds() {
        assert!(validate_discount_percent(Decimal::new(10, 0)).is_ok());
        assert!(validate_discount_percent(Decimal::new(100, 0)).is_ok());
        assert!(validate_discount_percent(Decimal::ZERO).is_err());
        assert!(validate_discount_percent(Decimal::new(-5, 0)).is_err());
        assert!(validate_discount_percent(Decimal::new(101, 0)).is_err());
    }

    #[test]
    fn validate_price_tier_quantities_bounds() {
        assert!(validate_price_tier_quantities(Some(1), Some(10)).is_ok());
        assert!(validate_price_tier_quantities(Some(5), Some(5)).is_ok());
        assert!(validate_price_tier_quantities(None, None).is_ok());
        assert!(validate_price_tier_quantities(Some(0), Some(10)).is_err());
        assert!(validate_price_tier_quantities(Some(10), Some(5)).is_err());
        assert!(validate_price_tier_quantities(Some(1), Some(-1)).is_err());
    }

    #[test]
    fn calculate_discount_percent_precision() {
        let amount = Decimal::new(80, 0);
        let compare_at = Some(Decimal::new(100, 0));
        assert_eq!(
            calculate_discount_percent(amount, compare_at),
            Some(Decimal::new(2000, 2))
        );

        let amount_equal = Decimal::new(100, 0);
        assert_eq!(calculate_discount_percent(amount_equal, compare_at), None);

        let amount_greater = Decimal::new(120, 0);
        assert_eq!(calculate_discount_percent(amount_greater, compare_at), None);
    }

    #[test]
    fn normalize_resolution_currency_accepts_valid_and_rejects_invalid() {
        assert_eq!(normalize_resolution_currency("usd").unwrap(), "USD");
        assert_eq!(normalize_resolution_currency("EUR").unwrap(), "EUR");
        assert!(normalize_resolution_currency("US").is_err());
        assert!(normalize_resolution_currency("USDT").is_err());
        assert!(normalize_resolution_currency("123").is_err());
    }

    #[test]
    fn normalize_resolution_quantity_bounds() {
        assert_eq!(normalize_resolution_quantity(Some(5)).unwrap(), 5);
        assert_eq!(normalize_resolution_quantity(None).unwrap(), 1);
        assert!(normalize_resolution_quantity(Some(0)).is_err());
        assert!(normalize_resolution_quantity(Some(-1)).is_err());
    }

    #[test]
    fn is_sale_price_behavior() {
        assert!(is_sale_price(
            Decimal::new(80, 0),
            Some(Decimal::new(100, 0))
        ));
        assert!(!is_sale_price(
            Decimal::new(100, 0),
            Some(Decimal::new(100, 0))
        ));
        assert!(!is_sale_price(
            Decimal::new(120, 0),
            Some(Decimal::new(100, 0))
        ));
        assert!(!is_sale_price(Decimal::new(80, 0), None));
    }
}

//! Database-neutral admission contract for Product -> SalesChannel relation snapshots.
//!
//! The Product owner persists resolved UUID membership under a dedicated monotonic relation epoch.
//! This contract validates locale fan-out identity and rejects epoch reuse for different membership.
#![allow(dead_code)]

use rustok_index::{IndexSourceEventIdError, LocaleKey, derive_index_source_event_id};
use thiserror::Error;
use uuid::Uuid;

const PRODUCT_SALES_CHANNEL_RELATION_EVENT_DOMAIN: &str =
    "rustok-distribution.product-sales-channel-relation";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProductSalesChannelRelationEpoch(u64);

impl ProductSalesChannelRelationEpoch {
    pub(crate) fn new(value: u64) -> Result<Self, ProductSalesChannelRelationAdmissionError> {
        if value == 0 {
            return Err(ProductSalesChannelRelationAdmissionError::ZeroEpoch);
        }
        Ok(Self(value))
    }

    pub(crate) fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductSalesChannelRelationAdmission {
    Initial,
    Retry,
    Advanced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductSalesChannelRelationSnapshot {
    tenant_id: Uuid,
    product_id: Uuid,
    locale: LocaleKey,
    epoch: ProductSalesChannelRelationEpoch,
    channel_ids: Vec<Uuid>,
    event_id: Uuid,
}

impl ProductSalesChannelRelationSnapshot {
    pub(crate) fn new(
        tenant_id: Uuid,
        product_id: Uuid,
        locale: LocaleKey,
        epoch: ProductSalesChannelRelationEpoch,
        channel_ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<Self, ProductSalesChannelRelationAdmissionError> {
        if tenant_id.is_nil() {
            return Err(ProductSalesChannelRelationAdmissionError::NilTenantId);
        }
        if product_id.is_nil() {
            return Err(ProductSalesChannelRelationAdmissionError::NilProductId);
        }

        let mut channel_ids = channel_ids.into_iter().collect::<Vec<_>>();
        if channel_ids.iter().any(Uuid::is_nil) {
            return Err(ProductSalesChannelRelationAdmissionError::NilChannelId);
        }
        channel_ids.sort_unstable();
        if channel_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ProductSalesChannelRelationAdmissionError::DuplicateChannelId);
        }

        let event_id = derive_index_source_event_id(
            PRODUCT_SALES_CHANNEL_RELATION_EVENT_DOMAIN,
            tenant_id,
            product_id,
            Some(&locale),
            epoch.get(),
        )?;

        Ok(Self {
            tenant_id,
            product_id,
            locale,
            epoch,
            channel_ids,
            event_id,
        })
    }

    pub(crate) fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub(crate) fn product_id(&self) -> Uuid {
        self.product_id
    }

    pub(crate) fn locale(&self) -> &LocaleKey {
        &self.locale
    }

    pub(crate) fn epoch(&self) -> ProductSalesChannelRelationEpoch {
        self.epoch
    }

    pub(crate) fn channel_ids(&self) -> &[Uuid] {
        &self.channel_ids
    }

    pub(crate) fn event_id(&self) -> Uuid {
        self.event_id
    }

    pub(crate) fn admit(
        previous: Option<&Self>,
        next: &Self,
    ) -> Result<ProductSalesChannelRelationAdmission, ProductSalesChannelRelationAdmissionError>
    {
        let Some(previous) = previous else {
            return Ok(ProductSalesChannelRelationAdmission::Initial);
        };
        if previous.tenant_id != next.tenant_id
            || previous.product_id != next.product_id
            || previous.locale != next.locale
        {
            return Err(ProductSalesChannelRelationAdmissionError::ScopeChanged);
        }
        if next.epoch < previous.epoch {
            return Err(ProductSalesChannelRelationAdmissionError::EpochRegressed {
                previous: previous.epoch.get(),
                next: next.epoch.get(),
            });
        }
        if next.epoch == previous.epoch {
            if next.channel_ids != previous.channel_ids || next.event_id != previous.event_id {
                return Err(ProductSalesChannelRelationAdmissionError::SameEpochMembershipChanged);
            }
            return Ok(ProductSalesChannelRelationAdmission::Retry);
        }
        Ok(ProductSalesChannelRelationAdmission::Advanced)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum ProductSalesChannelRelationAdmissionError {
    #[error("Product-SalesChannel relation epoch must be positive")]
    ZeroEpoch,
    #[error("Product-SalesChannel relation tenant id must not be nil")]
    NilTenantId,
    #[error("Product-SalesChannel relation product id must not be nil")]
    NilProductId,
    #[error("Product-SalesChannel relation channel id must not be nil")]
    NilChannelId,
    #[error("Product-SalesChannel relation channel ids must be unique")]
    DuplicateChannelId,
    #[error("Product-SalesChannel relation admission cannot change tenant, product, or locale")]
    ScopeChanged,
    #[error("Product-SalesChannel relation epoch regressed: previous={previous}, next={next}")]
    EpochRegressed { previous: u64, next: u64 },
    #[error("Product-SalesChannel membership changed without advancing its relation epoch")]
    SameEpochMembershipChanged,
    #[error(transparent)]
    EventIdentity(#[from] IndexSourceEventIdError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale(value: &str) -> LocaleKey {
        LocaleKey::new(value).unwrap()
    }

    fn scoped_snapshot(
        tenant_id: Uuid,
        product_id: Uuid,
        epoch: u64,
        locale: &str,
        channel_ids: impl IntoIterator<Item = Uuid>,
    ) -> ProductSalesChannelRelationSnapshot {
        ProductSalesChannelRelationSnapshot::new(
            tenant_id,
            product_id,
            self::locale(locale),
            ProductSalesChannelRelationEpoch::new(epoch).unwrap(),
            channel_ids,
        )
        .unwrap()
    }

    fn snapshot(
        epoch: u64,
        locale: &str,
        channel_ids: impl IntoIterator<Item = Uuid>,
    ) -> ProductSalesChannelRelationSnapshot {
        scoped_snapshot(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            epoch,
            locale,
            channel_ids,
        )
    }

    #[test]
    fn product_sales_channel_relation_canonical_membership_and_retry_identity_are_stable() {
        let first = snapshot(7, "en-US", [Uuid::from_u128(4), Uuid::from_u128(3)]);
        let retry = snapshot(7, "en-US", [Uuid::from_u128(3), Uuid::from_u128(4)]);

        assert_eq!(first, retry);
        assert_eq!(
            first.channel_ids(),
            &[Uuid::from_u128(3), Uuid::from_u128(4)]
        );
        assert!(!first.event_id().is_nil());
        assert_eq!(
            ProductSalesChannelRelationSnapshot::admit(None, &first),
            Ok(ProductSalesChannelRelationAdmission::Initial)
        );
        assert_eq!(
            ProductSalesChannelRelationSnapshot::admit(Some(&first), &retry),
            Ok(ProductSalesChannelRelationAdmission::Retry)
        );
    }

    #[test]
    fn product_sales_channel_relation_change_requires_a_strictly_larger_epoch() {
        let previous = snapshot(8, "en-US", [Uuid::from_u128(3)]);
        let changed_same_epoch = snapshot(8, "en-US", [Uuid::from_u128(4)]);
        let regressed = snapshot(7, "en-US", [Uuid::from_u128(4)]);
        let advanced = snapshot(9, "en-US", [Uuid::from_u128(4)]);

        assert_eq!(
            ProductSalesChannelRelationSnapshot::admit(Some(&previous), &changed_same_epoch),
            Err(ProductSalesChannelRelationAdmissionError::SameEpochMembershipChanged)
        );
        assert_eq!(
            ProductSalesChannelRelationSnapshot::admit(Some(&previous), &regressed),
            Err(ProductSalesChannelRelationAdmissionError::EpochRegressed {
                previous: 8,
                next: 7,
            })
        );
        assert_eq!(
            ProductSalesChannelRelationSnapshot::admit(Some(&previous), &advanced),
            Ok(ProductSalesChannelRelationAdmission::Advanced)
        );
        assert_ne!(previous.event_id(), advanced.event_id());
    }

    #[test]
    fn product_sales_channel_relation_invalid_identity_and_duplicates_fail_closed() {
        assert_eq!(
            ProductSalesChannelRelationEpoch::new(0),
            Err(ProductSalesChannelRelationAdmissionError::ZeroEpoch)
        );
        assert_eq!(
            ProductSalesChannelRelationSnapshot::new(
                Uuid::nil(),
                Uuid::from_u128(2),
                locale("en-US"),
                ProductSalesChannelRelationEpoch::new(1).unwrap(),
                [],
            ),
            Err(ProductSalesChannelRelationAdmissionError::NilTenantId)
        );
        assert_eq!(
            ProductSalesChannelRelationSnapshot::new(
                Uuid::from_u128(1),
                Uuid::nil(),
                locale("en-US"),
                ProductSalesChannelRelationEpoch::new(1).unwrap(),
                [],
            ),
            Err(ProductSalesChannelRelationAdmissionError::NilProductId)
        );
        assert_eq!(
            ProductSalesChannelRelationSnapshot::new(
                Uuid::from_u128(1),
                Uuid::from_u128(2),
                locale("en-US"),
                ProductSalesChannelRelationEpoch::new(1).unwrap(),
                [Uuid::nil()],
            ),
            Err(ProductSalesChannelRelationAdmissionError::NilChannelId)
        );
        assert_eq!(
            ProductSalesChannelRelationSnapshot::new(
                Uuid::from_u128(1),
                Uuid::from_u128(2),
                locale("en-US"),
                ProductSalesChannelRelationEpoch::new(1).unwrap(),
                [Uuid::from_u128(3), Uuid::from_u128(3)],
            ),
            Err(ProductSalesChannelRelationAdmissionError::DuplicateChannelId)
        );
    }

    #[test]
    fn product_sales_channel_relation_empty_membership_is_valid_but_scope_cannot_change() {
        let previous = snapshot(1, "en-US", []);
        let other_locale = snapshot(2, "fr-FR", []);
        let other_tenant = scoped_snapshot(Uuid::from_u128(9), Uuid::from_u128(2), 2, "en-US", []);
        let other_product = scoped_snapshot(Uuid::from_u128(1), Uuid::from_u128(9), 2, "en-US", []);

        assert!(previous.channel_ids().is_empty());
        for changed_scope in [&other_locale, &other_tenant, &other_product] {
            assert_eq!(
                ProductSalesChannelRelationSnapshot::admit(Some(&previous), changed_scope),
                Err(ProductSalesChannelRelationAdmissionError::ScopeChanged)
            );
        }
    }
}

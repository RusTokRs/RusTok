//! Cart buyer lifecycle and command planning, independent of the rendering framework.

use super::{
    add_variant_request, buyer_cart_context_matches, create_cart_request, CartAddVariantRequest,
    CartCreateRequest,
};
use crate::i18n::t;
use crate::model::StorefrontCart;
use rustok_pricing_storefront::{parse_optional_uuid_string, PricingResolutionContext};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuyerCartPhase {
    Restoring,
    Empty,
    Ready,
    AuthRefused,
    RestoreFailed,
    ContextMismatch,
    Unconfirmed,
    Added,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuyerCartSession {
    pub cart_id: Option<String>,
    pub cart: Option<StorefrontCart>,
    pub phase: BuyerCartPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuyerCartError {
    Invalid,
    Restore,
    Context,
    Unconfirmed,
}

#[derive(Clone, Debug)]
pub enum BuyerCartPlan {
    Create(CartCreateRequest),
    Add(CartAddVariantRequest),
}

impl BuyerCartSession {
    pub fn restoring(cart_id: Option<String>) -> Self {
        Self {
            cart_id,
            cart: None,
            phase: BuyerCartPhase::Restoring,
        }
    }
    pub fn empty() -> Self {
        Self {
            cart_id: None,
            cart: None,
            phase: BuyerCartPhase::Empty,
        }
    }
    pub fn restore_id(&self) -> Result<Option<String>, BuyerCartError> {
        parse_optional_uuid_string(self.cart_id.clone(), "cart_id")
            .map_err(|_| BuyerCartError::Restore)
    }
    pub fn adopt(&mut self, cart: StorefrontCart, added: bool) -> Result<(), BuyerCartError> {
        let id = parse_optional_uuid_string(Some(cart.id.clone()), "cart_id")
            .map_err(|_| BuyerCartError::Restore)?;
        if id.as_deref() != Some(cart.id.as_str())
            || self
                .restore_id()?
                .as_ref()
                .is_some_and(|expected| expected != &cart.id)
            || cart.status != "active"
        {
            return Err(BuyerCartError::Restore);
        }
        self.cart_id = id;
        self.cart = Some(cart);
        self.phase = if added {
            BuyerCartPhase::Added
        } else {
            BuyerCartPhase::Ready
        };
        Ok(())
    }
    /// Only a successful owner read may prove a missing reference. Inactive carts and failures retain it.
    pub fn finish_restore(&self, cart: Option<StorefrontCart>) -> Result<Self, BuyerCartError> {
        let expected = self.restore_id()?;
        let Some(cart) = cart else {
            return Ok(Self::empty());
        };
        let id = expected.ok_or(BuyerCartError::Restore)?;
        if cart.id != id {
            return Err(BuyerCartError::Restore);
        }
        let mut restored = self.clone();
        restored.adopt(cart, false)?;
        Ok(restored)
    }
    pub fn permits_write(&self) -> bool {
        matches!(
            self.phase,
            BuyerCartPhase::Empty
                | BuyerCartPhase::Ready
                | BuyerCartPhase::ContextMismatch
                | BuyerCartPhase::Added
        )
    }
    pub fn can_inspect(&self) -> bool {
        self.cart_id.is_some() && self.phase != BuyerCartPhase::Restoring
    }
    pub fn fail(&mut self, error: BuyerCartError) {
        self.phase = match error {
            BuyerCartError::Context => BuyerCartPhase::ContextMismatch,
            BuyerCartError::Unconfirmed => BuyerCartPhase::Unconfirmed,
            BuyerCartError::Restore | BuyerCartError::Invalid => BuyerCartPhase::RestoreFailed,
        };
    }
    pub fn plan_add(
        &self,
        variant_id: String,
        context: PricingResolutionContext,
        locale: Option<String>,
    ) -> Result<BuyerCartPlan, BuyerCartError> {
        if !self.permits_write() {
            return Err(BuyerCartError::Restore);
        }
        if let Some(cart) = self.cart.as_ref() {
            if self.cart_id.as_deref() != Some(cart.id.as_str())
                || !buyer_cart_context_matches(cart, &context, locale.as_deref())
            {
                return Err(BuyerCartError::Context);
            }
            add_variant_request(cart.id.clone(), variant_id, context, locale)
                .map(BuyerCartPlan::Add)
                .map_err(|_| BuyerCartError::Invalid)
        } else if self.cart_id.is_none() {
            // Validate the SKU and quantity before even creating an empty cart.
            super::buyer::validate_buyer_selection(
                &variant_id,
                context.quantity,
                &context,
                locale.as_deref(),
            )
            .map_err(|_| BuyerCartError::Invalid)?;
            create_cart_request(&context, locale)
                .map(BuyerCartPlan::Create)
                .map_err(|_| BuyerCartError::Invalid)
        } else {
            Err(BuyerCartError::Restore)
        }
    }
    pub fn notice(&self, locale: Option<&str>) -> Option<String> {
        let (key, fallback) = match self.phase {
            BuyerCartPhase::Restoring => (
                "cart.buyer.restoring",
                "Restoring the cart before enabling purchases.",
            ),
            BuyerCartPhase::AuthRefused => (
                "cart.buyer.authRefused",
                "The browser session does not match this storefront. Open it with the correct tenant session before changing the cart.",
            ),
            BuyerCartPhase::RestoreFailed => (
                "cart.buyer.restoreFailed",
                "The saved cart could not be verified. Inspect it, or explicitly reset its local reference.",
            ),
            BuyerCartPhase::ContextMismatch => (
                "cart.buyer.contextMismatch",
                "Cart currency, region, channel or locale differs from the quote. Use the cart context and request a new quote. A global cart may not be quotable on a channel-bound storefront.",
            ),
            BuyerCartPhase::Unconfirmed => (
                "cart.buyer.unconfirmed",
                "The operation is unconfirmed and will not be retried automatically. Inspect the cart before adding again. Resetting its reference does not cancel an operation.",
            ),
            BuyerCartPhase::Added => (
                "cart.buyer.added",
                "The server repriced and added the selected SKU.",
            ),
            BuyerCartPhase::Empty | BuyerCartPhase::Ready => return None,
        };
        Some(t(locale, key, fallback))
    }
}

/// Browser-owned recovery hints only. Neither a UUID nor a cleared warning proves owner access.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuyerCartCheckpoint {
    pub cart_id: Option<String>,
    pub mutation_pending: bool,
}

impl BuyerCartCheckpoint {
    pub fn validate(&self) -> Result<(), BuyerCartError> {
        let normalized = parse_optional_uuid_string(self.cart_id.clone(), "cart_id")
            .map_err(|_| BuyerCartError::Restore)?;
        if normalized != self.cart_id {
            return Err(BuyerCartError::Restore);
        }
        Ok(())
    }
}

/// A public tenant and opaque SHA-256 actor fingerprint scope local recovery, never authorization.
/// No bearer token, cart customer data or guest capability is persisted in the checkpoint.
pub fn buyer_cart_storage_key(tenant: &str, token: Option<&str>) -> Result<String, BuyerCartError> {
    use sha2::{Digest, Sha256};
    if tenant.trim().is_empty() || tenant.len() > 255 {
        return Err(BuyerCartError::Restore);
    }
    let digest = |value: &str| {
        Sha256::digest(value.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let actor = token.map(digest).unwrap_or_else(|| "guest".to_string());
    Ok(format!("rustok_cart_buyer:{}:{actor}", digest(tenant)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_commands_and_unverified_references_do_not_create_another_cart() {
        let context = PricingResolutionContext {
            currency_code: "EUR".into(),
            quantity: 1,
            region_id: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
        };
        let sku = uuid::Uuid::new_v4().to_string();
        let mut session = BuyerCartSession::empty();
        assert!(matches!(
            session.plan_add(sku.clone(), context.clone(), Some("en".into())),
            Ok(BuyerCartPlan::Create(_))
        ));
        session.fail(BuyerCartError::Unconfirmed);
        assert!(!session.permits_write());
        assert!(!session.can_inspect());
        assert!(session
            .plan_add(sku.clone(), context.clone(), Some("en".into()))
            .is_err());
        let session = BuyerCartSession::restoring(Some(uuid::Uuid::new_v4().to_string()));
        assert!(session.plan_add(sku, context, Some("en".into())).is_err());
        assert!(BuyerCartSession::restoring(Some(String::new()))
            .restore_id()
            .is_err());
    }
    fn cart(id: &str, status: &str) -> StorefrontCart {
        serde_json::from_value(serde_json::json!({
            "id": id, "status": status, "currency_code": "EUR", "subtotal_amount": "0",
            "adjustment_total": "0", "shipping_total": "0", "total_amount": "0",
            "line_items": [], "adjustments": [], "delivery_groups": []
        }))
        .expect("valid owner projection fixture")
    }
    #[test]
    fn restoration_is_an_exact_active_owner_observation_not_reference_adoption() {
        let id = uuid::Uuid::new_v4().to_string();
        let session = BuyerCartSession::restoring(Some(id.clone()));
        assert_eq!(
            session.finish_restore(None).unwrap().phase,
            BuyerCartPhase::Empty
        );
        assert_eq!(
            session
                .finish_restore(Some(cart(&id, "active")))
                .unwrap()
                .phase,
            BuyerCartPhase::Ready
        );
        for status in ["completed", "cancelled", "draft"] {
            assert!(session.finish_restore(Some(cart(&id, status))).is_err());
            assert_eq!(session.cart_id.as_ref(), Some(&id));
        }
        assert!(session
            .finish_restore(Some(cart(&uuid::Uuid::new_v4().to_string(), "active")))
            .is_err());
        assert!(BuyerCartSession::restoring(Some(String::new()))
            .finish_restore(None)
            .is_err());
    }
    #[test]
    fn checkpoint_scopes_are_opaque_strict_hints_not_credentials_or_authority() {
        let key = buyer_cart_storage_key("shop", Some("private-bearer")).unwrap();
        assert!(!key.contains("private-bearer"));
        assert_ne!(
            key,
            buyer_cart_storage_key("other-shop", Some("private-bearer")).unwrap()
        );
        assert_ne!(
            key,
            buyer_cart_storage_key("shop", Some("rotated-bearer")).unwrap()
        );
        assert_ne!(key, buyer_cart_storage_key("shop", None).unwrap());
        assert!(buyer_cart_storage_key(" ", None).is_err());
        let pending = BuyerCartCheckpoint {
            cart_id: None,
            mutation_pending: true,
        };
        assert!(pending.validate().is_ok());
        assert!(BuyerCartCheckpoint {
            cart_id: Some(uuid::Uuid::nil().to_string()),
            mutation_pending: false
        }
        .validate()
        .is_err());
        assert!(serde_json::from_str::<BuyerCartCheckpoint>(
            r#"{"cartId":null,"mutationPending":true,"token":"secret"}"#
        )
        .is_err());
        let mut refused = BuyerCartSession::empty();
        refused.phase = BuyerCartPhase::AuthRefused;
        assert!(!refused.permits_write());
    }
}

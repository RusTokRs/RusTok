use super::optional_value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShippingProfileSummaryViewModel {
    pub value: String,
}

pub fn shipping_profile_summary_view_model(
    profile: &crate::model::ShippingProfile,
    active_label: &str,
    no_description_label: &str,
) -> ShippingProfileSummaryViewModel {
    ShippingProfileSummaryViewModel {
        value: format!(
            "{} ({}) | {} | {}",
            profile.name,
            profile.slug,
            active_label,
            profile
                .description
                .clone()
                .unwrap_or_else(|| no_description_label.to_string())
        ),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromotionPreviewViewModel {
    pub line_item: String,
}

pub fn promotion_preview_view_model(
    preview: &crate::model::CommerceCartPromotionPreview,
) -> PromotionPreviewViewModel {
    PromotionPreviewViewModel {
        line_item: optional_value(preview.line_item_id.as_deref()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CartAdjustmentViewModel {
    pub source: String,
    pub scope: String,
    pub line_item: String,
    pub amount: String,
}

pub fn cart_adjustment_view_model(
    adjustment: &crate::model::CommerceAdminCartAdjustment,
) -> CartAdjustmentViewModel {
    CartAdjustmentViewModel {
        source: format!(
            "{} / {}",
            adjustment.source_type,
            optional_value(adjustment.source_id.as_deref())
        ),
        scope: optional_value(adjustment.scope.as_deref()),
        line_item: optional_value(adjustment.line_item_id.as_deref()),
        amount: format!("{} {}", adjustment.currency_code, adjustment.amount),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipping_profile_summary_uses_fallback_description_label() {
        let profile = crate::model::ShippingProfile {
            id: "profile-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            slug: "standard".to_string(),
            name: "Standard".to_string(),
            description: None,
            active: true,
            metadata: "{}".to_string(),
            created_at: "2026-06-07T00:00:00Z".to_string(),
            updated_at: "2026-06-07T00:00:00Z".to_string(),
        };

        let summary = shipping_profile_summary_view_model(&profile, "ACTIVE", "no description");

        assert_eq!(
            summary.value,
            "Standard (standard) | ACTIVE | no description"
        );
    }

    #[test]
    fn optional_display_view_models_use_dash_fallback() {
        let preview = crate::model::CommerceCartPromotionPreview {
            kind: crate::model::CommerceCartPromotionKind::FixedDiscount,
            scope: crate::model::CommerceCartPromotionScope::Shipping,
            line_item_id: None,
            currency_code: "USD".to_string(),
            base_amount: "10.00".to_string(),
            adjustment_amount: "-4.99".to_string(),
            adjusted_amount: "5.01".to_string(),
        };
        let adjustment = crate::model::CommerceAdminCartAdjustment {
            id: "adj-1".to_string(),
            line_item_id: None,
            source_type: "promotion".to_string(),
            source_id: None,
            scope: Some("shipping".to_string()),
            amount: "-4.99".to_string(),
            currency_code: "USD".to_string(),
            metadata: "{}".to_string(),
        };

        assert_eq!(promotion_preview_view_model(&preview).line_item, "-");
        let adjustment = cart_adjustment_view_model(&adjustment);
        assert_eq!(adjustment.source, "promotion / -");
        assert_eq!(adjustment.scope, "shipping");
        assert_eq!(adjustment.line_item, "-");
        assert_eq!(adjustment.amount, "USD -4.99");
    }
}

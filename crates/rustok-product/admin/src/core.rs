use crate::i18n::t;
use crate::model::{
    ProductAdminBootstrap, ProductDetail, ProductDraft, ProductListItem, ProductPricingDetail,
    ProductTranslation, ShippingProfile,
};

fn locale_tags_match(left: &str, right: &str) -> bool {
    left.trim()
        .replace('_', "-")
        .eq_ignore_ascii_case(&right.trim().replace('_', "-"))
}

pub(crate) fn translation_for_locale(
    translations: &[ProductTranslation],
    requested_locale: Option<&str>,
) -> Option<ProductTranslation> {
    requested_locale.and_then(|requested_locale| {
        translations
            .iter()
            .find(|translation| locale_tags_match(&translation.locale, requested_locale))
            .cloned()
    })
}

pub(crate) fn primary_catalog_currency(product: Option<&ProductDetail>) -> Option<String> {
    product.and_then(|item| {
        item.variants
            .first()
            .and_then(|variant| variant.prices.first())
            .map(|price| price.currency_code.clone())
    })
}

pub(crate) fn format_catalog_snapshot_price(
    locale: Option<&str>,
    product: Option<&ProductDetail>,
) -> String {
    product
        .and_then(|item| item.variants.first())
        .and_then(|variant| variant.prices.first())
        .map(|price| {
            format_scoped_price(
                locale,
                &price.currency_code,
                &price.amount,
                price.compare_at_amount.as_deref(),
                None,
            )
        })
        .unwrap_or_else(|| t(locale, "product.summary.noPricing", "no pricing"))
}

pub(crate) fn format_pricing_preview(
    locale: Option<&str>,
    pricing: Option<&ProductPricingDetail>,
) -> String {
    let Some(pricing) = pricing else {
        return t(
            locale,
            "product.summary.pricingUnavailable",
            "Pricing module preview is unavailable.",
        );
    };

    let Some(variant) = pricing.variants.first() else {
        return t(locale, "product.summary.noPricing", "no pricing");
    };

    if let Some(price) = variant.effective_price.as_ref() {
        let mut label = format_scoped_price(
            locale,
            &price.currency_code,
            &price.amount,
            price.compare_at_amount.as_deref(),
            price.discount_percent.as_deref(),
        );
        if let Some(scope) = format_pricing_scope(
            locale,
            price.price_list_id.as_deref(),
            price.channel_slug.as_deref(),
            price.channel_id.as_deref(),
        ) {
            label.push_str(format!(" | {scope}").as_str());
        }
        return label;
    }

    variant
        .prices
        .first()
        .map(|price| {
            format_scoped_price(
                locale,
                &price.currency_code,
                &price.amount,
                price.compare_at_amount.as_deref(),
                price.discount_percent.as_deref(),
            )
        })
        .unwrap_or_else(|| t(locale, "product.summary.noPricing", "no pricing"))
}

fn format_scoped_price(
    locale: Option<&str>,
    currency_code: &str,
    amount: &str,
    compare_at_amount: Option<&str>,
    discount_percent: Option<&str>,
) -> String {
    let mut label = if let Some(compare_at_amount) = compare_at_amount {
        format!(
            "{} {} ({})",
            currency_code,
            amount,
            t(locale, "product.summary.compareAt", "compare-at {value}")
                .replace("{value}", compare_at_amount),
        )
    } else {
        format!("{currency_code} {amount}")
    };

    if let Some(discount_percent) = discount_percent.filter(|value| !value.trim().is_empty()) {
        label.push_str(format!(" (-{discount_percent}%)").as_str());
    }

    label
}

fn format_pricing_scope(
    locale: Option<&str>,
    price_list_id: Option<&str>,
    channel_slug: Option<&str>,
    channel_id: Option<&str>,
) -> Option<String> {
    let price_list_id = price_list_id.filter(|value| !value.trim().is_empty());
    let channel_slug = channel_slug.filter(|value| !value.trim().is_empty());
    let channel_id = channel_id.filter(|value| !value.trim().is_empty());

    if price_list_id.is_none() && channel_slug.is_none() && channel_id.is_none() {
        return None;
    }

    let mut parts = Vec::new();
    if let Some(price_list_id) = price_list_id {
        parts.push(t(locale, "product.summary.priceList", "price list") + " " + price_list_id);
    }
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => parts.push(
            t(locale, "product.summary.channel", "channel")
                + " "
                + channel_slug
                + " ("
                + channel_id
                + ")",
        ),
        (Some(channel_slug), None) => {
            parts.push(t(locale, "product.summary.channel", "channel") + " " + channel_slug)
        }
        (None, Some(channel_id)) => {
            parts.push(t(locale, "product.summary.channel", "channel") + " " + channel_id)
        }
        (None, None) => {}
    }

    Some(parts.join(" | "))
}

pub(crate) fn build_admin_pricing_href(module_route_base: &str, product: &ProductDetail) -> String {
    let mut params = vec![format!("id={}", product.id)];
    if let Some(currency_code) =
        primary_catalog_currency(Some(product)).filter(|value| !value.trim().is_empty())
    {
        params.push(format!("currency={currency_code}"));
    }
    params.push("quantity=1".to_string());
    format!("{module_route_base}?{}", params.join("&"))
}

#[derive(Clone, Debug)]
pub(crate) enum ProductAdminPricingPreviewState<'a> {
    Loading,
    Error(&'a str),
    Unavailable,
    Ready(&'a ProductPricingDetail),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelectedProductSummaryViewModel {
    Empty {
        message: String,
    },
    Ready {
        title: String,
        status_line: String,
        catalog_snapshot_label: String,
        pricing_preview_label: String,
        pricing_href: String,
        open_pricing_label: String,
    },
}

pub(crate) fn build_selected_product_summary_view_model(
    locale: Option<&str>,
    product: Option<&ProductDetail>,
    pricing_state: ProductAdminPricingPreviewState<'_>,
    pricing_route_base: &str,
) -> SelectedProductSummaryViewModel {
    let Some(product) = product else {
        return SelectedProductSummaryViewModel::Empty {
            message: t(
                locale,
                "product.summary.empty",
                "Open a product to inspect its localized copy, catalog snapshot and pricing module preview.",
            ),
        };
    };

    let title = translation_for_locale(&product.translations, locale)
        .map(|item| item.title)
        .or_else(|| product.translations.first().map(|item| item.title.clone()))
        .unwrap_or_else(|| t(locale, "product.summary.untitled", "Untitled"));
    let inventory = product
        .variants
        .first()
        .map(|item| item.inventory_quantity)
        .unwrap_or(0);
    let shipping_profile = product
        .shipping_profile_slug
        .clone()
        .unwrap_or_else(|| t(locale, "product.summary.unassigned", "unassigned"));
    let pricing_preview = match pricing_state {
        ProductAdminPricingPreviewState::Loading => t(
            locale,
            "product.summary.pricingLoading",
            "Loading pricing module preview...",
        ),
        ProductAdminPricingPreviewState::Error(err) => format!(
            "{}: {err}",
            t(
                locale,
                "product.summary.pricingError",
                "Pricing module preview failed",
            )
        ),
        ProductAdminPricingPreviewState::Unavailable => t(
            locale,
            "product.summary.pricingUnavailable",
            "Pricing module preview is unavailable.",
        ),
        ProductAdminPricingPreviewState::Ready(pricing) => {
            format_pricing_preview(locale, Some(pricing))
        }
    };

    SelectedProductSummaryViewModel::Ready {
        title,
        status_line: format!(
            "{} {} | {} {inventory} | {} {shipping_profile}",
            t(locale, "product.summary.status", "status"),
            localized_product_status(locale, product.status.as_str()),
            t(locale, "product.summary.inventory", "inventory"),
            t(
                locale,
                "product.summary.shippingProfile",
                "shipping profile",
            ),
        ),
        catalog_snapshot_label: format!(
            "{}: {}",
            t(
                locale,
                "product.summary.catalogSnapshot",
                "catalog snapshot",
            ),
            format_catalog_snapshot_price(locale, Some(product)),
        ),
        pricing_preview_label: format!(
            "{}: {}",
            t(
                locale,
                "product.summary.pricingPreview",
                "pricing module preview",
            ),
            pricing_preview,
        ),
        pricing_href: build_admin_pricing_href(pricing_route_base, product),
        open_pricing_label: t(locale, "product.summary.openPricing", "Open pricing module"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminEditorMode {
    Create,
    Edit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminEditorViewModel {
    pub mode: ProductAdminEditorMode,
    pub title: String,
    pub subtitle: String,
    pub submit_label: String,
}

pub(crate) fn build_product_admin_editor_view_model(
    locale: Option<&str>,
    editing_product_id: Option<&str>,
) -> ProductAdminEditorViewModel {
    let is_editing = editing_product_id
        .map(|id| !id.trim().is_empty())
        .unwrap_or(false);

    if is_editing {
        ProductAdminEditorViewModel {
            mode: ProductAdminEditorMode::Edit,
            title: t(locale, "product.editor.editTitle", "Product Editor"),
            subtitle: t(
                locale,
                "product.editor.subtitle",
                "Single-SKU catalog editor backed by the existing commerce GraphQL contract.",
            ),
            submit_label: t(locale, "product.action.saveProduct", "Save product"),
        }
    } else {
        ProductAdminEditorViewModel {
            mode: ProductAdminEditorMode::Create,
            title: t(locale, "product.editor.createTitle", "Create Product"),
            subtitle: t(
                locale,
                "product.editor.subtitle",
                "Single-SKU catalog editor backed by the existing commerce GraphQL contract.",
            ),
            submit_label: t(locale, "product.action.createProduct", "Create product"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminDraftForm {
    pub locale: Option<String>,
    pub title: String,
    pub handle: String,
    pub description: String,
    pub seller_id: String,
    pub vendor: String,
    pub product_type: String,
    pub shipping_profile_slug: String,
    pub sku: String,
    pub barcode: String,
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: String,
    pub inventory_quantity: i32,
    pub publish_now: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminSaveMode {
    Create,
    Update { product_id: String },
}

#[derive(Clone, Debug)]
pub(crate) struct ProductAdminSaveCommand {
    pub mode: ProductAdminSaveMode,
    pub tenant_id: String,
    pub actor_id: String,
    pub draft: ProductDraft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminSaveValidationError {
    TitleRequired,
    LocaleUnavailable,
    BootstrapUnavailable,
}

impl ProductAdminSaveValidationError {
    pub(crate) fn message(&self, locale: Option<&str>) -> String {
        match self {
            Self::TitleRequired => t(locale, "product.error.titleRequired", "Title is required."),
            Self::LocaleUnavailable => t(
                locale,
                "product.error.localeUnavailable",
                "Host locale is unavailable.",
            ),
            Self::BootstrapUnavailable => t(
                locale,
                "product.error.bootstrapLoading",
                "Bootstrap is still loading.",
            ),
        }
    }
}

pub(crate) fn build_product_admin_save_command(
    form: ProductAdminDraftForm,
    editing_product_id: Option<String>,
    bootstrap: Option<&ProductAdminBootstrap>,
) -> Result<ProductAdminSaveCommand, ProductAdminSaveValidationError> {
    if form.title.trim().is_empty() {
        return Err(ProductAdminSaveValidationError::TitleRequired);
    }

    let locale = form
        .locale
        .filter(|value| !value.trim().is_empty())
        .ok_or(ProductAdminSaveValidationError::LocaleUnavailable)?;

    let bootstrap = bootstrap.ok_or(ProductAdminSaveValidationError::BootstrapUnavailable)?;

    Ok(ProductAdminSaveCommand {
        mode: editing_product_id
            .filter(|id| !id.trim().is_empty())
            .map(|product_id| ProductAdminSaveMode::Update { product_id })
            .unwrap_or(ProductAdminSaveMode::Create),
        tenant_id: bootstrap.current_tenant.id.clone(),
        actor_id: bootstrap.me.id.clone(),
        draft: ProductDraft {
            locale,
            title: form.title,
            handle: form.handle,
            description: form.description,
            seller_id: form.seller_id,
            vendor: form.vendor,
            product_type: form.product_type,
            shipping_profile_slug: text_or_none(form.shipping_profile_slug),
            sku: form.sku,
            barcode: form.barcode,
            currency_code: form.currency_code,
            amount: form.amount,
            compare_at_amount: form.compare_at_amount,
            inventory_quantity: form.inventory_quantity,
            publish_now: form.publish_now,
        },
    })
}

pub(crate) fn format_known_shipping_profiles(
    locale: Option<&str>,
    profiles: &[ShippingProfile],
) -> String {
    let slugs = profiles
        .iter()
        .filter(|profile| profile.active)
        .map(|profile| profile.slug.as_str())
        .collect::<Vec<_>>();
    if slugs.is_empty() {
        t(locale, "product.common.noneYet", "none yet")
    } else {
        slugs.join(", ")
    }
}

pub(crate) fn shipping_profile_choice_label(
    locale: Option<&str>,
    profile: &ShippingProfile,
) -> String {
    if profile.active {
        format!("{} ({})", profile.name, profile.slug)
    } else {
        format!(
            "{} ({}, {})",
            profile.name,
            profile.slug,
            t(locale, "product.common.inactive", "inactive")
        )
    }
}

pub(crate) fn localized_product_status(locale: Option<&str>, status: &str) -> String {
    match status {
        "ACTIVE" => t(locale, "product.status.active", "Active"),
        "ARCHIVED" => t(locale, "product.status.archived", "Archived"),
        _ => t(locale, "product.status.draft", "Draft"),
    }
}

pub(crate) fn format_product_meta(
    locale: Option<&str>,
    handle: &str,
    vendor: Option<&str>,
) -> String {
    let handle_label = t(locale, "product.summary.handle", "handle");
    let vendor_label = t(locale, "product.summary.vendor", "vendor");
    match vendor.filter(|value| !value.is_empty()) {
        Some(vendor) => format!("{handle_label}: {handle} | {vendor_label}: {vendor}"),
        None => format!("{handle_label}: {handle}"),
    }
}

pub(crate) fn format_product_shipping_profile(locale: Option<&str>, slug: &str) -> String {
    t(locale, "product.summary.profileChip", "profile {slug}").replace("{slug}", slug)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminListItemViewModel {
    pub id: String,
    pub status: String,
    pub status_label: String,
    pub status_badge_class: &'static str,
    pub type_label: String,
    pub title: String,
    pub meta_label: String,
    pub shipping_profile_label: Option<String>,
    pub timestamp_label: String,
}

pub(crate) fn build_product_admin_list_item_view_model(
    locale: Option<&str>,
    product: &ProductListItem,
) -> ProductAdminListItemViewModel {
    ProductAdminListItemViewModel {
        id: product.id.clone(),
        status: product.status.clone(),
        status_label: localized_product_status(locale, product.status.as_str()),
        status_badge_class: status_badge(product.status.as_str()),
        type_label: product
            .product_type
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| t(locale, "product.common.general", "general")),
        title: product.title.clone(),
        meta_label: format_product_meta(locale, product.handle.as_str(), product.vendor.as_deref()),
        shipping_profile_label: product
            .shipping_profile_slug
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|slug| format_product_shipping_profile(locale, slug)),
        timestamp_label: product
            .published_at
            .clone()
            .unwrap_or_else(|| product.created_at.clone()),
    }
}

pub(crate) fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn status_badge(status: &str) -> &'static str {
    match status {
        "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CurrentTenant, CurrentUser};

    fn admin_bootstrap() -> ProductAdminBootstrap {
        ProductAdminBootstrap {
            current_tenant: CurrentTenant {
                id: "tenant-1".to_string(),
                slug: "default".to_string(),
                name: "Default".to_string(),
            },
            me: CurrentUser {
                id: "user-1".to_string(),
                email: "operator@example.test".to_string(),
                name: None,
            },
        }
    }

    fn draft_form() -> ProductAdminDraftForm {
        ProductAdminDraftForm {
            locale: Some("en".to_string()),
            title: "Winter coat".to_string(),
            handle: "winter-coat".to_string(),
            description: "Warm coat".to_string(),
            seller_id: "seller-1".to_string(),
            vendor: "Acme".to_string(),
            product_type: "coat".to_string(),
            shipping_profile_slug: " standard ".to_string(),
            sku: "COAT-1".to_string(),
            barcode: "123".to_string(),
            currency_code: "USD".to_string(),
            amount: "10.00".to_string(),
            compare_at_amount: String::new(),
            inventory_quantity: 7,
            publish_now: true,
        }
    }

    #[test]
    fn product_admin_save_command_prepares_create_draft_in_core() {
        let command =
            build_product_admin_save_command(draft_form(), None, Some(&admin_bootstrap()))
                .expect("save command");

        assert!(matches!(command.mode, ProductAdminSaveMode::Create));
        assert_eq!(command.tenant_id, "tenant-1");
        assert_eq!(command.actor_id, "user-1");
        assert_eq!(command.draft.locale, "en");
        assert_eq!(command.draft.title, "Winter coat");
        assert_eq!(
            command.draft.shipping_profile_slug,
            Some("standard".to_string())
        );
        assert!(command.draft.publish_now);
    }

    #[test]
    fn product_admin_save_command_prepares_update_mode_and_validation() {
        let command = build_product_admin_save_command(
            draft_form(),
            Some("product-1".to_string()),
            Some(&admin_bootstrap()),
        )
        .expect("save command");

        assert!(matches!(
            command.mode,
            ProductAdminSaveMode::Update { ref product_id } if product_id == "product-1"
        ));

        let mut missing_title = draft_form();
        missing_title.title = "  ".to_string();
        assert_eq!(
            build_product_admin_save_command(missing_title, None, Some(&admin_bootstrap()))
                .unwrap_err(),
            ProductAdminSaveValidationError::TitleRequired
        );

        let mut missing_locale = draft_form();
        missing_locale.locale = None;
        assert_eq!(
            build_product_admin_save_command(missing_locale, None, Some(&admin_bootstrap()))
                .unwrap_err(),
            ProductAdminSaveValidationError::LocaleUnavailable
        );

        assert_eq!(
            build_product_admin_save_command(draft_form(), None, None).unwrap_err(),
            ProductAdminSaveValidationError::BootstrapUnavailable
        );
    }

    #[test]
    fn product_admin_editor_view_model_tracks_create_and_edit_modes() {
        let create = build_product_admin_editor_view_model(Some("en"), None);
        assert_eq!(create.mode, ProductAdminEditorMode::Create);
        assert_eq!(create.title, "Create Product");
        assert_eq!(create.submit_label, "Create product");

        let edit = build_product_admin_editor_view_model(Some("en"), Some("product-1"));
        assert_eq!(edit.mode, ProductAdminEditorMode::Edit);
        assert_eq!(edit.title, "Product Editor");
        assert_eq!(edit.submit_label, "Save product");
        assert_eq!(
            edit.subtitle,
            "Single-SKU catalog editor backed by the existing commerce GraphQL contract."
        );
    }

    #[test]
    fn product_admin_list_item_view_model_formats_render_state() {
        let product = ProductListItem {
            id: "product-1".to_string(),
            status: "ACTIVE".to_string(),
            title: "Winter coat".to_string(),
            handle: "winter-coat".to_string(),
            seller_id: None,
            vendor: Some("Acme".to_string()),
            product_type: None,
            shipping_profile_slug: Some("standard".to_string()),
            tags: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            published_at: Some("2026-01-02T00:00:00Z".to_string()),
        };

        let view_model = build_product_admin_list_item_view_model(Some("en"), &product);

        assert_eq!(view_model.status_label, "Active");
        assert_eq!(view_model.type_label, "general");
        assert_eq!(view_model.meta_label, "handle: winter-coat | vendor: Acme");
        assert_eq!(
            view_model.shipping_profile_label,
            Some("profile standard".to_string())
        );
        assert_eq!(view_model.timestamp_label, "2026-01-02T00:00:00Z");
        assert!(view_model.status_badge_class.contains("emerald"));
    }

    #[test]
    fn text_or_none_trims_empty_admin_filters() {
        assert_eq!(text_or_none("  ".to_string()), None);
        assert_eq!(
            text_or_none(" active ".to_string()),
            Some("active".to_string())
        );
    }

    #[test]
    fn admin_status_labels_and_badges_are_framework_agnostic() {
        assert_eq!(localized_product_status(Some("en"), "ACTIVE"), "Active");
        assert!(status_badge("ARCHIVED").contains("slate"));
        assert!(status_badge("DRAFT").contains("amber"));
    }

    #[test]
    fn product_meta_and_profile_chip_are_stable() {
        assert_eq!(
            format_product_meta(Some("en"), "winter-coat", Some("Acme")),
            "handle: winter-coat | vendor: Acme",
        );
        assert_eq!(
            format_product_shipping_profile(Some("en"), "standard"),
            "profile standard",
        );
    }

    #[test]
    fn selected_summary_view_model_handles_empty_state() {
        assert_eq!(
            build_selected_product_summary_view_model(
                Some("en"),
                None,
                ProductAdminPricingPreviewState::Loading,
                "/admin/pricing",
            ),
            SelectedProductSummaryViewModel::Empty {
                message: "Open a product to inspect its localized copy, catalog snapshot and pricing module preview."
                    .to_string(),
            },
        );
    }

    #[test]
    fn selected_summary_view_model_formats_ready_product() {
        let product = ProductDetail {
            id: "product-1".to_string(),
            status: "ACTIVE".to_string(),
            seller_id: None,
            vendor: Some("Acme".to_string()),
            product_type: Some("coat".to_string()),
            shipping_profile_slug: Some("standard".to_string()),
            tags: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            published_at: Some("2026-01-01T00:00:00Z".to_string()),
            translations: vec![ProductTranslation {
                locale: "en".to_string(),
                title: "Winter coat".to_string(),
                handle: "winter-coat".to_string(),
                description: None,
                meta_title: None,
                meta_description: None,
            }],
            options: Vec::new(),
            variants: vec![crate::model::ProductVariant {
                id: "variant-1".to_string(),
                sku: None,
                barcode: None,
                shipping_profile_slug: None,
                title: "Default".to_string(),
                option1: None,
                option2: None,
                option3: None,
                prices: vec![crate::model::ProductPrice {
                    currency_code: "USD".to_string(),
                    amount: "10.00".to_string(),
                    compare_at_amount: None,
                    on_sale: false,
                }],
                inventory_quantity: 7,
                inventory_policy: "DENY".to_string(),
                in_stock: true,
            }],
        };

        match build_selected_product_summary_view_model(
            Some("en"),
            Some(&product),
            ProductAdminPricingPreviewState::Unavailable,
            "/admin/pricing",
        ) {
            SelectedProductSummaryViewModel::Ready {
                title,
                status_line,
                catalog_snapshot_label,
                pricing_preview_label,
                pricing_href,
                open_pricing_label,
            } => {
                assert_eq!(title, "Winter coat");
                assert_eq!(
                    status_line,
                    "status Active | inventory 7 | shipping profile standard"
                );
                assert_eq!(catalog_snapshot_label, "catalog snapshot: USD 10.00");
                assert_eq!(
                    pricing_preview_label,
                    "pricing module preview: Pricing module preview is unavailable.",
                );
                assert_eq!(
                    pricing_href,
                    "/admin/pricing?id=product-1&currency=USD&quantity=1"
                );
                assert_eq!(open_pricing_label, "Open pricing module");
            }
            SelectedProductSummaryViewModel::Empty { .. } => panic!("expected ready summary"),
        }
    }
}

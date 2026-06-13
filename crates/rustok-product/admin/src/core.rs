use rustok_api::AdminQueryKey;

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

pub(crate) fn product_admin_pricing_preview_state_from_result<'a>(
    pricing_state: Option<&'a Result<Option<ProductPricingDetail>, String>>,
) -> ProductAdminPricingPreviewState<'a> {
    match pricing_state {
        None => ProductAdminPricingPreviewState::Loading,
        Some(Err(err)) => ProductAdminPricingPreviewState::Error(err.as_str()),
        Some(Ok(None)) => ProductAdminPricingPreviewState::Unavailable,
        Some(Ok(Some(pricing))) => ProductAdminPricingPreviewState::Ready(pricing),
    }
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
pub(crate) struct ProductAdminErrorCopy {
    pub bootstrap_loading: String,
    pub load_product: String,
    pub product_not_found: String,
    pub save_product: String,
    pub change_status: String,
}

pub(crate) fn build_product_admin_error_copy(locale: Option<&str>) -> ProductAdminErrorCopy {
    ProductAdminErrorCopy {
        bootstrap_loading: t(
            locale,
            "product.error.bootstrapLoading",
            "Bootstrap is still loading.",
        ),
        load_product: t(
            locale,
            "product.error.loadProduct",
            "Failed to load product",
        ),
        product_not_found: t(
            locale,
            "product.error.productNotFound",
            "Product not found.",
        ),
        save_product: t(
            locale,
            "product.error.saveProduct",
            "Failed to save product",
        ),
        change_status: t(
            locale,
            "product.error.changeStatus",
            "Failed to change status",
        ),
    }
}

impl ProductAdminErrorCopy {
    pub(crate) fn load_product_failure(&self, detail: impl std::fmt::Display) -> String {
        product_admin_error_with_detail(&self.load_product, detail)
    }

    pub(crate) fn save_product_failure(&self, detail: impl std::fmt::Display) -> String {
        product_admin_error_with_detail(&self.save_product, detail)
    }

    pub(crate) fn change_status_failure(&self, detail: impl std::fmt::Display) -> String {
        product_admin_error_with_detail(&self.change_status, detail)
    }
}

fn product_admin_error_with_detail(base: &str, detail: impl std::fmt::Display) -> String {
    format!("{base}: {detail}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminEditorCopy {
    pub new_action_label: String,
    pub handle_placeholder: String,
    pub title_placeholder: String,
    pub description_placeholder: String,
    pub seller_id_placeholder: String,
    pub vendor_placeholder: String,
    pub product_type_placeholder: String,
    pub primary_sku_placeholder: String,
    pub barcode_placeholder: String,
    pub currency_placeholder: String,
    pub price_placeholder: String,
    pub compare_at_price_placeholder: String,
    pub no_shipping_profile_label: String,
    pub inventory_quantity_placeholder: String,
    pub keep_published_label: String,
}

pub(crate) fn build_product_admin_editor_copy(locale: Option<&str>) -> ProductAdminEditorCopy {
    ProductAdminEditorCopy {
        new_action_label: t(locale, "product.action.new", "New"),
        handle_placeholder: t(locale, "product.field.handle", "Handle"),
        title_placeholder: t(locale, "product.field.title", "Title"),
        description_placeholder: t(locale, "product.field.description", "Description"),
        seller_id_placeholder: t(locale, "product.field.sellerId", "Seller ID"),
        vendor_placeholder: t(locale, "product.field.vendor", "Vendor"),
        product_type_placeholder: t(locale, "product.field.productType", "Product type"),
        primary_sku_placeholder: t(locale, "product.field.primarySku", "Primary SKU"),
        barcode_placeholder: t(locale, "product.field.barcode", "Barcode"),
        currency_placeholder: t(locale, "product.field.currency", "Currency"),
        price_placeholder: t(locale, "product.field.price", "Price"),
        compare_at_price_placeholder: t(locale, "product.field.compareAtPrice", "Compare-at price"),
        no_shipping_profile_label: t(
            locale,
            "product.field.noShippingProfile",
            "No shipping profile",
        ),
        inventory_quantity_placeholder: t(
            locale,
            "product.field.inventoryQuantity",
            "Inventory quantity",
        ),
        keep_published_label: t(
            locale,
            "product.field.keepPublished",
            "Keep published after save",
        ),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminEditorFormState {
    pub editing_id: Option<String>,
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

pub(crate) fn empty_product_admin_editor_form_state() -> ProductAdminEditorFormState {
    ProductAdminEditorFormState {
        editing_id: None,
        title: String::new(),
        handle: String::new(),
        description: String::new(),
        seller_id: String::new(),
        vendor: String::new(),
        product_type: String::new(),
        shipping_profile_slug: String::new(),
        sku: String::new(),
        barcode: String::new(),
        currency_code: "USD".to_string(),
        amount: "0.00".to_string(),
        compare_at_amount: String::new(),
        inventory_quantity: 0,
        publish_now: false,
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ProductAdminOpenProductViewModel {
    Ready {
        product: ProductDetail,
        form_state: ProductAdminEditorFormState,
    },
    Empty {
        form_state: ProductAdminEditorFormState,
        error_message: String,
    },
}

pub(crate) fn build_product_admin_open_product_view_model<E: std::fmt::Display>(
    requested_locale: Option<&str>,
    error_copy: &ProductAdminErrorCopy,
    result: Result<Option<ProductDetail>, E>,
) -> ProductAdminOpenProductViewModel {
    match result {
        Ok(Some(product)) => ProductAdminOpenProductViewModel::Ready {
            form_state: build_product_admin_editor_form_state(&product, requested_locale),
            product,
        },
        Ok(None) => ProductAdminOpenProductViewModel::Empty {
            form_state: empty_product_admin_editor_form_state(),
            error_message: error_copy.product_not_found.clone(),
        },
        Err(err) => ProductAdminOpenProductViewModel::Empty {
            form_state: empty_product_admin_editor_form_state(),
            error_message: error_copy.load_product_failure(err),
        },
    }
}

pub(crate) fn build_product_admin_editor_form_state(
    product: &ProductDetail,
    requested_locale: Option<&str>,
) -> ProductAdminEditorFormState {
    let translation = translation_for_locale(&product.translations, requested_locale);
    let variant = product.variants.first().cloned();
    let price = variant
        .as_ref()
        .and_then(|item| item.prices.first().cloned());

    ProductAdminEditorFormState {
        editing_id: Some(product.id.clone()),
        title: translation
            .as_ref()
            .map(|item| item.title.clone())
            .unwrap_or_default(),
        handle: translation
            .as_ref()
            .map(|item| item.handle.clone())
            .unwrap_or_default(),
        description: translation
            .and_then(|item| item.description)
            .unwrap_or_default(),
        seller_id: product.seller_id.clone().unwrap_or_default(),
        vendor: product.vendor.clone().unwrap_or_default(),
        product_type: product.product_type.clone().unwrap_or_default(),
        shipping_profile_slug: product.shipping_profile_slug.clone().unwrap_or_default(),
        sku: variant
            .as_ref()
            .and_then(|item| item.sku.clone())
            .unwrap_or_default(),
        barcode: variant.and_then(|item| item.barcode).unwrap_or_default(),
        currency_code: price
            .as_ref()
            .map(|item| item.currency_code.clone())
            .unwrap_or_else(|| "USD".to_string()),
        amount: price
            .as_ref()
            .map(|item| item.amount.clone())
            .unwrap_or_else(|| "0.00".to_string()),
        compare_at_amount: price
            .and_then(|item| item.compare_at_amount)
            .unwrap_or_default(),
        inventory_quantity: product
            .variants
            .first()
            .map(|item| item.inventory_quantity)
            .unwrap_or(0),
        publish_now: product.status == "ACTIVE",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminStatusTarget {
    Active,
    Draft,
    Archived,
}

impl ProductAdminStatusTarget {
    pub(crate) fn as_graphql_status(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Draft => "DRAFT",
            Self::Archived => "ARCHIVED",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminStatusMutationCommand {
    pub tenant_id: String,
    pub actor_id: String,
    pub product_id: String,
    pub status: ProductAdminStatusTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminStatusMutationValidationError {
    BootstrapUnavailable,
}

impl ProductAdminStatusMutationValidationError {
    pub(crate) fn message(&self, locale: Option<&str>) -> String {
        match self {
            Self::BootstrapUnavailable => t(
                locale,
                "product.error.bootstrapLoading",
                "Bootstrap is still loading.",
            ),
        }
    }
}

pub(crate) fn build_product_admin_status_mutation_command(
    bootstrap: Option<&ProductAdminBootstrap>,
    product_id: String,
    status: ProductAdminStatusTarget,
) -> Result<ProductAdminStatusMutationCommand, ProductAdminStatusMutationValidationError> {
    let bootstrap =
        bootstrap.ok_or(ProductAdminStatusMutationValidationError::BootstrapUnavailable)?;

    Ok(ProductAdminStatusMutationCommand {
        tenant_id: bootstrap.current_tenant.id.clone(),
        actor_id: bootstrap.me.id.clone(),
        product_id,
        status,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminStatusMutationOutcome {
    Changed,
    TransportError(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminStatusMutationResultViewModel {
    pub refresh: bool,
    pub error_message: Option<String>,
}

pub(crate) fn build_product_admin_status_mutation_result_view_model(
    locale: Option<&str>,
    outcome: ProductAdminStatusMutationOutcome,
) -> ProductAdminStatusMutationResultViewModel {
    match outcome {
        ProductAdminStatusMutationOutcome::Changed => ProductAdminStatusMutationResultViewModel {
            refresh: true,
            error_message: None,
        },
        ProductAdminStatusMutationOutcome::TransportError(err) => {
            let error_copy = build_product_admin_error_copy(locale);
            ProductAdminStatusMutationResultViewModel {
                refresh: false,
                error_message: Some(error_copy.change_status_failure(err)),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminDeleteCommand {
    pub tenant_id: String,
    pub actor_id: String,
    pub product_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminDeleteValidationError {
    BootstrapUnavailable,
}

impl ProductAdminDeleteValidationError {
    pub(crate) fn message(&self, locale: Option<&str>) -> String {
        match self {
            Self::BootstrapUnavailable => t(
                locale,
                "product.error.bootstrapLoading",
                "Bootstrap is still loading.",
            ),
        }
    }
}

pub(crate) fn build_product_admin_delete_command(
    bootstrap: Option<&ProductAdminBootstrap>,
    product_id: String,
) -> Result<ProductAdminDeleteCommand, ProductAdminDeleteValidationError> {
    let bootstrap = bootstrap.ok_or(ProductAdminDeleteValidationError::BootstrapUnavailable)?;

    Ok(ProductAdminDeleteCommand {
        tenant_id: bootstrap.current_tenant.id.clone(),
        actor_id: bootstrap.me.id.clone(),
        product_id,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminDeleteOutcome {
    Deleted,
    NotDeleted,
    TransportError(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminDeleteResultViewModel {
    pub clear_selection: bool,
    pub refresh: bool,
    pub error_message: Option<String>,
}

pub(crate) fn build_product_admin_delete_result_view_model(
    locale: Option<&str>,
    deleted_product_id: &str,
    open_product_id: Option<&str>,
    outcome: ProductAdminDeleteOutcome,
) -> ProductAdminDeleteResultViewModel {
    match outcome {
        ProductAdminDeleteOutcome::Deleted => ProductAdminDeleteResultViewModel {
            clear_selection: open_product_id == Some(deleted_product_id),
            refresh: true,
            error_message: None,
        },
        ProductAdminDeleteOutcome::NotDeleted => ProductAdminDeleteResultViewModel {
            clear_selection: false,
            refresh: false,
            error_message: Some(t(
                locale,
                "product.error.deleteReturnedFalse",
                "Delete returned false.",
            )),
        },
        ProductAdminDeleteOutcome::TransportError(err) => ProductAdminDeleteResultViewModel {
            clear_selection: false,
            refresh: false,
            error_message: Some(format!(
                "{}: {err}",
                t(
                    locale,
                    "product.error.deleteProduct",
                    "Failed to delete product",
                )
            )),
        },
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminShellViewModel {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
}

pub(crate) fn build_product_admin_shell_view_model(
    locale: Option<&str>,
) -> ProductAdminShellViewModel {
    ProductAdminShellViewModel {
        badge: t(locale, "product.badge", "product"),
        title: t(locale, "product.title", "Product Catalog"),
        subtitle: t(
            locale,
            "product.subtitle",
            "Product ownership now lives in the product module package. Commerce keeps delivery orchestration while catalog CRUD moves to the product route.",
        ),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminSeoPanelCopy {
    pub title: String,
    pub subtitle: String,
    pub empty_message: String,
}

pub(crate) fn build_product_admin_seo_panel_copy(locale: Option<&str>) -> ProductAdminSeoPanelCopy {
    ProductAdminSeoPanelCopy {
        title: t(locale, "product.seo.title", "Product SEO"),
        subtitle: t(
            locale,
            "product.seo.subtitle",
            "Explicit metadata, social tags and diagnostics for the selected product.",
        ),
        empty_message: t(
            locale,
            "product.seo.empty",
            "Create or open a product first. The SEO panel stays attached to the product editor.",
        ),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminProfilePanelViewModel {
    Loading { message: String },
    Error { message: String },
    Ready { message: String },
}

impl ProductAdminProfilePanelViewModel {
    pub(crate) fn into_message(self) -> String {
        match self {
            Self::Loading { message } | Self::Error { message } | Self::Ready { message } => {
                message
            }
        }
    }
}

pub(crate) fn build_product_admin_profile_panel_loading_view_model(
    locale: Option<&str>,
) -> ProductAdminProfilePanelViewModel {
    ProductAdminProfilePanelViewModel::Loading {
        message: t(
            locale,
            "product.profile.loading",
            "Shipping profiles are loading from the registry.",
        ),
    }
}

pub(crate) fn build_product_admin_profile_panel_error_view_model(
    locale: Option<&str>,
    error: impl std::fmt::Display,
) -> ProductAdminProfilePanelViewModel {
    ProductAdminProfilePanelViewModel::Error {
        message: format!(
            "{}: {error}",
            t(
                locale,
                "product.profile.error",
                "Failed to load shipping profiles"
            )
        ),
    }
}

pub(crate) fn build_product_admin_profile_panel_ready_view_model(
    locale: Option<&str>,
    profiles: &[ShippingProfile],
) -> ProductAdminProfilePanelViewModel {
    ProductAdminProfilePanelViewModel::Ready {
        message: t(
            locale,
            "product.profile.known",
            "Known profiles: {profiles}",
        )
        .replace(
            "{profiles}",
            format_known_shipping_profiles(locale, profiles).as_str(),
        ),
    }
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
pub(crate) struct ProductAdminListActionLabels {
    pub edit: String,
    pub publish: String,
    pub move_to_draft: String,
    pub archive: String,
    pub delete: String,
}

pub(crate) fn build_product_admin_list_action_labels(
    locale: Option<&str>,
) -> ProductAdminListActionLabels {
    ProductAdminListActionLabels {
        edit: t(locale, "product.action.edit", "Edit"),
        publish: t(locale, "product.action.publish", "Publish"),
        move_to_draft: t(locale, "product.action.moveToDraft", "Move to Draft"),
        archive: t(locale, "product.action.archive", "Archive"),
        delete: t(locale, "product.action.delete", "Delete"),
    }
}

pub(crate) fn product_admin_list_actions_disabled(is_busy: bool) -> bool {
    is_busy
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminRouteQueryIntent {
    Push { key: &'static str, value: String },
    Replace { key: &'static str, value: String },
    Clear { key: &'static str },
}

pub(crate) fn product_admin_open_product_query_intent(
    product_id: String,
) -> ProductAdminRouteQueryIntent {
    ProductAdminRouteQueryIntent::Push {
        key: AdminQueryKey::ProductId.as_str(),
        value: product_id,
    }
}

pub(crate) fn product_admin_saved_product_query_intent(
    product_id: String,
) -> ProductAdminRouteQueryIntent {
    ProductAdminRouteQueryIntent::Replace {
        key: AdminQueryKey::ProductId.as_str(),
        value: product_id,
    }
}

pub(crate) fn product_admin_clear_product_query_intent() -> ProductAdminRouteQueryIntent {
    ProductAdminRouteQueryIntent::Clear {
        key: AdminQueryKey::ProductId.as_str(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProductAdminListStateKind {
    Loading,
    Empty,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminListStateViewModel {
    pub kind: ProductAdminListStateKind,
    pub message: String,
}

pub(crate) fn build_product_admin_list_loading_view_model(
    locale: Option<&str>,
) -> ProductAdminListStateViewModel {
    ProductAdminListStateViewModel {
        kind: ProductAdminListStateKind::Loading,
        message: t(locale, "product.list.loading", "Loading products..."),
    }
}

pub(crate) fn build_product_admin_list_empty_view_model(
    locale: Option<&str>,
) -> ProductAdminListStateViewModel {
    ProductAdminListStateViewModel {
        kind: ProductAdminListStateKind::Empty,
        message: t(locale, "product.list.empty", "No products yet."),
    }
}

pub(crate) fn build_product_admin_list_error_view_model(
    locale: Option<&str>,
    error: impl std::fmt::Display,
) -> ProductAdminListStateViewModel {
    ProductAdminListStateViewModel {
        kind: ProductAdminListStateKind::Error,
        message: format!(
            "{}: {error}",
            t(
                locale,
                "product.error.loadProducts",
                "Failed to load products"
            )
        ),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminStatusFilterOption {
    pub value: &'static str,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductAdminListControlsViewModel {
    pub title: String,
    pub subtitle: String,
    pub search_placeholder: String,
    pub status_options: Vec<ProductAdminStatusFilterOption>,
}

pub(crate) fn build_product_admin_list_controls_view_model(
    locale: Option<&str>,
) -> ProductAdminListControlsViewModel {
    ProductAdminListControlsViewModel {
        title: t(locale, "product.list.title", "Catalog Feed"),
        subtitle: t(
            locale,
            "product.list.subtitle",
            "Search, open, publish and archive products from the product-owned package.",
        ),
        search_placeholder: t(locale, "product.list.search", "Search title"),
        status_options: vec![
            ProductAdminStatusFilterOption {
                value: "",
                label: t(locale, "product.status.all", "All statuses"),
            },
            ProductAdminStatusFilterOption {
                value: "DRAFT",
                label: t(locale, "product.status.draft", "Draft"),
            },
            ProductAdminStatusFilterOption {
                value: "ACTIVE",
                label: t(locale, "product.status.active", "Active"),
            },
            ProductAdminStatusFilterOption {
                value: "ARCHIVED",
                label: t(locale, "product.status.archived", "Archived"),
            },
        ],
    }
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

pub(crate) fn parse_product_admin_inventory_quantity_input(value: &str) -> i32 {
    value.trim().parse().unwrap_or(0)
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
    fn product_admin_route_query_intents_keep_product_selection_policy_in_core() {
        assert_eq!(
            product_admin_open_product_query_intent("product-1".to_string()),
            ProductAdminRouteQueryIntent::Push {
                key: AdminQueryKey::ProductId.as_str(),
                value: "product-1".to_string(),
            }
        );

        assert_eq!(
            product_admin_saved_product_query_intent("product-2".to_string()),
            ProductAdminRouteQueryIntent::Replace {
                key: AdminQueryKey::ProductId.as_str(),
                value: "product-2".to_string(),
            }
        );

        assert_eq!(
            product_admin_clear_product_query_intent(),
            ProductAdminRouteQueryIntent::Clear {
                key: AdminQueryKey::ProductId.as_str(),
            }
        );
    }

    #[test]
    fn product_admin_list_state_view_models_keep_copy_in_core() {
        let loading = build_product_admin_list_loading_view_model(Some("en"));
        assert_eq!(loading.kind, ProductAdminListStateKind::Loading);
        assert_eq!(loading.message, "Loading products...");

        let empty = build_product_admin_list_empty_view_model(Some("en"));
        assert_eq!(empty.kind, ProductAdminListStateKind::Empty);
        assert_eq!(empty.message, "No products yet.");

        let error = build_product_admin_list_error_view_model(Some("en"), "network");
        assert_eq!(error.kind, ProductAdminListStateKind::Error);
        assert_eq!(error.message, "Failed to load products: network");
    }

    #[test]
    fn product_admin_list_controls_view_model_keeps_filter_copy_in_core() {
        let controls = build_product_admin_list_controls_view_model(Some("en"));

        assert_eq!(controls.title, "Catalog Feed");
        assert_eq!(controls.search_placeholder, "Search title");
        assert_eq!(controls.status_options.len(), 4);
        assert_eq!(controls.status_options[0].value, "");
        assert_eq!(controls.status_options[0].label, "All statuses");
        assert_eq!(controls.status_options[2].value, "ACTIVE");
        assert_eq!(controls.status_options[2].label, "Active");
    }

    #[test]
    fn product_admin_delete_result_view_model_tracks_success_and_open_selection() {
        let open = build_product_admin_delete_result_view_model(
            Some("en"),
            "product-1",
            Some("product-1"),
            ProductAdminDeleteOutcome::Deleted,
        );

        assert!(open.clear_selection);
        assert!(open.refresh);
        assert_eq!(open.error_message, None);

        let other = build_product_admin_delete_result_view_model(
            Some("en"),
            "product-1",
            Some("product-2"),
            ProductAdminDeleteOutcome::Deleted,
        );
        assert!(!other.clear_selection);
        assert!(other.refresh);
    }

    #[test]
    fn product_admin_delete_result_view_model_formats_failures() {
        let not_deleted = build_product_admin_delete_result_view_model(
            Some("en"),
            "product-1",
            Some("product-1"),
            ProductAdminDeleteOutcome::NotDeleted,
        );
        assert_eq!(
            not_deleted.error_message,
            Some("Delete returned false.".to_string())
        );
        assert!(!not_deleted.refresh);
        assert!(!not_deleted.clear_selection);

        let failed = build_product_admin_delete_result_view_model(
            Some("en"),
            "product-1",
            Some("product-1"),
            ProductAdminDeleteOutcome::TransportError("network".to_string()),
        );
        assert_eq!(
            failed.error_message,
            Some("Failed to delete product: network".to_string())
        );
        assert!(!failed.refresh);
    }

    #[test]
    fn product_admin_delete_command_prepares_transport_payload() {
        let command =
            build_product_admin_delete_command(Some(&admin_bootstrap()), "product-1".to_string())
                .expect("delete command");

        assert_eq!(command.tenant_id, "tenant-1");
        assert_eq!(command.actor_id, "user-1");
        assert_eq!(command.product_id, "product-1");
    }

    #[test]
    fn product_admin_delete_command_validates_bootstrap() {
        assert_eq!(
            build_product_admin_delete_command(None, "product-1".to_string()).unwrap_err(),
            ProductAdminDeleteValidationError::BootstrapUnavailable
        );
    }

    #[test]
    fn product_admin_status_mutation_result_view_model_maps_outcomes() {
        assert_eq!(
            build_product_admin_status_mutation_result_view_model(
                Some("en"),
                ProductAdminStatusMutationOutcome::Changed,
            ),
            ProductAdminStatusMutationResultViewModel {
                refresh: true,
                error_message: None,
            },
        );
        assert_eq!(
            build_product_admin_status_mutation_result_view_model(
                Some("en"),
                ProductAdminStatusMutationOutcome::TransportError("network".to_string()),
            ),
            ProductAdminStatusMutationResultViewModel {
                refresh: false,
                error_message: Some("Failed to change status: network".to_string()),
            },
        );
    }

    #[test]
    fn product_admin_status_mutation_command_prepares_transport_payload() {
        let command = build_product_admin_status_mutation_command(
            Some(&admin_bootstrap()),
            "product-1".to_string(),
            ProductAdminStatusTarget::Archived,
        )
        .expect("status command");

        assert_eq!(command.tenant_id, "tenant-1");
        assert_eq!(command.actor_id, "user-1");
        assert_eq!(command.product_id, "product-1");
        assert_eq!(command.status.as_graphql_status(), "ARCHIVED");
    }

    #[test]
    fn product_admin_status_mutation_command_validates_bootstrap() {
        assert_eq!(
            build_product_admin_status_mutation_command(
                None,
                "product-1".to_string(),
                ProductAdminStatusTarget::Draft,
            )
            .unwrap_err(),
            ProductAdminStatusMutationValidationError::BootstrapUnavailable
        );
        assert_eq!(
            ProductAdminStatusTarget::Active.as_graphql_status(),
            "ACTIVE"
        );
        assert_eq!(ProductAdminStatusTarget::Draft.as_graphql_status(), "DRAFT");
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
    fn product_admin_editor_form_state_maps_empty_defaults() {
        let state = empty_product_admin_editor_form_state();

        assert_eq!(state.editing_id, None);
        assert_eq!(state.currency_code, "USD");
        assert_eq!(state.amount, "0.00");
        assert_eq!(state.inventory_quantity, 0);
        assert!(!state.publish_now);
    }

    #[test]
    fn product_admin_editor_form_state_maps_product_detail() {
        let product = ProductDetail {
            id: "product-1".to_string(),
            status: "ACTIVE".to_string(),
            seller_id: Some("seller-1".to_string()),
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
                description: Some("Warm coat".to_string()),
                meta_title: None,
                meta_description: None,
            }],
            options: Vec::new(),
            variants: vec![crate::model::ProductVariant {
                id: "variant-1".to_string(),
                sku: Some("COAT-1".to_string()),
                barcode: Some("123".to_string()),
                shipping_profile_slug: None,
                title: "Default".to_string(),
                option1: None,
                option2: None,
                option3: None,
                prices: vec![crate::model::ProductPrice {
                    currency_code: "EUR".to_string(),
                    amount: "12.00".to_string(),
                    compare_at_amount: Some("15.00".to_string()),
                    on_sale: true,
                }],
                inventory_quantity: 9,
                inventory_policy: "DENY".to_string(),
                in_stock: true,
            }],
        };

        let state = build_product_admin_editor_form_state(&product, Some("en"));

        assert_eq!(state.editing_id, Some("product-1".to_string()));
        assert_eq!(state.title, "Winter coat");
        assert_eq!(state.handle, "winter-coat");
        assert_eq!(state.description, "Warm coat");
        assert_eq!(state.seller_id, "seller-1");
        assert_eq!(state.vendor, "Acme");
        assert_eq!(state.product_type, "coat");
        assert_eq!(state.shipping_profile_slug, "standard");
        assert_eq!(state.sku, "COAT-1");
        assert_eq!(state.barcode, "123");
        assert_eq!(state.currency_code, "EUR");
        assert_eq!(state.amount, "12.00");
        assert_eq!(state.compare_at_amount, "15.00");
        assert_eq!(state.inventory_quantity, 9);
        assert!(state.publish_now);
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
    fn product_admin_error_copy_is_core_owned() {
        let copy = build_product_admin_error_copy(Some("en"));

        assert_eq!(copy.bootstrap_loading, "Bootstrap is still loading.");
        assert_eq!(copy.load_product, "Failed to load product");
        assert_eq!(copy.product_not_found, "Product not found.");
        assert_eq!(copy.save_product, "Failed to save product");
        assert_eq!(copy.change_status, "Failed to change status");
        assert_eq!(
            copy.load_product_failure("network unavailable"),
            "Failed to load product: network unavailable",
        );
        assert_eq!(
            copy.save_product_failure("validation failed"),
            "Failed to save product: validation failed",
        );
        assert_eq!(
            copy.change_status_failure("timeout"),
            "Failed to change status: timeout",
        );
    }

    #[test]
    fn product_admin_editor_copy_is_core_owned() {
        let copy = build_product_admin_editor_copy(Some("en"));

        assert_eq!(copy.new_action_label, "New");
        assert_eq!(copy.handle_placeholder, "Handle");
        assert_eq!(copy.seller_id_placeholder, "Seller ID");
        assert_eq!(copy.compare_at_price_placeholder, "Compare-at price");
        assert_eq!(copy.no_shipping_profile_label, "No shipping profile");
        assert_eq!(copy.keep_published_label, "Keep published after save");
    }

    #[test]
    fn product_admin_list_action_labels_and_availability_are_core_owned() {
        let labels = build_product_admin_list_action_labels(Some("en"));

        assert_eq!(labels.edit, "Edit");
        assert_eq!(labels.publish, "Publish");
        assert_eq!(labels.move_to_draft, "Move to Draft");
        assert_eq!(labels.archive, "Archive");
        assert_eq!(labels.delete, "Delete");
        assert!(product_admin_list_actions_disabled(true));
        assert!(!product_admin_list_actions_disabled(false));
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
    fn product_admin_shell_view_model_is_core_owned() {
        let view_model = build_product_admin_shell_view_model(Some("en"));

        assert_eq!(view_model.badge, "product");
        assert_eq!(view_model.title, "Product Catalog");
        assert_eq!(
            view_model.subtitle,
            "Product ownership now lives in the product module package. Commerce keeps delivery orchestration while catalog CRUD moves to the product route."
        );
    }

    #[test]
    fn product_admin_seo_panel_copy_is_core_owned() {
        let copy = build_product_admin_seo_panel_copy(Some("en"));

        assert_eq!(copy.title, "Product SEO");
        assert_eq!(
            copy.subtitle,
            "Explicit metadata, social tags and diagnostics for the selected product."
        );
        assert_eq!(
            copy.empty_message,
            "Create or open a product first. The SEO panel stays attached to the product editor."
        );
    }

    #[test]
    fn product_admin_profile_panel_view_models_are_core_owned() {
        let active = ShippingProfile {
            id: "profile-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            slug: "standard".to_string(),
            name: "Standard".to_string(),
            description: None,
            active: true,
            metadata: "{}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let inactive = ShippingProfile {
            id: "profile-2".to_string(),
            tenant_id: "tenant-1".to_string(),
            slug: "inactive".to_string(),
            name: "Inactive".to_string(),
            description: None,
            active: false,
            metadata: "{}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };

        assert_eq!(
            build_product_admin_profile_panel_loading_view_model(Some("en")),
            ProductAdminProfilePanelViewModel::Loading {
                message: "Shipping profiles are loading from the registry.".to_string(),
            },
        );
        assert_eq!(
            build_product_admin_profile_panel_error_view_model(Some("en"), "network unavailable"),
            ProductAdminProfilePanelViewModel::Error {
                message: "Failed to load shipping profiles: network unavailable".to_string(),
            },
        );
        assert_eq!(
            build_product_admin_profile_panel_ready_view_model(Some("en"), &[active, inactive]),
            ProductAdminProfilePanelViewModel::Ready {
                message: "Known profiles: standard".to_string(),
            },
        );
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
    fn product_admin_inventory_quantity_input_parsing_is_core_owned() {
        assert_eq!(parse_product_admin_inventory_quantity_input(" 42 "), 42);
        assert_eq!(parse_product_admin_inventory_quantity_input(""), 0);
        assert_eq!(
            parse_product_admin_inventory_quantity_input("not-a-number"),
            0
        );
    }

    #[test]
    fn product_admin_open_product_view_model_handles_missing_and_failed_loads() {
        let error_copy = build_product_admin_error_copy(Some("en"));

        match build_product_admin_open_product_view_model::<&str>(Some("en"), &error_copy, Ok(None))
        {
            ProductAdminOpenProductViewModel::Empty {
                form_state,
                error_message,
            } => {
                assert_eq!(form_state, empty_product_admin_editor_form_state());
                assert_eq!(error_message, "Product not found.");
            }
            ProductAdminOpenProductViewModel::Ready { .. } => panic!("expected empty view-model"),
        }

        match build_product_admin_open_product_view_model(
            Some("en"),
            &error_copy,
            Err("network unavailable"),
        ) {
            ProductAdminOpenProductViewModel::Empty {
                form_state,
                error_message,
            } => {
                assert_eq!(form_state, empty_product_admin_editor_form_state());
                assert_eq!(error_message, "Failed to load product: network unavailable");
            }
            ProductAdminOpenProductViewModel::Ready { .. } => panic!("expected empty view-model"),
        }
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
    fn product_admin_pricing_preview_state_maps_async_resource_results() {
        assert!(matches!(
            product_admin_pricing_preview_state_from_result(None),
            ProductAdminPricingPreviewState::Loading
        ));

        let unavailable = Ok(None);
        assert!(matches!(
            product_admin_pricing_preview_state_from_result(Some(&unavailable)),
            ProductAdminPricingPreviewState::Unavailable
        ));

        let failed = Err("pricing timeout".to_string());
        assert!(matches!(
            product_admin_pricing_preview_state_from_result(Some(&failed)),
            ProductAdminPricingPreviewState::Error("pricing timeout")
        ));
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

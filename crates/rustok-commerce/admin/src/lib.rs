mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    ProductDetail, ProductDraft, ProductListItem, ShippingOption, ShippingOptionDraft,
    ShippingProfile, ShippingProfileDraft,
};

#[component]
pub fn CommerceAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let initial_product_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);

    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<ProductDetail>::None);
    let (locale, set_locale) = signal(ui_locale.clone().unwrap_or_else(|| "en".to_string()));
    let (title, set_title) = signal(String::new());
    let (handle, set_handle) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (vendor, set_vendor) = signal(String::new());
    let (product_type, set_product_type) = signal(String::new());
    let (product_shipping_profile_slug, set_product_shipping_profile_slug) = signal(String::new());
    let (sku, set_sku) = signal(String::new());
    let (currency_code, set_currency_code) = signal("USD".to_string());
    let (amount, set_amount) = signal("0.00".to_string());
    let (inventory_quantity, set_inventory_quantity) = signal(0_i32);
    let (publish_now, set_publish_now) = signal(false);
    let (search, set_search) = signal(String::new());
    let (status_filter, set_status_filter) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let (shipping_editing_id, set_shipping_editing_id) = signal(Option::<String>::None);
    let (selected_shipping_option, set_selected_shipping_option) =
        signal(Option::<ShippingOption>::None);
    let (shipping_name, set_shipping_name) = signal(String::new());
    let (shipping_currency_code, set_shipping_currency_code) = signal("USD".to_string());
    let (shipping_amount, set_shipping_amount) = signal("0.00".to_string());
    let (shipping_provider_id, set_shipping_provider_id) = signal("manual".to_string());
    let (shipping_allowed_profiles, set_shipping_allowed_profiles) = signal(String::new());
    let (shipping_metadata_json, set_shipping_metadata_json) = signal(String::new());
    let (shipping_search, set_shipping_search) = signal(String::new());
    let (shipping_currency_filter, set_shipping_currency_filter) = signal(String::new());
    let (shipping_provider_filter, set_shipping_provider_filter) = signal(String::new());
    let (shipping_busy, set_shipping_busy) = signal(false);
    let (shipping_error, set_shipping_error) = signal(Option::<String>::None);

    let (shipping_profile_editing_id, set_shipping_profile_editing_id) =
        signal(Option::<String>::None);
    let (selected_shipping_profile, set_selected_shipping_profile) =
        signal(Option::<ShippingProfile>::None);
    let (shipping_profile_slug, set_shipping_profile_slug) = signal(String::new());
    let (shipping_profile_name, set_shipping_profile_name) = signal(String::new());
    let (shipping_profile_description, set_shipping_profile_description) = signal(String::new());
    let (shipping_profile_metadata_json, set_shipping_profile_metadata_json) =
        signal(String::new());
    let (shipping_profile_search, set_shipping_profile_search) = signal(String::new());
    let (shipping_profile_busy, set_shipping_profile_busy) = signal(false);
    let (shipping_profile_error, set_shipping_profile_error) = signal(Option::<String>::None);

    let badge_label = t(ui_locale.as_deref(), "commerce.badge", "commerce");
    let title_label = t(
        ui_locale.as_deref(),
        "commerce.title",
        "Commerce Control Room",
    );
    let subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.subtitle",
        "Module-owned operator workspace for catalog publishing, shipping-profile registry management and delivery option compatibility. Product CRUD, profile ownership and shipping-option rules all stay inside the commerce package.",
    );
    let bootstrap_loading_label = t(
        ui_locale.as_deref(),
        "commerce.error.bootstrapLoading",
        "Bootstrap is still loading.",
    );
    let catalog_title_label = t(
        ui_locale.as_deref(),
        "commerce.catalog.title",
        "Catalog Feed",
    );
    let catalog_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.catalog.subtitle",
        "Search, publish, archive and open products for editing.",
    );
    let search_title_placeholder = t(
        ui_locale.as_deref(),
        "commerce.catalog.searchPlaceholder",
        "Search title",
    );
    let all_statuses_label = t(
        ui_locale.as_deref(),
        "commerce.catalog.status.all",
        "All statuses",
    );
    let no_products_label = t(
        ui_locale.as_deref(),
        "commerce.catalog.empty",
        "No products yet.",
    );
    let load_products_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadProducts",
        "Failed to load products",
    );
    let product_not_found_label = t(
        ui_locale.as_deref(),
        "commerce.error.productNotFound",
        "Product not found.",
    );
    let load_product_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadProduct",
        "Failed to load product",
    );
    let locale_title_required_label = t(
        ui_locale.as_deref(),
        "commerce.error.localeTitleRequired",
        "Locale and title are required.",
    );
    let save_product_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.saveProduct",
        "Failed to save product",
    );
    let change_product_status_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.changeProductStatus",
        "Failed to change status",
    );
    let delete_false_label = t(
        ui_locale.as_deref(),
        "commerce.error.deleteReturnedFalse",
        "Delete returned false.",
    );
    let delete_product_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.deleteProduct",
        "Failed to delete product",
    );
    let edit_label = t(ui_locale.as_deref(), "commerce.action.edit", "Edit");
    let publish_label = t(ui_locale.as_deref(), "commerce.action.publish", "Publish");
    let move_to_draft_label = t(
        ui_locale.as_deref(),
        "commerce.action.moveToDraft",
        "Move to Draft",
    );
    let archive_label = t(ui_locale.as_deref(), "commerce.action.archive", "Archive");
    let delete_label = t(ui_locale.as_deref(), "commerce.action.delete", "Delete");
    let new_label = t(ui_locale.as_deref(), "commerce.action.new", "New");
    let product_editor_label = t(
        ui_locale.as_deref(),
        "commerce.product.editor",
        "Product Editor",
    );
    let create_product_label = t(
        ui_locale.as_deref(),
        "commerce.product.create",
        "Create Product",
    );
    let product_editor_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.product.subtitle",
        "Single-SKU product editor backed by the module GraphQL mutations.",
    );
    let locale_placeholder_label = t(ui_locale.as_deref(), "commerce.field.locale", "Locale");
    let handle_placeholder_label = t(ui_locale.as_deref(), "commerce.field.handle", "Handle");
    let title_placeholder_label = t(ui_locale.as_deref(), "commerce.field.title", "Title");
    let description_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.description",
        "Description",
    );
    let vendor_placeholder_label = t(ui_locale.as_deref(), "commerce.field.vendor", "Vendor");
    let product_type_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.productType",
        "Product type",
    );
    let no_shipping_profile_label = t(
        ui_locale.as_deref(),
        "commerce.field.noShippingProfile",
        "No shipping profile",
    );
    let known_profiles_template = t(
        ui_locale.as_deref(),
        "commerce.field.knownProfiles",
        "Known profiles: {profiles}",
    );
    let known_profiles_loading_label = t(
        ui_locale.as_deref(),
        "commerce.field.knownProfilesLoading",
        "Known profiles are loading from the shipping-profile registry.",
    );
    let primary_sku_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.primarySku",
        "Primary SKU",
    );
    let currency_placeholder_label = t(ui_locale.as_deref(), "commerce.field.currency", "Currency");
    let price_placeholder_label = t(ui_locale.as_deref(), "commerce.field.price", "Price");
    let inventory_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.inventoryQuantity",
        "Inventory quantity",
    );
    let keep_published_label = t(
        ui_locale.as_deref(),
        "commerce.field.keepPublished",
        "Keep published after save",
    );
    let save_product_label = t(
        ui_locale.as_deref(),
        "commerce.action.saveProduct",
        "Save product",
    );
    let create_product_action_label = t(
        ui_locale.as_deref(),
        "commerce.action.createProduct",
        "Create product",
    );
    let product_summary_empty_label = t(
        ui_locale.as_deref(),
        "commerce.summary.product.empty",
        "Open a product from the feed to inspect its localized copy, primary variant and shipping-profile mapping.",
    );
    let shipping_options_title_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOptions.title",
        "Shipping Options",
    );
    let shipping_options_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOptions.subtitle",
        "Review delivery options, provider bindings and shipping-profile compatibility rules.",
    );
    let shipping_search_placeholder = t(
        ui_locale.as_deref(),
        "commerce.shippingOptions.searchPlaceholder",
        "Search name",
    );
    let shipping_provider_placeholder = t(
        ui_locale.as_deref(),
        "commerce.field.providerId",
        "Provider ID",
    );
    let no_shipping_options_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOptions.empty",
        "No shipping options match the current filters.",
    );
    let load_shipping_options_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadShippingOptions",
        "Failed to load shipping options",
    );
    let shipping_option_editor_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOption.editor",
        "Shipping Option Editor",
    );
    let create_shipping_option_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOption.create",
        "Create Shipping Option",
    );
    let shipping_option_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOption.subtitle",
        "Typed operator surface over createShippingOption and updateShippingOption.",
    );
    let shipping_option_name_required_label = t(
        ui_locale.as_deref(),
        "commerce.error.shippingOptionNameRequired",
        "Shipping option name is required.",
    );
    let shipping_option_not_found_label = t(
        ui_locale.as_deref(),
        "commerce.error.shippingOptionNotFound",
        "Shipping option not found.",
    );
    let load_shipping_option_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadShippingOption",
        "Failed to load shipping option",
    );
    let save_shipping_option_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.saveShippingOption",
        "Failed to save shipping option",
    );
    let toggle_shipping_option_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.changeShippingOptionStatus",
        "Failed to change shipping option status",
    );
    let allowed_profiles_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOption.allowedProfiles",
        "Allowed shipping profiles",
    );
    let allow_all_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOption.allowAll",
        "Allow all",
    );
    let no_shipping_profiles_yet_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOption.noProfiles",
        "No shipping profiles exist yet. Create a profile first or keep this option available to all carts.",
    );
    let registry_loading_label = t(
        ui_locale.as_deref(),
        "commerce.shippingOption.registryLoading",
        "Registry slugs are loading from the shipping-profile registry.",
    );
    let load_registry_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadRegistrySlugs",
        "Failed to load registry slugs",
    );
    let selected_profiles_template = t(
        ui_locale.as_deref(),
        "commerce.shippingOption.selectedProfiles",
        "Selected profiles: {profiles}",
    );
    let metadata_patch_placeholder_label = t(
        ui_locale.as_deref(),
        "commerce.field.metadataJsonPatch",
        "Metadata JSON patch",
    );
    let save_shipping_option_label = t(
        ui_locale.as_deref(),
        "commerce.action.saveShippingOption",
        "Save shipping option",
    );
    let create_shipping_option_action_label = t(
        ui_locale.as_deref(),
        "commerce.action.createShippingOption",
        "Create shipping option",
    );
    let shipping_option_summary_empty_label = t(
        ui_locale.as_deref(),
        "commerce.summary.shippingOption.empty",
        "Open a shipping option to inspect its provider, pricing and shipping-profile compatibility set.",
    );
    let metadata_hint_label = t(
        ui_locale.as_deref(),
        "commerce.metadata.hint",
        "Metadata is sent as an optional JSON patch. Leaving the field blank during update keeps the existing metadata payload unchanged.",
    );
    let shipping_profiles_title_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfiles.title",
        "Shipping Profiles",
    );
    let shipping_profiles_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfiles.subtitle",
        "Manage the typed profile registry used by products and shipping-option compatibility rules.",
    );
    let shipping_profiles_search_placeholder = t(
        ui_locale.as_deref(),
        "commerce.shippingProfiles.searchPlaceholder",
        "Search slug or name",
    );
    let no_shipping_profiles_match_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfiles.empty",
        "No shipping profiles match the current filters.",
    );
    let load_shipping_profiles_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadShippingProfiles",
        "Failed to load shipping profiles",
    );
    let shipping_profile_editor_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfile.editor",
        "Shipping Profile Editor",
    );
    let create_shipping_profile_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfile.create",
        "Create Shipping Profile",
    );
    let shipping_profile_subtitle_label = t(
        ui_locale.as_deref(),
        "commerce.shippingProfile.subtitle",
        "Typed registry editor for the slugs referenced by products and shipping options.",
    );
    let slug_placeholder_label = t(ui_locale.as_deref(), "commerce.field.slug", "Slug");
    let name_placeholder_label = t(ui_locale.as_deref(), "commerce.field.name", "Name");
    let shipping_profile_required_label = t(
        ui_locale.as_deref(),
        "commerce.error.shippingProfileRequired",
        "Shipping profile slug and name are required.",
    );
    let shipping_profile_not_found_label = t(
        ui_locale.as_deref(),
        "commerce.error.shippingProfileNotFound",
        "Shipping profile not found.",
    );
    let load_shipping_profile_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.loadShippingProfile",
        "Failed to load shipping profile",
    );
    let save_shipping_profile_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.saveShippingProfile",
        "Failed to save shipping profile",
    );
    let toggle_shipping_profile_error_label = t(
        ui_locale.as_deref(),
        "commerce.error.changeShippingProfileStatus",
        "Failed to change shipping profile status",
    );
    let save_shipping_profile_label = t(
        ui_locale.as_deref(),
        "commerce.action.saveShippingProfile",
        "Save shipping profile",
    );
    let create_shipping_profile_action_label = t(
        ui_locale.as_deref(),
        "commerce.action.createShippingProfile",
        "Create shipping profile",
    );
    let shipping_profile_summary_empty_label = t(
        ui_locale.as_deref(),
        "commerce.summary.shippingProfile.empty",
        "Open a shipping profile to inspect its slug, description and lifecycle state.",
    );

    let bootstrap = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let products = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                locale.get(),
                search.get(),
                status_filter.get(),
            )
        },
        move |(token_value, tenant_value, _, locale_value, search_value, status_value)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_products(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                locale_value,
                text_or_none(search_value),
                text_or_none(status_value),
            )
            .await
        },
    );

    let shipping_options = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                shipping_search.get(),
                shipping_currency_filter.get(),
                shipping_provider_filter.get(),
            )
        },
        move |(token_value, tenant_value, _, search_value, currency_filter, provider_filter)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_shipping_options(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                text_or_none(search_value),
                text_or_none(currency_filter),
                text_or_none(provider_filter),
            )
            .await
        },
    );

    let shipping_profiles = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                shipping_profile_search.get(),
            )
        },
        move |(token_value, tenant_value, _, search_value)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_shipping_profiles(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                text_or_none(search_value),
            )
            .await
        },
    );

    let reset_form_initial_product_locale = initial_product_locale.clone();
    let reset_form = move || {
        set_editing_id.set(None);
        set_selected.set(None);
        set_locale.set(reset_form_initial_product_locale.clone());
        set_title.set(String::new());
        set_handle.set(String::new());
        set_description.set(String::new());
        set_vendor.set(String::new());
        set_product_type.set(String::new());
        set_product_shipping_profile_slug.set(String::new());
        set_sku.set(String::new());
        set_currency_code.set("USD".to_string());
        set_amount.set("0.00".to_string());
        set_inventory_quantity.set(0);
        set_publish_now.set(false);
    };

    let reset_shipping_form = move || {
        set_shipping_editing_id.set(None);
        set_selected_shipping_option.set(None);
        set_shipping_name.set(String::new());
        set_shipping_currency_code.set("USD".to_string());
        set_shipping_amount.set("0.00".to_string());
        set_shipping_provider_id.set("manual".to_string());
        set_shipping_allowed_profiles.set(String::new());
        set_shipping_metadata_json.set(String::new());
    };

    let reset_shipping_profile_form = move || {
        set_shipping_profile_editing_id.set(None);
        set_selected_shipping_profile.set(None);
        set_shipping_profile_slug.set(String::new());
        set_shipping_profile_name.set(String::new());
        set_shipping_profile_description.set(String::new());
        set_shipping_profile_metadata_json.set(String::new());
    };

    let edit_product_bootstrap_loading_label = bootstrap_loading_label.clone();
    let edit_product_not_found_label = product_not_found_label.clone();
    let edit_product_load_error_label = load_product_error_label.clone();
    let edit_product = Callback::new(move |product_id: String| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some(edit_product_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale_value = locale.get_untracked();
        let not_found_label = edit_product_not_found_label.clone();
        let load_error_label = edit_product_load_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match api::fetch_product(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                product_id,
                locale_value,
            )
            .await
            {
                Ok(Some(product)) => apply_product(
                    &product,
                    set_editing_id,
                    set_selected,
                    set_locale,
                    set_title,
                    set_handle,
                    set_description,
                    set_vendor,
                    set_product_type,
                    set_product_shipping_profile_slug,
                    set_sku,
                    set_currency_code,
                    set_amount,
                    set_inventory_quantity,
                    set_publish_now,
                ),
                Ok(None) => set_error.set(Some(not_found_label)),
                Err(err) => set_error.set(Some(format!("{load_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let edit_shipping_option_bootstrap_loading_label = bootstrap_loading_label.clone();
    let edit_shipping_option_not_found_label = shipping_option_not_found_label.clone();
    let edit_shipping_option_load_error_label = load_shipping_option_error_label.clone();
    let edit_shipping_option = Callback::new(move |shipping_option_id: String| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_error.set(Some(edit_shipping_option_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let not_found_label = edit_shipping_option_not_found_label.clone();
        let load_error_label = edit_shipping_option_load_error_label.clone();
        set_shipping_busy.set(true);
        set_shipping_error.set(None);
        spawn_local(async move {
            match api::fetch_shipping_option(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                shipping_option_id,
            )
            .await
            {
                Ok(Some(option)) => apply_shipping_option(
                    &option,
                    set_shipping_editing_id,
                    set_selected_shipping_option,
                    set_shipping_name,
                    set_shipping_currency_code,
                    set_shipping_amount,
                    set_shipping_provider_id,
                    set_shipping_allowed_profiles,
                    set_shipping_metadata_json,
                ),
                Ok(None) => set_shipping_error.set(Some(not_found_label)),
                Err(err) => set_shipping_error.set(Some(format!("{load_error_label}: {err}"))),
            }
            set_shipping_busy.set(false);
        });
    });

    let edit_shipping_profile_bootstrap_loading_label = bootstrap_loading_label.clone();
    let edit_shipping_profile_not_found_label = shipping_profile_not_found_label.clone();
    let edit_shipping_profile_load_error_label = load_shipping_profile_error_label.clone();
    let edit_shipping_profile = Callback::new(move |shipping_profile_id: String| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_profile_error
                .set(Some(edit_shipping_profile_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let not_found_label = edit_shipping_profile_not_found_label.clone();
        let load_error_label = edit_shipping_profile_load_error_label.clone();
        set_shipping_profile_busy.set(true);
        set_shipping_profile_error.set(None);
        spawn_local(async move {
            match api::fetch_shipping_profile(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                shipping_profile_id,
            )
            .await
            {
                Ok(Some(profile)) => apply_shipping_profile(
                    &profile,
                    set_shipping_profile_editing_id,
                    set_selected_shipping_profile,
                    set_shipping_profile_slug,
                    set_shipping_profile_name,
                    set_shipping_profile_description,
                    set_shipping_profile_metadata_json,
                ),
                Ok(None) => set_shipping_profile_error.set(Some(not_found_label)),
                Err(err) => {
                    set_shipping_profile_error.set(Some(format!("{load_error_label}: {err}")))
                }
            }
            set_shipping_profile_busy.set(false);
        });
    });

    let submit_product_bootstrap_loading_label = bootstrap_loading_label.clone();
    let submit_product_locale_title_required_label = locale_title_required_label.clone();
    let submit_product_save_error_label = save_product_error_label.clone();
    let submit_product = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some(submit_product_bootstrap_loading_label.clone()));
            return;
        };
        let draft = ProductDraft {
            locale: locale.get_untracked().trim().to_string(),
            title: title.get_untracked().trim().to_string(),
            handle: handle.get_untracked().trim().to_string(),
            description: description.get_untracked().trim().to_string(),
            vendor: vendor.get_untracked().trim().to_string(),
            product_type: product_type.get_untracked().trim().to_string(),
            shipping_profile_slug: text_or_none(product_shipping_profile_slug.get_untracked()),
            sku: sku.get_untracked().trim().to_string(),
            barcode: String::new(),
            currency_code: currency_code.get_untracked().trim().to_string(),
            amount: amount.get_untracked().trim().to_string(),
            compare_at_amount: String::new(),
            inventory_quantity: inventory_quantity.get_untracked(),
            publish_now: publish_now.get_untracked(),
        };
        if draft.locale.is_empty() || draft.title.is_empty() {
            set_error.set(Some(submit_product_locale_title_required_label.clone()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_id = editing_id.get_untracked();
        let save_product_error_label = submit_product_save_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let saved = match current_id.clone() {
                Some(product_id) => {
                    api::update_product(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        bootstrap.me.id.clone(),
                        product_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    api::create_product(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        bootstrap.me.id.clone(),
                        draft.clone(),
                    )
                    .await
                }
            };
            match saved {
                Ok(mut product) => {
                    if draft.publish_now && product.status != "ACTIVE" {
                        if let Ok(published) = api::publish_product(
                            token_value.clone(),
                            tenant_value.clone(),
                            bootstrap.current_tenant.id.clone(),
                            bootstrap.me.id.clone(),
                            product.id.clone(),
                        )
                        .await
                        {
                            product = published;
                        }
                    }
                    if !draft.publish_now && product.status == "ACTIVE" {
                        if let Ok(drafted) = api::change_product_status(
                            token_value.clone(),
                            tenant_value.clone(),
                            bootstrap.current_tenant.id.clone(),
                            bootstrap.me.id.clone(),
                            product.id.clone(),
                            "DRAFT",
                        )
                        .await
                        {
                            product = drafted;
                        }
                    }
                    apply_product(
                        &product,
                        set_editing_id,
                        set_selected,
                        set_locale,
                        set_title,
                        set_handle,
                        set_description,
                        set_vendor,
                        set_product_type,
                        set_product_shipping_profile_slug,
                        set_sku,
                        set_currency_code,
                        set_amount,
                        set_inventory_quantity,
                        set_publish_now,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("{save_product_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    };

    let submit_shipping_option_bootstrap_loading_label = bootstrap_loading_label.clone();
    let submit_shipping_option_required_label = shipping_option_name_required_label.clone();
    let submit_shipping_option_save_error_label = save_shipping_option_error_label.clone();
    let submit_shipping_option = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_error.set(Some(submit_shipping_option_bootstrap_loading_label.clone()));
            return;
        };
        let draft = ShippingOptionDraft {
            name: shipping_name.get_untracked().trim().to_string(),
            currency_code: shipping_currency_code.get_untracked().trim().to_string(),
            amount: shipping_amount.get_untracked().trim().to_string(),
            provider_id: shipping_provider_id.get_untracked().trim().to_string(),
            allowed_shipping_profile_slugs: shipping_allowed_profiles
                .get_untracked()
                .trim()
                .to_string(),
            metadata_json: shipping_metadata_json.get_untracked().trim().to_string(),
        };
        if draft.name.is_empty() {
            set_shipping_error.set(Some(submit_shipping_option_required_label.clone()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_id = shipping_editing_id.get_untracked();
        let save_shipping_option_error_label = submit_shipping_option_save_error_label.clone();
        set_shipping_busy.set(true);
        set_shipping_error.set(None);
        spawn_local(async move {
            let saved = match current_id.clone() {
                Some(shipping_option_id) => {
                    api::update_shipping_option(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        shipping_option_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    api::create_shipping_option(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        draft.clone(),
                    )
                    .await
                }
            };
            match saved {
                Ok(option) => {
                    apply_shipping_option(
                        &option,
                        set_shipping_editing_id,
                        set_selected_shipping_option,
                        set_shipping_name,
                        set_shipping_currency_code,
                        set_shipping_amount,
                        set_shipping_provider_id,
                        set_shipping_allowed_profiles,
                        set_shipping_metadata_json,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_shipping_error
                    .set(Some(format!("{save_shipping_option_error_label}: {err}"))),
            }
            set_shipping_busy.set(false);
        });
    };

    let submit_shipping_profile_bootstrap_loading_label = bootstrap_loading_label.clone();
    let submit_shipping_profile_required_label = shipping_profile_required_label.clone();
    let submit_shipping_profile_save_error_label = save_shipping_profile_error_label.clone();
    let submit_shipping_profile = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_profile_error.set(Some(
                submit_shipping_profile_bootstrap_loading_label.clone(),
            ));
            return;
        };
        let draft = ShippingProfileDraft {
            slug: shipping_profile_slug.get_untracked().trim().to_string(),
            name: shipping_profile_name.get_untracked().trim().to_string(),
            description: shipping_profile_description
                .get_untracked()
                .trim()
                .to_string(),
            metadata_json: shipping_profile_metadata_json
                .get_untracked()
                .trim()
                .to_string(),
        };
        if draft.slug.is_empty() || draft.name.is_empty() {
            set_shipping_profile_error.set(Some(submit_shipping_profile_required_label.clone()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_id = shipping_profile_editing_id.get_untracked();
        let save_shipping_profile_error_label = submit_shipping_profile_save_error_label.clone();
        set_shipping_profile_busy.set(true);
        set_shipping_profile_error.set(None);
        spawn_local(async move {
            let saved = match current_id.clone() {
                Some(shipping_profile_id) => {
                    api::update_shipping_profile(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        shipping_profile_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    api::create_shipping_profile(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        draft.clone(),
                    )
                    .await
                }
            };
            match saved {
                Ok(profile) => {
                    apply_shipping_profile(
                        &profile,
                        set_shipping_profile_editing_id,
                        set_selected_shipping_profile,
                        set_shipping_profile_slug,
                        set_shipping_profile_name,
                        set_shipping_profile_description,
                        set_shipping_profile_metadata_json,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_shipping_profile_error
                    .set(Some(format!("{save_shipping_profile_error_label}: {err}"))),
            }
            set_shipping_profile_busy.set(false);
        });
    };

    let toggle_publish_bootstrap_loading_label = bootstrap_loading_label.clone();
    let toggle_publish_change_status_error_label = change_product_status_error_label.clone();
    let toggle_publish = Callback::new(move |product: ProductListItem| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some(toggle_publish_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let change_status_error_label = toggle_publish_change_status_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = if product.status == "ACTIVE" {
                api::change_product_status(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    product.id.clone(),
                    "DRAFT",
                )
                .await
            } else {
                api::publish_product(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    product.id.clone(),
                )
                .await
            };
            match result {
                Ok(_) => set_refresh_nonce.update(|value| *value += 1),
                Err(err) => set_error.set(Some(format!("{change_status_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let archive_bootstrap_loading_label = bootstrap_loading_label.clone();
    let archive_change_status_error_label = change_product_status_error_label.clone();
    let archive_product = Callback::new(move |product_id: String| {
        mutate_status(
            bootstrap.get_untracked().and_then(Result::ok),
            token.get_untracked(),
            tenant.get_untracked(),
            product_id,
            "ARCHIVED",
            archive_bootstrap_loading_label.clone(),
            archive_change_status_error_label.clone(),
            set_busy,
            set_error,
            set_refresh_nonce,
        )
    });

    let delete_bootstrap_loading_label = bootstrap_loading_label.clone();
    let delete_false_label = delete_false_label.clone();
    let delete_product_error_label = delete_product_error_label.clone();
    let delete_product = Callback::new(move |product_id: String| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some(delete_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_editing_id = editing_id.get_untracked();
        let delete_false_label = delete_false_label.clone();
        let delete_product_error_label = delete_product_error_label.clone();
        let reset_locale = initial_product_locale.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match api::delete_product(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                bootstrap.me.id,
                product_id.clone(),
            )
            .await
            {
                Ok(true) => {
                    if current_editing_id.as_deref() == Some(product_id.as_str()) {
                        set_editing_id.set(None);
                        set_selected.set(None);
                        set_locale.set(reset_locale);
                        set_title.set(String::new());
                        set_handle.set(String::new());
                        set_description.set(String::new());
                        set_vendor.set(String::new());
                        set_product_type.set(String::new());
                        set_product_shipping_profile_slug.set(String::new());
                        set_sku.set(String::new());
                        set_currency_code.set("USD".to_string());
                        set_amount.set("0.00".to_string());
                        set_inventory_quantity.set(0);
                        set_publish_now.set(false);
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Ok(false) => set_error.set(Some(delete_false_label)),
                Err(err) => set_error.set(Some(format!("{delete_product_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let toggle_shipping_option_bootstrap_loading_label = bootstrap_loading_label.clone();
    let toggle_shipping_option_error_label = toggle_shipping_option_error_label.clone();
    let toggle_shipping_option = Callback::new(move |option: ShippingOption| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_error.set(Some(toggle_shipping_option_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let change_status_error_label = toggle_shipping_option_error_label.clone();
        set_shipping_busy.set(true);
        set_shipping_error.set(None);
        spawn_local(async move {
            let result = if option.active {
                api::deactivate_shipping_option(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    option.id.clone(),
                )
                .await
            } else {
                api::reactivate_shipping_option(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    option.id.clone(),
                )
                .await
            };
            match result {
                Ok(updated) => {
                    if shipping_editing_id.get_untracked().as_deref() == Some(option.id.as_str()) {
                        apply_shipping_option(
                            &updated,
                            set_shipping_editing_id,
                            set_selected_shipping_option,
                            set_shipping_name,
                            set_shipping_currency_code,
                            set_shipping_amount,
                            set_shipping_provider_id,
                            set_shipping_allowed_profiles,
                            set_shipping_metadata_json,
                        );
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => {
                    set_shipping_error.set(Some(format!("{change_status_error_label}: {err}")))
                }
            }
            set_shipping_busy.set(false);
        });
    });

    let toggle_shipping_profile_bootstrap_loading_label = bootstrap_loading_label.clone();
    let toggle_shipping_profile_error_label = toggle_shipping_profile_error_label.clone();
    let toggle_shipping_profile = Callback::new(move |profile: ShippingProfile| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_profile_error.set(Some(
                toggle_shipping_profile_bootstrap_loading_label.clone(),
            ));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let change_status_error_label = toggle_shipping_profile_error_label.clone();
        set_shipping_profile_busy.set(true);
        set_shipping_profile_error.set(None);
        spawn_local(async move {
            let result = if profile.active {
                api::deactivate_shipping_profile(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    profile.id.clone(),
                )
                .await
            } else {
                api::reactivate_shipping_profile(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    profile.id.clone(),
                )
                .await
            };
            match result {
                Ok(updated) => {
                    if shipping_profile_editing_id.get_untracked().as_deref()
                        == Some(profile.id.as_str())
                    {
                        apply_shipping_profile(
                            &updated,
                            set_shipping_profile_editing_id,
                            set_selected_shipping_profile,
                            set_shipping_profile_slug,
                            set_shipping_profile_name,
                            set_shipping_profile_description,
                            set_shipping_profile_metadata_json,
                        );
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_shipping_profile_error
                    .set(Some(format!("{change_status_error_label}: {err}"))),
            }
            set_shipping_profile_busy.set(false);
        });
    });

    let ui_locale_for_product_list = ui_locale.clone();
    let ui_locale_for_product_select = ui_locale.clone();
    let ui_locale_for_known_profiles = ui_locale.clone();
    let ui_locale_for_product_summary = ui_locale.clone();
    let ui_locale_for_shipping_options = ui_locale.clone();
    let ui_locale_for_shipping_profiles = ui_locale.clone();
    let ui_locale_for_allowed_profile_choices = ui_locale.clone();
    let ui_locale_for_selected_profiles = ui_locale.clone();
    let ui_locale_for_shipping_option_summary = ui_locale.clone();
    let ui_locale_for_shipping_profile_summary = ui_locale.clone();
    let edit_label_for_product_list = edit_label.clone();
    let edit_label_for_shipping_options = edit_label.clone();
    let edit_label_for_shipping_profiles = edit_label.clone();

    view! {
        <section class="space-y-6">
            <div class="rounded-3xl border border-border bg-card p-8 shadow-sm">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{badge_label.clone()}</span>
                <h2 class="mt-4 text-3xl font-semibold text-card-foreground">{title_label.clone()}</h2>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{subtitle_label.clone()}</p>
            </div>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{catalog_title_label.clone()}</h3>
                            <p class="text-sm text-muted-foreground">{catalog_subtitle_label.clone()}</p>
                        </div>
                        <div class="flex flex-col gap-3 md:flex-row">
                            <input class="min-w-56 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=search_title_placeholder.clone() prop:value=move || search.get() on:input=move |ev| set_search.set(event_target_value(&ev)) />
                            <select class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || status_filter.get() on:change=move |ev| set_status_filter.set(event_target_value(&ev))>
                                <option value="">{all_statuses_label.clone()}</option>
                                <option value="DRAFT">{localized_product_status(ui_locale.as_deref(), "DRAFT")}</option>
                                <option value="ACTIVE">{localized_product_status(ui_locale.as_deref(), "ACTIVE")}</option>
                                <option value="ARCHIVED">{localized_product_status(ui_locale.as_deref(), "ARCHIVED")}</option>
                            </select>
                        </div>
                    </div>
                    <div class="mt-5 space-y-3">
                        {move || match products.get() {
                            None => view! { <div class="space-y-3"><div class="h-24 animate-pulse rounded-2xl bg-muted"></div><div class="h-24 animate-pulse rounded-2xl bg-muted"></div></div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{no_products_label.clone()}</div> }.into_any(),
                            Some(Ok(list)) => view! { <>
                                {list.items.into_iter().map(|product| {
                                    let item_locale = ui_locale_for_product_list.clone();
                                    let edit_id = product.id.clone();
                                    let archive_id = product.id.clone();
                                    let delete_id = product.id.clone();
                                    let publish_item = product.clone();
                                    let shipping_profile_label = product
                                        .shipping_profile_slug
                                        .clone()
                                        .map(|value| format_product_shipping_profile(item_locale.as_deref(), value.as_str()));
                                    let shipping_profile_for_show = shipping_profile_label.clone();
                                    let product_status_label =
                                        localized_product_status(item_locale.as_deref(), product.status.as_str());
                                    let product_type_label = product
                                        .product_type
                                        .clone()
                                        .unwrap_or_else(|| t(item_locale.as_deref(), "commerce.common.general", "general"));
                                    let meta_line = format_product_meta(
                                        item_locale.as_deref(),
                                        product.handle.as_str(),
                                        product.vendor.as_deref(),
                                    );
                                    let publish_toggle_label = if product.status == "ACTIVE" {
                                        move_to_draft_label.clone()
                                    } else {
                                        publish_label.clone()
                                    };
                                    view! {
                                        <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                <div class="space-y-2">
                                                    <div class="flex flex-wrap items-center gap-2">
                                                        <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", status_badge(product.status.as_str()))>{product_status_label}</span>
                                                        <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{product_type_label}</span>
                                                        <Show when=move || shipping_profile_for_show.is_some()>
                                                            <span class="text-xs text-muted-foreground">{shipping_profile_label.clone().unwrap_or_default()}</span>
                                                        </Show>
                                                    </div>
                                                    <h4 class="text-base font-semibold text-card-foreground">{product.title.clone()}</h4>
                                                    <p class="text-sm text-muted-foreground">{meta_line}</p>
                                                    <p class="text-xs text-muted-foreground">{product.published_at.clone().unwrap_or_else(|| product.created_at.clone())}</p>
                                                </div>
                                                <div class="flex flex-wrap gap-2">
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| edit_product.run(edit_id.clone())>{edit_label_for_product_list.clone()}</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| toggle_publish.run(publish_item.clone())>{publish_toggle_label}</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| archive_product.run(archive_id.clone())>{archive_label.clone()}</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-destructive/40 px-3 py-2 text-sm font-medium text-destructive transition hover:bg-destructive/10 disabled:opacity-50" disabled=move || busy.get() || product.status == "ACTIVE" on:click=move |_| delete_product.run(delete_id.clone())>{delete_label.clone()}</button>
                                                </div>
                                            </div>
                                        </article>
                                    }
                                }).collect_view()}
                            </> }.into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{load_products_error_label}: {err}")}</div> }.into_any(),
                        }}
                    </div>
                </section>

                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{move || if editing_id.get().is_some() { product_editor_label.clone() } else { create_product_label.clone() }}</h3>
                            <p class="text-sm text-muted-foreground">{product_editor_subtitle_label.clone()}</p>
                        </div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| reset_form()>{new_label.clone()}</button>
                    </div>
                    <Show when=move || error.get().is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || error.get().unwrap_or_default()}</div>
                    </Show>
                    <form class="mt-5 space-y-4" on:submit=submit_product>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=locale_placeholder_label.clone() prop:value=move || locale.get() on:input=move |ev| set_locale.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=handle_placeholder_label.clone() prop:value=move || handle.get() on:input=move |ev| set_handle.set(event_target_value(&ev)) />
                        </div>
                        <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=title_placeholder_label.clone() prop:value=move || title.get() on:input=move |ev| set_title.set(event_target_value(&ev)) />
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=description_placeholder_label.clone() prop:value=move || description.get() on:input=move |ev| set_description.set(event_target_value(&ev)) />
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=vendor_placeholder_label.clone() prop:value=move || vendor.get() on:input=move |ev| set_vendor.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=product_type_placeholder_label.clone() prop:value=move || product_type.get() on:input=move |ev| set_product_type.set(event_target_value(&ev)) />
                        </div>
                        <div class="space-y-2">
                            <select class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || product_shipping_profile_slug.get() on:change=move |ev| set_product_shipping_profile_slug.set(event_target_value(&ev))>
                                <option value="">{no_shipping_profile_label.clone()}</option>
                                {move || match shipping_profiles.get() {
                                    Some(Ok(list)) => {
                                        let current_slug = product_shipping_profile_slug.get();
                                        list.items
                                            .into_iter()
                                            .filter(|profile| profile.active || profile.slug == current_slug)
                                            .map(|profile| {
                                                let label = shipping_profile_choice_label(ui_locale_for_product_select.as_deref(), &profile);
                                                let slug = profile.slug;
                                                view! { <option value=slug.clone()>{label}</option> }
                                            })
                                            .collect_view()
                                            .into_any()
                                    }
                                    _ => view! { <></> }.into_any(),
                                }}
                            </select>
                            <p class="text-xs text-muted-foreground">{move || shipping_profiles.get().and_then(Result::ok).map(|list| known_profiles_template.replace("{profiles}", format_known_shipping_profiles(ui_locale_for_known_profiles.as_deref(), &list.items).as_str())).unwrap_or_else(|| known_profiles_loading_label.clone())}</p>
                        </div>
                        <div class="grid gap-4 md:grid-cols-3">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=primary_sku_placeholder_label.clone() prop:value=move || sku.get() on:input=move |ev| set_sku.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=currency_placeholder_label.clone() prop:value=move || currency_code.get() on:input=move |ev| set_currency_code.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=price_placeholder_label.clone() prop:value=move || amount.get() on:input=move |ev| set_amount.set(event_target_value(&ev)) />
                        </div>
                        <div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=inventory_placeholder_label.clone() prop:value=move || inventory_quantity.get().to_string() on:input=move |ev| set_inventory_quantity.set(event_target_value(&ev).parse::<i32>().unwrap_or(0)) />
                            <label class="inline-flex items-center gap-3 rounded-2xl border border-border bg-background px-4 py-3 text-sm text-foreground">
                                <input type="checkbox" prop:checked=move || publish_now.get() on:change=move |ev| set_publish_now.set(event_target_checked(&ev)) />
                                <span>{keep_published_label.clone()}</span>
                            </label>
                        </div>
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>{move || if editing_id.get().is_some() { save_product_label.clone() } else { create_product_action_label.clone() }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected.get().map(|product| summarize_selected(ui_locale_for_product_summary.as_deref(), &product)).unwrap_or_else(|| product_summary_empty_label.clone())}
                    </div>
                </section>
            </div>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{shipping_options_title_label.clone()}</h3>
                            <p class="text-sm text-muted-foreground">{shipping_options_subtitle_label.clone()}</p>
                        </div>
                        <div class="grid gap-3 md:grid-cols-3">
                            <input class="min-w-40 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=shipping_search_placeholder.clone() prop:value=move || shipping_search.get() on:input=move |ev| set_shipping_search.set(event_target_value(&ev)) />
                            <input class="min-w-32 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=currency_placeholder_label.clone() prop:value=move || shipping_currency_filter.get() on:input=move |ev| set_shipping_currency_filter.set(event_target_value(&ev)) />
                            <input class="min-w-32 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=shipping_provider_placeholder.clone() prop:value=move || shipping_provider_filter.get() on:input=move |ev| set_shipping_provider_filter.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    <div class="mt-5 space-y-3">
                        {move || match shipping_options.get() {
                            None => view! { <div class="space-y-3"><div class="h-24 animate-pulse rounded-2xl bg-muted"></div><div class="h-24 animate-pulse rounded-2xl bg-muted"></div></div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{no_shipping_options_label.clone()}</div> }.into_any(),
                            Some(Ok(list)) => view! { <>
                                {list.items.into_iter().map(|option| {
                                    let item_locale = ui_locale_for_shipping_options.clone();
                                    let edit_id = option.id.clone();
                                    let toggle_item = option.clone();
                                    let option_status_label =
                                        localized_active_label(item_locale.as_deref(), option.active);
                                    let allowed_profiles_label_value =
                                        format_allowed_profiles(item_locale.as_deref(), option.allowed_shipping_profile_slugs.as_ref());
                                    let profiles_meta = t(
                                        item_locale.as_deref(),
                                        "commerce.shippingOption.profilesMeta",
                                        "profiles: {profiles}",
                                    )
                                    .replace("{profiles}", allowed_profiles_label_value.as_str());
                                    let toggle_label = if option.active {
                                        t(item_locale.as_deref(), "commerce.action.deactivate", "Deactivate")
                                    } else {
                                        t(item_locale.as_deref(), "commerce.action.reactivate", "Reactivate")
                                    };
                                    view! {
                                        <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                <div class="space-y-2">
                                                    <div class="flex flex-wrap items-center gap-2">
                                                        <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", shipping_option_active_badge(option.active))>{option_status_label}</span>
                                                        <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{option.provider_id.clone()}</span>
                                                    </div>
                                                    <h4 class="text-base font-semibold text-card-foreground">{option.name.clone()}</h4>
                                                    <p class="text-sm text-muted-foreground">{format!("{} {}", option.currency_code, option.amount)}</p>
                                                    <p class="text-xs text-muted-foreground">{profiles_meta}</p>
                                                    <p class="text-xs text-muted-foreground">{option.updated_at.clone()}</p>
                                                </div>
                                                <div class="flex flex-wrap gap-2">
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_busy.get() on:click=move |_| edit_shipping_option.run(edit_id.clone())>{edit_label_for_shipping_options.clone()}</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_busy.get() on:click=move |_| toggle_shipping_option.run(toggle_item.clone())>{toggle_label}</button>
                                                </div>
                                            </div>
                                        </article>
                                    }
                                }).collect_view()}
                            </> }.into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{load_shipping_options_error_label}: {err}")}</div> }.into_any(),
                        }}
                    </div>
                </section>

                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{move || if shipping_editing_id.get().is_some() { shipping_option_editor_label.clone() } else { create_shipping_option_label.clone() }}</h3>
                            <p class="text-sm text-muted-foreground">{shipping_option_subtitle_label.clone()}</p>
                        </div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_busy.get() on:click=move |_| reset_shipping_form()>{new_label.clone()}</button>
                    </div>
                    <Show when=move || shipping_error.get().is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || shipping_error.get().unwrap_or_default()}</div>
                    </Show>
                    <form class="mt-5 space-y-4" on:submit=submit_shipping_option>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=name_placeholder_label.clone() prop:value=move || shipping_name.get() on:input=move |ev| set_shipping_name.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=shipping_provider_placeholder.clone() prop:value=move || shipping_provider_id.get() on:input=move |ev| set_shipping_provider_id.set(event_target_value(&ev)) />
                        </div>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=currency_placeholder_label.clone() prop:value=move || shipping_currency_code.get() on:input=move |ev| set_shipping_currency_code.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=price_placeholder_label.clone() prop:value=move || shipping_amount.get() on:input=move |ev| set_shipping_amount.set(event_target_value(&ev)) />
                        </div>
                        <div class="space-y-3">
                            <div class="rounded-2xl border border-border bg-background p-4">
                                <div class="flex items-center justify-between gap-3">
                                    <p class="text-sm font-medium text-card-foreground">{allowed_profiles_label.clone()}</p>
                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_busy.get() on:click=move |_| set_shipping_allowed_profiles.set(String::new())>{allow_all_label.clone()}</button>
                                </div>
                                <div class="mt-3 flex flex-wrap gap-2">
                                    {move || match shipping_profiles.get() {
                                        Some(Ok(list)) if !list.items.is_empty() => list.items
                                            .into_iter()
                                            .map(|profile| {
                                                let slug = profile.slug.clone();
                                                let label = shipping_profile_choice_label(ui_locale_for_allowed_profile_choices.as_deref(), &profile);
                                                let inactive_disabled_slug = slug.clone();
                                                let toggle_slug = slug.clone();
                                                let is_inactive = !profile.active;
                                                view! {
                                                    <button
                                                        type="button"
                                                        class=move || shipping_profile_chip_class(
                                                            csv_contains(shipping_allowed_profiles.get().as_str(), slug.as_str()),
                                                            is_inactive,
                                                        )
                                                        disabled=move || shipping_busy.get() || (is_inactive && !csv_contains(shipping_allowed_profiles.get().as_str(), inactive_disabled_slug.as_str()))
                                                        on:click=move |_| {
                                                            set_shipping_allowed_profiles.update(|value| {
                                                                *value = toggle_csv_slug(value.as_str(), toggle_slug.as_str());
                                                            });
                                                        }
                                                    >
                                                        {label.clone()}
                                                    </button>
                                                }
                                            })
                                            .collect_view()
                                            .into_any(),
                                        Some(Ok(_)) => view! { <p class="text-sm text-muted-foreground">{no_shipping_profiles_yet_label.clone()}</p> }.into_any(),
                                        Some(Err(err)) => view! { <p class="text-sm text-destructive">{format!("{load_registry_error_label}: {err}")}</p> }.into_any(),
                                        None => view! { <p class="text-sm text-muted-foreground">{registry_loading_label.clone()}</p> }.into_any(),
                                    }}
                                </div>
                            </div>
                            <p class="text-xs text-muted-foreground">{move || selected_profiles_template.replace("{profiles}", format_selected_shipping_profiles(ui_locale_for_selected_profiles.as_deref(), shipping_allowed_profiles.get().as_str()).as_str())}</p>
                        </div>
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=metadata_patch_placeholder_label.clone() prop:value=move || shipping_metadata_json.get() on:input=move |ev| set_shipping_metadata_json.set(event_target_value(&ev)) />
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || shipping_busy.get()>{move || if shipping_editing_id.get().is_some() { save_shipping_option_label.clone() } else { create_shipping_option_action_label.clone() }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected_shipping_option.get().map(|option| summarize_shipping_option(ui_locale_for_shipping_option_summary.as_deref(), &option)).unwrap_or_else(|| shipping_option_summary_empty_label.clone())}
                    </div>
                    <p class="mt-3 text-xs text-muted-foreground">{metadata_hint_label.clone()}</p>
                </section>
            </div>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{shipping_profiles_title_label.clone()}</h3>
                            <p class="text-sm text-muted-foreground">{shipping_profiles_subtitle_label.clone()}</p>
                        </div>
                        <div class="flex flex-col gap-3 md:flex-row">
                            <input class="min-w-56 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=shipping_profiles_search_placeholder.clone() prop:value=move || shipping_profile_search.get() on:input=move |ev| set_shipping_profile_search.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    <div class="mt-5 space-y-3">
                        {move || match shipping_profiles.get() {
                            None => view! { <div class="space-y-3"><div class="h-24 animate-pulse rounded-2xl bg-muted"></div><div class="h-24 animate-pulse rounded-2xl bg-muted"></div></div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{no_shipping_profiles_match_label.clone()}</div> }.into_any(),
                            Some(Ok(list)) => view! { <>
                                {list.items.into_iter().map(|profile| {
                                    let item_locale = ui_locale_for_shipping_profiles.clone();
                                    let edit_id = profile.id.clone();
                                    let toggle_item = profile.clone();
                                    let has_description = profile.description.is_some();
                                    let description = profile.description.clone().unwrap_or_default();
                                    let profile_status_label =
                                        localized_active_label(item_locale.as_deref(), profile.active);
                                    let toggle_label = if profile.active {
                                        t(item_locale.as_deref(), "commerce.action.deactivate", "Deactivate")
                                    } else {
                                        t(item_locale.as_deref(), "commerce.action.reactivate", "Reactivate")
                                    };
                                    view! {
                                        <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                <div class="space-y-2">
                                                    <div class="flex flex-wrap items-center gap-2">
                                                        <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", shipping_option_active_badge(profile.active))>{profile_status_label}</span>
                                                        <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{profile.slug.clone()}</span>
                                                    </div>
                                                    <h4 class="text-base font-semibold text-card-foreground">{profile.name.clone()}</h4>
                                                    <Show when=move || has_description>
                                                        <p class="text-sm text-muted-foreground">{description.clone()}</p>
                                                    </Show>
                                                    <p class="text-xs text-muted-foreground">{profile.updated_at.clone()}</p>
                                                </div>
                                                <div class="flex flex-wrap gap-2">
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_profile_busy.get() on:click=move |_| edit_shipping_profile.run(edit_id.clone())>{edit_label_for_shipping_profiles.clone()}</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_profile_busy.get() on:click=move |_| toggle_shipping_profile.run(toggle_item.clone())>{toggle_label}</button>
                                                </div>
                                            </div>
                                        </article>
                                    }
                                }).collect_view()}
                            </> }.into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{load_shipping_profiles_error_label}: {err}")}</div> }.into_any(),
                        }}
                    </div>
                </section>

                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{move || if shipping_profile_editing_id.get().is_some() { shipping_profile_editor_label.clone() } else { create_shipping_profile_label.clone() }}</h3>
                            <p class="text-sm text-muted-foreground">{shipping_profile_subtitle_label.clone()}</p>
                        </div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_profile_busy.get() on:click=move |_| reset_shipping_profile_form()>{new_label.clone()}</button>
                    </div>
                    <Show when=move || shipping_profile_error.get().is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || shipping_profile_error.get().unwrap_or_default()}</div>
                    </Show>
                    <form class="mt-5 space-y-4" on:submit=submit_shipping_profile>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=slug_placeholder_label.clone() prop:value=move || shipping_profile_slug.get() on:input=move |ev| set_shipping_profile_slug.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=name_placeholder_label.clone() prop:value=move || shipping_profile_name.get() on:input=move |ev| set_shipping_profile_name.set(event_target_value(&ev)) />
                        </div>
                        <textarea class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=description_placeholder_label.clone() prop:value=move || shipping_profile_description.get() on:input=move |ev| set_shipping_profile_description.set(event_target_value(&ev)) />
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=metadata_patch_placeholder_label.clone() prop:value=move || shipping_profile_metadata_json.get() on:input=move |ev| set_shipping_profile_metadata_json.set(event_target_value(&ev)) />
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || shipping_profile_busy.get()>{move || if shipping_profile_editing_id.get().is_some() { save_shipping_profile_label.clone() } else { create_shipping_profile_action_label.clone() }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected_shipping_profile.get().map(|profile| summarize_shipping_profile(ui_locale_for_shipping_profile_summary.as_deref(), &profile)).unwrap_or_else(|| shipping_profile_summary_empty_label.clone())}
                    </div>
                    <p class="mt-3 text-xs text-muted-foreground">{metadata_hint_label.clone()}</p>
                </section>
            </div>
        </section>
    }
}

fn apply_product(
    product: &ProductDetail,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ProductDetail>>,
    set_locale: WriteSignal<String>,
    set_title: WriteSignal<String>,
    set_handle: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_vendor: WriteSignal<String>,
    set_product_type: WriteSignal<String>,
    set_shipping_profile_slug: WriteSignal<String>,
    set_sku: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_inventory_quantity: WriteSignal<i32>,
    set_publish_now: WriteSignal<bool>,
) {
    let translation = product.translations.first().cloned();
    let variant = product.variants.first().cloned();
    let price = variant
        .as_ref()
        .and_then(|item| item.prices.first().cloned());
    set_editing_id.set(Some(product.id.clone()));
    set_selected.set(Some(product.clone()));
    set_locale.set(
        translation
            .as_ref()
            .map(|item| item.locale.clone())
            .unwrap_or_else(|| "en".to_string()),
    );
    set_title.set(
        translation
            .as_ref()
            .map(|item| item.title.clone())
            .unwrap_or_default(),
    );
    set_handle.set(
        translation
            .as_ref()
            .map(|item| item.handle.clone())
            .unwrap_or_default(),
    );
    set_description.set(
        translation
            .and_then(|item| item.description)
            .unwrap_or_default(),
    );
    set_vendor.set(product.vendor.clone().unwrap_or_default());
    set_product_type.set(product.product_type.clone().unwrap_or_default());
    set_shipping_profile_slug.set(product.shipping_profile_slug.clone().unwrap_or_default());
    set_sku.set(variant.and_then(|item| item.sku).unwrap_or_default());
    set_currency_code.set(
        price
            .as_ref()
            .map(|item| item.currency_code.clone())
            .unwrap_or_else(|| "USD".to_string()),
    );
    set_amount.set(
        price
            .map(|item| item.amount)
            .unwrap_or_else(|| "0.00".to_string()),
    );
    set_inventory_quantity.set(
        product
            .variants
            .first()
            .map(|item| item.inventory_quantity)
            .unwrap_or(0),
    );
    set_publish_now.set(product.status == "ACTIVE");
}

fn apply_shipping_option(
    option: &ShippingOption,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ShippingOption>>,
    set_name: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_provider_id: WriteSignal<String>,
    set_allowed_profiles: WriteSignal<String>,
    set_metadata_json: WriteSignal<String>,
) {
    set_editing_id.set(Some(option.id.clone()));
    set_selected.set(Some(option.clone()));
    set_name.set(option.name.clone());
    set_currency_code.set(option.currency_code.clone());
    set_amount.set(option.amount.clone());
    set_provider_id.set(option.provider_id.clone());
    set_allowed_profiles.set(
        option
            .allowed_shipping_profile_slugs
            .clone()
            .unwrap_or_default()
            .join(", "),
    );
    set_metadata_json.set(option.metadata.clone());
}

fn apply_shipping_profile(
    profile: &ShippingProfile,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ShippingProfile>>,
    set_slug: WriteSignal<String>,
    set_name: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_metadata_json: WriteSignal<String>,
) {
    set_editing_id.set(Some(profile.id.clone()));
    set_selected.set(Some(profile.clone()));
    set_slug.set(profile.slug.clone());
    set_name.set(profile.name.clone());
    set_description.set(profile.description.clone().unwrap_or_default());
    set_metadata_json.set(profile.metadata.clone());
}

fn mutate_status(
    bootstrap: Option<crate::model::CommerceAdminBootstrap>,
    token: Option<String>,
    tenant: Option<String>,
    product_id: String,
    status: &str,
    bootstrap_loading_label: String,
    change_status_error_label: String,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) {
    let Some(bootstrap) = bootstrap else {
        set_error.set(Some(bootstrap_loading_label));
        return;
    };
    let status = status.to_string();
    set_busy.set(true);
    set_error.set(None);
    spawn_local(async move {
        match api::change_product_status(
            token,
            tenant,
            bootstrap.current_tenant.id,
            bootstrap.me.id,
            product_id,
            status.as_str(),
        )
        .await
        {
            Ok(_) => set_refresh_nonce.update(|value| *value += 1),
            Err(err) => set_error.set(Some(format!("{change_status_error_label}: {err}"))),
        }
        set_busy.set(false);
    });
}

fn summarize_selected(locale: Option<&str>, product: &ProductDetail) -> String {
    let title = product
        .translations
        .first()
        .map(|item| item.title.clone())
        .unwrap_or_else(|| t(locale, "commerce.summary.product.untitled", "Untitled"));
    let variant = product.variants.first();
    let price = variant
        .and_then(|item| item.prices.first())
        .map(|price| format!("{} {}", price.currency_code, price.amount))
        .unwrap_or_else(|| t(locale, "commerce.summary.product.noPricing", "no pricing"));
    let inventory = variant.map(|item| item.inventory_quantity).unwrap_or(0);
    let shipping_profile = product
        .shipping_profile_slug
        .clone()
        .unwrap_or_else(|| t(locale, "commerce.summary.product.unassigned", "unassigned"));
    format!(
        "{title} | {} {} | {} {price} | {} {inventory} | {} {shipping_profile}",
        t(locale, "commerce.summary.product.status", "status"),
        localized_product_status(locale, product.status.as_str()),
        t(
            locale,
            "commerce.summary.product.primaryVariantPrice",
            "primary variant price"
        ),
        t(locale, "commerce.summary.product.inventory", "inventory"),
        t(
            locale,
            "commerce.summary.product.shippingProfile",
            "shipping profile"
        ),
    )
}

fn summarize_shipping_option(locale: Option<&str>, option: &ShippingOption) -> String {
    format!(
        "{} | {} {} | {} {} | {} {}",
        option.name,
        option.currency_code,
        option.amount,
        t(
            locale,
            "commerce.summary.shippingOption.provider",
            "provider"
        ),
        option.provider_id,
        t(
            locale,
            "commerce.summary.shippingOption.profiles",
            "profiles"
        ),
        format_allowed_profiles(locale, option.allowed_shipping_profile_slugs.as_ref())
    )
}

fn summarize_shipping_profile(locale: Option<&str>, profile: &ShippingProfile) -> String {
    format!(
        "{} ({}) | {} | {}",
        profile.name,
        profile.slug,
        localized_active_label(locale, profile.active),
        profile.description.clone().unwrap_or_else(|| t(
            locale,
            "commerce.summary.shippingProfile.noDescription",
            "no description"
        ))
    )
}

fn format_allowed_profiles(locale: Option<&str>, profiles: Option<&Vec<String>>) -> String {
    match profiles {
        Some(values) if !values.is_empty() => values.join(", "),
        _ => t(locale, "commerce.common.all", "all"),
    }
}

fn format_known_shipping_profiles(locale: Option<&str>, profiles: &[ShippingProfile]) -> String {
    let slugs = profiles
        .iter()
        .filter(|profile| profile.active)
        .map(|profile| profile.slug.as_str())
        .collect::<Vec<_>>();
    if slugs.is_empty() {
        t(locale, "commerce.common.noneYet", "none yet")
    } else {
        slugs.join(", ")
    }
}

fn format_selected_shipping_profiles(locale: Option<&str>, value: &str) -> String {
    let slugs = csv_values(value);
    if slugs.is_empty() {
        t(locale, "commerce.common.allCarts", "all carts")
    } else {
        slugs.join(", ")
    }
}

fn shipping_profile_choice_label(locale: Option<&str>, profile: &ShippingProfile) -> String {
    if profile.active {
        format!("{} ({})", profile.name, profile.slug)
    } else {
        format!(
            "{} ({}, {})",
            profile.name,
            profile.slug,
            t(locale, "commerce.common.inactive", "inactive")
        )
    }
}

fn localized_product_status(locale: Option<&str>, status: &str) -> String {
    match status {
        "ACTIVE" => t(locale, "commerce.status.active", "Active"),
        "ARCHIVED" => t(locale, "commerce.status.archived", "Archived"),
        _ => t(locale, "commerce.status.draft", "Draft"),
    }
}

fn localized_active_label(locale: Option<&str>, active: bool) -> String {
    if active {
        t(locale, "commerce.common.active", "ACTIVE")
    } else {
        t(locale, "commerce.common.inactive", "INACTIVE")
    }
}

fn format_product_meta(locale: Option<&str>, handle: &str, vendor: Option<&str>) -> String {
    let handle_label = t(locale, "commerce.summary.product.handle", "handle");
    let vendor_label = t(locale, "commerce.summary.product.vendor", "vendor");
    match vendor.filter(|value| !value.is_empty()) {
        Some(vendor) => format!("{handle_label}: {handle} | {vendor_label}: {vendor}"),
        None => format!("{handle_label}: {handle}"),
    }
}

fn format_product_shipping_profile(locale: Option<&str>, slug: &str) -> String {
    t(
        locale,
        "commerce.summary.product.profileChip",
        "profile {slug}",
    )
    .replace("{slug}", slug)
}

fn shipping_profile_chip_class(selected: bool, inactive: bool) -> &'static str {
    match (selected, inactive) {
        (true, false) => "inline-flex rounded-full border border-primary bg-primary/10 px-3 py-2 text-xs font-medium text-primary transition hover:bg-primary/15",
        (true, true) => "inline-flex rounded-full border border-amber-300 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-700 transition hover:bg-amber-100",
        (false, true) => "inline-flex rounded-full border border-border bg-muted px-3 py-2 text-xs font-medium text-muted-foreground opacity-60",
        (false, false) => "inline-flex rounded-full border border-border bg-background px-3 py-2 text-xs font-medium text-foreground transition hover:bg-accent",
    }
}

fn toggle_csv_slug(current: &str, slug: &str) -> String {
    let slug = slug.trim();
    if slug.is_empty() {
        return current.trim().to_string();
    }
    let mut values = csv_values(current);
    if let Some(position) = values.iter().position(|value| value == slug) {
        values.remove(position);
    } else {
        values.push(slug.to_string());
        values.sort();
        values.dedup();
    }
    values.join(", ")
}

fn csv_contains(current: &str, slug: &str) -> bool {
    csv_values(current).iter().any(|value| value == slug)
}

fn csv_values(current: &str) -> Vec<String> {
    current
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn status_badge(status: &str) -> &'static str {
    match status {
        "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}

fn shipping_option_active_badge(active: bool) -> &'static str {
    if active {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    } else {
        "border-slate-200 bg-slate-100 text-slate-700"
    }
}

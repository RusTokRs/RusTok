from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text()


def write(path: str, text: str) -> None:
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


# Storefront host slot registry.
path = "apps/storefront/src/modules/registry.rs"
text = read(path)
text = replace_once(
    text,
    """pub enum StorefrontSlot {
    HomeAfterHero,
    HomeAfterCatalog,
    HomeBeforeFooter,
    CheckoutPaymentHandoff,
    CheckoutResultHandoff,
    CheckoutShippingHandoff,
}
""",
    """pub enum StorefrontSlot {
    HeaderNavigation,
    HomeAfterHero,
    HomeAfterCatalog,
    HomeBeforeFooter,
    FooterNavigation,
    CheckoutPaymentHandoff,
    CheckoutResultHandoff,
    CheckoutShippingHandoff,
}
""",
    "storefront slot enum",
)
write(path, text)


# Storefront build-time manifest contributions.
path = "apps/storefront/build.rs"
text = read(path)
text = replace_once(
    text,
    """struct LeptosUiContract {
    #[serde(default)]
    leptos_crate: Option<String>,
    #[serde(default)]
    slot: Option<String>,
    #[serde(default)]
    route_segment: Option<String>,
    #[serde(default)]
    page_title: Option<String>,
}
""",
    """struct LeptosUiContract {
    #[serde(default)]
    leptos_crate: Option<String>,
    #[serde(default)]
    slot: Option<String>,
    #[serde(default)]
    route_segment: Option<String>,
    #[serde(default)]
    page_title: Option<String>,
    #[serde(default)]
    components: Vec<StorefrontUiComponentContract>,
}

#[derive(Debug, Default, Deserialize)]
struct StorefrontUiComponentContract {
    #[serde(default)]
    id: String,
    #[serde(default)]
    component: String,
    #[serde(default)]
    slot: String,
    #[serde(default)]
    order: Option<usize>,
}
""",
    "storefront ui component manifest contract",
)
text = replace_once(
    text,
    """struct StorefrontUiEntry {
    slug: String,
    crate_ident: String,
    component_name: String,
    slot: StorefrontSlot,
    route_segment: String,
    page_title: String,
}
""",
    """struct StorefrontUiEntry {
    slug: String,
    crate_ident: String,
    component_name: String,
    slot: StorefrontSlot,
    route_segment: String,
    page_title: String,
    components: Vec<StorefrontUiComponentEntry>,
}

#[derive(Debug)]
struct StorefrontUiComponentEntry {
    id: String,
    component_name: String,
    slot: StorefrontSlot,
    order: usize,
}
""",
    "storefront ui entry components",
)
text = replace_once(
    text,
    """enum StorefrontSlot {
    HomeAfterHero,
    HomeAfterCatalog,
    HomeBeforeFooter,
    CheckoutPaymentHandoff,
    CheckoutResultHandoff,
    CheckoutShippingHandoff,
}
""",
    """enum StorefrontSlot {
    HeaderNavigation,
    HomeAfterHero,
    HomeAfterCatalog,
    HomeBeforeFooter,
    FooterNavigation,
    CheckoutPaymentHandoff,
    CheckoutResultHandoff,
    CheckoutShippingHandoff,
}
""",
    "build storefront slot enum",
)
text = replace_once(
    text,
    """        let slug = package_manifest.module.slug.clone();
        let name = package_manifest
            .module
            .name
            .clone()
            .unwrap_or_else(|| slug.clone());
        entries.push(StorefrontUiEntry {
            slug: slug.clone(),
            crate_ident: leptos_crate.replace('-', "_"),
            component_name: format!("{}View", pascal_case(&slug)),
            slot: storefront_slot_from_manifest(storefront_ui.slot.as_deref())?,
            route_segment: storefront_ui.route_segment.unwrap_or_else(|| slug.clone()),
            page_title: storefront_ui.page_title.unwrap_or(name),
        });
""",
    """        let slug = package_manifest.module.slug.clone();
        let name = package_manifest
            .module
            .name
            .clone()
            .unwrap_or_else(|| slug.clone());
        let mut component_ids = std::collections::BTreeSet::new();
        let components = storefront_ui
            .components
            .into_iter()
            .enumerate()
            .map(|(index, component)| {
                let id = required_manifest_value(
                    component.id,
                    format!("module '{slug}' storefront component id"),
                )?;
                if !component_ids.insert(id.clone()) {
                    return Err(format!(
                        "module '{slug}' declares duplicate storefront component id '{id}'"
                    )
                    .into());
                }
                let component_name = required_manifest_value(
                    component.component,
                    format!("module '{slug}' storefront component '{id}' Rust component"),
                )?;
                if !is_rust_component_path(component_name.as_str()) {
                    return Err(format!(
                        "module '{slug}' declares invalid storefront Rust component '{component_name}'"
                    )
                    .into());
                }
                Ok(StorefrontUiComponentEntry {
                    id,
                    component_name,
                    slot: storefront_slot_from_manifest(Some(component.slot.as_str()))?,
                    order: component.order.unwrap_or(1_000 + index),
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        entries.push(StorefrontUiEntry {
            slug: slug.clone(),
            crate_ident: leptos_crate.replace('-', "_"),
            component_name: format!("{}View", pascal_case(&slug)),
            slot: storefront_slot_from_manifest(storefront_ui.slot.as_deref())?,
            route_segment: storefront_ui.route_segment.unwrap_or_else(|| slug.clone()),
            page_title: storefront_ui.page_title.unwrap_or(name),
            components,
        });
""",
    "storefront module component collection",
)
text = replace_once(
    text,
    """        out.push_str(&format!(
            "    register_page(StorefrontPageRegistration {{ module_slug: \"{slug}\", route_segment: \"{route_segment}\", title: \"{title}\", render: {fn_name} }});\n",
            slug = entry.slug,
            route_segment = entry.route_segment,
            title = entry.page_title,
            fn_name = storefront_render_fn_name(&entry.slug),
        ));
""",
    """        out.push_str(&format!(
            "    register_page(StorefrontPageRegistration {{ module_slug: \"{slug}\", route_segment: \"{route_segment}\", title: \"{title}\", render: {fn_name} }});\n",
            slug = entry.slug,
            route_segment = entry.route_segment,
            title = entry.page_title,
            fn_name = storefront_render_fn_name(&entry.slug),
        ));
        for component in &entry.components {
            out.push_str(&format!(
                "    register_component(StorefrontComponentRegistration {{ id: \"{id}\", module_slug: Some(\"{slug}\"), slot: {slot_expr}, order: {order}, render: {fn_name} }});\n",
                id = component.id,
                slug = entry.slug,
                slot_expr = storefront_slot_expr(component.slot),
                order = component.order,
                fn_name = storefront_component_render_fn_name(&entry.slug, &component.id),
            ));
        }
""",
    "generated component registrations",
)
text = replace_once(
    text,
    """        out.push_str("}\n\n");
    }

    out
}
""",
    """        out.push_str("}\n\n");
        for component in &entry.components {
            let fn_name = storefront_component_render_fn_name(&entry.slug, &component.id);
            let component_path = format!("{}::{}", entry.crate_ident, component.component_name);
            out.push_str(&format!("fn {fn_name}() -> AnyView {{\n"));
            out.push_str("    view! {\n");
            out.push_str(&format!("        <{component_path} />\n"));
            out.push_str("    }\n");
            out.push_str("    .into_any()\n");
            out.push_str("}\n\n");
        }
    }

    out
}
""",
    "generated component render functions",
)
text = replace_once(
    text,
    """        "home_after_hero" => Ok(StorefrontSlot::HomeAfterHero),
        "home_after_catalog" => Ok(StorefrontSlot::HomeAfterCatalog),
        "home_before_footer" => Ok(StorefrontSlot::HomeBeforeFooter),
        "checkout_payment_handoff" => Ok(StorefrontSlot::CheckoutPaymentHandoff),
""",
    """        "header_navigation" => Ok(StorefrontSlot::HeaderNavigation),
        "home_after_hero" => Ok(StorefrontSlot::HomeAfterHero),
        "home_after_catalog" => Ok(StorefrontSlot::HomeAfterCatalog),
        "home_before_footer" => Ok(StorefrontSlot::HomeBeforeFooter),
        "footer_navigation" => Ok(StorefrontSlot::FooterNavigation),
        "checkout_payment_handoff" => Ok(StorefrontSlot::CheckoutPaymentHandoff),
""",
    "build slot parsing",
)
text = replace_once(
    text,
    """    match slot {
        StorefrontSlot::HomeAfterHero => "StorefrontSlot::HomeAfterHero",
        StorefrontSlot::HomeAfterCatalog => "StorefrontSlot::HomeAfterCatalog",
        StorefrontSlot::HomeBeforeFooter => "StorefrontSlot::HomeBeforeFooter",
        StorefrontSlot::CheckoutPaymentHandoff => "StorefrontSlot::CheckoutPaymentHandoff",
""",
    """    match slot {
        StorefrontSlot::HeaderNavigation => "StorefrontSlot::HeaderNavigation",
        StorefrontSlot::HomeAfterHero => "StorefrontSlot::HomeAfterHero",
        StorefrontSlot::HomeAfterCatalog => "StorefrontSlot::HomeAfterCatalog",
        StorefrontSlot::HomeBeforeFooter => "StorefrontSlot::HomeBeforeFooter",
        StorefrontSlot::FooterNavigation => "StorefrontSlot::FooterNavigation",
        StorefrontSlot::CheckoutPaymentHandoff => "StorefrontSlot::CheckoutPaymentHandoff",
""",
    "build slot expressions",
)
text = replace_once(
    text,
    """fn storefront_render_fn_name(slug: &str) -> String {
    format!("render_{}_storefront_view", slug.replace('-', "_"))
}

fn pascal_case(value: &str) -> String {
""",
    """fn storefront_render_fn_name(slug: &str) -> String {
    format!("render_{}_storefront_view", slug.replace('-', "_"))
}

fn storefront_component_render_fn_name(slug: &str, id: &str) -> String {
    format!(
        "render_{}_{}_storefront_component",
        rust_identifier_fragment(slug),
        rust_identifier_fragment(id),
    )
}

fn rust_identifier_fragment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn required_manifest_value(value: String, label: String) -> Result<String, Box<dyn Error>> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(format!("{label} must not be empty").into());
    }
    Ok(value)
}

fn is_rust_component_path(value: &str) -> bool {
    value.split("::").all(|segment| {
        let mut characters = segment.chars();
        characters
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
            && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
    })
}

fn pascal_case(value: &str) -> String {
""",
    "build component helper functions",
)
write(path, text)


# Host layout consumes header/footer FFA slots.
path = "apps/storefront/src/app/mod.rs"
text = read(path)
text = replace_once(
    text,
    """#[component]
fn StorefrontLayout(locale: String, body: AnyView) -> impl IntoView {
    let strings = locale_strings(locale.as_str());

    view! {
        <div class="min-h-screen bg-background text-foreground">
            <Header
                locale=locale.clone()
                nav_home=strings.nav_home
                nav_catalog=strings.nav_catalog
                nav_about=strings.nav_about
                nav_contact=strings.nav_contact
                nav_language=strings.nav_language
                cta_primary=strings.cta_primary
            />

            {body}

            <Footer tagline=strings.footer_tagline />
        </div>
    }
}
""",
    """#[component]
fn StorefrontLayout(locale: String, body: AnyView) -> impl IntoView {
    let strings = locale_strings(locale.as_str());
    let enabled_modules = use_enabled_modules().get_untracked();
    let header_navigation_views =
        components_for_slot(StorefrontSlot::HeaderNavigation, Some(&enabled_modules))
            .into_iter()
            .map(|module| (module.render)())
            .collect::<Vec<_>>();
    let footer_navigation_views =
        components_for_slot(StorefrontSlot::FooterNavigation, Some(&enabled_modules))
            .into_iter()
            .map(|module| (module.render)())
            .collect::<Vec<_>>();

    view! {
        <div class="min-h-screen bg-background text-foreground">
            <Header
                locale=locale.clone()
                nav_home=strings.nav_home
                nav_catalog=strings.nav_catalog
                nav_about=strings.nav_about
                nav_contact=strings.nav_contact
                nav_language=strings.nav_language
                cta_primary=strings.cta_primary
                navigation_views=header_navigation_views
            />

            {body}

            <Footer
                tagline=strings.footer_tagline
                navigation_views=footer_navigation_views
            />
        </div>
    }
}
""",
    "storefront layout slots",
)
write(path, text)


# Host header/fallback navigation.
write(
    "apps/storefront/src/widgets/header/mod.rs",
    """mod core;

use self::core::build_header_links;
use crate::shared::ui::UiButton;
use leptos::prelude::*;

#[component]
pub fn Header(
    locale: String,
    nav_home: &'static str,
    nav_catalog: &'static str,
    nav_about: &'static str,
    nav_contact: &'static str,
    nav_language: &'static str,
    cta_primary: &'static str,
    navigation_views: Vec<AnyView>,
) -> impl IntoView {
    let links = build_header_links(locale.as_str());
    let navigation = if navigation_views.is_empty() {
        view! {
            <nav class="hidden lg:flex items-center gap-6" aria-label="Primary navigation">
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#home">{nav_home}</a>
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#catalog">{nav_catalog}</a>
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#about">{nav_about}</a>
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#contact">{nav_contact}</a>
            </nav>
        }
        .into_any()
    } else {
        view! {
            <div class="contents">{navigation_views}</div>
        }
        .into_any()
    };

    view! {
        <header class="sticky top-0 z-40 border-b border-border bg-background/95 backdrop-blur">
            <div class="container-app flex h-14 w-full items-center px-4">
                <div class="flex-1">
                    <a class="text-xl font-bold text-foreground hover:text-primary transition-colors" href=links.home_href>
                        "RusToK"
                    </a>
                </div>
                {navigation}
                <div class="flex items-center gap-3 ml-6">
                    <div class="relative">
                        <details class="group">
                            <summary class="inline-flex items-center gap-1 rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground cursor-pointer hover:bg-accent hover:text-accent-foreground transition-colors list-none">
                                {nav_language}
                            </summary>
                            <ul class="absolute right-0 mt-1 w-32 rounded-md border border-border bg-popover p-1 shadow-md z-50">
                                <li>
                                    <a class="block rounded px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href=links.english_href.clone()>
                                        "English"
                                    </a>
                                </li>
                                <li>
                                    <a class="block rounded px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href=links.russian_href.clone()>
                                        "Русский"
                                    </a>
                                </li>
                            </ul>
                        </details>
                    </div>
                    <a href="#catalog">
                        <UiButton class="px-4 py-1.5 text-sm">
                            {cta_primary}
                        </UiButton>
                    </a>
                </div>
            </div>
        </header>
    }
}
""",
)


# Host footer slot.
write(
    "apps/storefront/src/widgets/footer/mod.rs",
    """use leptos::prelude::*;

#[component]
pub fn Footer(tagline: &'static str, navigation_views: Vec<AnyView>) -> impl IntoView {
    view! {
        <footer id="contact" class="mt-20 border-t border-border bg-muted/40 px-4 py-10">
            <div class="container-app space-y-5 text-center">
                {(!navigation_views.is_empty()).then(|| view! {
                    <div class="flex justify-center">{navigation_views}</div>
                })}
                <p class="text-sm text-muted-foreground">{tagline}</p>
                <div class="flex justify-center gap-3">
                    <span class="inline-flex items-center rounded-full bg-primary px-2.5 py-0.5 text-xs font-medium text-primary-foreground">"SSR"</span>
                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-medium text-secondary-foreground">"Tailwind"</span>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-foreground">"shadcn"</span>
                </div>
            </div>
        </footer>
    }
}
""",
)


# Pages manifest owns named layout contributions.
path = "crates/rustok-pages/rustok-module.toml"
text = read(path)
text = replace_once(
    text,
    """[provides.storefront_ui]
leptos_crate = "rustok-pages-storefront"
slot = "home_before_footer"
route_segment = "pages"
page_title = "Pages"

[provides.storefront_ui.i18n]
""",
    """[provides.storefront_ui]
leptos_crate = "rustok-pages-storefront"
slot = "home_before_footer"
route_segment = "pages"
page_title = "Pages"

[[provides.storefront_ui.components]]
id = "pages-header-navigation"
component = "PagesHeaderMenu"
slot = "header_navigation"
order = 100

[[provides.storefront_ui.components]]
id = "pages-footer-navigation"
component = "PagesFooterMenu"
slot = "footer_navigation"
order = 100

[provides.storefront_ui.i18n]
""",
    "pages storefront component manifest",
)
write(path, text)


# Xtask recognizes and validates named storefront contributions.
path = "xtask/src/xtask_types.rs"
text = read(path)
text = replace_once(
    text,
    """    #[serde(default)]
    pub(crate) page_title: Option<String>,
    #[serde(default)]
    pub(crate) i18n: Option<ModuleUiI18nProvides>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModuleUiI18nProvides {
""",
    """    #[serde(default)]
    pub(crate) page_title: Option<String>,
    #[serde(default)]
    pub(crate) components: Vec<ModuleUiComponentProvides>,
    #[serde(default)]
    pub(crate) i18n: Option<ModuleUiI18nProvides>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModuleUiComponentProvides {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) component: String,
    #[serde(default)]
    pub(crate) slot: String,
    #[serde(default)]
    pub(crate) order: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ModuleUiI18nProvides {
""",
    "xtask ui component types",
)
write(path, text)

path = "xtask/src/module_ui_metadata_contracts.rs"
text = read(path)
text = replace_once(
    text,
    """        validate_ui_surface_metadata_field(
            slug,
            "provides.storefront_ui.slot",
            storefront_ui.slot.as_deref(),
        )?;
        validate_ui_surface_metadata_field(
            slug,
            "provides.storefront_ui.page_title",
            storefront_ui.page_title.as_deref(),
        )?;
""",
    """        validate_ui_surface_metadata_field(
            slug,
            "provides.storefront_ui.slot",
            storefront_ui.slot.as_deref(),
        )?;
        validate_storefront_slot(
            slug,
            "provides.storefront_ui.slot",
            storefront_ui.slot.as_deref().unwrap_or_default(),
        )?;
        validate_ui_surface_metadata_field(
            slug,
            "provides.storefront_ui.page_title",
            storefront_ui.page_title.as_deref(),
        )?;
        let mut component_ids = std::collections::BTreeSet::new();
        for component in &storefront_ui.components {
            validate_ui_surface_metadata_field(
                slug,
                "provides.storefront_ui.components[].id",
                Some(component.id.as_str()),
            )?;
            validate_ui_surface_metadata_field(
                slug,
                "provides.storefront_ui.components[].component",
                Some(component.component.as_str()),
            )?;
            validate_ui_surface_metadata_field(
                slug,
                "provides.storefront_ui.components[].slot",
                Some(component.slot.as_str()),
            )?;
            validate_storefront_slot(
                slug,
                "provides.storefront_ui.components[].slot",
                component.slot.as_str(),
            )?;
            if !component_ids.insert(component.id.trim()) {
                anyhow::bail!(
                    "Module '{slug}' declares duplicate storefront component id '{}'",
                    component.id.trim()
                );
            }
            let _ = component.order;
        }
""",
    "xtask storefront component validation",
)
text = replace_once(
    text,
    """fn validate_ui_i18n_contract(
""",
    """fn validate_storefront_slot(slug: &str, field_name: &str, value: &str) -> Result<()> {
    match value.trim() {
        "header_navigation"
        | "home_after_hero"
        | "home_after_catalog"
        | "home_before_footer"
        | "footer_navigation"
        | "checkout_shipping_handoff"
        | "checkout_payment_handoff"
        | "checkout_result_handoff" => Ok(()),
        other => anyhow::bail!(
            "Module '{slug}' declares unsupported {field_name}='{other}'"
        ),
    }
}

fn validate_ui_i18n_contract(
""",
    "xtask storefront slot validator",
)
write(path, text)


# Document the current manifest contract.
path = "docs/modules/manifest.md"
text = read(path)
text = replace_once(
    text,
    """- `[provides.storefront_ui]` requires not only `leptos_crate`, but also non-empty `slot`, `route_segment`, `page_title` and `[provides.storefront_ui.i18n]` with `default_locale`, `supported_locales`, `leptos_locales_path`. The `slot` value must be one of the platform-known slots: `home_after_hero`, `home_after_catalog`, `home_before_footer`, `checkout_shipping_handoff`, `checkout_payment_handoff`, `checkout_result_handoff`.
""",
    """- `[provides.storefront_ui]` requires not only `leptos_crate`, but also non-empty `slot`, `route_segment`, `page_title` and `[provides.storefront_ui.i18n]` with `default_locale`, `supported_locales`, `leptos_locales_path`. The `slot` value must be one of the platform-known slots: `header_navigation`, `home_after_hero`, `home_after_catalog`, `home_before_footer`, `footer_navigation`, `checkout_shipping_handoff`, `checkout_payment_handoff`, `checkout_result_handoff`.
- `[[provides.storefront_ui.components]]` declares additional no-prop Leptos contributions from the same module UI crate. Each item requires a unique `id`, exported Rust `component`, platform-known `slot`, and may set deterministic `order`. The host registers these through the generic storefront registry; host source must not import module-specific layout components directly.
""",
    "manifest documentation slots",
)
write(path, text)


# Page showcase data no longer carries global layout menus.
path = "crates/rustok-pages/storefront/src/model.rs"
text = read(path)
text = replace_once(
    text,
    """pub struct StorefrontPagesData {
    pub selected_page: Option<PageDetail>,
    pub pages: PageList,
    #[serde(default, rename = "activeHeaderMenu")]
    pub active_header_menu: Option<StorefrontMenu>,
    #[serde(default, rename = "activeFooterMenu")]
    pub active_footer_menu: Option<StorefrontMenu>,
}
""",
    """pub struct StorefrontPagesData {
    pub selected_page: Option<PageDetail>,
    pub pages: PageList,
}
""",
    "remove bundled layout menus",
)
write(path, text)


# Dedicated GraphQL active-menu transport.
write(
    "crates/rustok-pages/storefront/src/transport/graphql_adapter.rs",
    """use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use super::{ApiError, configured_tenant_slug};
use crate::model::{
    PageDetail, PageList, StorefrontMenu, StorefrontMenuLocation, StorefrontPagesData,
};

const STOREFRONT_PAGES_QUERY: &str = r#"query StorefrontPages($pageSlug: String!, $filter: ListGqlPagesFilter, $locale: String) {
  selectedPage: pageBySlug(slug: $pageSlug, locale: $locale) {
    effectiveLocale
    translation { locale title slug metaTitle metaDescription }
    body { locale content format }
  }
  pages(filter: $filter) { total items { id title slug status template } }
}"#;

const STOREFRONT_ACTIVE_MENU_QUERY: &str = r#"query StorefrontActiveMenu($location: GqlMenuLocation!, $locale: String) {
  activeMenu(location: $location, locale: $locale) {
    id effectiveLocale name location
    items {
      id title url icon
      children {
        id title url icon
        children { id title url icon }
      }
    }
  }
}"#;

#[derive(Debug, Deserialize)]
struct StorefrontPagesResponse {
    #[serde(rename = "selectedPage")]
    selected_page: Option<PageDetail>,
    pages: PageList,
}

#[derive(Debug, Deserialize)]
struct StorefrontActiveMenuResponse {
    #[serde(rename = "activeMenu")]
    active_menu: Option<StorefrontMenu>,
}

#[derive(Debug, Serialize)]
struct StorefrontPagesVariables {
    #[serde(rename = "pageSlug")]
    page_slug: String,
    filter: ListPagesFilter,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct StorefrontActiveMenuVariables {
    location: StorefrontMenuLocation,
    locale: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ListPagesFilter {
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

pub async fn fetch_storefront_pages(
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    let response: StorefrontPagesResponse = request(
        STOREFRONT_PAGES_QUERY,
        StorefrontPagesVariables {
            page_slug,
            filter: ListPagesFilter {
                page: 1,
                per_page: 6,
            },
            locale,
        },
    )
    .await?;

    Ok(StorefrontPagesData {
        selected_page: response.selected_page,
        pages: response.pages,
    })
}

pub async fn fetch_active_menu(
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, ApiError> {
    let response: StorefrontActiveMenuResponse = request(
        STOREFRONT_ACTIVE_MENU_QUERY,
        StorefrontActiveMenuVariables { location, locale },
    )
    .await?;
    Ok(response.active_menu)
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}
""",
)


# Selected transport exposes a focused menu operation.
write(
    "crates/rustok-pages/storefront/src/transport/mod.rs",
    """mod graphql_adapter;
mod native_server_adapter;

use leptos::prelude::ServerFnError;
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{StorefrontMenu, StorefrontMenuLocation, StorefrontPagesData};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub type PagesTransportError = UiTransportError;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn fetch_pages(
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, PagesTransportError> {
    let native_page_slug = page_slug.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "pages",
        selected_transport_path(),
        move || {
            native_server_adapter::fetch_storefront_pages_server(
                configured_tenant_slug(),
                native_page_slug,
                native_locale,
            )
        },
        move || graphql_adapter::fetch_storefront_pages(page_slug, locale),
    )
    .await
}

pub async fn fetch_active_menu(
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, PagesTransportError> {
    let native_locale = locale.clone();
    execute_selected_transport(
        "pages",
        selected_transport_path(),
        move || {
            native_server_adapter::fetch_active_menu_server(
                configured_tenant_slug(),
                location,
                native_locale,
            )
        },
        move || graphql_adapter::fetch_active_menu(location, locale),
    )
    .await
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }
}
""",
)


# Native server transport separates layout menu reads from page caching.
path = "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs"
text = read(path)
text = replace_once(
    text,
    """use super::ApiError;
use crate::model::StorefrontPagesData;

#[cfg(feature = "ssr")]
use crate::model::{
    PageBody, PageDetail, PageList, PageListItem, PageTranslation, StorefrontMenu,
    StorefrontMenuItem, StorefrontMenuLocation,
};
""",
    """use super::ApiError;
use crate::model::{StorefrontMenu, StorefrontMenuLocation, StorefrontPagesData};

#[cfg(feature = "ssr")]
use crate::model::{
    PageBody, PageDetail, PageList, PageListItem, PageTranslation, StorefrontMenuItem,
};
""",
    "native model imports",
)
text = replace_once(
    text,
    """pub async fn fetch_storefront_pages_server(
    tenant_slug: Option<String>,
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    storefront_pages_native(tenant_slug, page_slug, locale)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "pages/storefront-data")]
""",
    """pub async fn fetch_storefront_pages_server(
    tenant_slug: Option<String>,
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    storefront_pages_native(tenant_slug, page_slug, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_active_menu_server(
    tenant_slug: Option<String>,
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, ApiError> {
    active_menu_native(tenant_slug, location, locale)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "pages/active-menu")]
async fn active_menu_native(
    tenant_slug: Option<String>,
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_channel::ChannelService;
        use rustok_core::SecurityContext;
        use rustok_outbox::TransactionalEventBus;
        use rustok_pages::{
            MENU_LOCALE_NOT_FOUND_ERROR_CODE, MenuBindingService, MenuLocation, PagesError,
        };
        use rustok_tenant::TenantService;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let Some(channel_id) = request_context.as_ref().and_then(|ctx| ctx.channel_id) else {
            return Ok(None);
        };
        let enabled = ChannelService::new(runtime_ctx.db_clone())
            .is_module_enabled(channel_id, MODULE_SLUG)
            .await
            .map_err(ServerFnError::new)?;
        if !enabled {
            return Ok(None);
        }

        let tenant_context = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .ok();
        let (tenant_id, fallback_locale) = if let Some(tenant) = tenant_context.as_ref() {
            (tenant.id, tenant.default_locale.clone())
        } else {
            let slug = tenant_slug
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ServerFnError::new(
                        "pages/active-menu requires tenant context or tenant slug",
                    )
                })?;
            let tenant = TenantService::new(runtime_ctx.db_clone())
                .get_tenant_by_slug(slug)
                .await
                .map_err(ServerFnError::new)?;
            let fallback = request_context
                .as_ref()
                .map(|ctx| ctx.locale.clone())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
            (tenant.id, fallback)
        };
        let requested_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| request_context.as_ref().map(|ctx| ctx.locale.clone()))
            .unwrap_or(fallback_locale);
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "pages/active-menu requires TransactionalEventBus in host runtime context",
                )
            })?;
        let location = match location {
            StorefrontMenuLocation::Header => MenuLocation::Header,
            StorefrontMenuLocation::Footer => MenuLocation::Footer,
            StorefrontMenuLocation::Sidebar => MenuLocation::Sidebar,
            StorefrontMenuLocation::Mobile => MenuLocation::Mobile,
        };

        match MenuBindingService::new(runtime_ctx.db_clone(), event_bus)
            .get_active(
                tenant_id,
                SecurityContext::public_read(),
                channel_id,
                location,
                requested_locale.as_str(),
            )
            .await
        {
            Ok(menu) => Ok(menu.map(map_storefront_menu)),
            Err(PagesError::MenuNotFound(_)) => Ok(None),
            Err(PagesError::Rich(rich))
                if rich.error_code.as_deref() == Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE) =>
            {
                Ok(None)
            }
            Err(error) => Err(ServerFnError::new(error)),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_slug, location, locale);
        Err(ServerFnError::new(
            "pages/active-menu requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pages/storefront-data")]
""",
    "native active menu server function",
)
text = replace_once(
    text,
    """        use rustok_pages::{
            ListPagesFilter as RuntimeListPagesFilter, MENU_LOCALE_NOT_FOUND_ERROR_CODE,
            MenuBindingService, MenuLocation, PageBuilderArtifactService, PageService,
            PagesCacheReadRuntime, PagesError, storefront_pages_cache_key,
        };
""",
    """        use rustok_pages::{
            ListPagesFilter as RuntimeListPagesFilter, PageBuilderArtifactService, PageService,
            PagesCacheReadRuntime, storefront_pages_cache_key,
        };
""",
    "native page imports",
)
start = text.index("        let (active_header_menu, active_footer_menu) =")
end = text.index("        let cache_runtime =", start)
text = text[:start] + text[end:]
text = replace_once(
    text,
    """                Ok(Some(mut cached)) => {
                    tracing::debug!(%tenant_id, "Pages storefront cache hit");
                    cached.active_header_menu = active_header_menu.clone();
                    cached.active_footer_menu = active_footer_menu.clone();
                    return Ok(cached);
                }
""",
    """                Ok(Some(cached)) => {
                    tracing::debug!(%tenant_id, "Pages storefront cache hit");
                    return Ok(cached);
                }
""",
    "native cache hit",
)
text = replace_once(
    text,
    """        let data = StorefrontPagesData {
            selected_page,
            pages: PageList {
                items: items.into_iter().map(map_page_list_item).collect(),
                total,
            },
            active_header_menu,
            active_footer_menu,
        };
""",
    """        let data = StorefrontPagesData {
            selected_page,
            pages: PageList {
                items: items.into_iter().map(map_page_list_item).collect(),
                total,
            },
        };
""",
    "native page data",
)
text = replace_once(
    text,
    """        if let (Some(cache_runtime), Some(cache_key)) =
            (cache_runtime.as_ref(), cache_key)
        {
            let mut cached_data = data.clone();
            cached_data.active_header_menu = None;
            cached_data.active_footer_menu = None;
            if let Err(error) = cache_runtime.put_json(cache_key, &cached_data).await {
                tracing::warn!(%error, %tenant_id, "Pages storefront cache fill failed");
            }
        }
""",
    """        if let (Some(cache_runtime), Some(cache_key)) =
            (cache_runtime.as_ref(), cache_key)
        {
            if let Err(error) = cache_runtime.put_json(cache_key, &data).await {
                tracing::warn!(%error, %tenant_id, "Pages storefront cache fill failed");
            }
        }
""",
    "native page cache fill",
)
write(path, text)


# Pages-owned header/footer views.
write(
    "crates/rustok-pages/storefront/src/ui/menu.rs",
    """use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::model::{StorefrontMenu, StorefrontMenuItem, StorefrontMenuLocation};
use crate::transport;

#[derive(Clone, Copy)]
enum MenuPresentation {
    Header,
    Footer,
}

#[component]
pub fn PagesHeaderMenu() -> impl IntoView {
    view! {
        <ActiveMenu
            location=StorefrontMenuLocation::Header
            presentation=MenuPresentation::Header
        />
    }
}

#[component]
pub fn PagesFooterMenu() -> impl IntoView {
    view! {
        <ActiveMenu
            location=StorefrontMenuLocation::Footer
            presentation=MenuPresentation::Footer
        />
    }
}

#[component]
fn ActiveMenu(
    location: StorefrontMenuLocation,
    presentation: MenuPresentation,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let menu_resource = Resource::new_blocking(
        move || locale.clone(),
        move |locale| async move { transport::fetch_active_menu(location, locale).await },
    );

    view! {
        <Suspense fallback=|| ()>
            {move || {
                let menu_resource = menu_resource;
                Suspend::new(async move {
                    match menu_resource.await {
                        Ok(Some(menu)) => render_menu(menu, presentation),
                        Ok(None) | Err(_) => view! { <span class="hidden"></span> }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}

fn render_menu(menu: StorefrontMenu, presentation: MenuPresentation) -> AnyView {
    match presentation {
        MenuPresentation::Header => render_header_menu(menu),
        MenuPresentation::Footer => render_footer_menu(menu),
    }
}

fn render_header_menu(menu: StorefrontMenu) -> AnyView {
    let label = menu.name;
    view! {
        <nav class="hidden lg:flex items-center" aria-label=label>
            <ul class="flex items-center gap-6">
                {menu.items.into_iter().map(render_header_item).collect_view()}
            </ul>
        </nav>
    }
    .into_any()
}

fn render_header_item(item: StorefrontMenuItem) -> AnyView {
    if item.children.is_empty() {
        return view! {
            <li>
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href=item.url>
                    {item.title}
                </a>
            </li>
        }
        .into_any();
    }

    view! {
        <li class="relative">
            <details class="group">
                <summary class="cursor-pointer list-none text-sm text-muted-foreground hover:text-foreground transition-colors">
                    {item.title}
                </summary>
                <ul class="absolute left-0 z-50 mt-2 min-w-48 space-y-1 rounded-md border border-border bg-popover p-2 shadow-md">
                    {item.children.into_iter().map(render_header_child).collect_view()}
                </ul>
            </details>
        </li>
    }
    .into_any()
}

fn render_header_child(item: StorefrontMenuItem) -> AnyView {
    let nested = (!item.children.is_empty()).then(|| {
        view! {
            <ul class="mt-1 border-l border-border pl-3">
                {item.children.into_iter().map(render_header_child).collect_view()}
            </ul>
        }
    });
    view! {
        <li>
            <a class="block rounded px-3 py-2 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground" href=item.url>
                {item.title}
            </a>
            {nested}
        </li>
    }
    .into_any()
}

fn render_footer_menu(menu: StorefrontMenu) -> AnyView {
    let label = menu.name;
    view! {
        <nav aria-label=label>
            <ul class="flex flex-wrap justify-center gap-x-6 gap-y-3">
                {menu.items.into_iter().map(render_footer_item).collect_view()}
            </ul>
        </nav>
    }
    .into_any()
}

fn render_footer_item(item: StorefrontMenuItem) -> AnyView {
    let nested = (!item.children.is_empty()).then(|| {
        view! {
            <ul class="mt-2 space-y-1">
                {item.children.into_iter().map(render_footer_item).collect_view()}
            </ul>
        }
    });
    view! {
        <li>
            <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href=item.url>
                {item.title}
            </a>
            {nested}
        </li>
    }
    .into_any()
}
""",
)
write(
    "crates/rustok-pages/storefront/src/ui/mod.rs",
    """pub mod leptos;
mod menu;

pub use menu::{PagesFooterMenu, PagesHeaderMenu};
""",
)
path = "crates/rustok-pages/storefront/src/lib.rs"
text = read(path)
text = replace_once(
    text,
    """pub use ui::leptos::PagesView;
""",
    """pub use ui::leptos::PagesView;
pub use ui::{PagesFooterMenu, PagesHeaderMenu};
""",
    "pages storefront component exports",
)
write(path, text)


# Current-only source guards for the dedicated transport and manifest slots.
write(
    "crates/rustok-pages/storefront/tests/active_menu_transport_contract.rs",
    """const MODEL: &str = include_str!("../src/model.rs");
const GRAPHQL: &str = include_str!("../src/transport/graphql_adapter.rs");
const NATIVE: &str = include_str!("../src/transport/native_server_adapter.rs");
const TRANSPORT: &str = include_str!("../src/transport/mod.rs");
const MENU_UI: &str = include_str!("../src/ui/menu.rs");
const MANIFEST: &str = include_str!("../../rustok-module.toml");

#[test]
fn active_menus_use_dedicated_current_channel_layout_transport() {
    for marker in [
        "query StorefrontActiveMenu",
        "activeMenu(location: $location",
        "StorefrontActiveMenuVariables",
        "pub async fn fetch_active_menu(",
    ] {
        assert!(
            GRAPHQL.contains(marker) || TRANSPORT.contains(marker),
            "dedicated active-menu transport must contain `{marker}`"
        );
    }

    for marker in [
        "endpoint = \"pages/active-menu\"",
        "async fn active_menu_native(",
        "MenuBindingService::new",
        ".get_active(",
        "request_context.as_ref().and_then(|ctx| ctx.channel_id)",
        "MENU_LOCALE_NOT_FOUND_ERROR_CODE",
    ] {
        assert!(
            NATIVE.contains(marker),
            "native active-menu transport must contain `{marker}`"
        );
    }

    for marker in [
        "pub fn PagesHeaderMenu()",
        "pub fn PagesFooterMenu()",
        "StorefrontMenuLocation::Header",
        "StorefrontMenuLocation::Footer",
    ] {
        assert!(MENU_UI.contains(marker), "menu UI must contain `{marker}`");
    }

    for marker in [
        "component = \"PagesHeaderMenu\"",
        "slot = \"header_navigation\"",
        "component = \"PagesFooterMenu\"",
        "slot = \"footer_navigation\"",
    ] {
        assert!(MANIFEST.contains(marker), "manifest must contain `{marker}`");
    }

    assert!(!GRAPHQL.contains("activeHeaderMenu: activeMenu"));
    assert!(!GRAPHQL.contains("activeFooterMenu: activeMenu"));
    assert!(!MODEL.contains("active_header_menu:"));
    assert!(!MODEL.contains("active_footer_menu:"));
    assert!(!NATIVE.contains("menu::Column::Location"));
    assert!(!NATIVE.contains(".first()"));
}
""",
)
write(
    "apps/storefront/tests/pages_menu_layout_slots_contract.rs",
    """const APP: &str = include_str!("../src/app/mod.rs");
const BUILD: &str = include_str!("../build.rs");
const REGISTRY: &str = include_str!("../src/modules/registry.rs");
const HEADER: &str = include_str!("../src/widgets/header/mod.rs");
const FOOTER: &str = include_str!("../src/widgets/footer/mod.rs");

#[test]
fn storefront_host_places_generic_header_and_footer_contributions() {
    for marker in [
        "HeaderNavigation",
        "FooterNavigation",
        "components_for_slot(StorefrontSlot::HeaderNavigation",
        "components_for_slot(StorefrontSlot::FooterNavigation",
    ] {
        assert!(
            REGISTRY.contains(marker) || APP.contains(marker),
            "storefront host slot contract must contain `{marker}`"
        );
    }

    for marker in [
        "components: Vec<StorefrontUiComponentContract>",
        "storefront_component_render_fn_name",
        "header_navigation",
        "footer_navigation",
    ] {
        assert!(BUILD.contains(marker), "storefront codegen must contain `{marker}`");
    }

    assert!(HEADER.contains("navigation_views: Vec<AnyView>"));
    assert!(FOOTER.contains("navigation_views: Vec<AnyView>"));
    assert!(!APP.contains("rustok_pages_storefront::PagesHeaderMenu"));
    assert!(!APP.contains("rustok_pages_storefront::PagesFooterMenu"));
}
""",
)

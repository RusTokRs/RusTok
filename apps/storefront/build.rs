use leptos_i18n_build::{Config, TranslationsInfos};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct ModulesManifest {
    #[serde(default)]
    modules: BTreeMap<String, ModuleSpec>,
}

#[derive(Debug, Deserialize)]
struct ModuleSpec {
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModulePackageManifest {
    module: ModuleMetadata,
    #[serde(default)]
    provides: ModuleProvides,
}

#[derive(Debug, Deserialize)]
struct ModuleMetadata {
    slug: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ModuleProvides {
    #[serde(default)]
    storefront_ui: Option<LeptosUiContract>,
}

#[derive(Debug, Default, Deserialize)]
struct LeptosUiContract {
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

#[derive(Debug)]
struct StorefrontUiEntry {
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

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy)]
enum StorefrontSlot {
    HeaderNavigation,
    HomeAfterHero,
    HomeAfterCatalog,
    HomeBeforeFooter,
    FooterNavigation,
    CheckoutPaymentHandoff,
    CheckoutResultHandoff,
    CheckoutShippingHandoff,
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=Cargo.toml");

    let i18n_mod_directory = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("i18n");
    let cfg = Config::new("en")?.add_locale("ru")?;
    let translations_infos = TranslationsInfos::parse(cfg)?;
    translations_infos.emit_diagnostics();
    translations_infos.rerun_if_locales_changed();
    translations_infos.generate_i18n_module(i18n_mod_directory)?;

    generate_storefront_module_codegen()?;

    Ok(())
}

fn generate_storefront_module_codegen() -> Result<(), Box<dyn Error>> {
    if std::env::var_os("CARGO_FEATURE_CSR").is_some()
        && std::env::var_os("CARGO_FEATURE_SSR").is_none()
    {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
        fs::write(
            out_dir.join("module_ui_codegen.rs"),
            empty_storefront_codegen(),
        )?;
        return Ok(());
    }

    let manifest_path = workspace_root().join("modules.toml");
    println!("cargo::rerun-if-changed={}", manifest_path.display());

    let storefront_cargo_toml =
        fs::read_to_string(workspace_root().join("apps/storefront/Cargo.toml"))?;

    let modules: ModulesManifest = toml::from_str(&fs::read_to_string(&manifest_path)?)?;
    let mut entries = Vec::new();

    for spec in modules.modules.into_values() {
        let Some(module_root) = spec.path.map(|value| workspace_root().join(value)) else {
            continue;
        };
        let package_manifest_path = module_root.join("rustok-module.toml");
        if !package_manifest_path.exists() {
            continue;
        }
        println!(
            "cargo::rerun-if-changed={}",
            package_manifest_path.display()
        );

        let package_manifest: ModulePackageManifest =
            toml::from_str(&fs::read_to_string(&package_manifest_path)?)?;
        validate_storefront_ui_wiring(&module_root, &package_manifest)?;
        let Some(storefront_ui) = package_manifest.provides.storefront_ui else {
            continue;
        };
        let Some(leptos_crate) = storefront_ui.leptos_crate.as_deref() else {
            continue;
        };

        if !storefront_cargo_toml.contains(&format!("{} =", leptos_crate))
            && !storefront_cargo_toml.contains(&format!("\"{}\"", leptos_crate))
        {
            continue;
        }

        let slug = package_manifest.module.slug.clone();
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
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    fs::write(
        out_dir.join("module_ui_codegen.rs"),
        render_storefront_codegen(&entries),
    )?;

    Ok(())
}

fn empty_storefront_codegen() -> String {
    "pub fn register_generated_components() {}\n".to_string()
}

fn validate_storefront_ui_wiring(
    module_root: &Path,
    package_manifest: &ModulePackageManifest,
) -> Result<(), Box<dyn Error>> {
    let ui_manifest_path = module_root.join("storefront").join("Cargo.toml");
    let declared_crate = package_manifest
        .provides
        .storefront_ui
        .as_ref()
        .and_then(|ui| ui.leptos_crate.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if ui_manifest_path.exists() && declared_crate.is_none() {
        return Err(format!(
            "module '{}' contains {}, but rustok-module.toml is missing [provides.storefront_ui].leptos_crate",
            package_manifest.module.slug,
            ui_manifest_path.display()
        )
        .into());
    }

    if !ui_manifest_path.exists() && declared_crate.is_some() {
        return Err(format!(
            "module '{}' declares [provides.storefront_ui].leptos_crate, but {} is missing",
            package_manifest.module.slug,
            ui_manifest_path.display()
        )
        .into());
    }

    Ok(())
}

fn render_storefront_codegen(entries: &[StorefrontUiEntry]) -> String {
    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use crate::modules::{register_component, register_page, StorefrontComponentRegistration, StorefrontPageRegistration, StorefrontSlot};\n\n");
    out.push_str("pub fn register_generated_components() {\n");
    for (index, entry) in entries.iter().enumerate() {
        out.push_str(&format!(
            "    register_component(StorefrontComponentRegistration {{ id: \"{slug}-slot\", module_slug: Some(\"{slug}\"), slot: {slot_expr}, order: {order}, render: {fn_name} }});\n",
            slug = entry.slug,
            slot_expr = storefront_slot_expr(entry.slot),
            order = 100 + index,
            fn_name = storefront_render_fn_name(&entry.slug),
        ));
        out.push_str(&format!(
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
    }
    out.push_str("}\n\n");

    for entry in entries {
        let fn_name = storefront_render_fn_name(&entry.slug);
        let component_path = if entry.slug == "search" {
            "crate::modules::SearchStorefrontComposition".to_string()
        } else {
            format!("{}::{}", entry.crate_ident, entry.component_name)
        };
        out.push_str(&format!(
            "fn {fn_name}() -> AnyView {{\n",
            fn_name = fn_name
        ));
        out.push_str("    view! {\n");
        out.push_str(&format!(
            "        <{component_path} />\n",
            component_path = component_path,
        ));
        out.push_str("    }\n");
        out.push_str("    .into_any()\n");
        out.push_str("}\n\n");
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

fn storefront_slot_from_manifest(raw: Option<&str>) -> Result<StorefrontSlot, Box<dyn Error>> {
    match raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home_after_hero")
    {
        "header_navigation" => Ok(StorefrontSlot::HeaderNavigation),
        "home_after_hero" => Ok(StorefrontSlot::HomeAfterHero),
        "home_after_catalog" => Ok(StorefrontSlot::HomeAfterCatalog),
        "home_before_footer" => Ok(StorefrontSlot::HomeBeforeFooter),
        "footer_navigation" => Ok(StorefrontSlot::FooterNavigation),
        "checkout_payment_handoff" => Ok(StorefrontSlot::CheckoutPaymentHandoff),
        "checkout_result_handoff" => Ok(StorefrontSlot::CheckoutResultHandoff),
        "checkout_shipping_handoff" => Ok(StorefrontSlot::CheckoutShippingHandoff),
        other => Err(format!("unsupported storefront slot `{other}`").into()),
    }
}

fn storefront_slot_expr(slot: StorefrontSlot) -> &'static str {
    match slot {
        StorefrontSlot::HeaderNavigation => "StorefrontSlot::HeaderNavigation",
        StorefrontSlot::HomeAfterHero => "StorefrontSlot::HomeAfterHero",
        StorefrontSlot::HomeAfterCatalog => "StorefrontSlot::HomeAfterCatalog",
        StorefrontSlot::HomeBeforeFooter => "StorefrontSlot::HomeBeforeFooter",
        StorefrontSlot::FooterNavigation => "StorefrontSlot::FooterNavigation",
        StorefrontSlot::CheckoutPaymentHandoff => "StorefrontSlot::CheckoutPaymentHandoff",
        StorefrontSlot::CheckoutResultHandoff => "StorefrontSlot::CheckoutResultHandoff",
        StorefrontSlot::CheckoutShippingHandoff => "StorefrontSlot::CheckoutShippingHandoff",
    }
}

fn storefront_render_fn_name(slug: &str) -> String {
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
    value
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from apps/storefront")
}

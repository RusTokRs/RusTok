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
    #[serde(default)]
    required: bool,
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
    name: String,
    #[serde(default = "default_module_ownership")]
    ownership: String,
    #[serde(default = "default_module_trust_level")]
    trust_level: String,
    #[serde(default)]
    recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    showcase_admin_surfaces: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ModuleProvides {
    #[serde(default)]
    admin_ui: Option<LeptosUiContract>,
}

#[derive(Debug, Default, Deserialize)]
struct LeptosUiContract {
    #[serde(default)]
    leptos_crate: Option<String>,
    #[serde(default)]
    route_segment: Option<String>,
    #[serde(default)]
    nav_label: Option<String>,
    #[serde(default)]
    pages: Vec<AdminNestedPageContract>,
}

#[derive(Debug, Default, Deserialize)]
struct AdminNestedPageContract {
    subpath: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    nav_label: Option<String>,
}

#[derive(Debug)]
struct AdminUiEntry {
    slug: String,
    name: String,
    crate_ident: String,
    component_name: String,
    route_segment: String,
    nav_label: String,
    child_pages: Vec<AdminChildPageEntry>,
}

#[derive(Debug)]
struct CoreModuleEntry {
    slug: String,
}

#[derive(Debug)]
struct ModuleRuntimeMetadataEntry {
    slug: String,
    ownership: String,
    trust_level: String,
    recommended_admin_surfaces: Vec<String>,
    showcase_admin_surfaces: Vec<String>,
}

#[derive(Debug)]
struct AdminChildPageEntry {
    subpath: String,
    title: String,
    nav_label: String,
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

    generate_admin_module_codegen()?;

    Ok(())
}

fn generate_admin_module_codegen() -> Result<(), Box<dyn Error>> {
    let manifest_path = workspace_root().join("modules.toml");
    println!("cargo::rerun-if-changed={}", manifest_path.display());

    let modules: ModulesManifest = toml::from_str(&fs::read_to_string(&manifest_path)?)?;
    let mut entries = Vec::new();
    let mut core_modules = Vec::new();
    let mut metadata_entries = Vec::new();

    for spec in modules.modules.into_values() {
        if spec.required {
            if let Some(module_root) = spec.path.as_ref().map(|value| workspace_root().join(value))
            {
                let package_manifest_path = module_root.join("rustok-module.toml");
                if package_manifest_path.exists() {
                    let package_manifest: ModulePackageManifest =
                        toml::from_str(&fs::read_to_string(&package_manifest_path)?)?;
                    core_modules.push(CoreModuleEntry {
                        slug: package_manifest.module.slug,
                    });
                }
            }
        }

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
        metadata_entries.push(ModuleRuntimeMetadataEntry {
            slug: package_manifest.module.slug.clone(),
            ownership: package_manifest.module.ownership.clone(),
            trust_level: package_manifest.module.trust_level.clone(),
            recommended_admin_surfaces: package_manifest.module.recommended_admin_surfaces.clone(),
            showcase_admin_surfaces: package_manifest.module.showcase_admin_surfaces.clone(),
        });
        let Some(admin_ui) = package_manifest.provides.admin_ui else {
            continue;
        };
        let Some(leptos_crate) = admin_ui.leptos_crate else {
            continue;
        };

        let slug = package_manifest.module.slug;
        let name = package_manifest.module.name;
        entries.push(AdminUiEntry {
            component_name: format!("{}Admin", pascal_case(&slug)),
            route_segment: admin_ui.route_segment.unwrap_or_else(|| slug.clone()),
            nav_label: admin_ui.nav_label.unwrap_or_else(|| name.clone()),
            child_pages: admin_ui
                .pages
                .into_iter()
                .filter_map(|page| {
                    let subpath = page.subpath.trim_matches('/').to_string();
                    if subpath.is_empty() {
                        return None;
                    }
                    let fallback = title_case(subpath.rsplit('/').next().unwrap_or("page"));
                    Some(AdminChildPageEntry {
                        title: page.title.unwrap_or_else(|| fallback.clone()),
                        nav_label: page.nav_label.unwrap_or_else(|| fallback.clone()),
                        subpath,
                    })
                })
                .collect(),
            slug,
            name,
            crate_ident: leptos_crate.replace('-', "_"),
        });
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    fs::write(
        out_dir.join("module_registry_codegen.rs"),
        render_admin_registry_codegen(&entries, &core_modules, &metadata_entries),
    )?;

    Ok(())
}

fn render_admin_registry_codegen(
    entries: &[AdminUiEntry],
    core_modules: &[CoreModuleEntry],
    metadata_entries: &[ModuleRuntimeMetadataEntry],
) -> String {
    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n");
    out.push_str(
        "use crate::app::modules::{register_component, register_page, AdminChildPageRegistration, AdminComponentRegistration, AdminPageRegistration, AdminSlot};\n\n",
    );
    out.push_str("pub fn core_module_slugs() -> &'static [&'static str] {\n");
    out.push_str("    &[\n");
    for module in core_modules {
        out.push_str(&format!("        \"{}\",\n", module.slug));
    }
    out.push_str("    ]\n}\n\n");

    for entry in entries {
        if entry.child_pages.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "const {const_name}: &[AdminChildPageRegistration] = &[\n",
            const_name = admin_child_pages_const_name(&entry.slug),
        ));
        for page in &entry.child_pages {
            out.push_str(&format!(
                "    AdminChildPageRegistration {{ subpath: \"{subpath}\", title: \"{title}\", nav_label: \"{nav_label}\" }},\n",
                subpath = page.subpath,
                title = page.title,
                nav_label = page.nav_label,
            ));
        }
        out.push_str("];\n\n");
    }

    out.push_str("pub fn module_runtime_metadata(slug: &str) -> Option<super::GeneratedModuleRuntimeMetadata> {\n");
    out.push_str("    match slug {\n");
    for entry in metadata_entries {
        let recommended = if entry.recommended_admin_surfaces.is_empty() {
            "&[]".to_string()
        } else {
            format!(
                "&[{}]",
                entry.recommended_admin_surfaces
                    .iter()
                    .map(|surface| format!("\"{surface}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let showcase = if entry.showcase_admin_surfaces.is_empty() {
            "&[]".to_string()
        } else {
            format!(
                "&[{}]",
                entry.showcase_admin_surfaces
                    .iter()
                    .map(|surface| format!("\"{surface}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        out.push_str(&format!(
            "        \"{slug}\" => Some(super::GeneratedModuleRuntimeMetadata {{ ownership: \"{ownership}\", trust_level: \"{trust_level}\", recommended_admin_surfaces: {recommended}, showcase_admin_surfaces: {showcase} }}),\n",
            slug = entry.slug,
            ownership = entry.ownership,
            trust_level = entry.trust_level,
            recommended = recommended,
            showcase = showcase,
        ));
    }
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("pub fn register_generated_components() {\n");
    if entries.is_empty() {
        out.push_str("}\n\n");
    } else {
        for (index, entry) in entries.iter().enumerate() {
            out.push_str(&format!(
                "    register_component(AdminComponentRegistration {{ id: \"{slug}-dashboard\", module_slug: Some(\"{slug}\"), slot: AdminSlot::DashboardSection, order: {order}, render: {fn_name} }});\n",
                slug = entry.slug,
                order = 100 + index,
                fn_name = admin_render_fn_name(&entry.slug),
            ));
            out.push_str(&format!(
                "    register_page(AdminPageRegistration {{ module_slug: \"{slug}\", route_segment: \"{route_segment}\", title: \"{title}\", child_pages: {child_pages}, render: {page_fn} }});\n",
                slug = entry.slug,
                route_segment = entry.route_segment,
                title = entry.nav_label,
                child_pages = if entry.child_pages.is_empty() {
                    "&[]".to_string()
                } else {
                    admin_child_pages_const_name(&entry.slug)
                },
                page_fn = admin_page_render_fn_name(&entry.slug),
            ));
            out.push_str(&format!(
                "    register_component(AdminComponentRegistration {{ id: \"{slug}-nav\", module_slug: Some(\"{slug}\"), slot: AdminSlot::NavItem, order: {order}, render: {fn_name} }});\n",
                slug = entry.slug,
                order = 200 + index,
                fn_name = admin_nav_render_fn_name(&entry.slug),
            ));
        }
        out.push_str("}\n\n");
    }

    for entry in entries {
        let fn_name = admin_render_fn_name(&entry.slug);
        out.push_str(&format!(
            "fn {fn_name}() -> AnyView {{\n",
            fn_name = fn_name
        ));
        out.push_str("    view! {\n");
        out.push_str(
            "        <section class=\"rounded-xl border border-border bg-card p-6 shadow-sm\">\n",
        );
        out.push_str("            <div class=\"mb-4 flex items-center justify-between gap-3\">\n");
        out.push_str(&format!(
            "                <h3 class=\"text-lg font-semibold text-card-foreground\">\"{label}\"</h3>\n",
            label = entry.name,
        ));
        out.push_str(&format!(
            "                <span class=\"inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground\">\"{slug}\"</span>\n",
            slug = entry.slug,
        ));
        out.push_str("            </div>\n");
        out.push_str(&format!(
            "            <{crate_ident}::{component_name} />\n",
            crate_ident = entry.crate_ident,
            component_name = entry.component_name,
        ));
        out.push_str("        </section>\n");
        out.push_str("    }\n");
        out.push_str("    .into_any()\n");
        out.push_str("}\n\n");

        out.push_str(&format!(
            "fn {fn_name}() -> AnyView {{\n",
            fn_name = admin_page_render_fn_name(&entry.slug)
        ));
        out.push_str("    view! {\n");
        out.push_str(&format!(
            "        <{crate_ident}::{component_name} />\n",
            crate_ident = entry.crate_ident,
            component_name = entry.component_name,
        ));
        out.push_str("    }\n");
        out.push_str("    .into_any()\n");
        out.push_str("}\n\n");

        out.push_str(&format!(
            "fn {fn_name}() -> AnyView {{\n",
            fn_name = admin_nav_render_fn_name(&entry.slug)
        ));
        out.push_str("    use leptos_router::components::A;\n");
        out.push_str("    use leptos_router::hooks::use_location;\n\n");
        out.push_str("    let location = use_location();\n");
        out.push_str(&format!(
            "    let is_active = move || location.pathname.get().starts_with(\"/modules/{route_segment}\");\n\n",
            route_segment = entry.route_segment,
        ));
        out.push_str("    view! {\n");
        out.push_str("        <A\n");
        out.push_str(&format!(
            "            href=\"/modules/{route_segment}\"\n",
            route_segment = entry.route_segment,
        ));
        out.push_str(
            "            attr:class=move || format!(\n                \"flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-all hover:bg-accent hover:text-accent-foreground {}\",\n                if is_active() { \"bg-accent text-accent-foreground shadow-sm\" } else { \"text-muted-foreground\" }\n            )\n",
        );
        out.push_str("        >\n");
        out.push_str("            <svg class=\"h-4 w-4 shrink-0\" fill=\"none\" viewBox=\"0 0 24 24\" stroke=\"currentColor\" stroke-width=\"2\">\n");
        out.push_str("                <path stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"M4 7h16M4 12h16M4 17h10\" />\n");
        out.push_str("            </svg>\n");
        out.push_str(&format!(
            "            \"{label}\"\n",
            label = entry.nav_label,
        ));
        out.push_str("        </A>\n");
        out.push_str("    }\n");
        out.push_str("    .into_any()\n");
        out.push_str("}\n\n");
    }

    out
}

fn default_module_ownership() -> String {
    "third_party".to_string()
}

fn default_module_trust_level() -> String {
    "unverified".to_string()
}

fn admin_render_fn_name(slug: &str) -> String {
    format!("render_{}_dashboard_section", slug.replace('-', "_"))
}

fn admin_page_render_fn_name(slug: &str) -> String {
    format!("render_{}_admin_page", slug.replace('-', "_"))
}

fn admin_nav_render_fn_name(slug: &str) -> String {
    format!("render_{}_nav_item", slug.replace('-', "_"))
}

fn admin_child_pages_const_name(slug: &str) -> String {
    format!(
        "{}_ADMIN_CHILD_PAGES",
        slug.replace('-', "_").to_ascii_uppercase()
    )
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

fn title_case(value: &str) -> String {
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
        .collect::<Vec<_>>()
        .join(" ")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from apps/admin")
}

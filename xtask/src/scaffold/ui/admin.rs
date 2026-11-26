use std::path::PathBuf;

use super::ScaffoldOptions;

pub(super) fn render(opts: &ScaffoldOptions, module_stem: &str) -> Vec<(PathBuf, String)> {
    let package = opts.crate_name();
    let owner = package.replace('-', "_");
    let route = opts.kebab_name();
    let entity = crate::manifest_validation::to_pascal_case(
        opts.entity
            .as_deref()
            .expect("validated admin record entity"),
    );
    let field = super::super::options::graphql_field_prefix(module_stem);
    let resource = opts
        .admin_resource
        .as_deref()
        .expect("validated admin resource");
    [
        ("Cargo.toml", include_str!("admin/Cargo.toml.in")),
        ("README.md", include_str!("admin/README.md.in")),
        ("CRATE_API.md", include_str!("admin/CRATE_API.md.in")),
        ("locales/en.ftl", include_str!("admin/locales/en.ftl.in")),
        ("locales/ru.ftl", include_str!("admin/locales/ru.ftl.in")),
        ("src/lib.rs", include_str!("admin/src/lib.rs.in")),
        ("src/core/mod.rs", include_str!("admin/src/core/mod.rs.in")),
        (
            "src/core/editor.rs",
            include_str!("admin/src/core/editor.rs.in"),
        ),
        ("src/model.rs", include_str!("admin/src/model.rs.in")),
        (
            "src/core/settings.rs",
            include_str!("admin/src/core/settings.rs.in"),
        ),
        (
            "src/transport/settings.rs",
            include_str!("admin/src/transport/settings.rs.in"),
        ),
        (
            "src/ui/leptos/settings.rs",
            include_str!("admin/src/ui/leptos/settings.rs.in"),
        ),
        (
            "contracts/graphql/tenant_module_settings.graphql",
            include_str!("admin/contracts/graphql/tenant_module_settings.graphql.in"),
        ),
        (
            "contracts/graphql/update_module_settings.graphql",
            include_str!("admin/contracts/graphql/update_module_settings.graphql.in"),
        ),
        ("src/i18n.rs", "rustok_ui_i18n::declare_module_i18n!();\n"),
        (
            "src/transport/mod.rs",
            include_str!("admin/src/transport/mod.rs.in"),
        ),
        (
            "src/transport/graphql_adapter.rs",
            include_str!("admin/src/transport/graphql_adapter.rs.in"),
        ),
        (
            "src/transport/native_server_adapter.rs",
            include_str!("admin/src/transport/native_server_adapter.rs.in"),
        ),
        (
            "src/transport/native_server_adapter/runtime.rs",
            include_str!("admin/src/transport/native_server_adapter/runtime.rs.in"),
        ),
        (
            "src/ui/mod.rs",
            "mod leptos;\npub use leptos::__MODULE_STEM__Admin;\n",
        ),
        (
            "src/ui/leptos/mod.rs",
            include_str!("admin/src/ui/leptos/mod.rs.in"),
        ),
        (
            "src/ui/leptos/editor.rs",
            include_str!("admin/src/ui/leptos/editor.rs.in"),
        ),
        (
            "src/tests/mod.rs",
            "mod contract;\n\n#[cfg(feature = \"ssr\")]\nmod native;\n",
        ),
        (
            "src/tests/contract.rs",
            include_str!("admin/src/tests/contract.rs.in"),
        ),
        (
            "src/tests/native.rs",
            include_str!("admin/src/tests/native.rs.in"),
        ),
    ]
    .into_iter()
    .map(|(path, source)| {
        (
            PathBuf::from("admin").join(path),
            source
                .replace("__CRATE__", &package)
                .replace("__OWNER__", &owner)
                .replace("__SLUG__", &opts.slug)
                .replace("__ROUTE__", &route)
                .replace("__ENTITY__", &entity)
                .replace("__MODULE_STEM__", module_stem)
                .replace("__MODULE_FIELD__", &field)
                .replace("__RESOURCE__", resource)
                .replace(
                    "__PUBLIC_DISCOVERY_KEY__",
                    super::super::persistence::PUBLIC_DISCOVERY_KEY,
                )
                .replace(
                    "__PAGE_SIZE_KEY__",
                    super::super::persistence::PAGE_SIZE_KEY,
                )
                .replace(
                    "__DEFAULT_PAGE_SIZE__",
                    &super::super::persistence::DEFAULT_PAGE_SIZE.to_string(),
                )
                .replace(
                    "__MAX_PAGE_SIZE__",
                    &super::super::persistence::MAX_PAGE_SIZE.to_string(),
                ),
        )
    })
    .collect()
}

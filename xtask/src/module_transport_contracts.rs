use super::*;

pub(crate) fn validate_module_transport_surface_contract(
    slug: &str,
    manifest: &ModulePackageManifest,
    module_root: &Path,
) -> Result<()> {
    let source_files = collect_module_source_files(module_root)?;

    if let Some(graphql) = manifest.provides.graphql.as_ref() {
        if let Some(query) = graphql.query.as_deref() {
            let symbol = provided_path_symbol(query)?;
            validate_declared_symbol_exists(
                slug,
                "provides.graphql.query",
                query,
                symbol,
                &source_files,
            )?;
        }
        if let Some(mutation) = graphql.mutation.as_deref() {
            let symbol = provided_path_symbol(mutation)?;
            validate_declared_symbol_exists(
                slug,
                "provides.graphql.mutation",
                mutation,
                symbol,
                &source_files,
            )?;
        }
    }

    if let Some(http) = manifest.provides.http.as_ref() {
        if http.routes.is_some() && http.axum_router.is_some() {
            anyhow::bail!(
                "Module '{slug}' declares both provides.http.routes and provides.http.axum_router; choose one transport entrypoint"
            );
        }
        if let Some(routes) = http.routes.as_deref() {
            let symbol = provided_path_symbol(routes)?;
            validate_declared_symbol_exists(
                slug,
                "provides.http.routes",
                routes,
                symbol,
                &source_files,
            )?;
        }
        if let Some(axum_router) = http.axum_router.as_deref() {
            let symbol = provided_path_symbol(axum_router)?;
            validate_declared_symbol_exists(
                slug,
                "provides.http.axum_router",
                axum_router,
                symbol,
                &source_files,
            )?;
        }
        if let Some(webhook_routes) = http.webhook_routes.as_deref() {
            let symbol = provided_path_symbol(webhook_routes)?;
            validate_declared_symbol_exists(
                slug,
                "provides.http.webhook_routes",
                webhook_routes,
                symbol,
                &source_files,
            )?;
        }
    }

    Ok(())
}

pub(crate) fn validate_module_server_http_surface_contract(
    manifest_path: &Path,
    slug: &str,
    manifest: &ModulePackageManifest,
) -> Result<()> {
    let Some(http) = manifest.provides.http.as_ref() else {
        return Ok(());
    };

    let workspace_root = manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve workspace root from modules manifest {}",
            manifest_path.display()
        )
    })?;
    let Some(server_controller_path) = find_existing_server_controller_path(workspace_root, slug)
    else {
        return Ok(());
    };

    let controller_content = fs::read_to_string(&server_controller_path)
        .with_context(|| format!("Failed to read {}", server_controller_path.display()))?;

    if http.routes.as_deref().is_some()
        && !server_controller_exports_symbol(&controller_content, "routes")
    {
        anyhow::bail!(
            "Module '{slug}' declares provides.http.routes, but server controller shim {} does not export pub routes() for build.rs/codegen",
            server_controller_path.display()
        );
    }

    if http.webhook_routes.as_deref().is_some()
        && !server_controller_exports_symbol(&controller_content, "webhook_routes")
    {
        anyhow::bail!(
            "Module '{slug}' declares provides.http.webhook_routes, but server controller shim {} does not export pub webhook_routes() for build.rs/codegen",
            server_controller_path.display()
        );
    }

    Ok(())
}

fn find_existing_server_controller_path(workspace_root: &Path, slug: &str) -> Option<PathBuf> {
    [
        workspace_root
            .join("apps")
            .join("server")
            .join("src")
            .join("controllers")
            .join(slug)
            .join("mod.rs"),
        workspace_root
            .join("apps")
            .join("server")
            .join("src")
            .join("controllers")
            .join(format!("{slug}.rs")),
    ]
    .into_iter()
    .find(|path| path.exists())
}

fn server_controller_exports_symbol(content: &str, symbol: &str) -> bool {
    content.contains(&format!("pub fn {symbol}("))
        || content.contains("pub use ")
            && (content.contains("controllers::*")
                || content.contains(&format!("::{symbol};"))
                || content.contains(&format!("::{symbol},"))
                || content.contains(&format!("{symbol}::*")))
}

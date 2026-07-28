use super::*;

pub(crate) fn validate_module_index_search_boundary_contract(
    slug: &str,
    module_root: &Path,
) -> Result<()> {
    match slug {
        "index" => {
            let lib_rs_path = module_root.join("src").join("lib.rs");
            let readme_path = module_root.join("README.md");
            let lib_rs = fs::read_to_string(&lib_rs_path)
                .with_context(|| format!("Failed to read {}", lib_rs_path.display()))?;
            let readme = fs::read_to_string(&readme_path)
                .with_context(|| format!("Failed to read {}", readme_path.display()))?;

            for fragment in [
                "PostgresMutationStore",
                "PostgresSchemaRegistrationStore",
                "PostgresSecondaryIndexManager",
            ] {
                if !(lib_rs.contains(fragment) || readme.contains(fragment)) {
                    anyhow::bail!(
                        "Module '{slug}' boundary contract drift: expected '{}' in {} or {}",
                        fragment,
                        lib_rs_path.display(),
                        readme_path.display()
                    );
                }
            }

            for forbidden in [
                "SearchEngineKind",
                "PgSearchEngine",
                "SearchIngestionHandler",
                "IndexerRuntimeConfig",
                "ContentIndexer::with_runtime",
                "ProductIndexer::with_runtime",
            ] {
                if lib_rs.contains(forbidden) {
                    anyhow::bail!(
                        "Module '{slug}' boundary contract drift: {} must not expose search-owned symbol '{}'",
                        lib_rs_path.display(),
                        forbidden
                    );
                }
            }
        }
        "search" => {
            let lib_rs_path = module_root.join("src").join("lib.rs");
            let readme_path = module_root.join("README.md");
            let lib_rs = fs::read_to_string(&lib_rs_path)
                .with_context(|| format!("Failed to read {}", lib_rs_path.display()))?;
            let readme = fs::read_to_string(&readme_path)
                .with_context(|| format!("Failed to read {}", readme_path.display()))?;

            for fragment in [
                "SearchEngineKind",
                "PgSearchEngine",
                "SearchIngestionHandler",
                "search_documents",
            ] {
                if !(lib_rs.contains(fragment) || readme.contains(fragment)) {
                    anyhow::bail!(
                        "Module '{slug}' boundary contract drift: expected '{}' in {} or {}",
                        fragment,
                        lib_rs_path.display(),
                        readme_path.display()
                    );
                }
            }

            for forbidden in [
                "IndexerRuntimeConfig",
                "ContentIndexer::with_runtime",
                "ProductIndexer::with_runtime",
            ] {
                if lib_rs.contains(forbidden) {
                    anyhow::bail!(
                        "Module '{slug}' boundary contract drift: {} must not expose index-owned symbol '{}'",
                        lib_rs_path.display(),
                        forbidden
                    );
                }
            }
        }
        _ => {}
    }

    Ok(())
}

pub(crate) fn validate_module_search_operator_surface_contract(
    slug: &str,
    module_root: &Path,
) -> Result<()> {
    if slug != "search" {
        return Ok(());
    }

    let lib_rs_path = module_root.join("src").join("lib.rs");
    let readme_path = module_root.join("README.md");
    let runbook_path = module_root.join("docs").join("observability-runbook.md");
    let lib_rs = fs::read_to_string(&lib_rs_path)
        .with_context(|| format!("Failed to read {}", lib_rs_path.display()))?;
    let readme = fs::read_to_string(&readme_path)
        .with_context(|| format!("Failed to read {}", readme_path.display()))?;

    for fragment in [
        "SearchDiagnosticsService",
        "SearchAnalyticsService",
        "SearchSettingsService",
        "SearchDictionaryService",
    ] {
        if !lib_rs.contains(fragment) {
            anyhow::bail!(
                "Module '{slug}' operator surface contract drift: {} must expose '{}'",
                lib_rs_path.display(),
                fragment
            );
        }
    }

    for fragment in [
        "searchDiagnostics",
        "searchAnalytics",
        "searchSettingsPreview",
        "triggerSearchRebuild",
    ] {
        if !readme.contains(fragment) {
            anyhow::bail!(
                "Module '{slug}' operator surface contract drift: {} must document '{}'",
                readme_path.display(),
                fragment
            );
        }
    }

    if !runbook_path.exists() {
        anyhow::bail!(
            "Module '{slug}' operator surface contract drift: {} must exist",
            runbook_path.display()
        );
    }

    Ok(())
}

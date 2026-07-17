from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    return text.replace(old, new, 1)

runtime_path = Path("apps/server/src/middleware/tenant.rs")
runtime = runtime_path.read_text()

runtime = replace_once(
    runtime,
    """                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                        .map_err(|error| CoreError::Cache(error.to_string()))?;
                    return Err(CoreError::NotFound(error.message));
""",
    """                    if let Err(cache_error) = infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                    {
                        tracing::warn!(%cache_error, "Tenant not-found negative cache write failed");
                    }
                    return Err(CoreError::NotFound(error.message));
""",
    "not-found cache degradation",
)
runtime = replace_once(
    runtime,
    """                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::Disabled)
                        .await
                        .map_err(|error| CoreError::Cache(error.to_string()))?;
                    Err(CoreError::Forbidden("tenant disabled".to_string()))
""",
    """                    if let Err(cache_error) = infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::Disabled)
                        .await
                    {
                        tracing::warn!(%cache_error, "Disabled-tenant negative cache write failed");
                    }
                    Err(CoreError::Forbidden("tenant disabled".to_string()))
""",
    "disabled cache degradation",
)
runtime = replace_once(
    runtime,
    """                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                        .map_err(|error| CoreError::Cache(error.to_string()))?;
                    Err(CoreError::NotFound("tenant not found".to_string()))
""",
    """                    if let Err(cache_error) = infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                    {
                        tracing::warn!(%cache_error, "Tenant projection negative cache write failed");
                    }
                    Err(CoreError::NotFound("tenant not found".to_string()))
""",
    "projection cache degradation",
)
runtime_path.write_text(runtime)

graphql_path = Path("apps/server/src/controllers/graphql.rs")
graphql = graphql_path.read_text()
graphql = replace_once(
    graphql,
    """        })?;

    let access_token = token
""",
    """        })?;
    rustok_telemetry::metrics::record_cache_operation(
        "tenant_resolution",
        "resolve",
        "graphql_ws_payload",
    );

    let access_token = token
""",
    "GraphQL WebSocket resolution telemetry",
)
graphql_path.write_text(graphql)

gate_path = Path("scripts/verify/verify-tenant-resolution-architecture.mjs")
gate = gate_path.read_text()
gate = replace_once(
    gate,
    'requireMatch(graphql, /tenant::load_tenant_context_by_slug/, "GraphQL WebSocket must use canonical tenant loading");',
    'requireMatch(graphql, /tenant::load_tenant_context_by_slug/, "GraphQL WebSocket must use canonical tenant loading");\nrequireMatch(graphql, /graphql_ws_payload/, "GraphQL WebSocket tenant resolution must emit source telemetry");',
    "WebSocket telemetry architecture assertion",
)
gate_path.write_text(gate)

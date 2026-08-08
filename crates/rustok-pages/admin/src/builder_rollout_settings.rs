use leptos::prelude::*;
use rustok_page_builder::rollout::{BuilderCapabilityFlags, BuilderRolloutError};
use serde_json::Value;

fn nested_bool(settings: &Value, path: &[&str]) -> Result<bool, BuilderRolloutError> {
    let mut current = settings;
    for segment in path {
        let Value::Object(values) = current else {
            return Err(BuilderRolloutError::InvalidFlagCombination(format!(
                "Page Builder rollout setting `{}` must be an object",
                path.join(".")
            )));
        };
        let Some(next) = values.get(*segment) else {
            return Ok(true);
        };
        current = next;
    }
    current.as_bool().ok_or_else(|| {
        BuilderRolloutError::InvalidFlagCombination(format!(
            "Page Builder rollout setting `{}` must be a boolean",
            path.join(".")
        ))
    })
}

pub(crate) fn pages_builder_flags_from_settings(
    settings: &Value,
) -> Result<BuilderCapabilityFlags, BuilderRolloutError> {
    let flags = BuilderCapabilityFlags {
        builder_enabled: nested_bool(settings, &["builder", "enabled"])?,
        preview_enabled: nested_bool(settings, &["builder", "preview", "enabled"])?,
        properties_enabled: nested_bool(settings, &["builder", "properties", "enabled"])?,
        publish_enabled: nested_bool(settings, &["builder", "publish", "enabled"])?,
    };
    flags.validate()?;
    Ok(flags)
}

#[cfg(feature = "ssr")]
fn ensure_trusted_tenant(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<(), ServerFnError> {
    use rustok_api::{Action, Permission, Resource, has_effective_permission};

    if auth.tenant_id != tenant.id {
        return Err(ServerFnError::new("Pages builder rollout access is denied"));
    }
    if !has_effective_permission(
        &auth.permissions,
        &Permission::new(Resource::Pages, Action::Read),
    ) {
        return Err(ServerFnError::new(
            "Pages read permission is required for builder rollout status",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
pub(crate) struct TrustedPagesBuilderRolloutSnapshot {
    pub flags: BuilderCapabilityFlags,
    pub tenant_slug: String,
}

#[cfg(feature = "ssr")]
pub(crate) async fn load_trusted_pages_builder_rollout_snapshot(
) -> Result<TrustedPagesBuilderRolloutSnapshot, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, TenantContext, tenant_module_settings};

    let runtime = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(ServerFnError::new)?;
    ensure_trusted_tenant(&auth, &tenant)?;

    let settings = tenant_module_settings(runtime.db(), tenant.id, "pages")
        .await
        .map_err(|_| ServerFnError::new("Unable to read Pages builder rollout settings"))?
        .ok_or_else(|| ServerFnError::new("Pages module is not enabled for the routed tenant"))?;
    let flags = pages_builder_flags_from_settings(&settings)
        .map_err(|_| ServerFnError::new("Pages builder rollout settings are invalid"))?;
    Ok(TrustedPagesBuilderRolloutSnapshot {
        flags,
        tenant_slug: tenant.slug,
    })
}

#[server(prefix = "/api/fn", endpoint = "pages/builder-rollout-flags")]
pub(crate) async fn pages_builder_rollout_flags() -> Result<BuilderCapabilityFlags, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(load_trusted_pages_builder_rollout_snapshot().await?.flags)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "pages/builder-rollout-flags requires the `ssr` feature",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_page_builder::rollout::BuilderToggleProfile;
    use serde_json::json;

    fn settings(profile: BuilderToggleProfile) -> Value {
        let flags = profile.flags();
        json!({
            "builder": {
                "enabled": flags.builder_enabled,
                "preview": { "enabled": flags.preview_enabled },
                "properties": { "enabled": flags.properties_enabled },
                "publish": { "enabled": flags.publish_enabled }
            }
        })
    }

    #[test]
    fn declared_profiles_normalize_to_their_exact_flags() {
        for profile in BuilderToggleProfile::ALL {
            assert_eq!(
                pages_builder_flags_from_settings(&settings(profile)).unwrap(),
                profile.flags()
            );
        }
    }

    #[test]
    fn omitted_builder_settings_preserve_backward_compatible_all_on_defaults() {
        assert_eq!(
            pages_builder_flags_from_settings(&json!({})).unwrap(),
            BuilderCapabilityFlags::default()
        );
    }

    #[test]
    fn malformed_setting_types_fail_closed() {
        for value in [
            json!({ "builder": "disabled" }),
            json!({ "builder": { "enabled": "false" } }),
            json!({ "builder": { "preview": false } }),
            json!({ "builder": { "publish": { "enabled": 0 } } }),
        ] {
            assert!(pages_builder_flags_from_settings(&value).is_err());
        }
    }

    #[test]
    fn invalid_flag_combinations_fail_closed() {
        assert!(
            pages_builder_flags_from_settings(&json!({
                "builder": {
                    "enabled": true,
                    "preview": { "enabled": false },
                    "properties": { "enabled": true },
                    "publish": { "enabled": true }
                }
            }))
            .is_err()
        );
        assert!(
            pages_builder_flags_from_settings(&json!({
                "builder": { "enabled": false }
            }))
            .is_err()
        );
    }
}

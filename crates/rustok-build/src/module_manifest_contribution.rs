use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

pub const OWNER_PROVIDER_METADATA_KEY: &str = "ownerProvider";
pub const PROVIDER_VERSION_METADATA_KEY: &str = "providerVersion";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleContributionManifestError {
    message: String,
}

impl ModuleContributionManifestError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ModuleContributionManifestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ModuleContributionManifestError {}

pub type Result<T> = std::result::Result<T, ModuleContributionManifestError>;

#[derive(Debug, Clone, PartialEq)]
pub struct ContributionRoleExport {
    pub role: String,
    pub surface: String,
    pub id: String,
    pub provider: String,
    pub required_capabilities: Vec<String>,
    pub blocks: Vec<String>,
    pub property_editor_id: Option<String>,
    pub property_editor_component_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedModuleContributionManifest {
    pub module_id: String,
    pub owner_provider: String,
    pub owner_version: String,
    pub target_providers: BTreeMap<String, String>,
    pub dependencies: BTreeSet<String>,
    pub required_permissions: BTreeSet<String>,
    pub builder_capabilities: Vec<String>,
    pub admin: Vec<serde_json::Value>,
    pub storefront: Vec<serde_json::Value>,
    roles: BTreeMap<String, ContributionRoleExport>,
}

impl NormalizedModuleContributionManifest {
    pub fn role(&self, role: &str) -> Option<&ContributionRoleExport> {
        self.roles.get(role.trim())
    }

    pub fn manifest_json(&self) -> Result<String> {
        serde_json::to_string(&json!({
            "module_id": self.module_id,
            "owner_provider": self.owner_provider,
            "owner_version": self.owner_version,
            "target_providers": self.target_providers,
            "dependencies": self.dependencies,
            "required_permissions": self.required_permissions,
            "admin": self.admin,
            "storefront": self.storefront,
        }))
        .map_err(|error| {
            ModuleContributionManifestError::new(format!(
                "normalized contribution manifest cannot serialize: {error}"
            ))
        })
    }
}

#[derive(Debug, Deserialize, Default)]
struct ModuleManifestRoot {
    #[serde(default)]
    module: ModuleMetadata,
    #[serde(default)]
    fba: FbaMetadata,
}

#[derive(Debug, Deserialize, Default)]
struct ModuleMetadata {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    version: String,
}

#[derive(Debug, Deserialize, Default)]
struct FbaMetadata {
    #[serde(default)]
    builder_consumer: Option<BuilderConsumerMetadata>,
}

#[derive(Debug, Deserialize, Default)]
struct BuilderConsumerMetadata {
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    contribution_manifest: Option<ContributionManifestSource>,
}

#[derive(Debug, Deserialize, Default)]
struct ContributionManifestSource {
    #[serde(default)]
    owner_provider: String,
    #[serde(default)]
    target_providers: BTreeMap<String, String>,
    #[serde(default)]
    dependencies: BTreeSet<String>,
    #[serde(default)]
    required_permissions: BTreeSet<String>,
    #[serde(default)]
    admin: Vec<toml::Value>,
    #[serde(default)]
    storefront: Vec<toml::Value>,
}

pub fn normalize_module_contribution_manifest(
    source: &str,
) -> Result<Option<NormalizedModuleContributionManifest>> {
    let root: ModuleManifestRoot = toml::from_str(source).map_err(|error| {
        ModuleContributionManifestError::new(format!("invalid rustok-module.toml: {error}"))
    })?;
    let Some(builder_consumer) = root.fba.builder_consumer else {
        return Ok(None);
    };
    let Some(manifest) = builder_consumer.contribution_manifest else {
        return Ok(None);
    };

    let module_id = required(&root.module.slug, "module.slug")?;
    let owner_version = required(&root.module.version, "module.version")?;
    let owner_provider = required(
        &manifest.owner_provider,
        "fba.builder_consumer.contribution_manifest.owner_provider",
    )?;
    let builder_capabilities = normalize_vec(
        builder_consumer.capabilities,
        "fba.builder_consumer.capabilities",
    )?;
    if builder_capabilities.is_empty() {
        return fail(
            "fba.builder_consumer.capabilities must not be empty when contribution_manifest is declared",
        );
    }
    let builder_capability_set = builder_capabilities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    let target_providers =
        normalize_targets(manifest.target_providers, &owner_provider, &owner_version)?;
    let dependencies = normalize_set(
        manifest.dependencies,
        "fba.builder_consumer.contribution_manifest.dependencies",
    )?;
    let required_permissions = normalize_set(
        manifest.required_permissions,
        "fba.builder_consumer.contribution_manifest.required_permissions",
    )?;

    let mut roles = BTreeMap::new();
    let admin = normalize_contributions(
        manifest.admin,
        "admin",
        &owner_provider,
        &owner_version,
        &target_providers,
        &builder_capability_set,
        &mut roles,
    )?;
    let storefront = normalize_contributions(
        manifest.storefront,
        "storefront",
        &owner_provider,
        &owner_version,
        &target_providers,
        &builder_capability_set,
        &mut roles,
    )?;

    if admin.is_empty() && storefront.is_empty() {
        return fail(
            "contribution_manifest must declare at least one admin or storefront contribution",
        );
    }

    Ok(Some(NormalizedModuleContributionManifest {
        module_id,
        owner_provider,
        owner_version,
        target_providers,
        dependencies,
        required_permissions,
        builder_capabilities,
        admin,
        storefront,
        roles,
    }))
}

fn normalize_targets(
    targets: BTreeMap<String, String>,
    owner_provider: &str,
    owner_version: &str,
) -> Result<BTreeMap<String, String>> {
    let mut normalized = BTreeMap::new();
    for (provider, version) in targets {
        let provider = required(&provider, "target provider")?;
        let version = required(&version, "target provider version")?;
        if provider == owner_provider {
            if version != owner_version {
                return fail(format!(
                    "target provider '{provider}@{version}' conflicts with owner version '{owner_version}'"
                ));
            }
            continue;
        }
        if normalized.insert(provider.clone(), version).is_some() {
            return fail(format!(
                "target provider '{provider}' is duplicated after normalization"
            ));
        }
    }
    Ok(normalized)
}

fn normalize_contributions(
    values: Vec<toml::Value>,
    surface: &str,
    owner_provider: &str,
    owner_version: &str,
    target_providers: &BTreeMap<String, String>,
    builder_capabilities: &BTreeSet<String>,
    roles: &mut BTreeMap<String, ContributionRoleExport>,
) -> Result<Vec<serde_json::Value>> {
    values
        .into_iter()
        .map(|mut value| {
            let table = value.as_table_mut().ok_or_else(|| {
                ModuleContributionManifestError::new(format!(
                    "{surface} contribution must be a TOML table"
                ))
            })?;
            let id = table_required_string(table, "id", surface)?;
            let provider = table_required_string(table, "provider", &id)?;
            let role = take_optional_string(table, "role", &id)?;
            let expected_version = if provider == owner_provider {
                owner_version
            } else {
                target_providers.get(&provider).map(String::as_str).ok_or_else(|| {
                    ModuleContributionManifestError::new(format!(
                        "contribution '{id}' targets undeclared provider '{provider}'; declare an exact target_providers version"
                    ))
                })?
            };

            let required_capabilities = normalize_table_string_array(
                table,
                "required_capabilities",
                &id,
            )?;
            for capability in &required_capabilities {
                if !builder_capabilities.contains(capability) {
                    return fail(format!(
                        "contribution '{id}' requires capability '{capability}' outside fba.builder_consumer.capabilities"
                    ));
                }
            }
            let blocks = normalize_table_string_array(table, "blocks", &id)?;
            validate_nested_provider_array(table, "renderers", &provider, &id)?;
            validate_nested_provider_array(table, "property_editors", &provider, &id)?;

            let metadata = table
                .entry("metadata".to_string())
                .or_insert_with(|| toml::Value::Table(Default::default()))
                .as_table_mut()
                .ok_or_else(|| {
                    ModuleContributionManifestError::new(format!(
                        "contribution '{id}' metadata must be a TOML table"
                    ))
                })?;
            if metadata.contains_key(OWNER_PROVIDER_METADATA_KEY)
                || metadata.contains_key(PROVIDER_VERSION_METADATA_KEY)
            {
                return fail(format!(
                    "contribution '{id}' must not hand-author {OWNER_PROVIDER_METADATA_KEY}/{PROVIDER_VERSION_METADATA_KEY}; shared generation owns those fields"
                ));
            }
            metadata.insert(
                OWNER_PROVIDER_METADATA_KEY.to_string(),
                toml::Value::String(owner_provider.to_string()),
            );
            metadata.insert(
                PROVIDER_VERSION_METADATA_KEY.to_string(),
                toml::Value::String(expected_version.to_string()),
            );

            if let Some(role) = role {
                let (property_editor_id, property_editor_component_type) =
                    single_property_editor_export(table, &id)?;
                let export = ContributionRoleExport {
                    role: role.clone(),
                    surface: surface.to_string(),
                    id: id.clone(),
                    provider: provider.clone(),
                    required_capabilities: required_capabilities.clone(),
                    blocks: blocks.clone(),
                    property_editor_id,
                    property_editor_component_type,
                };
                if roles.insert(role.clone(), export).is_some() {
                    return fail(format!("contribution role '{role}' is duplicated"));
                }
            }

            serde_json::to_value(value).map_err(|error| {
                ModuleContributionManifestError::new(format!(
                    "contribution '{id}' cannot serialize: {error}"
                ))
            })
        })
        .collect()
}

fn validate_nested_provider_array(
    table: &toml::Table,
    key: &str,
    provider: &str,
    contribution_id: &str,
) -> Result<()> {
    let Some(items) = table.get(key) else {
        return Ok(());
    };
    let items = items.as_array().ok_or_else(|| {
        ModuleContributionManifestError::new(format!(
            "contribution '{contribution_id}' {key} must be an array"
        ))
    })?;
    for item in items {
        let item = item.as_table().ok_or_else(|| {
            ModuleContributionManifestError::new(format!(
                "contribution '{contribution_id}' {key} entries must be tables"
            ))
        })?;
        let nested_provider = table_required_string(item, "provider", contribution_id)?;
        if nested_provider != provider {
            return fail(format!(
                "contribution '{contribution_id}' {key} provider '{nested_provider}' must match contribution provider '{provider}'"
            ));
        }
    }
    Ok(())
}

fn single_property_editor_export(
    table: &toml::Table,
    contribution_id: &str,
) -> Result<(Option<String>, Option<String>)> {
    let editors = table
        .get("property_editors")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if editors.len() != 1 {
        return Ok((None, None));
    }
    let editor = editors[0].as_table().ok_or_else(|| {
        ModuleContributionManifestError::new(format!(
            "contribution '{contribution_id}' property editor must be a TOML table"
        ))
    })?;
    Ok((
        Some(table_required_string(
            editor,
            "id",
            &format!("{contribution_id} property editor"),
        )?),
        Some(table_required_string(
            editor,
            "component_type",
            &format!("{contribution_id} property editor"),
        )?),
    ))
}

fn normalize_table_string_array(
    table: &mut toml::Table,
    key: &str,
    label: &str,
) -> Result<Vec<String>> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| {
        ModuleContributionManifestError::new(format!("{label}.{key} must be an array"))
    })?;
    let normalized = normalize_vec(
        values
            .iter()
            .map(|value| {
                value.as_str().map(ToString::to_string).ok_or_else(|| {
                    ModuleContributionManifestError::new(format!(
                        "{label}.{key} entries must be strings"
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?,
        &format!("{label}.{key}"),
    )?;
    table.insert(
        key.to_string(),
        toml::Value::Array(
            normalized
                .iter()
                .map(|value| toml::Value::String(value.clone()))
                .collect(),
        ),
    );
    Ok(normalized)
}

fn normalize_vec(values: Vec<String>, label: &str) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let value = required(&value, label)?;
        if !seen.insert(value.clone()) {
            return fail(format!("{label} contains duplicate '{value}'"));
        }
        normalized.push(value);
    }
    Ok(normalized)
}

fn normalize_set(values: BTreeSet<String>, label: &str) -> Result<BTreeSet<String>> {
    let mut normalized = BTreeSet::new();
    for value in values {
        let value = required(&value, label)?;
        if !normalized.insert(value.clone()) {
            return fail(format!(
                "{label} contains duplicate '{value}' after normalization"
            ));
        }
    }
    Ok(normalized)
}

fn take_optional_string(table: &mut toml::Table, key: &str, label: &str) -> Result<Option<String>> {
    let Some(value) = table.remove(key) else {
        return Ok(None);
    };
    let value = value.as_str().ok_or_else(|| {
        ModuleContributionManifestError::new(format!("{label}.{key} must be a string"))
    })?;
    Ok(Some(required(value, &format!("{label}.{key}"))?))
}

fn table_required_string(table: &toml::Table, key: &str, label: &str) -> Result<String> {
    let value = table
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            ModuleContributionManifestError::new(format!("{label} is missing string '{key}'"))
        })?;
    required(value, &format!("{label}.{key}"))
}

fn required(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return fail(format!("{label} must not be empty"));
    }
    Ok(value.to_string())
}

fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(ModuleContributionManifestError::new(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
[module]
slug = "pages"
version = "0.1.0"

[fba.builder_consumer]
capabilities = ["preview", "properties"]

[fba.builder_consumer.contribution_manifest]
owner_provider = "rustok.pages"
target_providers = { "fly.builtin" = "1" }

[[fba.builder_consumer.contribution_manifest.admin]]
role = "landing"
id = "rustok.pages.landing"
provider = "fly.builtin"
required_capabilities = ["properties"]
blocks = ["fly.hero"]

[[fba.builder_consumer.contribution_manifest.admin.property_editors]]
id = "fly.hero.editor"
component_type = "hero"
provider = "fly.builtin"
"#;

    #[test]
    fn normalizes_and_injects_versioned_provider_metadata() {
        let normalized = normalize_module_contribution_manifest(VALID)
            .expect("valid metadata")
            .expect("contribution manifest");
        assert_eq!(normalized.module_id, "pages");
        assert_eq!(normalized.owner_version, "0.1.0");
        assert_eq!(
            normalized.target_providers.get("fly.builtin"),
            Some(&"1".to_string())
        );
        assert_eq!(normalized.role("landing").expect("role").surface, "admin");
        assert!(
            normalized
                .manifest_json()
                .unwrap()
                .contains("\"module_id\":\"pages\"")
        );
        let metadata = normalized.admin[0]
            .get("metadata")
            .and_then(serde_json::Value::as_object)
            .expect("metadata");
        assert_eq!(metadata[OWNER_PROVIDER_METADATA_KEY], "rustok.pages");
        assert_eq!(metadata[PROVIDER_VERSION_METADATA_KEY], "1");
    }

    #[test]
    fn rejects_hand_authored_reserved_identity_metadata() {
        let invalid = VALID.replace(
            "blocks = [\"fly.hero\"]",
            "blocks = [\"fly.hero\"]\nmetadata = { providerVersion = \"1\" }",
        );
        let error = normalize_module_contribution_manifest(&invalid).expect_err("must fail");
        assert!(error.to_string().contains("must not hand-author"));
    }

    #[test]
    fn rejects_contribution_capability_outside_builder_contract() {
        let invalid = VALID.replace(
            "required_capabilities = [\"properties\"]",
            "required_capabilities = [\"properties\", \"publish\"]",
        );
        let error = normalize_module_contribution_manifest(&invalid).expect_err("must fail");
        assert!(
            error
                .to_string()
                .contains("outside fba.builder_consumer.capabilities")
        );
    }

    #[test]
    fn modules_without_contribution_metadata_are_accepted() {
        assert!(
            normalize_module_contribution_manifest("[module]\nslug='plain'\nversion='0.1.0'")
                .expect("plain module")
                .is_none()
        );
    }
}

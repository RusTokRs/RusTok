use rustok_core::{Error, Result};
use std::collections::HashSet;

use crate::ranking::SearchRankingProfile;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SearchFilterPreset {
    pub key: String,
    pub label: String,
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
    pub ranking_profile: Option<SearchRankingProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSearchFilterPreset {
    pub preset: Option<SearchFilterPreset>,
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
    pub ranking_profile: Option<SearchRankingProfile>,
}

pub struct SearchFilterPresetService;

impl SearchFilterPresetService {
    pub fn validate_config(config: &serde_json::Value) -> Result<()> {
        let Some(filter_presets) = config.get("filter_presets") else {
            return Ok(());
        };
        let object = filter_presets.as_object().ok_or_else(|| {
            Error::Validation("search_settings.config.filter_presets must be an object".to_string())
        })?;

        for (surface, presets) in object {
            validate_surface_name(surface)?;
            let presets = presets.as_array().ok_or_else(|| {
                Error::Validation(format!(
                    "search_settings.config.filter_presets.{surface} must be an array"
                ))
            })?;
            if presets.len() > 32 {
                return Err(Error::Validation(format!(
                    "search_settings.config.filter_presets.{surface} exceeds the maximum size of 32 presets"
                )));
            }

            let mut seen_keys = HashSet::new();
            for preset in presets {
                let parsed = parse_preset(preset)?;
                if !seen_keys.insert(parsed.key.clone()) {
                    return Err(Error::Validation(format!(
                        "search_settings.config.filter_presets.{surface} contains duplicate preset key '{}'",
                        parsed.key
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn list(config: &serde_json::Value, surface: &str) -> Vec<SearchFilterPreset> {
        config
            .get("filter_presets")
            .and_then(|value| value.get(surface).or_else(|| value.get("default")))
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| parse_preset(item).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub fn resolve(
        config: &serde_json::Value,
        surface: &str,
        preset_key: Option<&str>,
        entity_types: Vec<String>,
        source_modules: Vec<String>,
        statuses: Vec<String>,
    ) -> Result<ResolvedSearchFilterPreset> {
        let presets = Self::list(config, surface);
        let requested_key = preset_key
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());

        let preset = match requested_key {
            Some(ref key) => presets
                .into_iter()
                .find(|preset| preset.key == *key)
                .ok_or_else(|| Error::Validation(format!("Unknown filter preset '{}'", key)))?
                .into(),
            None => None,
        };

        let resolved = match preset.clone() {
            Some(preset) => ResolvedSearchFilterPreset {
                ranking_profile: preset.ranking_profile,
                entity_types: if entity_types.is_empty() {
                    preset.entity_types.clone()
                } else {
                    entity_types
                },
                source_modules: if source_modules.is_empty() {
                    preset.source_modules.clone()
                } else {
                    source_modules
                },
                statuses: if statuses.is_empty() {
                    preset.statuses.clone()
                } else {
                    statuses
                },
                preset: Some(preset),
            },
            None => ResolvedSearchFilterPreset {
                preset: None,
                entity_types,
                source_modules,
                statuses,
                ranking_profile: None,
            },
        };

        Ok(resolved)
    }
}

fn parse_preset(value: &serde_json::Value) -> Result<SearchFilterPreset> {
    let key = value
        .get("key")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Validation("filter preset is missing key".to_string()))?
        .to_ascii_lowercase();
    validate_key(&key)?;
    let label = value
        .get("label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&key)
        .to_string();
    validate_label(&label)?;

    let key_for_error = key.clone();

    Ok(SearchFilterPreset {
        key,
        label,
        entity_types: parse_string_array("entity_types", value.get("entity_types"))?,
        source_modules: parse_string_array("source_modules", value.get("source_modules"))?,
        statuses: parse_string_array("statuses", value.get("statuses"))?,
        ranking_profile: value
            .get("ranking_profile")
            .and_then(serde_json::Value::as_str)
            .map(|value| {
                SearchRankingProfile::try_from_str(value).ok_or_else(|| {
                    Error::Validation(format!(
                        "filter preset '{}' contains unsupported ranking_profile '{}'",
                        key_for_error, value
                    ))
                })
            })
            .transpose()?,
    })
}

fn parse_string_array(field_name: &str, value: Option<&serde_json::Value>) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value.as_array().ok_or_else(|| {
        Error::Validation(format!(
            "filter preset field '{field_name}' must be an array"
        ))
    })?;
    if items.len() > 16 {
        return Err(Error::Validation(format!(
            "filter preset field '{field_name}' exceeds the maximum size of 16 values"
        )));
    }

    items
        .iter()
        .map(|item| {
            let value = item.as_str().ok_or_else(|| {
                Error::Validation(format!(
                    "filter preset field '{field_name}' must contain only strings"
                ))
            })?;
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                return Err(Error::Validation(format!(
                    "filter preset field '{field_name}' contains an empty value"
                )));
            }
            if normalized.len() > 64
                || !normalized
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
            {
                return Err(Error::Validation(format!(
                    "filter preset field '{field_name}' contains invalid value '{}'",
                    value
                )));
            }
            Ok(normalized)
        })
        .collect()
}

fn validate_surface_name(surface: &str) -> Result<()> {
    let surface = surface.trim();
    if surface.is_empty() || surface.len() > 64 {
        return Err(Error::Validation(
            "filter preset surface must be 1..=64 characters long".to_string(),
        ));
    }
    if !surface
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(Error::Validation(format!(
            "filter preset surface '{}' contains invalid characters",
            surface
        )));
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<()> {
    if key.len() > 64
        || !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
    {
        return Err(Error::Validation(format!(
            "filter preset key '{}' contains invalid characters",
            key
        )));
    }
    Ok(())
}

fn validate_label(label: &str) -> Result<()> {
    if label.trim().is_empty() || label.len() > 96 {
        return Err(Error::Validation(
            "filter preset label must be 1..=96 characters long".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SearchFilterPresetService;
    use crate::SearchRankingProfile;

    #[test]
    fn resolve_uses_preset_defaults_when_explicit_filters_are_empty() {
        let config = serde_json::json!({
            "filter_presets": {
                "storefront_search": [
                    {
                        "key": "products",
                        "label": "Products",
                        "entity_types": ["product"],
                        "source_modules": ["commerce"],
                        "ranking_profile": "catalog"
                    }
                ]
            }
        });

        let resolved = SearchFilterPresetService::resolve(
            &config,
            "storefront_search",
            Some("products"),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("preset should resolve");

        assert_eq!(resolved.entity_types, vec!["product".to_string()]);
        assert_eq!(resolved.source_modules, vec!["commerce".to_string()]);
        assert_eq!(
            resolved.ranking_profile,
            Some(SearchRankingProfile::Catalog)
        );
    }

    #[test]
    fn resolve_keeps_explicit_filters_over_preset_values() {
        let config = serde_json::json!({
            "filter_presets": {
                "search_preview": [
                    {
                        "key": "content",
                        "label": "Content",
                        "entity_types": ["node"]
                    }
                ]
            }
        });

        let resolved = SearchFilterPresetService::resolve(
            &config,
            "search_preview",
            Some("content"),
            vec!["product".to_string()],
            Vec::new(),
            Vec::new(),
        )
        .expect("preset should resolve");

        assert_eq!(resolved.entity_types, vec!["product".to_string()]);
    }

    #[test]
    fn validate_config_rejects_duplicate_keys() {
        let error = SearchFilterPresetService::validate_config(&serde_json::json!({
            "filter_presets": {
                "storefront_search": [
                    { "key": "products", "label": "Products" },
                    { "key": "products", "label": "More products" }
                ]
            }
        }))
        .expect_err("duplicate keys should fail");

        assert!(error.to_string().contains("duplicate preset key"));
    }

    #[test]
    fn validate_config_rejects_invalid_filter_values() {
        let error = SearchFilterPresetService::validate_config(&serde_json::json!({
            "filter_presets": {
                "search_preview": [
                    {
                        "key": "content",
                        "label": "Content",
                        "entity_types": ["bad value"]
                    }
                ]
            }
        }))
        .expect_err("invalid filter values should fail");

        assert!(error.to_string().contains("contains invalid value"));
    }
}

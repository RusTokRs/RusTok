use std::collections::{BTreeMap, BTreeSet};

use rustok_core::field_schema::FieldDefinition;

use crate::{
    FlexSchemaTranslationError, FlexSchemaTranslationLeaf, FlexSchemaTranslationLeafSnapshot,
    FlexSchemaTranslationResult, validate_flex_schema_translation_locale_pair,
};

/// Extract only structurally declared field-definition copy for one exact source/target
/// locale pair. No arbitrary JSON traversal is performed here or by callers.
pub fn schema_definition_translation_leaves(
    definitions: &[FieldDefinition],
    source_locale: &str,
    target_locale: &str,
) -> FlexSchemaTranslationResult<Vec<FlexSchemaTranslationLeafSnapshot>> {
    validate_flex_schema_translation_locale_pair(source_locale, target_locale)?;

    let mut ordered = definitions.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.field_key.cmp(&right.field_key));

    let mut leaves = Vec::new();
    for definition in ordered {
        push_exact_map_leaf(
            &mut leaves,
            FlexSchemaTranslationLeaf::FieldLabel {
                field_key: definition.field_key.clone(),
            },
            &definition.label,
            source_locale,
            target_locale,
        )?;

        if let Some(description) = &definition.description {
            push_exact_map_leaf(
                &mut leaves,
                FlexSchemaTranslationLeaf::FieldDescription {
                    field_key: definition.field_key.clone(),
                },
                description,
                source_locale,
                target_locale,
            )?;
        }

        if let Some(validation) = &definition.validation {
            if let Some(error_message) = &validation.error_message {
                push_exact_map_leaf(
                    &mut leaves,
                    FlexSchemaTranslationLeaf::FieldValidationErrorMessage {
                        field_key: definition.field_key.clone(),
                    },
                    error_message,
                    source_locale,
                    target_locale,
                )?;
            }

            if let Some(options) = &validation.options {
                let mut options = options.iter().collect::<Vec<_>>();
                options.sort_by(|left, right| left.value.cmp(&right.value));
                for option in options {
                    push_exact_map_leaf(
                        &mut leaves,
                        FlexSchemaTranslationLeaf::FieldOptionLabel {
                            field_key: definition.field_key.clone(),
                            option_value: option.value.clone(),
                        },
                        &option.label,
                        source_locale,
                        target_locale,
                    )?;
                }
            }
        }
    }

    leaves.sort_by(|left, right| left.leaf.cmp(&right.leaf));
    Ok(leaves)
}

/// Return every normalized runtime locale explicitly present in declared field-definition
/// copy. Storage-only `und` provenance is intentionally ignored rather than promoted into
/// Translation authoring inventory.
pub fn schema_definition_translation_locales(
    definitions: &[FieldDefinition],
) -> FlexSchemaTranslationResult<BTreeSet<String>> {
    let mut locales = BTreeSet::new();
    for definition in definitions {
        collect_map_locales(&mut locales, &definition.label)?;
        if let Some(description) = &definition.description {
            collect_map_locales(&mut locales, description)?;
        }
        if let Some(validation) = &definition.validation {
            if let Some(error_message) = &validation.error_message {
                collect_map_locales(&mut locales, error_message)?;
            }
            if let Some(options) = &validation.options {
                for option in options {
                    collect_map_locales(&mut locales, &option.label)?;
                }
            }
        }
    }
    Ok(locales)
}

/// Apply exact target-locale values to declared field-definition maps only.
///
/// `targets` must be a subset of leaves that have exact source-locale values. Required
/// label leaves cannot be removed. Optional description/error-message leaves use `None`
/// to remove only the exact target-locale entry while preserving every other locale.
pub fn apply_schema_definition_translation_targets(
    definitions: &mut [FieldDefinition],
    source_locale: &str,
    target_locale: &str,
    targets: &BTreeMap<FlexSchemaTranslationLeaf, Option<String>>,
) -> FlexSchemaTranslationResult<bool> {
    validate_flex_schema_translation_locale_pair(source_locale, target_locale)?;
    let mut changed = false;

    for (leaf, value) in targets {
        match leaf {
            FlexSchemaTranslationLeaf::SchemaName | FlexSchemaTranslationLeaf::SchemaDescription => {
                return Err(FlexSchemaTranslationError::Invalid(
                    "schema row copy must not be applied through fields_config".to_string(),
                ));
            }
            FlexSchemaTranslationLeaf::FieldLabel { field_key } => {
                let definition = definition_mut(definitions, field_key)?;
                require_exact_source(&definition.label, source_locale, leaf)?;
                let value = required_target(value, leaf)?;
                changed |= set_exact_map_value(&mut definition.label, target_locale, Some(value));
            }
            FlexSchemaTranslationLeaf::FieldDescription { field_key } => {
                let definition = definition_mut(definitions, field_key)?;
                let map = definition.description.as_mut().ok_or_else(|| unknown_leaf(leaf))?;
                require_exact_source(map, source_locale, leaf)?;
                changed |= set_exact_map_value(map, target_locale, value.as_deref());
            }
            FlexSchemaTranslationLeaf::FieldValidationErrorMessage { field_key } => {
                let definition = definition_mut(definitions, field_key)?;
                let map = definition
                    .validation
                    .as_mut()
                    .and_then(|validation| validation.error_message.as_mut())
                    .ok_or_else(|| unknown_leaf(leaf))?;
                require_exact_source(map, source_locale, leaf)?;
                changed |= set_exact_map_value(map, target_locale, value.as_deref());
            }
            FlexSchemaTranslationLeaf::FieldOptionLabel {
                field_key,
                option_value,
            } => {
                let definition = definition_mut(definitions, field_key)?;
                let option = definition
                    .validation
                    .as_mut()
                    .and_then(|validation| validation.options.as_mut())
                    .and_then(|options| {
                        options
                            .iter_mut()
                            .find(|option| option.value.as_str() == option_value.as_str())
                    })
                    .ok_or_else(|| unknown_leaf(leaf))?;
                require_exact_source(&option.label, source_locale, leaf)?;
                let value = required_target(value, leaf)?;
                changed |= set_exact_map_value(&mut option.label, target_locale, Some(value));
            }
        }
    }

    Ok(changed)
}

fn definition_mut<'a>(
    definitions: &'a mut [FieldDefinition],
    field_key: &str,
) -> FlexSchemaTranslationResult<&'a mut FieldDefinition> {
    definitions
        .iter_mut()
        .find(|definition| definition.field_key == field_key)
        .ok_or_else(|| {
            FlexSchemaTranslationError::Invalid(format!(
                "Flex schema translation leaf references unknown field `{field_key}`"
            ))
        })
}

fn push_exact_map_leaf(
    leaves: &mut Vec<FlexSchemaTranslationLeafSnapshot>,
    leaf: FlexSchemaTranslationLeaf,
    values: &std::collections::HashMap<String, String>,
    source_locale: &str,
    target_locale: &str,
) -> FlexSchemaTranslationResult<()> {
    let Some(source_value) = values.get(source_locale) else {
        return Ok(());
    };
    validate_stored_value(source_value, &leaf, source_locale)?;
    let target_value = values
        .get(target_locale)
        .map(|value| {
            validate_stored_value(value, &leaf, target_locale)?;
            Ok(value.clone())
        })
        .transpose()?;
    leaves.push(FlexSchemaTranslationLeafSnapshot {
        leaf,
        source_value: source_value.clone(),
        target_value,
    });
    Ok(())
}

fn collect_map_locales(
    locales: &mut BTreeSet<String>,
    values: &std::collections::HashMap<String, String>,
) -> FlexSchemaTranslationResult<()> {
    for (locale, value) in values {
        if locale == "und" {
            continue;
        }
        if rustok_api::normalize_locale_tag(locale).as_deref() != Some(locale) {
            return Err(FlexSchemaTranslationError::OwnerInvariant(format!(
                "declared Flex schema copy contains invalid locale `{locale}`"
            )));
        }
        if value.trim().is_empty() {
            return Err(FlexSchemaTranslationError::OwnerInvariant(format!(
                "declared Flex schema copy contains blank value for locale `{locale}`"
            )));
        }
        locales.insert(locale.clone());
    }
    Ok(())
}

fn require_exact_source(
    values: &std::collections::HashMap<String, String>,
    source_locale: &str,
    leaf: &FlexSchemaTranslationLeaf,
) -> FlexSchemaTranslationResult<()> {
    values
        .get(source_locale)
        .filter(|value| !value.trim().is_empty())
        .map(|_| ())
        .ok_or_else(|| unknown_leaf(leaf))
}

fn required_target<'a>(
    value: &'a Option<String>,
    leaf: &FlexSchemaTranslationLeaf,
) -> FlexSchemaTranslationResult<&'a str> {
    value
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            FlexSchemaTranslationError::Invalid(format!(
                "Flex schema translation required leaf {leaf:?} cannot be removed"
            ))
        })
}

fn set_exact_map_value(
    values: &mut std::collections::HashMap<String, String>,
    target_locale: &str,
    value: Option<&str>,
) -> bool {
    match value {
        Some(value) => {
            if values.get(target_locale).is_some_and(|current| current == value) {
                false
            } else {
                values.insert(target_locale.to_string(), value.to_string());
                true
            }
        }
        None => values.remove(target_locale).is_some(),
    }
}

fn validate_stored_value(
    value: &str,
    leaf: &FlexSchemaTranslationLeaf,
    locale: &str,
) -> FlexSchemaTranslationResult<()> {
    if value.trim().is_empty() {
        return Err(FlexSchemaTranslationError::OwnerInvariant(format!(
            "Flex schema translation leaf {leaf:?} contains blank exact value for locale `{locale}`"
        )));
    }
    Ok(())
}

fn unknown_leaf(leaf: &FlexSchemaTranslationLeaf) -> FlexSchemaTranslationError {
    FlexSchemaTranslationError::Invalid(format!(
        "Flex schema translation apply references a leaf absent from exact source copy: {leaf:?}"
    ))
}

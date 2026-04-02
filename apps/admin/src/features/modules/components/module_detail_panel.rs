use leptos::prelude::*;
use std::collections::HashMap;

use crate::entities::module::{MarketplaceModule, ModuleSettingField, TenantModule};

#[derive(Clone)]
struct MetadataChecklistItem {
    label: &'static str,
    state: &'static str,
    priority: &'static str,
    summary: &'static str,
    detail: String,
}

fn short_checksum(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.len() > 16 {
        Some(format!("{}...", &value[..12]))
    } else {
        Some(value.to_string())
    }
}

fn humanize_token(value: &str) -> String {
    value
        .split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn humanize_setting_key(value: &str) -> String {
    let mut rendered = String::new();
    let mut previous_was_lowercase = false;

    for ch in value.chars() {
        if (ch == '_' || ch == '-') && !rendered.ends_with(' ') {
            rendered.push(' ');
            previous_was_lowercase = false;
            continue;
        }

        if ch.is_ascii_uppercase() && previous_was_lowercase && !rendered.ends_with(' ') {
            rendered.push(' ');
        }

        rendered.push(ch);
        previous_was_lowercase = ch.is_ascii_lowercase() || ch.is_ascii_digit();
    }

    humanize_token(rendered.trim())
}

fn setting_field_hint(field: &ModuleSettingField) -> Option<String> {
    let mut parts = Vec::new();
    if field.required {
        parts.push("Required".to_string());
    }
    if let Some(default) = &field.default_value {
        parts.push(format!("Default: {}", default));
    }
    match (field.min, field.max) {
        (Some(min), Some(max)) => parts.push(format!("Range: {}..{}", min, max)),
        (Some(min), None) => parts.push(format!("Min: {}", min)),
        (None, Some(max)) => parts.push(format!("Max: {}", max)),
        (None, None) => {}
    }
    if !field.options.is_empty() {
        parts.push(format!(
            "Options: {}",
            field
                .options
                .iter()
                .map(setting_option_label)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !field.object_keys.is_empty() {
        parts.push(format!(
            "Object keys: {}",
            field
                .object_keys
                .iter()
                .map(|key| humanize_setting_key(key))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(item_type) = field.item_type.as_deref() {
        parts.push(format!("Array items: {}", humanize_token(item_type)));
    }

    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn setting_field_placeholder(field: &ModuleSettingField) -> Option<&'static str> {
    match field.value_type.as_str() {
        "object" => Some("{\n  \"key\": \"value\"\n}"),
        "array" => Some("[\n  \"item\"\n]"),
        "json" | "any" => Some("{\n  \"any\": true\n}"),
        _ => None,
    }
}

fn setting_option_draft_value(value_type: &str, value: &serde_json::Value) -> String {
    match value_type {
        "string" => value.as_str().unwrap_or_default().to_string(),
        "integer" => value
            .as_i64()
            .map(|number| number.to_string())
            .or_else(|| value.as_u64().map(|number| number.to_string()))
            .unwrap_or_else(|| value.to_string()),
        "number" => value
            .as_f64()
            .map(|number| {
                let mut rendered = number.to_string();
                if rendered.ends_with(".0") {
                    rendered.truncate(rendered.len() - 2);
                }
                rendered
            })
            .unwrap_or_else(|| value.to_string()),
        "boolean" => value
            .as_bool()
            .map(|flag| flag.to_string())
            .unwrap_or_else(|| value.to_string()),
        _ => value.to_string(),
    }
}

fn setting_option_label(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

fn setting_shape_properties(shape: Option<&serde_json::Value>) -> Vec<(String, serde_json::Value)> {
    let Some(shape) = shape else {
        return Vec::new();
    };
    let Some(properties) = shape.get("properties").and_then(|value| value.as_object()) else {
        return Vec::new();
    };

    let mut entries = properties
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

fn setting_shape_items(shape: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    shape.and_then(|shape| shape.get("items")).cloned()
}

fn setting_shape_property(
    shape: Option<&serde_json::Value>,
    key: &str,
) -> Option<serde_json::Value> {
    shape
        .and_then(|shape| shape.get("properties"))
        .and_then(|value| value.as_object())
        .and_then(|properties| properties.get(key))
        .cloned()
}

fn setting_shape_type(shape: Option<&serde_json::Value>) -> Option<String> {
    shape
        .and_then(|shape| shape.get("type"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn setting_shape_options(shape: Option<&serde_json::Value>) -> Vec<serde_json::Value> {
    shape
        .and_then(|shape| shape.get("options"))
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
}

fn setting_shape_numeric_bound(shape: Option<&serde_json::Value>, key: &str) -> Option<String> {
    let value = shape.and_then(|shape| shape.get(key))?;

    value
        .as_i64()
        .map(|number| number.to_string())
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| {
            value.as_f64().map(|number| {
                let mut rendered = number.to_string();
                if rendered.ends_with(".0") {
                    rendered.truncate(rendered.len() - 2);
                }
                rendered
            })
        })
}

fn parse_scalar_input_value(raw: &str, value_type: &str) -> Option<serde_json::Value> {
    match value_type {
        "string" => Some(serde_json::Value::String(raw.to_string())),
        "boolean" => raw.parse::<bool>().ok().map(serde_json::Value::Bool),
        "integer" => raw
            .parse::<i64>()
            .ok()
            .map(|number| serde_json::Value::Number(number.into()))
            .or_else(|| {
                raw.parse::<u64>()
                    .ok()
                    .map(|number| serde_json::Value::Number(number.into()))
            }),
        "number" => raw
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        _ => None,
    }
}

fn render_scalar_value_editor(
    current_value: serde_json::Value,
    shape: Option<serde_json::Value>,
    #[allow(unused_variables)] disabled: Signal<bool>,
    on_input: Callback<serde_json::Value>,
) -> AnyView {
    let value_type = setting_shape_type(shape.as_ref())
        .unwrap_or_else(|| json_value_kind(&current_value).to_string());
    let options = setting_shape_options(shape.as_ref());
    let current_raw = setting_option_draft_value(&value_type, &current_value);
    let min = setting_shape_numeric_bound(shape.as_ref(), "min");
    let max = setting_shape_numeric_bound(shape.as_ref(), "max");

    match value_type.as_str() {
        "boolean" if options.is_empty() => {
            let checked = current_value.as_bool().unwrap_or(false);
            view! {
                <label class="inline-flex items-center gap-3 text-sm text-card-foreground">
                    <input
                        type="checkbox"
                        class="h-4 w-4 rounded border-border text-primary focus:ring-primary/20"
                        checked=checked
                        disabled=move || disabled.get()
                        on:change=move |event| {
                            on_input.run(serde_json::Value::Bool(event_target_checked(&event)))
                        }
                    />
                    <span>"Enabled"</span>
                </label>
            }
            .into_any()
        }
        "string" | "integer" | "number" | "boolean" if !options.is_empty() => {
            let options_for_select = options.clone();
            let value_type_for_select = value_type.clone();
            view! {
                <select
                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                    prop:value=current_raw.clone()
                    disabled=move || disabled.get()
                    on:change=move |event| {
                        if let Some(next_value) = parse_scalar_input_value(
                            &event_target_value(&event),
                            &value_type_for_select,
                        ) {
                            on_input.run(next_value);
                        }
                    }
                >
                    {options_for_select.into_iter().map(|option| {
                        let option_value = setting_option_draft_value(&value_type, &option);
                        let option_label = setting_option_label(&option);
                        view! {
                            <option value=option_value>{option_label}</option>
                        }
                    }).collect_view()}
                </select>
            }
            .into_any()
        }
        "integer" | "number" => {
            let step = if value_type == "integer" { "1" } else { "any" };
            let value_type_for_input = value_type.clone();
            view! {
                <input
                    type="number"
                    step=step
                    min=min
                    max=max
                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                    value=current_raw
                    disabled=move || disabled.get()
                    on:input=move |event| {
                        if let Some(next_value) = parse_scalar_input_value(
                            &event_target_value(&event),
                            &value_type_for_input,
                        ) {
                            on_input.run(next_value);
                        }
                    }
                />
            }
            .into_any()
        }
        _ => view! {
            <input
                type="text"
                class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                value=current_raw
                disabled=move || disabled.get()
                on:input=move |event| {
                    on_input.run(serde_json::Value::String(event_target_value(&event)))
                }
            />
        }
        .into_any(),
    }
}

fn default_value_for_schema_shape(shape: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(shape) = shape else {
        return serde_json::Value::Null;
    };

    if let Some(default) = shape.get("default") {
        return default.clone();
    }

    match setting_shape_type(Some(shape)).as_deref() {
        Some("object") => {
            let object = setting_shape_properties(Some(shape))
                .into_iter()
                .map(|(key, property_shape)| {
                    (key, default_value_for_schema_shape(Some(&property_shape)))
                })
                .collect::<serde_json::Map<String, serde_json::Value>>();
            serde_json::Value::Object(object)
        }
        Some("array") => serde_json::json!([]),
        Some(value_type) => default_value_for_setting_type(value_type),
        None => serde_json::Value::Null,
    }
}

fn schema_action_label(shape: Option<&serde_json::Value>) -> String {
    match setting_shape_type(shape).as_deref() {
        Some(value_type) => add_item_button_label(value_type),
        None => "Add item".to_string(),
    }
}

fn pretty_json_value(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn parse_json_editor_value(
    raw: &str,
    expected_type: &str,
) -> Result<Option<serde_json::Value>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let value = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|err| format!("Invalid JSON: {err}"))?;

    match expected_type {
        "object" if !value.is_object() => Err("Expected a JSON object".to_string()),
        "array" if !value.is_array() => Err("Expected a JSON array".to_string()),
        _ => Ok(Some(value)),
    }
}

fn reset_json_editor_value(field_type: &str) -> String {
    let value = match field_type {
        "object" => serde_json::json!({}),
        "array" => serde_json::json!([]),
        "json" | "any" => serde_json::Value::Null,
        _ => serde_json::Value::Null,
    };
    pretty_json_value(&value)
}

fn append_object_property(raw: &str) -> Result<String, String> {
    let mut object = match parse_json_editor_value(raw, "object")? {
        Some(serde_json::Value::Object(object)) => object,
        Some(_) => return Err("Expected a JSON object".to_string()),
        None => serde_json::Map::new(),
    };

    let mut next_index = 1;
    let key = loop {
        let candidate = if next_index == 1 {
            "newKey".to_string()
        } else {
            format!("newKey{}", next_index)
        };
        if !object.contains_key(&candidate) {
            break candidate;
        }
        next_index += 1;
    };
    object.insert(key, serde_json::Value::String(String::new()));
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn append_array_item(raw: &str) -> Result<String, String> {
    let mut array = match parse_json_editor_value(raw, "array")? {
        Some(serde_json::Value::Array(array)) => array,
        Some(_) => return Err("Expected a JSON array".to_string()),
        None => Vec::new(),
    };
    array.push(serde_json::Value::Null);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

fn json_editor_summary(field_type: &str, raw: &str) -> (bool, String, Vec<String>) {
    match parse_json_editor_value(raw, field_type) {
        Ok(Some(serde_json::Value::Object(object))) => {
            let preview = object.keys().take(4).cloned().collect::<Vec<_>>();
            (true, format!("{} keys", object.len()), preview)
        }
        Ok(Some(serde_json::Value::Array(array))) => {
            let preview = array
                .iter()
                .take(4)
                .map(|item| match item {
                    serde_json::Value::Null => "null".to_string(),
                    serde_json::Value::Bool(_) => "bool".to_string(),
                    serde_json::Value::Number(_) => "number".to_string(),
                    serde_json::Value::String(_) => "string".to_string(),
                    serde_json::Value::Array(_) => "array".to_string(),
                    serde_json::Value::Object(_) => "object".to_string(),
                })
                .collect::<Vec<_>>();
            (true, format!("{} items", array.len()), preview)
        }
        Ok(Some(value)) => (true, format!("{} value ready", value), Vec::new()),
        Ok(None) => (
            true,
            "Empty value; server defaults apply if declared.".to_string(),
            Vec::new(),
        ),
        Err(message) => (false, message, Vec::new()),
    }
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn metadata_status_badge_classes(state: &str) -> &'static str {
    match state {
        "ready" => {
            "inline-flex items-center rounded-full border border-emerald-500/40 bg-emerald-500/10 px-2 py-0.5 font-medium text-emerald-700"
        }
        "warn" => {
            "inline-flex items-center rounded-full border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 font-medium text-amber-700"
        }
        _ => {
            "inline-flex items-center rounded-full border border-border px-2 py-0.5 font-medium text-muted-foreground"
        }
    }
}

fn metadata_status_panel_classes(state: &str) -> &'static str {
    match state {
        "ready" => "border-emerald-500/30 bg-emerald-500/5",
        "warn" => "border-amber-500/30 bg-amber-500/5",
        _ => "border-border bg-background",
    }
}

fn looks_like_absolute_http_url(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("https://") || value.starts_with("http://")
}

fn asset_path_without_query(value: &str) -> &str {
    value.split(['?', '#']).next().unwrap_or(value)
}

fn looks_like_svg_url(value: &str) -> bool {
    looks_like_absolute_http_url(value) && asset_path_without_query(value).ends_with(".svg")
}

fn looks_like_image_url(value: &str) -> bool {
    if !looks_like_absolute_http_url(value) {
        return false;
    }

    let lower = asset_path_without_query(value).to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".webp", ".svg"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

fn marketplace_metadata_checklist(module: &MarketplaceModule) -> Vec<MetadataChecklistItem> {
    let description_length = module.description.trim().chars().count();
    let icon_url = module
        .icon_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let banner_url = module
        .banner_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let screenshots_count = module
        .screenshots
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .count();
    let publisher = module
        .publisher
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let has_registry_publish_signal =
        module.checksum_sha256.is_some() || !module.versions.is_empty();

    vec![
        if description_length >= 20 {
            MetadataChecklistItem {
                label: "Description",
                state: "ready",
                priority: "required",
                summary: "Ready",
                detail: format!("{description_length} characters available for catalog detail."),
            }
        } else {
            MetadataChecklistItem {
                label: "Description",
                state: "warn",
                priority: "required",
                summary: "Required",
                detail: "Needs at least 20 characters to satisfy local manifest validation."
                    .to_string(),
            }
        },
        match icon_url {
            Some(value) if looks_like_svg_url(value) => MetadataChecklistItem {
                label: "Icon asset",
                state: "ready",
                priority: "recommended",
                summary: "Ready",
                detail: "Absolute SVG icon is present for registry cards and detail previews."
                    .to_string(),
            },
            Some(_) => MetadataChecklistItem {
                label: "Icon asset",
                state: "warn",
                priority: "required",
                summary: "Required",
                detail: "Icon URL should be an absolute http(s) SVG asset.".to_string(),
            },
            None => MetadataChecklistItem {
                label: "Icon asset",
                state: "warn",
                priority: "recommended",
                summary: "Recommended",
                detail: "Add an SVG icon URL so registry lists and cards have a visual identity."
                    .to_string(),
            },
        },
        match banner_url {
            Some(value) if looks_like_image_url(value) => MetadataChecklistItem {
                label: "Banner asset",
                state: "ready",
                priority: "recommended",
                summary: "Ready",
                detail: "Banner image is present for richer marketplace detail layouts."
                    .to_string(),
            },
            Some(_) => MetadataChecklistItem {
                label: "Banner asset",
                state: "warn",
                priority: "required",
                summary: "Required",
                detail: "Banner URL should be an absolute http(s) image asset.".to_string(),
            },
            None => MetadataChecklistItem {
                label: "Banner asset",
                state: "warn",
                priority: "recommended",
                summary: "Recommended",
                detail:
                    "Optional for local validation, but useful for richer registry presentation."
                        .to_string(),
            },
        },
        if screenshots_count > 0 {
            MetadataChecklistItem {
                label: "Screenshots",
                state: "ready",
                priority: "recommended",
                summary: "Ready",
                detail: format!("{screenshots_count} screenshot(s) available for discovery UX."),
            }
        } else {
            MetadataChecklistItem {
                label: "Screenshots",
                state: "warn",
                priority: "recommended",
                summary: "Recommended",
                detail:
                    "Add one or more screenshots to make module capabilities easier to evaluate."
                        .to_string(),
            }
        },
        if let Some(publisher) = publisher {
            MetadataChecklistItem {
                label: "Publisher identity",
                state: "ready",
                priority: "info",
                summary: "Known",
                detail: format!("Publisher is exposed as {publisher}."),
            }
        } else {
            MetadataChecklistItem {
                label: "Publisher identity",
                state: "info",
                priority: "info",
                summary: "Local only",
                detail: "Workspace modules can stay unpublished; external registry entries should declare a publisher."
                    .to_string(),
            }
        },
        if has_registry_publish_signal {
            MetadataChecklistItem {
                label: "Registry publish signal",
                state: "ready",
                priority: "info",
                summary: "Present",
                detail:
                    "Checksum and/or published versions indicate a registry-backed release trail."
                        .to_string(),
            }
        } else {
            MetadataChecklistItem {
                label: "Registry publish signal",
                state: "info",
                priority: "info",
                summary: "Not published",
                detail:
                    "No checksum or version history is visible yet, which is expected for workspace-only modules."
                        .to_string(),
            }
        },
    ]
}

fn json_value_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(value) => format!("{} items", value.len()),
        serde_json::Value::Object(value) => format!("{} keys", value.len()),
    }
}

fn parse_object_root(raw: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match parse_json_editor_value(raw, "object")? {
        Some(serde_json::Value::Object(object)) => Ok(object),
        Some(_) => Err("Expected a JSON object".to_string()),
        None => Ok(serde_json::Map::new()),
    }
}

fn parse_array_root(raw: &str) -> Result<Vec<serde_json::Value>, String> {
    match parse_json_editor_value(raw, "array")? {
        Some(serde_json::Value::Array(array)) => Ok(array),
        Some(_) => Err("Expected a JSON array".to_string()),
        None => Ok(Vec::new()),
    }
}

fn unique_object_key(
    object: &serde_json::Map<String, serde_json::Value>,
    preferred: &str,
) -> String {
    if !object.contains_key(preferred) {
        return preferred.to_string();
    }

    let mut index = 2;
    loop {
        let candidate = format!("{preferred}{index}");
        if !object.contains_key(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn object_with_new_property(
    raw: &str,
    preferred_key: &str,
    value: serde_json::Value,
) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    let key = unique_object_key(&object, preferred_key);
    object.insert(key, value);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn object_with_updated_property(
    raw: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    object.insert(key.to_string(), value);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn object_without_property(raw: &str, key: &str) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    object.remove(key);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn object_with_renamed_property(raw: &str, old_key: &str, new_key: &str) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    let new_key = new_key.trim();
    if new_key.is_empty() {
        return Err("Property name must not be empty".to_string());
    }
    if old_key == new_key {
        return Ok(pretty_json_value(&serde_json::Value::Object(object)));
    }
    if object.contains_key(new_key) {
        return Err(format!("Property `{new_key}` already exists"));
    }
    let Some(value) = object.remove(old_key) else {
        return Err("Property key is out of bounds".to_string());
    };
    object.insert(new_key.to_string(), value);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

fn array_with_appended_item(raw: &str, value: serde_json::Value) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    array.push(value);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

fn array_with_updated_item(
    raw: &str,
    index: usize,
    value: serde_json::Value,
) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    let Some(item) = array.get_mut(index) else {
        return Err("Array item is out of bounds".to_string());
    };
    *item = value;
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

fn array_without_item(raw: &str, index: usize) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    if index >= array.len() {
        return Err("Array item is out of bounds".to_string());
    }
    array.remove(index);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

fn array_item_moved(raw: &str, index: usize, delta: isize) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    if index >= array.len() {
        return Err("Array item is out of bounds".to_string());
    }
    let next_index = index as isize + delta;
    if next_index < 0 || next_index >= array.len() as isize {
        return Ok(pretty_json_value(&serde_json::Value::Array(array)));
    }
    array.swap(index, next_index as usize);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

#[derive(Clone, Debug)]
enum JsonPathSegment {
    Key(String),
    Index(usize),
}

fn default_json_root(root_type: &str) -> serde_json::Value {
    match root_type {
        "object" => serde_json::json!({}),
        "array" => serde_json::json!([]),
        _ => serde_json::Value::Null,
    }
}

fn default_value_for_setting_type(value_type: &str) -> serde_json::Value {
    match value_type {
        "string" => serde_json::Value::String(String::new()),
        "integer" | "number" => serde_json::json!(0),
        "boolean" => serde_json::Value::Bool(false),
        "object" => serde_json::json!({}),
        "array" => serde_json::json!([]),
        "json" | "any" => serde_json::Value::Null,
        _ => serde_json::Value::Null,
    }
}

fn add_item_button_label(value_type: &str) -> String {
    match value_type {
        "string" => "Add text".to_string(),
        "boolean" => "Add flag".to_string(),
        "integer" | "number" => "Add number".to_string(),
        "object" => "Add object".to_string(),
        "array" => "Add array".to_string(),
        "json" | "any" => "Add item".to_string(),
        _ => format!("Add {}", humanize_token(value_type)),
    }
}

fn parse_json_root(raw: &str, root_type: &str) -> Result<serde_json::Value, String> {
    Ok(parse_json_editor_value(raw, root_type)?.unwrap_or_else(|| default_json_root(root_type)))
}

fn value_at_path_mut<'a>(
    value: &'a mut serde_json::Value,
    path: &[JsonPathSegment],
) -> Option<&'a mut serde_json::Value> {
    let mut current = value;
    for segment in path {
        match segment {
            JsonPathSegment::Key(key) => current = current.as_object_mut()?.get_mut(key)?,
            JsonPathSegment::Index(index) => current = current.as_array_mut()?.get_mut(*index)?,
        }
    }
    Some(current)
}

fn with_updated_json_root(
    raw: &str,
    root_type: &str,
    updater: impl FnOnce(&mut serde_json::Value) -> Result<(), String>,
) -> Result<String, String> {
    let mut root = parse_json_root(raw, root_type)?;
    updater(&mut root)?;
    Ok(pretty_json_value(&root))
}

fn nested_value_updated(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    next_value: serde_json::Value,
) -> Result<String, String> {
    with_updated_json_root(raw, root_type, |root| {
        let Some(target) = value_at_path_mut(root, path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        *target = next_value;
        Ok(())
    })
}

fn nested_value_removed(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
) -> Result<String, String> {
    if path.is_empty() {
        return Ok(pretty_json_value(&default_json_root(root_type)));
    }

    let parent_path = &path[..path.len() - 1];
    let last_segment = path.last().expect("checked non-empty path");
    with_updated_json_root(raw, root_type, |root| {
        let Some(parent) = value_at_path_mut(root, parent_path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        match (parent, last_segment) {
            (serde_json::Value::Object(object), JsonPathSegment::Key(key)) => {
                object.remove(key);
                Ok(())
            }
            (serde_json::Value::Array(array), JsonPathSegment::Index(index)) => {
                if *index >= array.len() {
                    return Err("Array item is out of bounds".to_string());
                }
                array.remove(*index);
                Ok(())
            }
            _ => Err("JSON path does not match the current structure".to_string()),
        }
    })
}

fn nested_object_key_renamed(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    new_key: &str,
) -> Result<String, String> {
    if path.is_empty() {
        return Err("JSON path is out of bounds".to_string());
    }
    let new_key = new_key.trim();
    if new_key.is_empty() {
        return Err("Property name must not be empty".to_string());
    }
    let parent_path = &path[..path.len() - 1];
    let JsonPathSegment::Key(old_key) = path.last().expect("checked non-empty path") else {
        return Err("JSON path does not point to an object property".to_string());
    };
    with_updated_json_root(raw, root_type, |root| {
        let Some(parent) = value_at_path_mut(root, parent_path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        let Some(object) = parent.as_object_mut() else {
            return Err("Expected a JSON object".to_string());
        };
        if old_key == new_key {
            return Ok(());
        }
        if object.contains_key(new_key) {
            return Err(format!("Property `{new_key}` already exists"));
        }
        let Some(value) = object.remove(old_key) else {
            return Err("JSON path is out of bounds".to_string());
        };
        object.insert(new_key.to_string(), value);
        Ok(())
    })
}

fn nested_array_item_moved(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    delta: isize,
) -> Result<String, String> {
    if path.is_empty() {
        return Err("JSON path is out of bounds".to_string());
    }
    let parent_path = &path[..path.len() - 1];
    let JsonPathSegment::Index(index) = path.last().expect("checked non-empty path") else {
        return Err("JSON path does not point to an array item".to_string());
    };
    with_updated_json_root(raw, root_type, |root| {
        let Some(parent) = value_at_path_mut(root, parent_path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        let Some(array) = parent.as_array_mut() else {
            return Err("Expected a JSON array".to_string());
        };
        if *index >= array.len() {
            return Err("Array item is out of bounds".to_string());
        }
        let next_index = *index as isize + delta;
        if next_index < 0 || next_index >= array.len() as isize {
            return Ok(());
        }
        array.swap(*index, next_index as usize);
        Ok(())
    })
}

fn nested_object_child_added(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    preferred_key: &str,
    value: serde_json::Value,
) -> Result<String, String> {
    with_updated_json_root(raw, root_type, |root| {
        let Some(target) = value_at_path_mut(root, path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        let Some(object) = target.as_object_mut() else {
            return Err("Expected a JSON object".to_string());
        };
        let key = unique_object_key(object, preferred_key);
        object.insert(key, value);
        Ok(())
    })
}

fn nested_array_child_added(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    value: serde_json::Value,
) -> Result<String, String> {
    with_updated_json_root(raw, root_type, |root| {
        let Some(target) = value_at_path_mut(root, path) else {
            return Err("JSON path is out of bounds".to_string());
        };
        let Some(array) = target.as_array_mut() else {
            return Err("Expected a JSON array".to_string());
        };
        array.push(value);
        Ok(())
    })
}

fn nested_object_contains_key(
    raw: &str,
    root_type: &str,
    path: &[JsonPathSegment],
    key: &str,
) -> bool {
    let Ok(mut root) = parse_json_root(raw, root_type) else {
        return false;
    };

    value_at_path_mut(&mut root, path)
        .and_then(|target| target.as_object().map(|object| object.contains_key(key)))
        .unwrap_or(false)
}

fn render_nested_json_children(
    root_type: String,
    root_value: Signal<String>,
    path: Vec<JsonPathSegment>,
    current: serde_json::Value,
    current_shape: Option<serde_json::Value>,
    disabled: Signal<bool>,
    on_input: Callback<String>,
) -> AnyView {
    match current {
        serde_json::Value::Object(object) => {
            let declared_properties = setting_shape_properties(current_shape.as_ref());
            let schema_locks_keys = !declared_properties.is_empty();
            view! {
            <div class="space-y-3">
                <div class="flex flex-wrap gap-2">
                    {if declared_properties.is_empty() {
                        view! {
                            <>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newText", serde_json::Value::String(String::new())) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add text"</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newFlag", serde_json::Value::Bool(false)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add flag"</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newNumber", serde_json::json!(0)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add number"</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newObject", serde_json::json!({})) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add object"</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newArray", serde_json::json!([])) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add array"</button>
                            </>
                        }.into_any()
                    } else {
                        declared_properties.clone().into_iter().map(|(property_key, property_shape)| {
                            let button_label = format!("Add {}", humanize_setting_key(&property_key));
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                    disabled={
                                        let root_type = root_type.clone();
                                        let root_value = root_value;
                                        let path = path.clone();
                                        let property_key = property_key.clone();
                                        move || {
                                            disabled.get()
                                                || nested_object_contains_key(
                                                    &root_value.get(),
                                                    &root_type,
                                                    &path,
                                                    &property_key,
                                                )
                                        }
                                    }
                                    on:click={
                                        let root_type = root_type.clone();
                                        let root_value = root_value;
                                        let path = path.clone();
                                        let property_key = property_key.clone();
                                        let property_shape = property_shape.clone();
                                        move |_| {
                                            if let Ok(next) = nested_object_child_added(
                                                &root_value.get(),
                                                &root_type,
                                                &path,
                                                &property_key,
                                                default_value_for_schema_shape(Some(&property_shape)),
                                            ) {
                                                on_input.run(next);
                                            }
                                        }
                                    }
                                >
                                    {button_label}
                                </button>
                            }
                        }).collect_view().into_any()
                    }}
                </div>
                {object.into_iter().map(|(key, item_value)| {
                    let kind = json_value_kind(&item_value).to_string();
                    let preview = json_value_preview(&item_value);
                    let property_shape = setting_shape_property(current_shape.as_ref(), &key);
                    let mut item_path = path.clone();
                    item_path.push(JsonPathSegment::Key(key.clone()));
                    match item_value.clone() {
                        scalar_value @ (serde_json::Value::String(_) | serde_json::Value::Bool(_) | serde_json::Value::Number(_)) => {
                            let item_path_for_input = item_path.clone();
                            let item_path_for_remove = item_path.clone();
                            let item_path_for_rename = item_path.clone();
                            view! {
                                <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <input type="text" class="rounded-md border border-border bg-background px-2 py-1 text-sm font-medium text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70" value=key.clone() disabled=move || disabled.get() || schema_locks_keys on:change={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |event| {
                                                    if let Ok(next) = nested_object_key_renamed(&root_value.get(), &root_type, &item_path_for_rename, &event_target_value(&event)) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            } />
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                            let root_type = root_type.clone();
                                            let root_value = root_value;
                                            move |_| {
                                                if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }>"Remove"</button>
                                    </div>
                                    {render_scalar_value_editor(
                                        scalar_value,
                                        property_shape.clone(),
                                        disabled,
                                        Callback::new({
                                            let root_type = root_type.clone();
                                            let root_value = root_value;
                                            move |next_value| {
                                                if let Ok(next) = nested_value_updated(
                                                    &root_value.get(),
                                                    &root_type,
                                                    &item_path_for_input,
                                                    next_value,
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }),
                                    )}
                                </div>
                            }.into_any()
                        }
                        _ => {
                            let item_path_for_remove = item_path.clone();
                            let item_path_for_rename = item_path.clone();
                            view! {
                                <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <input type="text" class="rounded-md border border-border bg-background px-2 py-1 text-sm font-medium text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70" value=key.clone() disabled=move || disabled.get() || schema_locks_keys on:change={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |event| {
                                                    if let Ok(next) = nested_object_key_renamed(&root_value.get(), &root_type, &item_path_for_rename, &event_target_value(&event)) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            } />
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                            let root_type = root_type.clone();
                                            let root_value = root_value;
                                            move |_| {
                                                if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }>"Remove"</button>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{preview}</p>
                                    {render_nested_json_children(root_type.clone(), root_value, item_path.clone(), item_value, property_shape.clone(), disabled, on_input)}
                                </div>
                            }.into_any()
                        }
                    }
                }).collect_view()}
            </div>
        }.into_any()
        }
        serde_json::Value::Array(items) => {
            let item_shape = setting_shape_items(current_shape.as_ref());
            view! {
            <div class="space-y-3">
                <div class="flex flex-wrap gap-2">
                    {if let Some(item_shape) = item_shape.clone() {
                        let button_label = schema_action_label(Some(&item_shape));
                        view! {
                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                let root_type = root_type.clone();
                                let root_value = root_value;
                                let path = path.clone();
                                let item_shape = item_shape.clone();
                                move |_| {
                                    if let Ok(next) = nested_array_child_added(
                                        &root_value.get(),
                                        &root_type,
                                        &path,
                                        default_value_for_schema_shape(Some(&item_shape)),
                                    ) {
                                        on_input.run(next);
                                    }
                                }
                            }>{button_label}</button>
                        }.into_any()
                    } else {
                        view! {
                            <>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::Value::String(String::new())) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add text"</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::Value::Bool(false)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add flag"</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!(0)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add number"</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!({})) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add object"</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let root_value = root_value;
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!([])) {
                                            on_input.run(next);
                                        }
                                    }
                                }>"Add array"</button>
                            </>
                        }.into_any()
                    }}
                </div>
                {items.into_iter().enumerate().map(|(index, item_value)| {
                    let kind = json_value_kind(&item_value).to_string();
                    let preview = json_value_preview(&item_value);
                    let mut item_path = path.clone();
                    item_path.push(JsonPathSegment::Index(index));
                    match item_value.clone() {
                        scalar_value @ (serde_json::Value::String(_)
                        | serde_json::Value::Bool(_)
                        | serde_json::Value::Number(_)) => {
                            let item_path_for_input = item_path.clone();
                            let item_path_for_remove = item_path.clone();
                            let item_path_for_move_up = item_path.clone();
                            let item_path_for_move_down = item_path.clone();
                            view! {
                                <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <span class="text-sm font-medium text-card-foreground">{format!("Item {}", index + 1)}</span>
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_up, -1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>"Up"</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_down, 1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>"Down"</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>"Remove"</button>
                                        </div>
                                    </div>
                                    {render_scalar_value_editor(
                                        scalar_value,
                                        item_shape.clone(),
                                        disabled,
                                        Callback::new({
                                            let root_type = root_type.clone();
                                            let root_value = root_value;
                                            move |next_value| {
                                                if let Ok(next) = nested_value_updated(
                                                    &root_value.get(),
                                                    &root_type,
                                                    &item_path_for_input,
                                                    next_value,
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }),
                                    )}
                                </div>
                            }.into_any()
                        }
                        _ => {
                            let item_path_for_remove = item_path.clone();
                            let item_path_for_move_up = item_path.clone();
                            let item_path_for_move_down = item_path.clone();
                            view! {
                                <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                    <div class="flex flex-wrap items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <span class="text-sm font-medium text-card-foreground">{format!("Item {}", index + 1)}</span>
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_up, -1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>"Up"</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_down, 1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>"Down"</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                let root_value = root_value;
                                                move |_| {
                                                    if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>"Remove"</button>
                                        </div>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{preview}</p>
                                    {render_nested_json_children(root_type.clone(), root_value, item_path.clone(), item_value, item_shape.clone(), disabled, on_input)}
                                </div>
                            }.into_any()
                        }
                    }
                }).collect_view()}
            </div>
        }.into_any()
        }
        _ => ().into_any(),
    }
}

#[component]
fn StructuredObjectEditor(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    object_shape: Option<serde_json::Value>,
    on_input: Callback<String>,
) -> impl IntoView {
    let object_entries = Signal::derive(move || parse_object_root(&value.get()));
    let declared_properties = setting_shape_properties(object_shape.as_ref());
    let object_shape_for_items = StoredValue::new(object_shape.clone());
    let schema_locks_keys = !declared_properties.is_empty();

    view! {
        <Show when=move || object_entries.get().is_ok()>
            <div class="rounded-lg border border-dashed border-border bg-muted/30 p-3">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        "Structured object editor"
                    </p>
                    <div class="flex flex-wrap gap-2">
                        {if declared_properties.is_empty() {
                            view! {
                                <>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = object_with_new_property(
                                                    &value.get(),
                                                    "newText",
                                                    serde_json::Value::String(String::new()),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        "Add text"
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = object_with_new_property(
                                                    &value.get(),
                                                    "newFlag",
                                                    serde_json::Value::Bool(false),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        "Add flag"
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = object_with_new_property(
                                                    &value.get(),
                                                    "newNumber",
                                                    serde_json::json!(0),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        "Add number"
                                    </button>
                                </>
                            }.into_any()
                        } else {
                            declared_properties
                                .clone()
                                .into_iter()
                                .map(|(property_key, property_shape)| {
                                    let button_label = format!(
                                        "Add {}",
                                        humanize_setting_key(&property_key)
                                    );
                                    view! {
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                            disabled={
                                                let value = value;
                                                let property_key = property_key.clone();
                                                move || {
                                                    disabled.get()
                                                        || parse_object_root(&value.get())
                                                            .map(|object| object.contains_key(&property_key))
                                                            .unwrap_or(false)
                                                }
                                            }
                                            on:click={
                                                let value = value;
                                                let property_key = property_key.clone();
                                                let property_shape = property_shape.clone();
                                                move |_| {
                                                    if let Ok(next) = object_with_updated_property(
                                                        &value.get(),
                                                        &property_key,
                                                        default_value_for_schema_shape(Some(&property_shape)),
                                                    ) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }
                                        >
                                            {button_label}
                                        </button>
                                    }
                                })
                                .collect_view()
                                .into_any()
                        }}
                    </div>
                </div>
                <div class="mt-3 space-y-3">
                    {move || {
                        let object_shape_for_items = object_shape_for_items.get_value();
                        match object_entries.get() {
                        Ok(object) if object.is_empty() => view! {
                            <p class="text-sm text-muted-foreground">
                                "Object is empty. Use the quick actions to add top-level properties."
                            </p>
                        }.into_any(),
                        Ok(object) => object
                            .into_iter()
                            .map(|(key, item_value)| {
                                let kind = json_value_kind(&item_value).to_string();
                                let preview = json_value_preview(&item_value);
                                let property_shape = setting_shape_property(object_shape_for_items.as_ref(), &key);
                                let key_for_remove = key.clone();
                                let key_for_rename = key.clone();
                                let mut item_path = Vec::new();
                                item_path.push(JsonPathSegment::Key(key.clone()));
                                view! {
                                    <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                        <div class="flex flex-wrap items-center justify-between gap-2">
                                            <div class="flex flex-wrap items-center gap-2">
                                                <input type="text" class="rounded-md border border-border bg-background px-2 py-1 text-sm font-medium text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70" value=key.clone() disabled=move || disabled.get() || schema_locks_keys on:change={
                                                    let value = value;
                                                    move |event| {
                                                        if let Ok(next) = object_with_renamed_property(&value.get(), &key_for_rename, &event_target_value(&event)) {
                                                            on_input.run(next);
                                                        }
                                                    }
                                                } />
                                                <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                    {kind.clone()}
                                                </span>
                                            </div>
                                            <button
                                                type="button"
                                                class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                disabled=move || disabled.get()
                                                on:click={
                                                    let value = value;
                                                    move |_| {
                                                        if let Ok(next) = object_without_property(&value.get(), &key_for_remove) {
                                                            on_input.run(next);
                                                        }
                                                    }
                                                }
                                            >
                                                "Remove"
                                            </button>
                                        </div>
                                        {match item_value {
                                            scalar_value @ (serde_json::Value::String(_)
                                            | serde_json::Value::Bool(_)
                                            | serde_json::Value::Number(_)) => {
                                                let key_for_input = key.clone();
                                                view! {
                                                    {render_scalar_value_editor(
                                                        scalar_value,
                                                        property_shape.clone(),
                                                        disabled,
                                                        Callback::new({
                                                            let value = value;
                                                            move |next_value| {
                                                                if let Ok(next) = object_with_updated_property(
                                                                    &value.get(),
                                                                    &key_for_input,
                                                                    next_value,
                                                                ) {
                                                                    on_input.run(next);
                                                                }
                                                            }
                                                        }),
                                                    )}
                                                }.into_any()
                                            }
                                            nested_value => {
                                                let nested_path = item_path.clone();
                                                let nested_shape = property_shape.clone();
                                                view! {
                                                    <>
                                                        <p class="text-sm text-muted-foreground">
                                                            {format!("Nested {}: {}.", kind, preview)}
                                                        </p>
                                                        {render_nested_json_children(
                                                            "object".to_string(),
                                                            value,
                                                            nested_path,
                                                            nested_value,
                                                            nested_shape,
                                                            disabled,
                                                            on_input,
                                                        )}
                                                    </>
                                                }.into_any()
                                            }
                                        }}
                                    </div>
                                }
                            })
                            .collect_view()
                            .into_any(),
                        Err(_) => ().into_any(),
                    }}}
                </div>
            </div>
        </Show>
    }
}

#[component]
fn StructuredArrayEditor(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    array_item_type: Option<String>,
    array_item_shape: Option<serde_json::Value>,
    on_input: Callback<String>,
) -> impl IntoView {
    let array_entries = Signal::derive(move || parse_array_root(&value.get()));
    let array_item_shape_for_items = StoredValue::new(array_item_shape.clone());

    view! {
        <Show when=move || array_entries.get().is_ok()>
            <div class="rounded-lg border border-dashed border-border bg-muted/30 p-3">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        "Structured array editor"
                    </p>
                    <div class="flex flex-wrap gap-2">
                        {if let Some(item_shape) = array_item_shape.clone() {
                            let button_label = schema_action_label(Some(&item_shape));
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                    disabled=move || disabled.get()
                                    on:click={
                                        let value = value;
                                        let item_shape = item_shape.clone();
                                        move |_| {
                                            if let Ok(next) = array_with_appended_item(
                                                &value.get(),
                                                default_value_for_schema_shape(Some(&item_shape)),
                                            ) {
                                                on_input.run(next);
                                            }
                                        }
                                    }
                                >
                                    {button_label}
                                </button>
                            }.into_any()
                        } else if let Some(item_type) = array_item_type
                            .clone()
                            .map(|value| value.trim().to_string())
                            .filter(|value| !value.is_empty())
                        {
                            let button_label = add_item_button_label(&item_type);
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                    disabled=move || disabled.get()
                                    on:click={
                                        let value = value;
                                        let item_type = item_type.clone();
                                        move |_| {
                                            if let Ok(next) = array_with_appended_item(
                                                &value.get(),
                                                default_value_for_setting_type(&item_type),
                                            ) {
                                                on_input.run(next);
                                            }
                                        }
                                    }
                                >
                                    {button_label}
                                </button>
                            }.into_any()
                        } else {
                            view! {
                                <>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = array_with_appended_item(
                                                    &value.get(),
                                                    serde_json::Value::String(String::new()),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        "Add text"
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) = array_with_appended_item(
                                                    &value.get(),
                                                    serde_json::Value::Bool(false),
                                                ) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        "Add flag"
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            let value = value;
                                            move |_| {
                                                if let Ok(next) =
                                                    array_with_appended_item(&value.get(), serde_json::json!(0))
                                                {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        "Add number"
                                    </button>
                                </>
                            }.into_any()
                        }}
                    </div>
                </div>
                <div class="mt-3 space-y-3">
                    {move || {
                        let array_item_shape_for_items = array_item_shape_for_items.get_value();
                        match array_entries.get() {
                        Ok(items) if items.is_empty() => view! {
                            <p class="text-sm text-muted-foreground">
                                "Array is empty. Use the quick actions to add top-level items."
                            </p>
                        }.into_any(),
                        Ok(items) => items
                            .into_iter()
                            .enumerate()
                            .map(|(index, item_value)| {
                                let kind = json_value_kind(&item_value).to_string();
                                let preview = json_value_preview(&item_value);
                                let mut item_path = Vec::new();
                                item_path.push(JsonPathSegment::Index(index));
                                view! {
                                    <div class="space-y-2 rounded-md border border-border bg-background px-3 py-3">
                                        <div class="flex flex-wrap items-center justify-between gap-2">
                                            <div class="flex flex-wrap items-center gap-2">
                                                <span class="text-sm font-medium text-card-foreground">{format!("Item {}", index + 1)}</span>
                                                <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                    {kind.clone()}
                                                </span>
                                            </div>
                                            <div class="flex flex-wrap gap-2">
                                                <button
                                                    type="button"
                                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                    disabled=move || disabled.get()
                                                    on:click={
                                                        let value = value;
                                                        move |_| {
                                                            if let Ok(next) = array_item_moved(&value.get(), index, -1) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    "Up"
                                                </button>
                                                <button
                                                    type="button"
                                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                    disabled=move || disabled.get()
                                                    on:click={
                                                        let value = value;
                                                        move |_| {
                                                            if let Ok(next) = array_item_moved(&value.get(), index, 1) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    "Down"
                                                </button>
                                                <button
                                                    type="button"
                                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                    disabled=move || disabled.get()
                                                    on:click={
                                                        let value = value;
                                                        move |_| {
                                                            if let Ok(next) = array_without_item(&value.get(), index) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    "Remove"
                                                </button>
                                            </div>
                                        </div>
                                        {match item_value {
                                            scalar_value @ (serde_json::Value::String(_)
                                            | serde_json::Value::Bool(_)
                                            | serde_json::Value::Number(_)) => {
                                                view! {
                                                    {render_scalar_value_editor(
                                                        scalar_value,
                                                        array_item_shape_for_items.clone(),
                                                        disabled,
                                                        Callback::new({
                                                            let value = value;
                                                            move |next_value| {
                                                                if let Ok(next) = array_with_updated_item(
                                                                    &value.get(),
                                                                    index,
                                                                    next_value,
                                                                ) {
                                                                    on_input.run(next);
                                                                }
                                                            }
                                                        }),
                                                    )}
                                                }.into_any()
                                            }
                                            nested_value => {
                                                let nested_path = item_path.clone();
                                                let nested_shape = array_item_shape_for_items.clone();
                                                view! {
                                                    <>
                                                        <p class="text-sm text-muted-foreground">
                                                            {format!("Nested {}: {}.", kind, preview)}
                                                        </p>
                                                    {render_nested_json_children(
                                                        "array".to_string(),
                                                        value,
                                                        nested_path,
                                                        nested_value,
                                                        nested_shape.or_else(|| array_item_shape_for_items.clone()),
                                                        disabled,
                                                        on_input,
                                                    )}
                                                    </>
                                                }.into_any()
                                            }
                                        }}
                                    </div>
                                }
                            })
                            .collect_view()
                            .into_any(),
                        Err(_) => ().into_any(),
                    }}}
                </div>
            </div>
        </Show>
    }
}

#[component]
fn ComplexSettingEditor(
    field_type: String,
    placeholder: &'static str,
    array_item_type: Option<String>,
    schema_shape: Option<serde_json::Value>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    on_input: Callback<String>,
) -> impl IntoView {
    let status = Signal::derive({
        let value = value;
        let field_type = field_type.clone();
        move || json_editor_summary(&field_type, &value.get())
    });
    let nested_root = Signal::derive({
        let value = value;
        let field_type = field_type.clone();
        move || parse_json_root(&value.get(), &field_type).ok()
    });

    let show_add_button = matches!(field_type.as_str(), "object" | "array");
    let add_button_label = if field_type == "object" {
        "Add property"
    } else {
        "Add item"
    };

    view! {
        <div class="space-y-3">
            <div class="flex flex-wrap items-center gap-2 text-xs">
                <span class=move || {
                    if status.get().0 {
                        "inline-flex items-center rounded-full border border-border px-2 py-0.5 font-medium text-muted-foreground"
                    } else {
                        "inline-flex items-center rounded-full border border-destructive/40 bg-destructive/10 px-2 py-0.5 font-medium text-destructive"
                    }
                }>
                    {move || status.get().1}
                </span>
                <Show when=move || !status.get().2.is_empty()>
                    <div class="flex flex-wrap gap-1">
                        {move || status.get().2.into_iter().map(|item| {
                            view! {
                                <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] text-muted-foreground">
                                    {item}
                                </span>
                            }
                        }).collect_view()}
                    </div>
                </Show>
            </div>
            {if field_type == "object" {
                view! { <StructuredObjectEditor value=value disabled=disabled object_shape=schema_shape.clone() on_input=on_input /> }.into_any()
            } else if field_type == "array" {
                view! {
                    <StructuredArrayEditor
                        value=value
                        disabled=disabled
                        array_item_type=array_item_type.clone()
                        array_item_shape=setting_shape_items(schema_shape.as_ref())
                        on_input=on_input
                    />
                }.into_any()
            } else {
                ().into_any()
            }}
            {if matches!(field_type.as_str(), "json" | "any") {
                {
                    let field_type_for_nested = field_type.clone();
                    move || {
                    nested_root
                        .get()
                        .filter(|value| matches!(value, serde_json::Value::Object(_) | serde_json::Value::Array(_)))
                        .map(|root| {
                            view! {
                                <div class="space-y-2 rounded-lg border border-border/60 bg-background/60 p-3">
                                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                                        "Nested editor"
                                    </p>
                                    {render_nested_json_children(
                                        field_type_for_nested.clone(),
                                        value,
                                        Vec::new(),
                                        root,
                                        schema_shape.clone(),
                                        disabled,
                                        on_input,
                                    )}
                                </div>
                            }
                        })
                    }
                }.into_any()
            } else {
                ().into_any()
            }}
            <div class="flex flex-wrap items-center gap-2">
                <button
                    type="button"
                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                    disabled=move || disabled.get()
                    on:click={
                        let value = value;
                        let field_type = field_type.clone();
                        move |_| {
                            match parse_json_editor_value(&value.get(), &field_type) {
                                Ok(Some(parsed)) => on_input.run(pretty_json_value(&parsed)),
                                Ok(None) => on_input.run(reset_json_editor_value(&field_type)),
                                Err(_) => {}
                            }
                        }
                    }
                >
                    "Format JSON"
                </button>
                <button
                    type="button"
                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                    disabled=move || disabled.get()
                    on:click={
                        let field_type = field_type.clone();
                        move |_| on_input.run(reset_json_editor_value(&field_type))
                    }
                >
                    "Reset"
                </button>
                {if show_add_button {
                    view! {
                        <button
                            type="button"
                            class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                            disabled=move || disabled.get()
                            on:click={
                                let value = value;
                                let field_type = field_type.clone();
                                move |_| {
                                    let next = match field_type.as_str() {
                                        "object" => append_object_property(&value.get()),
                                        "array" => append_array_item(&value.get()),
                                        _ => Ok(value.get()),
                                    };
                                    if let Ok(next) = next {
                                        on_input.run(next);
                                    }
                                }
                            }
                        >
                            {add_button_label}
                        </button>
                    }.into_any()
                } else {
                    ().into_any()
                }}
            </div>
            <textarea
                class="min-h-32 w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                prop:value=move || value.get()
                prop:placeholder=placeholder
                disabled=move || disabled.get()
                on:input=move |event| on_input.run(event_target_value(&event))
            ></textarea>
        </div>
    }
}

#[component]
pub fn ModuleDetailPanel(
    admin_surface: String,
    selected_slug: String,
    module: Option<MarketplaceModule>,
    tenant_module: Option<TenantModule>,
    settings_schema: Vec<ModuleSettingField>,
    #[prop(into)] settings_form_supported: Signal<bool>,
    #[prop(into)] settings_form_draft: Signal<HashMap<String, String>>,
    #[prop(into)] settings_draft: Signal<String>,
    #[prop(into)] settings_editable: Signal<bool>,
    #[prop(into)] settings_saving: Signal<bool>,
    #[prop(into)] loading: Signal<bool>,
    on_settings_field_input: Callback<(String, String)>,
    on_settings_input: Callback<String>,
    on_save_settings: Callback<()>,
    on_close: Callback<()>,
) -> impl IntoView {
    let detail = module.clone();
    let detail_for_body = StoredValue::new(module.clone());
    let admin_surface_for_body = StoredValue::new(admin_surface.clone());
    let selected_slug_for_body = StoredValue::new(selected_slug.clone());
    let tenant_module_for_body = StoredValue::new(tenant_module.clone());
    let settings_schema_for_body = StoredValue::new(settings_schema.clone());

    view! {
        <div class="rounded-xl border border-primary/20 bg-primary/5 p-6 shadow-sm">
            <div class="flex items-start justify-between gap-3">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">"Module detail"</h3>
                    <p class="text-sm text-muted-foreground">
                        {match detail.as_ref() {
                            Some(module) => format!(
                                "{} metadata from the internal marketplace catalog.",
                                module.name
                            ),
                            None if loading.get() => format!(
                                "Loading {} from the internal marketplace catalog.",
                                selected_slug
                            ),
                            None => format!("No catalog entry resolved for {}.", selected_slug),
                        }}
                    </p>
                </div>
                <button
                    type="button"
                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition-colors hover:bg-accent"
                    on:click=move |_| on_close.run(())
                >
                    "Close"
                </button>
            </div>

            <Show
                when=move || detail.is_some()
                fallback=move || view! {
                    <p class="mt-4 text-sm text-muted-foreground">
                        "The selected module is not available in the current catalog snapshot."
                    </p>
                }
            >
                {move || {
                    detail_for_body.get_value().as_ref().map(|module| {
                        let module = module.clone();
                        let module_name = module.name.clone();
                        let module_tags = module.tags.clone();
                        let module_tags_for_show = module_tags.clone();
                        let module_icon_url = module.icon_url.clone();
                        let module_banner_url = module.banner_url.clone();
                        let module_banner_url_for_body = module_banner_url.clone();
                        let module_screenshots = module.screenshots.clone();
                        let module_screenshots_for_body = module_screenshots.clone();
                        let has_marketplace_visuals = module_banner_url.is_some() || !module_screenshots.is_empty();
                        let has_marketplace_screenshots = !module_screenshots.is_empty();
                        let metadata_checklist = marketplace_metadata_checklist(&module);
                        let metadata_checklist_for_show = metadata_checklist.clone();
                        let metadata_required_issues = metadata_checklist
                            .iter()
                            .filter(|item| item.state == "warn" && item.priority == "required")
                            .count();
                        let metadata_recommended_gaps = metadata_checklist
                            .iter()
                            .filter(|item| item.state == "warn" && item.priority == "recommended")
                            .count();
                        let metadata_ready_count = metadata_checklist
                            .iter()
                            .filter(|item| item.state == "ready")
                            .count();
                        let version_trail = module.versions.clone().into_iter().take(5).collect::<Vec<_>>();
                        let checksum = short_checksum(module.checksum_sha256.as_deref());
                        let admin_surface = admin_surface_for_body.get_value();
                        let primary_here = module
                            .recommended_admin_surfaces
                            .iter()
                            .any(|surface| surface == &admin_surface);
                        let showcase_here = module
                            .showcase_admin_surfaces
                            .iter()
                            .any(|surface| surface == &admin_surface);
                        view! {
                            <div class="mt-4 space-y-4">
                                <div class="space-y-2">
                                    <div class="flex flex-wrap items-center gap-2">
                                        {module_icon_url.clone().map(|icon_url| {
                                            let module_name = module_name.clone();
                                            view! {
                                                <img
                                                    class="h-10 w-10 rounded-lg border border-border bg-background object-cover"
                                                    src=icon_url
                                                    alt=format!("{} icon", module_name)
                                                />
                                            }
                                        })}
                                        <h4 class="text-lg font-semibold text-card-foreground">{module.name.clone()}</h4>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {format!("v{}", module.latest_version)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                            {humanize_token(&module.source)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {humanize_token(&module.category)}
                                        </span>
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {if module.compatible { "Compatible" } else { "Compatibility risk" }}
                                        </span>
                                        {module.signature_present.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                "Signed"
                                            </span>
                                        })}
                                        {module.installed.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                {format!(
                                                    "Installed{}",
                                                    module
                                                        .installed_version
                                                        .as_ref()
                                                        .map(|value| format!(" v{}", value))
                                                        .unwrap_or_default()
                                                )}
                                            </span>
                                        })}
                                        {module.update_available.then(|| view! {
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                "Update available"
                                            </span>
                                        })}
                                    </div>
                                    <Show when=move || !module_tags_for_show.is_empty()>
                                        <div class="flex flex-wrap items-center gap-2 text-xs">
                                            {module_tags.clone().into_iter().map(|tag| {
                                                view! {
                                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                        {format!("#{}", tag)}
                                                    </span>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                    <p class="text-sm text-muted-foreground">{module.description.clone()}</p>
                                </div>

                                <div class="flex flex-wrap items-center gap-2 text-xs">
                                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 font-semibold text-secondary-foreground">
                                        {humanize_token(&module.ownership)}
                                    </span>
                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                        {humanize_token(&module.trust_level)}
                                    </span>
                                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                        {if primary_here {
                                            "Primary for this admin"
                                        } else if showcase_here {
                                            "Showcase for this admin"
                                        } else {
                                            "No dedicated UI for this admin"
                                        }}
                                    </span>
                                </div>

                                <div class="grid gap-4 lg:grid-cols-2">
                                    <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">"Package metadata"</p>
                                        <dl class="mt-3 space-y-2">
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"Slug"</dt>
                                                <dd class="font-mono text-right">{module.slug.clone()}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"Crate"</dt>
                                                <dd class="font-mono text-right">{module.crate_name.clone()}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"Publisher"</dt>
                                                <dd class="text-right">{module.publisher.clone().unwrap_or_else(|| "Workspace / unknown".to_string())}</dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"RusTok range"</dt>
                                                <dd class="text-right">
                                                    {format!(
                                                        "{}{}",
                                                        module
                                                            .rustok_min_version
                                                            .as_ref()
                                                            .map(|value| format!(">= {}", value))
                                                            .unwrap_or_else(|| "no min".to_string()),
                                                        module
                                                            .rustok_max_version
                                                            .as_ref()
                                                            .map(|value| format!(", <= {}", value))
                                                            .unwrap_or_else(|| ", no max".to_string())
                                                    )}
                                                </dd>
                                            </div>
                                            <div class="flex items-start justify-between gap-3">
                                                <dt class="text-muted-foreground">"Checksum"</dt>
                                                <dd class="font-mono text-right">{checksum.unwrap_or_else(|| "Not published".to_string())}</dd>
                                            </div>
                                        </dl>
                                    </div>

                                <div class="rounded-lg border border-border bg-background/70 p-4 text-sm">
                                    <p class="text-xs uppercase tracking-wide text-muted-foreground">"Surface policy"</p>
                                    <div class="mt-3 space-y-3">
                                            <div class="flex flex-wrap gap-2">
                                                {if module.recommended_admin_surfaces.is_empty() {
                                                    view! {
                                                        <span class="text-xs text-muted-foreground">
                                                            "No primary admin surface declared."
                                                        </span>
                                                    }
                                                        .into_any()
                                                } else {
                                                    module
                                                        .recommended_admin_surfaces
                                                        .clone()
                                                        .into_iter()
                                                        .map(|surface| {
                                                            view! {
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("Primary: {}", humanize_token(&surface))}
                                                                </span>
                                                            }
                                                        })
                                                        .collect_view()
                                                        .into_any()
                                                }}
                                            </div>
                                            <div class="flex flex-wrap gap-2">
                                                {if module.showcase_admin_surfaces.is_empty() {
                                                    view! {
                                                        <span class="text-xs text-muted-foreground">
                                                            "No showcase admin surface declared."
                                                        </span>
                                                    }
                                                        .into_any()
                                                } else {
                                                    module
                                                        .showcase_admin_surfaces
                                                        .clone()
                                                        .into_iter()
                                                        .map(|surface| {
                                                            view! {
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("Showcase: {}", humanize_token(&surface))}
                                                                </span>
                                                            }
                                                        })
                                                        .collect_view()
                                                        .into_any()
                                                }}
                                            </div>
                                            <div class="text-xs text-muted-foreground">
                                                {if module.dependencies.is_empty() {
                                                    "No module dependencies declared.".to_string()
                                                } else {
                                                    format!("Depends on: {}", module.dependencies.join(", "))
                                                }}
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <Show when=move || !metadata_checklist_for_show.is_empty()>
                                    <div class="rounded-lg border border-border bg-background/70 p-4">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">"Registry readiness"</p>
                                            <span class=metadata_status_badge_classes(if metadata_required_issues > 0 { "warn" } else { "ready" })>
                                                {if metadata_required_issues > 0 {
                                                    format!("{} required issue(s)", metadata_required_issues)
                                                } else {
                                                    "No required metadata gaps".to_string()
                                                }}
                                            </span>
                                            <span class=metadata_status_badge_classes(if metadata_recommended_gaps > 0 { "warn" } else { "ready" })>
                                                {if metadata_recommended_gaps > 0 {
                                                    format!("{} recommended gap(s)", metadata_recommended_gaps)
                                                } else {
                                                    "Recommended visuals look complete".to_string()
                                                }}
                                            </span>
                                            <span class=metadata_status_badge_classes("info")>
                                                {format!("{} ready signal(s)", metadata_ready_count)}
                                            </span>
                                        </div>
                                        <div class="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                                            {metadata_checklist.clone().into_iter().map(|item| {
                                                view! {
                                                    <div class=format!(
                                                        "rounded-lg border p-3 text-sm {}",
                                                        metadata_status_panel_classes(item.state)
                                                    )>
                                                        <div class="flex flex-wrap items-center justify-between gap-2">
                                                            <p class="font-medium text-card-foreground">{item.label}</p>
                                                            <span class=metadata_status_badge_classes(item.state)>
                                                                {item.summary}
                                                            </span>
                                                        </div>
                                                        <p class="mt-2 text-xs text-muted-foreground">{item.detail}</p>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                        <p class="mt-3 text-xs text-muted-foreground">
                                            {if module.source.eq_ignore_ascii_case("path") {
                                                "Workspace path modules can stay unpublished; this checklist is meant to surface what is already registry-ready versus what still needs operator follow-up."
                                            } else {
                                                "Registry-backed modules should ideally arrive here with the required metadata already satisfied."
                                            }}
                                        </p>
                                    </div>
                                </Show>

                                {if has_marketplace_visuals {
                                    view! {
                                        <div class="rounded-lg border border-border bg-background/70 p-4">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">"Marketplace visuals"</p>
                                            <div class="mt-3 space-y-3">
                                                {module_banner_url_for_body.clone().map(|banner_url| {
                                                    let module_name = module_name.clone();
                                                    view! {
                                                        <div class="space-y-2">
                                                            <p class="text-xs text-muted-foreground">"Banner"</p>
                                                            <img
                                                                class="max-h-48 w-full rounded-lg border border-border object-cover"
                                                                src=banner_url
                                                                alt=format!("{} banner", module_name)
                                                            />
                                                        </div>
                                                    }
                                                })}
                                                {if has_marketplace_screenshots {
                                                    view! {
                                                        <div class="space-y-2">
                                                            <p class="text-xs text-muted-foreground">"Screenshots"</p>
                                                            <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                                                                {module_screenshots_for_body.clone().into_iter().map(|screenshot_url| {
                                                                    let module_name = module_name.clone();
                                                                    view! {
                                                                        <img
                                                                            class="h-32 w-full rounded-lg border border-border object-cover"
                                                                            src=screenshot_url
                                                                            alt=format!("{} screenshot", module_name)
                                                                        />
                                                                    }
                                                                }).collect_view()}
                                                            </div>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    ().into_any()
                                                }}
                                            </div>
                                        </div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}

                                <div class="rounded-lg border border-border bg-background/70 p-4">
                                    <div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                                        <div class="space-y-1">
                                            <p class="text-xs uppercase tracking-wide text-muted-foreground">"Tenant settings"</p>
                                            <p class="text-sm text-muted-foreground">
                                                {if settings_form_supported.get() {
                                                    "This module exposes schema-driven tenant settings from rustok-module.toml."
                                                } else if settings_editable.get() {
                                                    "Persist raw JSON settings for the current tenant. The payload is stored in tenant_modules.settings."
                                                } else {
                                                    "Enable this module for the current tenant before saving settings."
                                                }}
                                            </p>
                                        </div>
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                                            disabled=move || !settings_editable.get() || settings_saving.get()
                                            on:click=move |_| on_save_settings.run(())
                                        >
                                            {move || if settings_saving.get() { "Saving..." } else { "Save settings" }}
                                        </button>
                                    </div>
                                    <div class="mt-3 flex flex-wrap items-center gap-2 text-xs">
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                            {move || match tenant_module_for_body.get_value().as_ref() {
                                                Some(module) if module.enabled => "Tenant-enabled".to_string(),
                                                Some(_) => "Tenant-disabled".to_string(),
                                                None if settings_editable.get() => "No tenant override yet".to_string(),
                                                None => "Unavailable until enabled".to_string(),
                                            }}
                                        </span>
                                        <Show when=move || settings_form_supported.get() && !settings_schema_for_body.get_value().is_empty()>
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 font-medium text-muted-foreground">
                                                {format!(
                                                    "{} fields",
                                                    settings_schema_for_body.get_value().len()
                                                )}
                                            </span>
                                        </Show>
                                    </div>
                                    <Show
                                        when=move || settings_form_supported.get() && !settings_schema_for_body.get_value().is_empty()
                                        fallback=move || view! {
                                            <textarea
                                                class="mt-3 min-h-48 w-full rounded-lg border border-border bg-background px-3 py-3 font-mono text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                prop:value=move || settings_draft.get()
                                                disabled=move || !settings_editable.get() || settings_saving.get()
                                                on:input=move |event| on_settings_input.run(event_target_value(&event))
                                            ></textarea>
                                        }
                                    >
                                        <div class="mt-4 grid gap-4 md:grid-cols-2">
                                            {move || {
                                                settings_schema_for_body
                                                    .get_value()
                                                    .into_iter()
                                                    .map(|field| {
                                                        let field_key = field.key.clone();
                                                        let field_label = humanize_setting_key(&field.key);
                                                        let field_hint = setting_field_hint(&field);
                                                        let field_description = field.description.clone();
                                                        let field_type = field.value_type.clone();
                                                        let field_options = field.options.clone();
                                                        let value_for_text = {
                                                            let field_key = field_key.clone();
                                                            move || {
                                                                settings_form_draft
                                                                    .get()
                                                                    .get(&field_key)
                                                                    .cloned()
                                                                    .unwrap_or_default()
                                                            }
                                                        };
                                                        let disabled = Signal::derive(move || {
                                                            !settings_editable.get() || settings_saving.get()
                                                        });

                                                        view! {
                                                            <div class="space-y-2 rounded-lg border border-border bg-background px-4 py-3">
                                                                <div class="space-y-1">
                                                                    <div class="flex flex-wrap items-center gap-2">
                                                                        <label class="text-sm font-medium text-card-foreground">
                                                                            {field_label}
                                                                        </label>
                                                                        <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                                            {field.value_type.clone()}
                                                                        </span>
                                                                    </div>
                                                                    {field_description.map(|description| view! {
                                                                        <p class="text-xs text-muted-foreground">{description}</p>
                                                                    })}
                                                                    {field_hint.map(|hint| view! {
                                                                        <p class="text-[11px] text-muted-foreground">{hint}</p>
                                                                    })}
                                                                </div>

                                                                {match field_type.as_str() {
                                                                    "boolean" => {
                                                                        if !field_options.is_empty() {
                                                                            let field_key_for_select = field_key.clone();
                                                                            let field_type_for_select = field_type.clone();
                                                                            let options_for_select = field_options.clone();
                                                                            view! {
                                                                                <select
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:change=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_select.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                >
                                                                                    {options_for_select.into_iter().map(|option| {
                                                                                        let option_value = setting_option_draft_value(&field_type_for_select, &option);
                                                                                        let option_label = setting_option_label(&option);
                                                                                        view! {
                                                                                            <option value=option_value>{option_label}</option>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                </select>
                                                                            }.into_any()
                                                                        } else {
                                                                            let field_key_for_toggle = field_key.clone();
                                                                            view! {
                                                                                <label class="inline-flex items-center gap-3 text-sm text-card-foreground">
                                                                                    <input
                                                                                        type="checkbox"
                                                                                        class="h-4 w-4 rounded border-border text-primary focus:ring-primary/20"
                                                                                        prop:checked=move || value_for_text() == "true"
                                                                                        disabled=move || disabled.get()
                                                                                        on:change=move |event| {
                                                                                            on_settings_field_input.run((
                                                                                                field_key_for_toggle.clone(),
                                                                                                if event_target_checked(&event) {
                                                                                                    "true".to_string()
                                                                                                } else {
                                                                                                    "false".to_string()
                                                                                                },
                                                                                            ))
                                                                                        }
                                                                                    />
                                                                                    <span>"Enabled"</span>
                                                                                </label>
                                                                            }.into_any()
                                                                        }
                                                                    }
                                                                    "integer" | "number" => {
                                                                        if !field_options.is_empty() {
                                                                            let field_key_for_select = field_key.clone();
                                                                            let field_type_for_select = field_type.clone();
                                                                            let options_for_select = field_options.clone();
                                                                            view! {
                                                                                <select
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:change=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_select.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                >
                                                                                    {options_for_select.into_iter().map(|option| {
                                                                                        let option_value = setting_option_draft_value(&field_type_for_select, &option);
                                                                                        let option_label = setting_option_label(&option);
                                                                                        view! {
                                                                                            <option value=option_value>{option_label}</option>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                </select>
                                                                            }.into_any()
                                                                        } else {
                                                                            let field_key_for_input = field_key.clone();
                                                                            let step = if field_type == "integer" { "1" } else { "any" };
                                                                            view! {
                                                                                <input
                                                                                    type="number"
                                                                                    step=step
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:input=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_input.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                />
                                                                            }.into_any()
                                                                        }
                                                                    }
                                                                    "object" | "array" | "json" | "any" => {
                                                                        let field_key_for_input = field_key.clone();
                                                                        let placeholder = setting_field_placeholder(&field).unwrap_or_default();
                                                                        view! {
                                                                            <ComplexSettingEditor
                                                                                field_type=field_type.clone()
                                                                                placeholder=placeholder
                                                                                array_item_type=field.item_type.clone()
                                                                                schema_shape=field.shape.clone()
                                                                                value=Signal::derive(value_for_text)
                                                                                disabled=disabled
                                                                                on_input=Callback::new(move |next| {
                                                                                    on_settings_field_input.run((
                                                                                        field_key_for_input.clone(),
                                                                                        next,
                                                                                    ))
                                                                                })
                                                                            />
                                                                        }.into_any()
                                                                    }
                                                                    _ => {
                                                                        if !field_options.is_empty() {
                                                                            let field_key_for_select = field_key.clone();
                                                                            let field_type_for_select = field_type.clone();
                                                                            let options_for_select = field_options.clone();
                                                                            view! {
                                                                                <select
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:change=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_select.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                >
                                                                                    {options_for_select.into_iter().map(|option| {
                                                                                        let option_value = setting_option_draft_value(&field_type_for_select, &option);
                                                                                        let option_label = setting_option_label(&option);
                                                                                        view! {
                                                                                            <option value=option_value>{option_label}</option>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                </select>
                                                                            }.into_any()
                                                                        } else {
                                                                            let field_key_for_input = field_key.clone();
                                                                            view! {
                                                                                <input
                                                                                    type="text"
                                                                                    class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-card-foreground shadow-sm outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/20 disabled:cursor-not-allowed disabled:opacity-70"
                                                                                    prop:value=value_for_text
                                                                                    disabled=move || disabled.get()
                                                                                    on:input=move |event| {
                                                                                        on_settings_field_input.run((
                                                                                            field_key_for_input.clone(),
                                                                                            event_target_value(&event),
                                                                                        ))
                                                                                    }
                                                                                />
                                                                            }.into_any()
                                                                        }
                                                                    }
                                                                }}
                                                            </div>
                                                        }
                                                    })
                                                    .collect_view()
                                            }}
                                        </div>
                                    </Show>
                                    <p class="mt-2 text-xs text-muted-foreground">
                                        {move || {
                                            if settings_form_supported.get() && !settings_schema_for_body.get_value().is_empty() {
                                                format!("Editing schema-driven settings for `{}`. Complex fields accept JSON per field.", selected_slug_for_body.get_value())
                                            } else {
                                                format!(
                                                    "Editing raw JSON settings for `{}`.",
                                                    selected_slug_for_body.get_value()
                                                )
                                            }
                                        }}
                                    </p>
                                </div>

                                <div class="rounded-lg border border-border bg-background/70 p-4">
                                    <div class="flex items-center gap-2">
                                        <p class="text-xs uppercase tracking-wide text-muted-foreground">"Version history"</p>
                                        <Show when=move || loading.get()>
                                            <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                "Refreshing"
                                            </span>
                                        </Show>
                                    </div>
                                    {if version_trail.is_empty() {
                                        view! {
                                            <p class="mt-3 text-sm text-muted-foreground">
                                                "No version history has been published for this module yet."
                                            </p>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <div class="mt-3 space-y-3">
                                                {version_trail.into_iter().map(|version| {
                                                    let checksum = short_checksum(version.checksum_sha256.as_deref());
                                                    view! {
                                                        <div class="flex flex-col gap-2 rounded-lg border border-border px-3 py-3 text-sm">
                                                            <div class="flex flex-wrap items-center gap-2">
                                                                <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                                                    {format!("v{}", version.version)}
                                                                </span>
                                                                {version.yanked.then(|| view! {
                                                                    <span class="inline-flex items-center rounded-full bg-destructive px-2.5 py-0.5 text-xs font-semibold text-destructive-foreground">
                                                                        "Yanked"
                                                                    </span>
                                                                })}
                                                                {version.signature_present.then(|| view! {
                                                                    <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                                        "Signed"
                                                                    </span>
                                                                })}
                                                                <span class="text-xs text-muted-foreground">
                                                                    {version.published_at.unwrap_or_else(|| "Unknown".to_string())}
                                                                </span>
                                                            </div>
                                                            {version.changelog.map(|changelog| view! {
                                                                <p class="text-sm text-muted-foreground">{changelog}</p>
                                                            })}
                                                            {checksum.map(|checksum| view! {
                                                                <div class="text-xs text-muted-foreground">
                                                                    <span class="font-mono">{format!("sha256 {}", checksum)}</span>
                                                                </div>
                                                            })}
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        }
                                            .into_any()
                                    }}
                                </div>
                            </div>
                        }
                    })
                }}
            </Show>
        </div>
    }
}

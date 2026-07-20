use super::{humanize_setting_key, humanize_token, tr};
use crate::Locale;
use crate::use_i18n;
use leptos::prelude::*;

#[derive(Clone, Debug)]
pub enum JsonPathSegment {
    Key(String),
    Index(usize),
}

#[derive(Clone)]
pub struct NestedJsonRenderContext {
    root_type: String,
    root_value: Signal<String>,
    locale: Locale,
    disabled: Signal<bool>,
    on_input: Callback<String>,
}

pub fn json_value_kind(value: &serde_json::Value) -> &'static str {
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

pub fn json_value_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(value) => format!("{} items", value.len()),
        serde_json::Value::Object(value) => format!("{} keys", value.len()),
    }
}

pub fn parse_object_root(raw: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match parse_json_editor_value(raw, "object", Locale::en)? {
        Some(serde_json::Value::Object(object)) => Ok(object),
        Some(_) => Err("Expected a JSON object".to_string()),
        None => Ok(serde_json::Map::new()),
    }
}

pub fn parse_array_root(raw: &str) -> Result<Vec<serde_json::Value>, String> {
    match parse_json_editor_value(raw, "array", Locale::en)? {
        Some(serde_json::Value::Array(array)) => Ok(array),
        Some(_) => Err("Expected a JSON array".to_string()),
        None => Ok(Vec::new()),
    }
}

pub fn unique_object_key(
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

pub fn object_with_new_property(
    raw: &str,
    preferred_key: &str,
    value: serde_json::Value,
) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    let key = unique_object_key(&object, preferred_key);
    object.insert(key, value);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

pub fn object_with_updated_property(
    raw: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    object.insert(key.to_string(), value);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

pub fn object_without_property(raw: &str, key: &str) -> Result<String, String> {
    let mut object = parse_object_root(raw)?;
    object.remove(key);
    Ok(pretty_json_value(&serde_json::Value::Object(object)))
}

pub fn object_with_renamed_property(
    raw: &str,
    old_key: &str,
    new_key: &str,
) -> Result<String, String> {
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

pub fn array_with_appended_item(raw: &str, value: serde_json::Value) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    array.push(value);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

pub fn array_with_updated_item(
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

pub fn array_without_item(raw: &str, index: usize) -> Result<String, String> {
    let mut array = parse_array_root(raw)?;
    if index >= array.len() {
        return Err("Array item is out of bounds".to_string());
    }
    array.remove(index);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

pub fn array_item_moved(raw: &str, index: usize, delta: isize) -> Result<String, String> {
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

pub fn default_json_root(root_type: &str) -> serde_json::Value {
    match root_type {
        "object" => serde_json::json!({}),
        "array" => serde_json::json!([]),
        _ => serde_json::Value::Null,
    }
}

pub fn default_value_for_setting_type(value_type: &str) -> serde_json::Value {
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

pub fn add_item_button_label(value_type: &str, locale: Locale) -> String {
    match value_type {
        "string" => tr(locale, "Add text", "Добавить текст").to_string(),
        "boolean" => tr(locale, "Add flag", "Добавить флаг").to_string(),
        "integer" | "number" => tr(locale, "Add number", "Добавить число").to_string(),
        "object" => tr(locale, "Add object", "Добавить объект").to_string(),
        "array" => tr(locale, "Add array", "Добавить массив").to_string(),
        "json" | "any" => tr(locale, "Add item", "Добавить элемент").to_string(),
        _ => format!(
            "{} {}",
            tr(locale, "Add", "Добавить"),
            humanize_token(value_type)
        ),
    }
}

pub fn parse_json_root(raw: &str, root_type: &str) -> Result<serde_json::Value, String> {
    Ok(parse_json_editor_value(raw, root_type, Locale::en)?
        .unwrap_or_else(|| default_json_root(root_type)))
}

pub fn value_at_path_mut<'a>(
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

pub fn with_updated_json_root(
    raw: &str,
    root_type: &str,
    updater: impl FnOnce(&mut serde_json::Value) -> Result<(), String>,
) -> Result<String, String> {
    let mut root = parse_json_root(raw, root_type)?;
    updater(&mut root)?;
    Ok(pretty_json_value(&root))
}

pub fn nested_value_updated(
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

pub fn nested_value_removed(
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

pub fn nested_object_key_renamed(
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

pub fn nested_array_item_moved(
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

pub fn nested_object_child_added(
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

pub fn nested_array_child_added(
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

pub fn nested_object_contains_key(
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

pub fn setting_shape_properties(
    shape: Option<&serde_json::Value>,
) -> Vec<(String, serde_json::Value)> {
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

pub fn setting_shape_items(shape: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    shape.and_then(|shape| shape.get("items")).cloned()
}

pub fn setting_shape_property(
    shape: Option<&serde_json::Value>,
    key: &str,
) -> Option<serde_json::Value> {
    shape
        .and_then(|shape| shape.get("properties"))
        .and_then(|value| value.as_object())
        .and_then(|properties| properties.get(key))
        .cloned()
}

pub fn setting_shape_type(shape: Option<&serde_json::Value>) -> Option<String> {
    shape
        .and_then(|shape| shape.get("type"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

pub fn setting_shape_options(shape: Option<&serde_json::Value>) -> Vec<serde_json::Value> {
    shape
        .and_then(|shape| shape.get("options"))
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
}

pub fn setting_shape_numeric_bound(shape: Option<&serde_json::Value>, key: &str) -> Option<String> {
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

pub fn parse_scalar_input_value(raw: &str, value_type: &str) -> Option<serde_json::Value> {
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

pub fn setting_option_draft_value(value_type: &str, value: &serde_json::Value) -> String {
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

pub fn setting_option_label(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

pub fn default_value_for_schema_shape(shape: Option<&serde_json::Value>) -> serde_json::Value {
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

pub fn schema_action_label(shape: Option<&serde_json::Value>, locale: Locale) -> String {
    match setting_shape_type(shape).as_deref() {
        Some(value_type) => add_item_button_label(value_type, locale),
        None => tr(locale, "Add item", "Добавить элемент").to_string(),
    }
}

pub fn pretty_json_value(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

pub fn parse_json_editor_value(
    raw: &str,
    expected_type: &str,
    locale: Locale,
) -> Result<Option<serde_json::Value>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let value = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|err| format!("{}: {err}", tr(locale, "Invalid JSON", "Некорректный JSON")))?;

    match expected_type {
        "object" if !value.is_object() => {
            Err(tr(locale, "Expected a JSON object", "Ожидался JSON-объект").to_string())
        }
        "array" if !value.is_array() => {
            Err(tr(locale, "Expected a JSON array", "Ожидался JSON-массив").to_string())
        }
        _ => Ok(Some(value)),
    }
}

pub fn reset_json_editor_value(field_type: &str) -> String {
    let value = match field_type {
        "object" => serde_json::json!({}),
        "array" => serde_json::json!([]),
        "json" | "any" => serde_json::Value::Null,
        _ => serde_json::Value::Null,
    };
    pretty_json_value(&value)
}

pub fn append_object_property(raw: &str) -> Result<String, String> {
    let mut object = match parse_json_editor_value(raw, "object", Locale::en)? {
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

pub fn append_array_item(raw: &str) -> Result<String, String> {
    let mut array = match parse_json_editor_value(raw, "array", Locale::en)? {
        Some(serde_json::Value::Array(array)) => array,
        Some(_) => return Err("Expected a JSON array".to_string()),
        None => Vec::new(),
    };
    array.push(serde_json::Value::Null);
    Ok(pretty_json_value(&serde_json::Value::Array(array)))
}

pub fn json_editor_summary(
    field_type: &str,
    raw: &str,
    locale: Locale,
) -> (bool, String, Vec<String>) {
    match parse_json_editor_value(raw, field_type, locale) {
        Ok(Some(serde_json::Value::Object(object))) => {
            let preview = object.keys().take(4).cloned().collect::<Vec<_>>();
            (
                true,
                format!("{} {}", object.len(), tr(locale, "keys", "ключей")),
                preview,
            )
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
            (
                true,
                format!("{} {}", array.len(), tr(locale, "items", "элементов")),
                preview,
            )
        }
        Ok(Some(value)) => (
            true,
            format!("{} {}", value, tr(locale, "value ready", "значение готово")),
            Vec::new(),
        ),
        Ok(None) => (
            true,
            tr(
                locale,
                "Empty value; server defaults apply if declared.",
                "Пустое значение; серверные значения по умолчанию применятся, если они объявлены.",
            )
            .to_string(),
            Vec::new(),
        ),
        Err(message) => (false, message, Vec::new()),
    }
}

pub fn render_scalar_value_editor(
    current_value: serde_json::Value,
    shape: Option<serde_json::Value>,
    locale: Locale,
    disabled: Signal<bool>,
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
                    <span>{tr(locale, "Enabled", "Включено")}</span>
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

pub fn render_nested_json_children(
    context: &NestedJsonRenderContext,
    path: Vec<JsonPathSegment>,
    current: serde_json::Value,
    current_shape: Option<serde_json::Value>,
) -> AnyView {
    let root_type = context.root_type.clone();
    let root_value = context.root_value;
    let locale = context.locale;
    let disabled = context.disabled;
    let on_input = context.on_input;
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
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newText", serde_json::Value::String(String::new())) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add text", "Добавить текст")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newFlag", serde_json::Value::Bool(false)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add flag", "Добавить флаг")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newNumber", serde_json::json!(0)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add number", "Добавить число")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newObject", serde_json::json!({})) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add object", "Добавить объект")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_object_child_added(&root_value.get(), &root_type, &path, "newArray", serde_json::json!([])) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add array", "Добавить массив")}</button>
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
                                            move |_| {
                                                if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }>{tr(locale, "Remove", "Удалить")}</button>
                                    </div>
                                    {render_scalar_value_editor(
                                        scalar_value,
                                        property_shape.clone(),
                                        locale,
                                        disabled,
                                        Callback::new({
                                            let root_type = root_type.clone();
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
                                            move |_| {
                                                if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                    on_input.run(next);
                                                }
                                            }
                                        }>{tr(locale, "Remove", "Удалить")}</button>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{preview}</p>
                                    {render_nested_json_children(context, item_path.clone(), item_value, property_shape.clone())}
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
                        let button_label = schema_action_label(Some(&item_shape), locale);
                        view! {
                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                let root_type = root_type.clone();
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
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::Value::String(String::new())) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add text", "Добавить текст")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::Value::Bool(false)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add flag", "Добавить флаг")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!(0)) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add number", "Добавить число")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!({})) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add object", "Добавить объект")}</button>
                                <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                    let root_type = root_type.clone();
                                    let path = path.clone();
                                    move |_| {
                                        if let Ok(next) = nested_array_child_added(&root_value.get(), &root_type, &path, serde_json::json!([])) {
                                            on_input.run(next);
                                        }
                                    }
                                }>{tr(locale, "Add array", "Добавить массив")}</button>
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
                                            <span class="text-sm font-medium text-card-foreground">{format!("{} {}", tr(locale, "Item", "Элемент"), index + 1)}</span>
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_up, -1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Up", "Вверх")}</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_down, 1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Down", "Вниз")}</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                move |_| {
                                                    if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Remove", "Удалить")}</button>
                                        </div>
                                    </div>
                                    {render_scalar_value_editor(
                                        scalar_value,
                                        item_shape.clone(),
                                        locale,
                                        disabled,
                                        Callback::new({
                                            let root_type = root_type.clone();
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
                                            <span class="text-sm font-medium text-card-foreground">{format!("{} {}", tr(locale, "Item", "Элемент"), index + 1)}</span>
                                            <span class="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">{kind.clone()}</span>
                                        </div>
                                        <div class="flex flex-wrap gap-2">
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_up, -1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Up", "Вверх")}</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                move |_| {
                                                    if let Ok(next) = nested_array_item_moved(&root_value.get(), &root_type, &item_path_for_move_down, 1) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Down", "Вниз")}</button>
                                            <button type="button" class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50" disabled=move || disabled.get() on:click={
                                                let root_type = root_type.clone();
                                                move |_| {
                                                    if let Ok(next) = nested_value_removed(&root_value.get(), &root_type, &item_path_for_remove) {
                                                        on_input.run(next);
                                                    }
                                                }
                                            }>{tr(locale, "Remove", "Удалить")}</button>
                                        </div>
                                    </div>
                                    <p class="text-sm text-muted-foreground">{preview}</p>
                                    {render_nested_json_children(context, item_path.clone(), item_value, item_shape.clone())}
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
pub fn StructuredObjectEditor(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    object_shape: Option<serde_json::Value>,
    on_input: Callback<String>,
) -> impl IntoView {
    let locale = use_i18n().get_locale();
    let object_entries = Signal::derive(move || parse_object_root(&value.get()));
    let declared_properties = setting_shape_properties(object_shape.as_ref());
    let object_shape_for_items = StoredValue::new(object_shape.clone());
    let schema_locks_keys = !declared_properties.is_empty();

    view! {
        <Show when=move || object_entries.get().is_ok()>
            <div class="rounded-lg border border-dashed border-border bg-muted/30 p-3">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        {tr(locale, "Structured object editor", "Структурный редактор объекта")}
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
                                        {tr(locale, "Add text", "Добавить текст")}
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
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
                                        {tr(locale, "Add flag", "Добавить флаг")}
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
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
                                        {tr(locale, "Add number", "Добавить число")}
                                    </button>
                                </>
                            }.into_any()
                        } else {
                            declared_properties
                                .clone()
                                .into_iter()
                                .map(|(property_key, property_shape)| {
                                    let button_label = format!(
                                        "{} {}",
                                        tr(locale, "Add", "Добавить"),
                                        humanize_setting_key(&property_key)
                                    );
                                    view! {
                                        <button
                                            type="button"
                                            class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                            disabled={
                                                let property_key = property_key.clone();
                                                move || {
                                                    disabled.get()
                                                        || parse_object_root(&value.get())
                                                            .map(|object| object.contains_key(&property_key))
                                                            .unwrap_or(false)
                                                }
                                            }
                                            on:click={
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
                                {tr(locale, "Object is empty. Use the quick actions to add top-level properties.", "Объект пуст. Используйте быстрые действия, чтобы добавить поля верхнего уровня.")}
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
                                                    move |_| {
                                                        if let Ok(next) = object_without_property(&value.get(), &key_for_remove) {
                                                            on_input.run(next);
                                                        }
                                                    }
                                                }
                                            >
                                                {tr(locale, "Remove", "Удалить")}
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
                                                        locale,
                                                        disabled,
                                                        Callback::new({
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
                                                            {format!(
                                                                "{} {}: {}.",
                                                                tr(locale, "Nested", "Вложенный"),
                                                                kind,
                                                                preview
                                                            )}
                                                        </p>
                                                        {render_nested_json_children(&NestedJsonRenderContext {
                                                            root_type: "object".to_string(),
                                                            root_value: value,
                                                            locale,
                                                            disabled,
                                                            on_input,
                                                        },
                                                            nested_path,
                                                            nested_value,
                                                            nested_shape,
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
pub fn StructuredArrayEditor(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    array_item_type: Option<String>,
    array_item_shape: Option<serde_json::Value>,
    on_input: Callback<String>,
) -> impl IntoView {
    let locale = use_i18n().get_locale();
    let array_entries = Signal::derive(move || parse_array_root(&value.get()));
    let array_item_shape_for_items = StoredValue::new(array_item_shape.clone());

    view! {
        <Show when=move || array_entries.get().is_ok()>
            <div class="rounded-lg border border-dashed border-border bg-muted/30 p-3">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        {tr(locale, "Structured array editor", "Структурный редактор массива")}
                    </p>
                    <div class="flex flex-wrap gap-2">
                        {if let Some(item_shape) = array_item_shape.clone() {
                            let button_label = schema_action_label(Some(&item_shape), locale);
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                    disabled=move || disabled.get()
                                    on:click={
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
                            let button_label = add_item_button_label(&item_type, locale);
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                    disabled=move || disabled.get()
                                    on:click={
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
                                        {tr(locale, "Add text", "Добавить текст")}
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
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
                                        {tr(locale, "Add flag", "Добавить флаг")}
                                    </button>
                                    <button
                                        type="button"
                                        class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                        disabled=move || disabled.get()
                                        on:click={
                                            move |_| {
                                                if let Ok(next) =
                                                    array_with_appended_item(&value.get(), serde_json::json!(0))
                                                {
                                                    on_input.run(next);
                                                }
                                            }
                                        }
                                    >
                                        {tr(locale, "Add number", "Добавить число")}
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
                                {tr(locale, "Array is empty. Use the quick actions to add top-level items.", "Массив пуст. Используйте быстрые действия, чтобы добавить элементы верхнего уровня.")}
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
                                                <span class="text-sm font-medium text-card-foreground">{format!("{} {}", tr(locale, "Item", "Элемент"), index + 1)}</span>
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
                                                        move |_| {
                                                            if let Ok(next) = array_item_moved(&value.get(), index, -1) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    {tr(locale, "Up", "Вверх")}
                                                </button>
                                                <button
                                                    type="button"
                                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                    disabled=move || disabled.get()
                                                    on:click={
                                                        move |_| {
                                                            if let Ok(next) = array_item_moved(&value.get(), index, 1) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    {tr(locale, "Down", "Вниз")}
                                                </button>
                                                <button
                                                    type="button"
                                                    class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                                                    disabled=move || disabled.get()
                                                    on:click={
                                                        move |_| {
                                                            if let Ok(next) = array_without_item(&value.get(), index) {
                                                                on_input.run(next);
                                                            }
                                                        }
                                                    }
                                                >
                                                    {tr(locale, "Remove", "Удалить")}
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
                                                        locale,
                                                        disabled,
                                                        Callback::new({
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
                                                            {format!(
                                                                "{} {}: {}.",
                                                                tr(locale, "Nested", "Вложенный"),
                                                                kind,
                                                                preview
                                                            )}
                                                        </p>
                                                    {render_nested_json_children(&NestedJsonRenderContext {
                                                        root_type: "array".to_string(),
                                                        root_value: value,
                                                        locale,
                                                        disabled,
                                                        on_input,
                                                    },
                                                        nested_path,
                                                        nested_value,
                                                        nested_shape.or_else(|| array_item_shape_for_items.clone()),
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
pub fn ComplexSettingEditor(
    field_type: String,
    placeholder: &'static str,
    array_item_type: Option<String>,
    schema_shape: Option<serde_json::Value>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] disabled: Signal<bool>,
    on_input: Callback<String>,
) -> impl IntoView {
    let locale = use_i18n().get_locale();
    let status = Signal::derive({
        let field_type = field_type.clone();
        move || json_editor_summary(&field_type, &value.get(), locale)
    });
    let nested_root = Signal::derive({
        let field_type = field_type.clone();
        move || parse_json_root(&value.get(), &field_type).ok()
    });

    let show_add_button = matches!(field_type.as_str(), "object" | "array");
    let add_button_label = if field_type == "object" {
        tr(locale, "Add property", "Добавить поле")
    } else {
        tr(locale, "Add item", "Добавить элемент")
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
                                        {tr(locale, "Nested editor", "Вложенный редактор")}
                                    </p>
                                    {render_nested_json_children(&NestedJsonRenderContext {
                                        root_type: field_type_for_nested.clone(),
                                        root_value: value,
                                        locale,
                                        disabled,
                                        on_input,
                                    },
                                        Vec::new(),
                                        root,
                                        schema_shape.clone(),
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
                        let field_type = field_type.clone();
                        move |_| {
                            match parse_json_editor_value(&value.get(), &field_type, locale) {
                                Ok(Some(parsed)) => on_input.run(pretty_json_value(&parsed)),
                                Ok(None) => on_input.run(reset_json_editor_value(&field_type)),
                                Err(_) => {}
                            }
                        }
                    }
                >
                    {tr(locale, "Format JSON", "Форматировать JSON")}
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
                    {tr(locale, "Reset", "Сбросить")}
                </button>
                {if show_add_button {
                    view! {
                        <button
                            type="button"
                            class="inline-flex items-center justify-center rounded-md border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                            disabled=move || disabled.get()
                            on:click={
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

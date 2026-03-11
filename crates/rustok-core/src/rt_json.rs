use serde_json::{json, Map, Value};
use url::Url;

const SUPPORTED_VERSION: &str = "rt_json_v1";
const KNOWN_FUTURE_VERSION: &str = "rt_json_v2";
const MAX_DEPTH: usize = 8;
const MAX_NODES: usize = 2000;
const MAX_TEXT_CHARS: usize = 100_000;
const MAX_MARKS_PER_TEXT: usize = 8;

#[derive(Debug, Clone)]
pub struct RtJsonValidationConfig {
    pub expected_locale: String,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_text_chars: usize,
}

impl RtJsonValidationConfig {
    pub fn for_locale(locale: &str) -> Self {
        Self {
            expected_locale: locale.to_string(),
            max_depth: MAX_DEPTH,
            max_nodes: MAX_NODES,
            max_text_chars: MAX_TEXT_CHARS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RtJsonValidationResult {
    pub sanitized: Value,
    pub transformed_from_legacy: bool,
}

pub fn validate_and_sanitize_rt_json(
    payload: &Value,
    config: &RtJsonValidationConfig,
) -> Result<RtJsonValidationResult, String> {
    let mut transformed_from_legacy = false;

    let mut normalized = if payload.get("version").is_none() {
        transformed_from_legacy = true;
        if payload.get("doc").is_some() {
            let mut obj = payload
                .as_object()
                .cloned()
                .ok_or_else(|| "rt_json payload must be an object".to_string())?;
            obj.insert(
                "version".to_string(),
                Value::String(SUPPORTED_VERSION.to_string()),
            );
            if obj.get("locale").is_none() {
                obj.insert(
                    "locale".to_string(),
                    Value::String(config.expected_locale.clone()),
                );
            }
            Value::Object(obj)
        } else {
            json!({
                "version": SUPPORTED_VERSION,
                "locale": config.expected_locale,
                "doc": payload,
            })
        }
    } else {
        payload.clone()
    };

    let version = normalized
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| "rt_json payload.version must be a string".to_string())?;
    if version != SUPPORTED_VERSION {
        if version == KNOWN_FUTURE_VERSION {
            return Err(format!(
                "rt_json version '{version}' is recognized but not supported by this backend yet"
            ));
        }
        return Err(format!(
            "Unsupported rt_json version '{version}'. Known versions: '{SUPPORTED_VERSION}', '{KNOWN_FUTURE_VERSION}'"
        ));
    }

    let locale = normalized
        .get("locale")
        .and_then(Value::as_str)
        .ok_or_else(|| "rt_json payload.locale is required".to_string())?;
    if !is_valid_locale(locale) {
        return Err("rt_json payload.locale must be a valid locale (e.g. en or ru-RU)".to_string());
    }
    if locale != config.expected_locale {
        return Err(format!(
            "rt_json payload.locale '{locale}' must match request locale '{}'",
            config.expected_locale
        ));
    }

    let doc = normalized
        .get("doc")
        .ok_or_else(|| "rt_json payload.doc is required".to_string())?;

    let mut stats = Stats::default();
    let sanitized_doc = sanitize_node(doc, 1, config, &mut stats)?
        .ok_or_else(|| "rt_json payload.doc must contain supported nodes".to_string())?;

    if stats.node_count > config.max_nodes {
        return Err(format!(
            "rt_json exceeds max node count ({})",
            config.max_nodes
        ));
    }
    if stats.text_chars > config.max_text_chars {
        return Err(format!(
            "rt_json exceeds max text size ({})",
            config.max_text_chars
        ));
    }

    if let Value::Object(map) = &mut normalized {
        map.insert("doc".to_string(), sanitized_doc);
    }

    Ok(RtJsonValidationResult {
        sanitized: normalized,
        transformed_from_legacy,
    })
}

/// Backend sanitize/normalization gate for rt_json before HTML rendering.
///
/// Renderers should operate only on this sanitized payload.
pub fn sanitize_rt_json_before_html_render(
    payload: &Value,
    config: &RtJsonValidationConfig,
) -> Result<Value, String> {
    validate_and_sanitize_rt_json(payload, config).map(|result| result.sanitized)
}

#[derive(Default)]
struct Stats {
    node_count: usize,
    text_chars: usize,
}

fn sanitize_node(
    node: &Value,
    depth: usize,
    config: &RtJsonValidationConfig,
    stats: &mut Stats,
) -> Result<Option<Value>, String> {
    if depth > config.max_depth {
        return Err(format!("rt_json exceeds max depth ({})", config.max_depth));
    }

    let mut obj = node
        .as_object()
        .cloned()
        .ok_or_else(|| "rt_json node must be an object".to_string())?;

    let node_type = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "rt_json node.type is required".to_string())?
        .to_string();

    if !is_allowed_node(&node_type) {
        return Ok(None);
    }

    stats.node_count += 1;

    if node_type == "text" {
        let text = obj.get("text").and_then(Value::as_str).unwrap_or_default();
        stats.text_chars += text.chars().count();
        obj.insert("text".to_string(), Value::String(text.to_string()));
    }

    if let Some(marks) = obj.get("marks") {
        let sanitized_marks = sanitize_marks(marks)?;
        if sanitized_marks.is_empty() {
            obj.remove("marks");
        } else {
            obj.insert("marks".to_string(), Value::Array(sanitized_marks));
        }
    }

    sanitize_attrs(&node_type, &mut obj)?;

    if let Some(content) = obj.get("content") {
        let list = content
            .as_array()
            .ok_or_else(|| "rt_json node.content must be an array".to_string())?;
        let mut sanitized = Vec::new();
        for child in list {
            if let Some(child_node) = sanitize_node(child, depth + 1, config, stats)? {
                sanitized.push(child_node);
            }
        }
        obj.insert("content".to_string(), Value::Array(sanitized));
    }

    Ok(Some(Value::Object(obj)))
}

fn sanitize_marks(marks: &Value) -> Result<Vec<Value>, String> {
    let arr = marks
        .as_array()
        .ok_or_else(|| "rt_json marks must be an array".to_string())?;
    let mut sanitized = Vec::new();
    for mark in arr.iter().take(MAX_MARKS_PER_TEXT) {
        let mut m = match mark.as_object() {
            Some(m) => m.clone(),
            None => continue,
        };
        let mark_type = match m.get("type").and_then(Value::as_str) {
            Some(v) if is_allowed_mark(v) => v,
            _ => continue,
        };

        if mark_type == "link" {
            sanitize_link_attrs(&mut m)?;
        } else {
            m.remove("attrs");
        }
        sanitized.push(Value::Object(m));
    }
    Ok(sanitized)
}

fn sanitize_attrs(node_type: &str, obj: &mut Map<String, Value>) -> Result<(), String> {
    match node_type {
        "heading" => {
            let level = obj
                .get("attrs")
                .and_then(|attrs| attrs.get("level"))
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 6);
            obj.insert("attrs".to_string(), json!({ "level": level }));
        }
        "link" => sanitize_link_attrs(obj)?,
        "image" => {
            let url = obj
                .get("attrs")
                .and_then(|attrs| attrs.get("src"))
                .and_then(Value::as_str)
                .ok_or_else(|| "rt_json image.attrs.src is required".to_string())?;
            if !is_allowed_url(url, false) {
                return Err("rt_json image URL is not allowed".to_string());
            }
            obj.insert("attrs".to_string(), json!({"src": url}));
        }
        "embed" => {
            let provider = obj
                .get("attrs")
                .and_then(|attrs| attrs.get("provider"))
                .and_then(Value::as_str)
                .ok_or_else(|| "rt_json embed.attrs.provider is required".to_string())?;
            let url = obj
                .get("attrs")
                .and_then(|attrs| attrs.get("url"))
                .and_then(Value::as_str)
                .ok_or_else(|| "rt_json embed.attrs.url is required".to_string())?;
            if !is_allowed_embed(provider, url) {
                return Err("rt_json embed provider or URL is not allowed".to_string());
            }
            obj.insert(
                "attrs".to_string(),
                json!({"provider": provider, "url": url}),
            );
        }
        _ => {
            obj.remove("attrs");
        }
    }
    Ok(())
}

fn sanitize_link_attrs(obj: &mut Map<String, Value>) -> Result<(), String> {
    let href = obj
        .get("attrs")
        .and_then(|attrs| attrs.get("href"))
        .and_then(Value::as_str)
        .ok_or_else(|| "rt_json link.attrs.href is required".to_string())?;
    if !is_allowed_url(href, true) {
        return Err("rt_json link URL is not allowed".to_string());
    }
    obj.insert("attrs".to_string(), json!({"href": href}));
    Ok(())
}

fn is_allowed_node(node_type: &str) -> bool {
    matches!(
        node_type,
        "doc"
            | "paragraph"
            | "heading"
            | "bullet_list"
            | "ordered_list"
            | "list_item"
            | "blockquote"
            | "code_block"
            | "horizontal_rule"
            | "hard_break"
            | "text"
            | "image"
            | "embed"
    )
}

fn is_allowed_mark(mark_type: &str) -> bool {
    matches!(mark_type, "bold" | "italic" | "strike" | "code" | "link")
}

fn is_allowed_url(raw: &str, allow_mailto: bool) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    match url.scheme() {
        "http" | "https" => true,
        "mailto" => allow_mailto,
        _ => false,
    }
}

fn is_allowed_embed(provider: &str, raw: &str) -> bool {
    if provider != "youtube" && provider != "vimeo" {
        return false;
    }
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https" {
        return false;
    }

    let host = url.host_str().unwrap_or_default();
    match provider {
        "youtube" => matches!(host, "youtube.com" | "www.youtube.com" | "youtu.be"),
        "vimeo" => matches!(host, "vimeo.com" | "player.vimeo.com"),
        _ => false,
    }
}

fn is_valid_locale(locale: &str) -> bool {
    let parts: Vec<&str> = locale.split('-').collect();
    if parts.is_empty() || parts.len() > 2 {
        return false;
    }
    let lang = parts[0];
    if lang.len() < 2 || lang.len() > 3 || !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    if let Some(region) = parts.get(1) {
        if region.len() != 2 || !region.chars().all(|c| c.is_ascii_uppercase()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_legacy_payload_without_version() {
        let payload = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hi"}]}]});
        let res =
            validate_and_sanitize_rt_json(&payload, &RtJsonValidationConfig::for_locale("en"))
                .expect("legacy payload should be transformed");
        assert!(res.transformed_from_legacy);
        assert_eq!(res.sanitized["version"], SUPPORTED_VERSION);
        assert_eq!(res.sanitized["locale"], "en");
    }

    #[test]
    fn rejects_unknown_version() {
        let payload =
            json!({"version":"rt_json_v2","locale":"en","doc":{"type":"doc","content":[]}});
        assert!(
            validate_and_sanitize_rt_json(&payload, &RtJsonValidationConfig::for_locale("en"))
                .is_err()
        );
    }

    #[test]
    fn rejects_known_future_version_with_explicit_message() {
        let payload =
            json!({"version":"rt_json_v2","locale":"en","doc":{"type":"doc","content":[]}});
        let err =
            validate_and_sanitize_rt_json(&payload, &RtJsonValidationConfig::for_locale("en"))
                .expect_err("v2 must be rejected by v1 backend");
        assert!(err.contains("recognized but not supported"));
    }

    #[test]
    fn rejects_unsafe_payload_link() {
        let payload = json!({
            "version":"rt_json_v1",
            "locale":"en",
            "doc":{
                "type":"doc",
                "content":[
                    {"type":"paragraph","content":[{"type":"text","text":"x","marks":[{"type":"link","attrs":{"href":"javascript:alert(1)"}}]}]}
                ]
            }
        });
        assert!(
            validate_and_sanitize_rt_json(&payload, &RtJsonValidationConfig::for_locale("en"))
                .is_err()
        );
    }

    #[test]
    fn rejects_oversized_payload() {
        let text = "a".repeat(100_001);
        let payload = json!({
            "version":"rt_json_v1",
            "locale":"en",
            "doc":{
                "type":"doc",
                "content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]
            }
        });
        assert!(
            validate_and_sanitize_rt_json(&payload, &RtJsonValidationConfig::for_locale("en"))
                .is_err()
        );
    }

    #[test]
    fn sanitize_before_html_render_uses_same_policy() {
        let payload = json!({
            "version":"rt_json_v1",
            "locale":"en",
            "doc":{
                "type":"doc",
                "content":[{"type":"unknown"},{"type":"paragraph","content":[{"type":"text","text":"ok"}]}]
            }
        });

        let sanitized = sanitize_rt_json_before_html_render(
            &payload,
            &RtJsonValidationConfig::for_locale("en"),
        )
        .expect("payload should sanitize before render");

        assert_eq!(sanitized["doc"]["content"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn drops_unknown_nodes_and_marks() {
        let payload = json!({
            "version":"rt_json_v1",
            "locale":"en",
            "doc":{
                "type":"doc",
                "content":[
                    {"type":"unknown"},
                    {"type":"paragraph","content":[{"type":"text","text":"ok","marks":[{"type":"bad"},{"type":"bold"}]}]}
                ]
            }
        });
        let res =
            validate_and_sanitize_rt_json(&payload, &RtJsonValidationConfig::for_locale("en"))
                .expect("valid payload");
        assert_eq!(res.sanitized["doc"]["content"].as_array().unwrap().len(), 1);
        let marks = &res.sanitized["doc"]["content"][0]["content"][0]["marks"];
        assert_eq!(marks.as_array().unwrap().len(), 1);
    }
}

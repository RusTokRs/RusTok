use serde_json::Value;

use crate::rt_json::{validate_and_sanitize_rt_json, RtJsonValidationConfig};

pub const CONTENT_FORMAT_MARKDOWN: &str = "markdown";
pub const CONTENT_FORMAT_RT_JSON_V1: &str = "rt_json_v1";
const LEGACY_CONTENT_FORMAT_RT_JSON: &str = "rt_json";

#[derive(Debug, Clone)]
pub struct PreparedContent {
    pub format: String,
    pub body: String,
}

pub fn normalize_content_format(format: Option<&str>) -> Result<String, String> {
    let normalized = format
        .unwrap_or(CONTENT_FORMAT_MARKDOWN)
        .trim()
        .to_ascii_lowercase();

    match normalized.as_str() {
        CONTENT_FORMAT_MARKDOWN => Ok(CONTENT_FORMAT_MARKDOWN.to_string()),
        CONTENT_FORMAT_RT_JSON_V1 | LEGACY_CONTENT_FORMAT_RT_JSON => {
            Ok(CONTENT_FORMAT_RT_JSON_V1.to_string())
        }
        _ => Err(format!(
            "Unsupported content format '{normalized}'. Supported formats: markdown, rt_json_v1"
        )),
    }
}

pub fn prepare_content_payload(
    format: Option<&str>,
    markdown_text: Option<&str>,
    content_json: Option<&Value>,
    locale: &str,
    field_name: &str,
) -> Result<PreparedContent, String> {
    let normalized = normalize_content_format(format)?;

    if normalized == CONTENT_FORMAT_MARKDOWN {
        let body = markdown_text
            .ok_or_else(|| format!("{field_name} is required for markdown format"))?
            .to_string();
        if body.trim().is_empty() {
            return Err(format!("{field_name} cannot be empty"));
        }
        return Ok(PreparedContent {
            format: normalized,
            body,
        });
    }

    let json_payload = if let Some(content_json) = content_json {
        content_json.clone()
    } else {
        let raw = markdown_text
            .ok_or_else(|| "content_json is required for rt_json_v1 format".to_string())?;
        serde_json::from_str(raw)
            .map_err(|_| "content_json must be valid rt_json_v1 JSON payload".to_string())?
    };

    let validation =
        validate_and_sanitize_rt_json(&json_payload, &RtJsonValidationConfig::for_locale(locale))?;

    Ok(PreparedContent {
        format: normalized,
        body: validation.sanitized.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_legacy_rt_json_alias() {
        assert_eq!(
            normalize_content_format(Some("rt_json")).expect("format"),
            CONTENT_FORMAT_RT_JSON_V1
        );
    }
}

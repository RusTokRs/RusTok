use serde_json::Value;

pub fn validate_grapesjs_project(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "content_json must be a JSON object for grapesjs format".to_string())?;

    if let Some(pages) = object.get("pages") {
        if !pages.is_array() {
            return Err("content_json.pages must be an array for grapesjs format".to_string());
        }
    }

    if let Some(styles) = object.get("styles") {
        if !styles.is_array() {
            return Err("content_json.styles must be an array for grapesjs format".to_string());
        }
    }

    if let Some(assets) = object.get("assets") {
        if !assets.is_array() {
            return Err("content_json.assets must be an array for grapesjs format".to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_grapesjs_project;

    #[test]
    fn accepts_minimal_project_object() {
        assert!(validate_grapesjs_project(&serde_json::json!({})).is_ok());
        assert!(
            validate_grapesjs_project(&serde_json::json!({
                "pages": [],
                "styles": [],
                "assets": [],
            }))
            .is_ok()
        );
    }

    #[test]
    fn rejects_non_object_or_invalid_known_collections() {
        assert!(validate_grapesjs_project(&serde_json::json!(["bad"])).is_err());
        assert!(
            validate_grapesjs_project(&serde_json::json!({
                "pages": {}
            }))
            .is_err()
        );
        assert!(
            validate_grapesjs_project(&serde_json::json!({
                "styles": {}
            }))
            .is_err()
        );
        assert!(
            validate_grapesjs_project(&serde_json::json!({
                "assets": {}
            }))
            .is_err()
        );
    }
}

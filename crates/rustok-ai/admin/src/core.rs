use rustok_ui_core::{normalize_ui_text, parse_ui_csv};

pub fn parse_csv(value: String) -> Vec<String> {
    parse_ui_csv(value.as_str())
}

pub fn optional_text(value: String) -> Option<String> {
    normalize_ui_text(value.as_str())
}

pub fn alloy_task_payload(
    operation: String,
    script_id: Option<String>,
    script_name: Option<String>,
    script_source: Option<String>,
    runtime_payload_json: Option<String>,
    assistant_prompt: Option<String>,
) -> Result<String, serde_json::Error> {
    let payload = serde_json::json!({
        "operation": operation,
        "script_id": script_id,
        "script_name": script_name,
        "script_source": script_source,
        "runtime_payload_json": runtime_payload_json,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}

pub struct ImageTaskPayloadInput {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub file_name: Option<String>,
    pub size: Option<String>,
    pub assistant_prompt: Option<String>,
}

pub fn image_task_payload(input: ImageTaskPayloadInput) -> Result<String, serde_json::Error> {
    let ImageTaskPayloadInput {
        prompt,
        negative_prompt,
        title,
        alt_text,
        caption,
        file_name,
        size,
        assistant_prompt,
    } = input;
    let payload = serde_json::json!({
        "prompt": prompt,
        "negative_prompt": negative_prompt,
        "title": title,
        "alt_text": alt_text,
        "caption": caption,
        "file_name": file_name,
        "size": size,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}

pub struct ProductTaskPayloadInput {
    pub product_id: String,
    pub source_locale: Option<String>,
    pub source_title: Option<String>,
    pub source_description: Option<String>,
    pub source_meta_title: Option<String>,
    pub source_meta_description: Option<String>,
    pub copy_instructions: Option<String>,
    pub assistant_prompt: Option<String>,
}

pub fn product_task_payload(input: ProductTaskPayloadInput) -> Result<String, serde_json::Error> {
    let ProductTaskPayloadInput {
        product_id,
        source_locale,
        source_title,
        source_description,
        source_meta_title,
        source_meta_description,
        copy_instructions,
        assistant_prompt,
    } = input;
    let product_id = uuid::Uuid::parse_str(product_id.trim()).map_err(invalid_input_error)?;
    let payload = serde_json::json!({
        "product_id": product_id,
        "source_locale": source_locale,
        "source_title": source_title,
        "source_description": source_description,
        "source_meta_title": source_meta_title,
        "source_meta_description": source_meta_description,
        "copy_instructions": copy_instructions,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}

fn parse_csv_urls(value: String) -> Result<Vec<String>, serde_json::Error> {
    let entries = parse_csv(value);
    let mut parsed = Vec::with_capacity(entries.len());
    for entry in entries {
        let normalized = entry.trim();
        let is_http = normalized.starts_with("http://") || normalized.starts_with("https://");
        if !is_http {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "only http/https image URLs are allowed",
            )));
        }
        if normalized.split("//").nth(1).unwrap_or_default().is_empty() {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "image URL host is required",
            )));
        }
        parsed.push(normalized.to_string());
    }
    Ok(parsed)
}

pub struct ProductAttributesTaskPayloadInput {
    pub product_id: String,
    pub category_slug: Option<String>,
    pub source_locale: Option<String>,
    pub source_title: Option<String>,
    pub source_description: Option<String>,
    pub image_urls_csv: String,
    pub copy_instructions: Option<String>,
    pub assistant_prompt: Option<String>,
}

pub fn product_attributes_task_payload(
    input: ProductAttributesTaskPayloadInput,
) -> Result<String, serde_json::Error> {
    let ProductAttributesTaskPayloadInput {
        product_id,
        category_slug,
        source_locale,
        source_title,
        source_description,
        image_urls_csv,
        copy_instructions,
        assistant_prompt,
    } = input;
    let product_id = uuid::Uuid::parse_str(product_id.trim()).map_err(invalid_input_error)?;

    let source_title = source_title.map(|value| value.trim().to_string());
    let source_description = source_description.map(|value| value.trim().to_string());

    if source_title.as_deref().unwrap_or_default().is_empty()
        && source_description.as_deref().unwrap_or_default().is_empty()
    {
        return Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "source title or source description is required",
        )));
    }

    let image_urls = parse_csv_urls(image_urls_csv)?;

    let payload = serde_json::json!({
        "product_id": product_id,
        "category_slug": category_slug
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty()),
        "source_locale": source_locale,
        "source_title": source_title,
        "source_description": source_description,
        "image_urls": image_urls,
        "copy_instructions": copy_instructions,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}

pub struct BlogTaskPayloadInput {
    pub post_id: Option<String>,
    pub source_locale: Option<String>,
    pub source_title: Option<String>,
    pub source_body: Option<String>,
    pub source_excerpt: Option<String>,
    pub source_seo_title: Option<String>,
    pub source_seo_description: Option<String>,
    pub tags: Vec<String>,
    pub category_id: Option<String>,
    pub featured_image_url: Option<String>,
    pub copy_instructions: Option<String>,
    pub assistant_prompt: Option<String>,
}

pub fn blog_task_payload(input: BlogTaskPayloadInput) -> Result<String, serde_json::Error> {
    let BlogTaskPayloadInput {
        post_id,
        source_locale,
        source_title,
        source_body,
        source_excerpt,
        source_seo_title,
        source_seo_description,
        tags,
        category_id,
        featured_image_url,
        copy_instructions,
        assistant_prompt,
    } = input;
    let post_id = post_id
        .map(|value| uuid::Uuid::parse_str(value.trim()).map_err(invalid_input_error))
        .transpose()?;
    let category_id = category_id
        .map(|value| uuid::Uuid::parse_str(value.trim()).map_err(invalid_input_error))
        .transpose()?;
    let payload = serde_json::json!({
        "post_id": post_id,
        "source_locale": source_locale,
        "source_title": source_title,
        "source_body": source_body,
        "source_excerpt": source_excerpt,
        "source_seo_title": source_seo_title,
        "source_seo_description": source_seo_description,
        "tags": tags,
        "category_id": category_id,
        "featured_image_url": featured_image_url,
        "copy_instructions": copy_instructions,
        "assistant_prompt": assistant_prompt,
    });
    serde_json::to_string(&payload)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentRunSummaryStats {
    pub total: usize,
    pub failed: usize,
    pub waiting_approval: usize,
    pub average_latency_ms: u64,
}

impl RecentRunSummaryStats {
    pub const fn empty() -> Self {
        Self {
            total: 0,
            failed: 0,
            waiting_approval: 0,
            average_latency_ms: 0,
        }
    }
}

pub fn average_latency_ms(total_latency_ms: u64, samples: u64) -> u64 {
    total_latency_ms.checked_div(samples).unwrap_or_default()
}

pub fn summarize_recent_runs<I, S>(runs: I) -> RecentRunSummaryStats
where
    I: IntoIterator<Item = (S, i64)>,
    S: AsRef<str>,
{
    let mut stats = RecentRunSummaryStats::empty();
    let mut total_latency_ms = 0_u64;

    for (status, duration_ms) in runs {
        stats.total += 1;
        match status.as_ref() {
            "failed" => stats.failed += 1,
            "waiting_approval" => stats.waiting_approval += 1,
            _ => {}
        }
        total_latency_ms += duration_ms.max(0) as u64;
    }

    if stats.total > 0 {
        stats.average_latency_ms = total_latency_ms / stats.total as u64;
    }

    stats
}

fn invalid_input_error(error: uuid::Error) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_attributes_payload_requires_seed_content() {
        let result = product_attributes_task_payload(ProductAttributesTaskPayloadInput {
            product_id: uuid::Uuid::new_v4().to_string(),
            category_slug: Some("Electronics".to_string()),
            source_locale: Some("en".to_string()),
            source_title: Some("  ".to_string()),
            source_description: Some("".to_string()),
            image_urls_csv: String::new(),
            copy_instructions: None,
            assistant_prompt: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn product_attributes_payload_normalizes_category_slug() {
        let payload = product_attributes_task_payload(ProductAttributesTaskPayloadInput {
            product_id: uuid::Uuid::new_v4().to_string(),
            category_slug: Some("  Electronics / Audio  ".to_string()),
            source_locale: Some("en".to_string()),
            source_title: Some("Title".to_string()),
            source_description: None,
            image_urls_csv: "https://example.com/a.jpg".to_string(),
            copy_instructions: None,
            assistant_prompt: None,
        })
        .expect("payload should be valid");

        let json: serde_json::Value =
            serde_json::from_str(payload.as_str()).expect("payload must be JSON");
        assert_eq!(
            json.get("category_slug").and_then(|value| value.as_str()),
            Some("electronics / audio")
        );
    }

    #[test]
    fn product_attributes_payload_accepts_multiple_https_urls() {
        let payload = product_attributes_task_payload(ProductAttributesTaskPayloadInput {
            product_id: uuid::Uuid::new_v4().to_string(),
            category_slug: Some("electronics".to_string()),
            source_locale: Some("en".to_string()),
            source_title: Some("Title".to_string()),
            source_description: None,
            image_urls_csv: "https://example.com/a.jpg, https://cdn.example.com/b.webp".to_string(),
            copy_instructions: None,
            assistant_prompt: None,
        })
        .expect("payload should be valid");

        let json: serde_json::Value =
            serde_json::from_str(payload.as_str()).expect("payload must be JSON");
        let urls = json
            .get("image_urls")
            .and_then(|value| value.as_array())
            .expect("image_urls must be array");
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn product_attributes_payload_rejects_non_http_urls() {
        let result = product_attributes_task_payload(ProductAttributesTaskPayloadInput {
            product_id: uuid::Uuid::new_v4().to_string(),
            category_slug: Some("Electronics".to_string()),
            source_locale: Some("en".to_string()),
            source_title: Some("Title".to_string()),
            source_description: None,
            image_urls_csv: "ftp://example.com/a.jpg".to_string(),
            copy_instructions: None,
            assistant_prompt: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn product_attributes_payload_rejects_http_without_host() {
        let result = product_attributes_task_payload(ProductAttributesTaskPayloadInput {
            product_id: uuid::Uuid::new_v4().to_string(),
            category_slug: Some("Electronics".to_string()),
            source_locale: Some("en".to_string()),
            source_title: Some("Title".to_string()),
            source_description: None,
            image_urls_csv: "http://".to_string(),
            copy_instructions: None,
            assistant_prompt: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn product_attributes_payload_allows_empty_image_urls() {
        let result = product_attributes_task_payload(ProductAttributesTaskPayloadInput {
            product_id: uuid::Uuid::new_v4().to_string(),
            category_slug: Some("Electronics".to_string()),
            source_locale: Some("en".to_string()),
            source_title: Some("Title".to_string()),
            source_description: None,
            image_urls_csv: String::new(),
            copy_instructions: None,
            assistant_prompt: None,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn recent_run_summary_stats_counts_failures_waiting_and_non_negative_latency() {
        let stats = summarize_recent_runs([
            ("completed", 100),
            ("failed", -20),
            ("waiting_approval", 50),
        ]);

        assert_eq!(stats.total, 3);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.waiting_approval, 1);
        assert_eq!(stats.average_latency_ms, 50);
    }

    #[test]
    fn average_latency_ms_returns_zero_without_samples() {
        assert_eq!(average_latency_ms(120, 0), 0);
        assert_eq!(average_latency_ms(120, 3), 40);
    }
}

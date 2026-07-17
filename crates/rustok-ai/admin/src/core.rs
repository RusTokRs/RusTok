use chrono::{DateTime, Utc};
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

pub struct OrderAnalyticsTaskPayloadInput {
    pub order_ids_csv: String,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub focus: Option<String>,
    pub assistant_prompt: Option<String>,
}

pub fn order_analytics_task_payload(
    input: OrderAnalyticsTaskPayloadInput,
) -> Result<String, serde_json::Error> {
    let OrderAnalyticsTaskPayloadInput {
        order_ids_csv,
        date_from,
        date_to,
        focus,
        assistant_prompt,
    } = input;
    let order_ids = parse_csv(order_ids_csv)
        .into_iter()
        .map(|value| uuid::Uuid::parse_str(value.as_str()).map_err(invalid_input_error))
        .collect::<Result<Vec<_>, _>>()?;
    if order_ids.is_empty() {
        return Err(invalid_input_message("at least one order id is required"));
    }

    let date_from = parse_optional_rfc3339(date_from)?;
    let date_to = parse_optional_rfc3339(date_to)?;
    if date_from
        .as_ref()
        .zip(date_to.as_ref())
        .is_some_and(|(date_from, date_to)| date_from > date_to)
    {
        return Err(invalid_input_message("date_from must not be after date_to"));
    }

    let payload = serde_json::json!({
        "order_ids": order_ids,
        "date_from": date_from,
        "date_to": date_to,
        "focus": focus.and_then(|value| optional_text(value)),
        "assistant_prompt": assistant_prompt.and_then(|value| optional_text(value)),
    });
    serde_json::to_string(&payload)
}

pub struct OrderOpsAssistantTaskPayloadInput {
    pub order_id: String,
    pub recommended_action: Option<String>,
    pub context: Option<String>,
    pub assistant_prompt: Option<String>,
}

pub fn order_ops_assistant_task_payload(
    input: OrderOpsAssistantTaskPayloadInput,
) -> Result<String, serde_json::Error> {
    let OrderOpsAssistantTaskPayloadInput {
        order_id,
        recommended_action,
        context,
        assistant_prompt,
    } = input;
    let order_id = uuid::Uuid::parse_str(order_id.trim()).map_err(invalid_input_error)?;
    let payload = serde_json::json!({
        "order_id": order_id,
        "recommended_action": recommended_action.and_then(|value| optional_text(value)),
        "context": context.and_then(|value| optional_text(value)),
        "assistant_prompt": assistant_prompt.and_then(|value| optional_text(value)),
    });
    serde_json::to_string(&payload)
}

fn parse_optional_rfc3339(
    value: Option<String>,
) -> Result<Option<DateTime<Utc>>, serde_json::Error> {
    value
        .and_then(|value| optional_text(value))
        .map(|value| {
            DateTime::parse_from_rfc3339(value.as_str())
                .map(|value| value.with_timezone(&Utc))
                .map_err(|error| invalid_input_message(error.to_string().as_str()))
        })
        .transpose()
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
    invalid_input_message(error.to_string().as_str())
}

fn invalid_input_message(message: &str) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_analytics_payload_normalizes_ids_and_period() {
        let first = uuid::Uuid::new_v4();
        let second = uuid::Uuid::new_v4();
        let payload = order_analytics_task_payload(OrderAnalyticsTaskPayloadInput {
            order_ids_csv: format!(" {first},\n{second} "),
            date_from: Some("2026-07-01T00:00:00Z".to_string()),
            date_to: Some("2026-07-02T00:00:00+00:00".to_string()),
            focus: Some("  shipping risk  ".to_string()),
            assistant_prompt: Some("  summarize  ".to_string()),
        })
        .expect("payload should be valid");

        let json: serde_json::Value =
            serde_json::from_str(payload.as_str()).expect("payload must be JSON");
        assert_eq!(json["order_ids"].as_array().map(Vec::len), Some(2));
        assert_eq!(json["focus"].as_str(), Some("shipping risk"));
        assert_eq!(json["assistant_prompt"].as_str(), Some("summarize"));
        assert_eq!(json["date_from"].as_str(), Some("2026-07-01T00:00:00Z"));
    }

    #[test]
    fn order_analytics_payload_requires_ids_and_a_valid_period() {
        assert!(
            order_analytics_task_payload(OrderAnalyticsTaskPayloadInput {
                order_ids_csv: String::new(),
                date_from: None,
                date_to: None,
                focus: None,
                assistant_prompt: None,
            })
            .is_err()
        );
        assert!(
            order_analytics_task_payload(OrderAnalyticsTaskPayloadInput {
                order_ids_csv: uuid::Uuid::new_v4().to_string(),
                date_from: Some("2026-07-03T00:00:00Z".to_string()),
                date_to: Some("2026-07-02T00:00:00Z".to_string()),
                focus: None,
                assistant_prompt: None,
            })
            .is_err()
        );
        assert!(
            order_analytics_task_payload(OrderAnalyticsTaskPayloadInput {
                order_ids_csv: uuid::Uuid::new_v4().to_string(),
                date_from: Some("2026-07-03T00:00:00Z".to_string()),
                date_to: None,
                focus: None,
                assistant_prompt: None,
            })
            .is_ok()
        );
    }

    #[test]
    fn order_ops_payload_requires_a_valid_order_id() {
        let order_id = uuid::Uuid::new_v4();
        let payload = order_ops_assistant_task_payload(OrderOpsAssistantTaskPayloadInput {
            order_id: format!(" {order_id} "),
            recommended_action: Some("  contact_customer  ".to_string()),
            context: Some("  address mismatch  ".to_string()),
            assistant_prompt: None,
        })
        .expect("payload should be valid");
        let json: serde_json::Value =
            serde_json::from_str(payload.as_str()).expect("payload must be JSON");
        assert_eq!(
            json["order_id"].as_str(),
            Some(order_id.to_string().as_str())
        );
        assert_eq!(
            json["recommended_action"].as_str(),
            Some("contact_customer")
        );
        assert!(
            order_ops_assistant_task_payload(OrderOpsAssistantTaskPayloadInput {
                order_id: "not-a-uuid".to_string(),
                recommended_action: None,
                context: None,
                assistant_prompt: None,
            })
            .is_err()
        );
    }

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

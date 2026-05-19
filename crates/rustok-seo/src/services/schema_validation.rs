use serde_json::Value;

use crate::dto::{SeoDiagnosticSeverity, SeoSchemaBlockKind, SeoStructuredDataBlock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaValidationIssue {
    pub code: &'static str,
    pub severity: SeoDiagnosticSeverity,
    pub message: String,
}

pub fn validate_schema_block(block: &SeoStructuredDataBlock) -> Vec<SchemaValidationIssue> {
    let mut issues = Vec::new();
    let payload = &block.payload.0;

    if block.schema_kind == SeoSchemaBlockKind::Unknown {
        issues.push(SchemaValidationIssue {
            code: "unsupported_schema_source_payload",
            severity: SeoDiagnosticSeverity::Warning,
            message: "Schema block has no recognized schema.org @type.".to_string(),
        });
        return issues;
    }

    let object = match payload {
        Value::Object(obj) => obj,
        _ => {
            issues.push(SchemaValidationIssue {
                code: "unsupported_schema_source_payload",
                severity: SeoDiagnosticSeverity::Warning,
                message: "Schema block payload is not a JSON object.".to_string(),
            });
            return issues;
        }
    };

    for field in required_fields_for_kind(block.schema_kind) {
        if !object.contains_key(*field) {
            issues.push(SchemaValidationIssue {
                code: "missing_required_schema_field",
                severity: SeoDiagnosticSeverity::Warning,
                message: format!(
                    "Schema `{}` is missing required field `{}`.",
                    block.schema_type.as_deref().unwrap_or("unknown"),
                    field
                ),
            });
        }
    }

    match block.schema_kind {
        SeoSchemaBlockKind::BreadcrumbList | SeoSchemaBlockKind::ItemList => {
            if let Some(value) = object.get("itemListElement") {
                if !value.is_array() {
                    issues.push(SchemaValidationIssue {
                        code: "invalid_schema_shape",
                        severity: SeoDiagnosticSeverity::Error,
                        message:
                            "itemListElement must be an array for BreadcrumbList/ItemList."
                                .to_string(),
                    });
                }
            }
        }
        SeoSchemaBlockKind::FAQPage => {
            if let Some(value) = object.get("mainEntity") {
                if !value.is_array() {
                    issues.push(SchemaValidationIssue {
                        code: "invalid_schema_shape",
                        severity: SeoDiagnosticSeverity::Error,
                        message: "mainEntity must be an array for FAQPage.".to_string(),
                    });
                }
            }
        }
        _ => {}
    }

    issues
}

pub fn required_fields_for_kind(kind: SeoSchemaBlockKind) -> &'static [&'static str] {
    match kind {
        SeoSchemaBlockKind::Product => &["name"],
        SeoSchemaBlockKind::Offer => &["price", "priceCurrency"],
        SeoSchemaBlockKind::BreadcrumbList => &["itemListElement"],
        SeoSchemaBlockKind::ItemList => &["itemListElement"],
        SeoSchemaBlockKind::FAQPage => &["mainEntity"],
        SeoSchemaBlockKind::HowTo => &["name"],
        SeoSchemaBlockKind::Organization => &["name"],
        SeoSchemaBlockKind::LocalBusiness => &["name"],
        SeoSchemaBlockKind::WebSite => &["name"],
        SeoSchemaBlockKind::Article
        | SeoSchemaBlockKind::BlogPosting
        | SeoSchemaBlockKind::NewsArticle => &["headline"],
        SeoSchemaBlockKind::DiscussionForumPosting => &["headline"],
        SeoSchemaBlockKind::WebPage | SeoSchemaBlockKind::CollectionPage => &["name"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::Json;
    use serde_json::json;

    use crate::dto::{SeoFieldSource, SeoSchemaBlockKind, SeoStructuredDataBlock};

    use super::*;

    fn block(kind: SeoSchemaBlockKind, payload: Value) -> SeoStructuredDataBlock {
        let schema_type = match kind {
            SeoSchemaBlockKind::Unknown => None,
            _ => Some(kind.as_str().to_string()),
        };
        SeoStructuredDataBlock {
            id: None,
            schema_kind: kind,
            schema_type: schema_type.clone(),
            kind: schema_type,
            source: SeoFieldSource::Explicit,
            payload: Json(payload),
        }
    }

    #[test]
    fn product_missing_name_emits_required_field_issue() {
        let issues = validate_schema_block(&block(
            SeoSchemaBlockKind::Product,
            json!({"@type": "Product", "description": "Demo"}),
        ));
        assert!(issues.iter().any(|i| i.code == "missing_required_schema_field"));
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn breadcrumb_list_with_non_array_item_list_element_emits_shape_error() {
        let issues = validate_schema_block(&block(
            SeoSchemaBlockKind::BreadcrumbList,
            json!({"@type": "BreadcrumbList", "itemListElement": "not-array"}),
        ));
        assert!(issues.iter().any(|i| i.code == "invalid_schema_shape"));
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn unknown_kind_emits_unsupported_payload() {
        let issues = validate_schema_block(&SeoStructuredDataBlock {
            id: None,
            schema_kind: SeoSchemaBlockKind::Unknown,
            schema_type: None,
            kind: None,
            source: SeoFieldSource::Explicit,
            payload: Json(json!({"name": "No type"})),
        });
        assert!(issues
            .iter()
            .any(|i| i.code == "unsupported_schema_source_payload"));
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn valid_product_has_no_issues() {
        let issues = validate_schema_block(&block(
            SeoSchemaBlockKind::Product,
            json!({"@type": "Product", "name": "Demo"}),
        ));
        assert!(issues.is_empty());
    }

    #[test]
    fn faq_page_missing_main_entity_emits_required_field() {
        let issues = validate_schema_block(&block(
            SeoSchemaBlockKind::FAQPage,
            json!({"@type": "FAQPage"}),
        ));
        assert!(issues.iter().any(|i| i.code == "missing_required_schema_field"));
    }

    #[test]
    fn faq_page_non_array_main_entity_emits_shape_error() {
        let issues = validate_schema_block(&block(
            SeoSchemaBlockKind::FAQPage,
            json!({"@type": "FAQPage", "mainEntity": "not-array"}),
        ));
        assert!(issues.iter().any(|i| i.code == "invalid_schema_shape"));
    }
}

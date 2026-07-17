use super::{
    LandingReadinessCategory, LandingReadinessCategorySummary, LandingReadinessIssue,
    LandingReadinessPolicy, LandingReadinessReport,
};
use crate::{
    analyze_project_locale_coverage, audit_page, localized_page_route_index,
    materialize_component_actions, materialize_project_with_runtime_context, validate_project,
    AuditSeverity, PageLocator, ProjectDocument, RegistrySet, ValidationDiagnostic,
    ValidationLimits, ValidationSeverity, FLY_PAGE_METADATA_FIELD, LOCALIZED_VALUES_FIELD,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn evaluate_landing_readiness(
    document: &ProjectDocument,
    policy: LandingReadinessPolicy,
) -> LandingReadinessReport {
    let mut issues = validate_project(
        document,
        &RegistrySet::with_builtins(),
        ValidationLimits::default(),
    )
    .diagnostics
    .into_iter()
    .map(|diagnostic| LandingReadinessIssue {
        category: classify_validation_diagnostic(&diagnostic),
        diagnostic,
    })
    .collect::<Vec<_>>();

    let runtime_materialization =
        materialize_project_with_runtime_context(document, &Value::Object(Default::default()));
    issues.extend(
        runtime_materialization
            .diagnostics
            .iter()
            .cloned()
            .map(|diagnostic| LandingReadinessIssue {
                category: classify_validation_diagnostic(&diagnostic),
                diagnostic,
            }),
    );
    let action_materialization = materialize_component_actions(
        &runtime_materialization.document,
        &runtime_materialization.effective_context,
    );
    issues.extend(
        action_materialization
            .diagnostics
            .iter()
            .cloned()
            .map(|diagnostic| LandingReadinessIssue {
                category: classify_validation_diagnostic(&diagnostic),
                diagnostic,
            }),
    );

    let audit_document = &action_materialization.document;
    let routes = localized_page_route_index(document);
    for (page_index, page) in document.project.pages.iter().enumerate() {
        let path = format!("project.pages[{page_index}]");
        if policy.require_page_id
            && page
                .id
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty)
        {
            issues.push(issue(
                LandingReadinessCategory::Routes,
                ValidationSeverity::Error,
                "landing_page_id_required",
                format!("{path}.id"),
                "landing page must have a stable page id",
            ));
        }

        let metadata = page
            .extensions
            .get(FLY_PAGE_METADATA_FIELD)
            .and_then(Value::as_object);
        if policy.require_title && !metadata_has_text(metadata, "title") {
            issues.push(issue(
                LandingReadinessCategory::Seo,
                ValidationSeverity::Error,
                "landing_page_title_required",
                format!("{path}.{FLY_PAGE_METADATA_FIELD}.title"),
                "landing page must define a plain or localized SEO title",
            ));
        }
        if policy.require_description && !metadata_has_text(metadata, "description") {
            issues.push(issue(
                LandingReadinessCategory::Seo,
                ValidationSeverity::Error,
                "landing_page_description_required",
                format!("{path}.{FLY_PAGE_METADATA_FIELD}.description"),
                "landing page must define a plain or localized SEO description",
            ));
        }
        if policy.require_slug && !routes.iter().any(|route| route.page_index == page_index) {
            issues.push(issue(
                LandingReadinessCategory::Routes,
                ValidationSeverity::Error,
                "landing_page_slug_required",
                format!("{path}.{FLY_PAGE_METADATA_FIELD}.slug"),
                "landing page must define at least one valid plain or localized slug",
            ));
        }

        let audit = audit_page(audit_document, &PageLocator::by_index(page_index));
        for diagnostic in audit.diagnostics {
            if matches!(
                diagnostic.code.as_str(),
                "page_missing_title" | "page_missing_description" | "page_missing_slug"
            ) {
                continue;
            }
            let mut severity = match diagnostic.severity {
                AuditSeverity::Info => ValidationSeverity::Info,
                AuditSeverity::Warning => ValidationSeverity::Warning,
                AuditSeverity::Error => ValidationSeverity::Error,
            };
            if policy.require_h1 && diagnostic.code == "missing_h1" {
                severity = ValidationSeverity::Error;
            }
            issues.push(issue(
                LandingReadinessCategory::Content,
                severity,
                format!("landing_{}", diagnostic.code),
                format!("{path}.{}", diagnostic.path),
                diagnostic.message,
            ));
        }
    }

    deduplicate_issues(&mut issues);
    let locale_coverage = analyze_project_locale_coverage(document);
    let ready = !issues.iter().any(|issue| {
        issue.diagnostic.severity == ValidationSeverity::Error
            || (policy.block_on_warnings
                && issue.diagnostic.severity == ValidationSeverity::Warning)
    });
    let categories = category_summaries(&issues);

    LandingReadinessReport {
        ready,
        block_on_warnings: policy.block_on_warnings,
        page_count: document.project.pages.len(),
        issues,
        categories,
        locale_coverage,
    }
}

fn metadata_has_text(metadata: Option<&serde_json::Map<String, Value>>, field: &str) -> bool {
    let Some(value) = metadata.and_then(|metadata| metadata.get(field)) else {
        return false;
    };
    match value {
        Value::String(value) => !value.trim().is_empty(),
        Value::Object(wrapper) => wrapper
            .get(LOCALIZED_VALUES_FIELD)
            .and_then(Value::as_object)
            .is_some_and(|values| {
                values.values().any(|value| {
                    value
                        .as_str()
                        .is_some_and(|value| !value.trim().is_empty())
                })
            }),
        _ => false,
    }
}

fn classify_validation_diagnostic(
    diagnostic: &ValidationDiagnostic,
) -> LandingReadinessCategory {
    let code = diagnostic.code.as_str();
    if code.contains("locale")
        || code.starts_with("translation_")
        || code.starts_with("localized_metadata_")
    {
        LandingReadinessCategory::Locales
    } else if code.contains("slug")
        || code.contains("route")
        || code.starts_with("internal_page_link_")
        || code.starts_with("component_navigation_")
    {
        LandingReadinessCategory::Routes
    } else if code.starts_with("seo_")
        || code.contains("metadata_url")
        || code.starts_with("landing_page_title")
        || code.starts_with("landing_page_description")
    {
        LandingReadinessCategory::Seo
    } else if code.starts_with("runtime_")
        || code.starts_with("action_")
        || code.starts_with("form_")
        || code.starts_with("duplicate_form_")
        || code.starts_with("binding_")
        || code.starts_with("dynamic_")
    {
        LandingReadinessCategory::RuntimeContracts
    } else {
        LandingReadinessCategory::Content
    }
}

fn category_summaries(
    issues: &[LandingReadinessIssue],
) -> Vec<LandingReadinessCategorySummary> {
    let mut counts = BTreeMap::<LandingReadinessCategory, (usize, usize, usize)>::new();
    for issue in issues {
        let entry = counts.entry(issue.category).or_default();
        match issue.diagnostic.severity {
            ValidationSeverity::Error => entry.0 = entry.0.saturating_add(1),
            ValidationSeverity::Warning => entry.1 = entry.1.saturating_add(1),
            ValidationSeverity::Info => entry.2 = entry.2.saturating_add(1),
        }
    }
    LandingReadinessCategory::all()
        .into_iter()
        .map(|category| {
            let (error_count, warning_count, info_count) =
                counts.get(&category).copied().unwrap_or_default();
            LandingReadinessCategorySummary {
                category,
                error_count,
                warning_count,
                info_count,
            }
        })
        .collect()
}

fn deduplicate_issues(issues: &mut Vec<LandingReadinessIssue>) {
    let mut seen = BTreeSet::new();
    issues.retain(|issue| {
        seen.insert((
            issue.category,
            issue.diagnostic.severity as u8,
            issue.diagnostic.code.clone(),
            issue.diagnostic.path.clone(),
            issue.diagnostic.message.clone(),
        ))
    });
}

fn issue(
    category: LandingReadinessCategory,
    severity: ValidationSeverity,
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> LandingReadinessIssue {
    LandingReadinessIssue {
        category,
        diagnostic: ValidationDiagnostic {
            severity,
            code: code.into(),
            path: path.into(),
            message: message.into(),
        },
    }
}

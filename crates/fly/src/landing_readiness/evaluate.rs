use super::{
    LandingReadinessCategory, LandingReadinessCategorySummary, LandingReadinessIssue,
    LandingReadinessPolicy, LandingReadinessReport,
};
use crate::{
    AuditSeverity, FLY_PAGE_METADATA_FIELD, LOCALIZED_VALUES_FIELD, LocaleCoverageKind,
    PageLocator, ProjectDocument, RegistrySet, ValidationDiagnostic, ValidationLimits,
    ValidationSeverity, analyze_project_locale_coverage, audit_page,
    extract_runtime_context_contract, localized_page_route_index, materialize_bindings,
    materialize_component_actions, materialize_context, materialize_internal_page_links,
    materialize_localized_page_metadata, materialize_project_locale_context,
    materialize_project_translations, materialize_project_with_runtime_context,
    materialize_runtime_locale_context, validate_component_actions, validate_internal_page_links,
    validate_project,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub fn evaluate_landing_readiness(
    document: &ProjectDocument,
    policy: LandingReadinessPolicy,
) -> LandingReadinessReport {
    evaluate_landing_readiness_with_context(document, None, policy)
}

pub fn evaluate_landing_readiness_with_context(
    document: &ProjectDocument,
    runtime_context: Option<&Value>,
    policy: LandingReadinessPolicy,
) -> LandingReadinessReport {
    let mut issues = validate_project(
        document,
        &RegistrySet::with_builtins(),
        ValidationLimits::default(),
    )
    .diagnostics
    .into_iter()
    .map(classified_issue)
    .collect::<Vec<_>>();

    let audit_document = match runtime_context {
        Some(context) => {
            // A publish gate may provide the exact runtime context that will be rendered. In that
            // case readiness must audit the fully materialized output, including translations,
            // defaults, computed values, bindings, repeaters, forms, and actions.
            let materialized = materialize_project_with_runtime_context(document, context);
            issues.extend(
                materialized
                    .diagnostics
                    .iter()
                    .cloned()
                    .map(classified_issue),
            );
            materialized.document
        }
        None => materialize_structural_document(document, &mut issues),
    };

    let routes = localized_page_route_index(document);
    for (page_index, page) in document.project.pages.iter().enumerate() {
        let path = format!("project.pages[{page_index}]");
        if policy.require_page_id && page.id.as_deref().map(str::trim).is_none_or(str::is_empty) {
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

        let audit = audit_page(&audit_document, &PageLocator::by_index(page_index));
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

    let locale_coverage = analyze_project_locale_coverage(document);
    for gap in &locale_coverage.gaps {
        let (code, subject) = match gap.kind {
            LocaleCoverageKind::Translation => (
                "landing_translation_locale_missing",
                format!("translation `{}`", gap.label),
            ),
            LocaleCoverageKind::PageMetadata => (
                "landing_metadata_locale_missing",
                format!("metadata field `{}`", gap.label),
            ),
        };
        issues.push(issue(
            LandingReadinessCategory::Locales,
            if gap.required {
                ValidationSeverity::Error
            } else {
                ValidationSeverity::Warning
            },
            code,
            gap.path.clone(),
            format!("{subject} is missing locale `{}`", gap.locale),
        ));
    }

    deduplicate_issues(&mut issues);
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

fn materialize_structural_document(
    document: &ProjectDocument,
    issues: &mut Vec<LandingReadinessIssue>,
) -> ProjectDocument {
    // A standalone report has no business-data instance, but it still must apply everything the
    // project itself guarantees: locale policy, translations, schema defaults, computed fallbacks,
    // binding fallbacks, localized metadata, links, forms, and actions. Conditions and repeaters
    // deliberately remain unexpanded until a real publish context is supplied.
    let locale_materialization =
        materialize_project_locale_context(document, &Value::Object(Map::new()));
    issues.extend(
        locale_materialization
            .diagnostics
            .iter()
            .cloned()
            .map(classified_issue),
    );

    let translation_materialization =
        materialize_project_translations(document, &locale_materialization.context);
    issues.extend(
        translation_materialization
            .diagnostics
            .iter()
            .cloned()
            .map(classified_issue),
    );

    let locale_context_materialization =
        materialize_runtime_locale_context(&translation_materialization.context);
    issues.extend(
        locale_context_materialization
            .diagnostics
            .iter()
            .cloned()
            .map(classified_issue),
    );
    let structural_context = locale_context_materialization.context;

    let metadata_materialization =
        materialize_localized_page_metadata(document, &structural_context);
    issues.extend(
        metadata_materialization
            .diagnostics
            .iter()
            .cloned()
            .map(classified_issue),
    );

    let contract = extract_runtime_context_contract(&metadata_materialization.document);
    let effective_context = if contract.is_valid() {
        let context_materialization =
            materialize_context(&metadata_materialization.document, &structural_context);
        issues.extend(
            context_materialization
                .diagnostics
                .iter()
                .filter(|diagnostic| include_structural_runtime_diagnostic(diagnostic))
                .cloned()
                .map(classified_issue),
        );
        context_materialization.context
    } else {
        structural_context
    };

    let binding_materialization =
        materialize_bindings(&metadata_materialization.document, &effective_context);
    issues.extend(
        binding_materialization
            .diagnostics
            .iter()
            .filter(|diagnostic| include_structural_runtime_diagnostic(diagnostic))
            .cloned()
            .map(classified_issue),
    );

    issues.extend(
        validate_internal_page_links(&binding_materialization.document)
            .into_iter()
            .map(classified_issue),
    );
    issues.extend(
        validate_component_actions(&binding_materialization.document)
            .into_iter()
            .map(classified_issue),
    );

    let link_materialization =
        materialize_internal_page_links(&binding_materialization.document, &effective_context);
    issues.extend(
        link_materialization
            .diagnostics
            .iter()
            .cloned()
            .map(classified_issue),
    );

    let action_materialization =
        materialize_component_actions(&link_materialization.document, &effective_context);
    issues.extend(
        action_materialization
            .diagnostics
            .iter()
            .cloned()
            .map(classified_issue),
    );
    action_materialization.document
}

fn include_structural_runtime_diagnostic(diagnostic: &ValidationDiagnostic) -> bool {
    !matches!(
        diagnostic.code.as_str(),
        "runtime_context_required_missing"
            | "runtime_binding_unresolved"
            | "runtime_computed_unresolved"
            | "runtime_computed_evaluation_failed"
    )
}

fn classified_issue(mut diagnostic: ValidationDiagnostic) -> LandingReadinessIssue {
    if publish_materialization_failure(&diagnostic.code) {
        diagnostic.severity = ValidationSeverity::Error;
    }
    LandingReadinessIssue {
        category: classify_validation_diagnostic(&diagnostic),
        diagnostic,
    }
}

fn publish_materialization_failure(code: &str) -> bool {
    matches!(
        code,
        "runtime_action_invalid"
            | "runtime_action_unresolved"
            | "runtime_form_invalid"
            | "internal_page_link_slug_unresolved"
            | "internal_page_link_target_missing"
            | "internal_page_link_invalid"
            | "runtime_binding_transform_failed"
            | "runtime_binding_target_missing"
            | "runtime_condition_target_missing"
            | "runtime_repeater_failed"
    )
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
                values
                    .values()
                    .any(|value| value.as_str().is_some_and(|value| !value.trim().is_empty()))
            }),
        _ => false,
    }
}

fn classify_validation_diagnostic(diagnostic: &ValidationDiagnostic) -> LandingReadinessCategory {
    let code = diagnostic.code.as_str();
    if code.contains("slug")
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
    } else if code.contains("locale")
        || code.starts_with("translation_")
        || code.starts_with("localized_metadata_")
    {
        LandingReadinessCategory::Locales
    } else if code.starts_with("runtime_")
        || code.starts_with("action_")
        || code.starts_with("form_")
        || code.starts_with("duplicate_form_")
        || code.starts_with("component_form_")
        || code.starts_with("binding_")
        || code.starts_with("dynamic_")
    {
        LandingReadinessCategory::RuntimeContracts
    } else {
        LandingReadinessCategory::Content
    }
}

fn category_summaries(issues: &[LandingReadinessIssue]) -> Vec<LandingReadinessCategorySummary> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_form_conflicts_are_runtime_contracts() {
        let diagnostic = ValidationDiagnostic {
            severity: ValidationSeverity::Error,
            code: "component_form_interaction_contract_conflict".to_string(),
            path: "component:form".to_string(),
            message: "conflict".to_string(),
        };
        assert!(matches!(
            classify_validation_diagnostic(&diagnostic),
            LandingReadinessCategory::RuntimeContracts
        ));
    }
}

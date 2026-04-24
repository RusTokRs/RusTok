use std::collections::{BTreeSet, HashMap};

use rustok_api::TenantContext;
use rustok_seo_targets::{SeoTargetBulkListRequest, SeoTargetCapabilityKind};

use crate::dto::{
    SeoDiagnosticIssueRecord, SeoDiagnosticSeverity, SeoDiagnosticsSummaryRecord, SeoFieldSource,
};
use crate::{SeoError, SeoResult};

use super::SeoService;

const MAX_EXPOSED_ISSUES: usize = 50;

impl SeoService {
    pub async fn diagnostics_summary(
        &self,
        tenant: &TenantContext,
        locale: Option<&str>,
    ) -> SeoResult<SeoDiagnosticsSummaryRecord> {
        let locale = super::normalize_effective_locale(
            locale.unwrap_or(tenant.default_locale.as_str()),
            tenant.default_locale.as_str(),
        )?;
        let mut issues = Vec::new();
        let mut canonical_usage: HashMap<
            String,
            Vec<(rustok_seo_targets::SeoTargetSlug, uuid::Uuid, String)>,
        > = HashMap::new();
        let mut sitemap_targets = BTreeSet::new();
        let mut total_targets = 0_i32;
        let mut explicit_count = 0_i32;
        let mut generated_count = 0_i32;
        let mut fallback_count = 0_i32;

        for provider in self
            .registry
            .providers_with_capability(SeoTargetCapabilityKind::Sitemaps)
        {
            let candidates = provider
                .sitemap_candidates(
                    &self.target_runtime(),
                    rustok_seo_targets::SeoTargetSitemapRequest {
                        tenant_id: tenant.id,
                        default_locale: tenant.default_locale.as_str(),
                    },
                )
                .await
                .map_err(|error| {
                    SeoError::validation(format!(
                        "SEO target provider `{}` failed to list sitemap candidates: {error}",
                        provider.slug().as_str()
                    ))
                })?;
            for candidate in candidates {
                sitemap_targets.insert((candidate.target_kind, candidate.target_id));
            }
        }

        for provider in self
            .registry
            .providers_with_capability(SeoTargetCapabilityKind::Bulk)
        {
            let summaries = provider
                .list_bulk_summaries(
                    &self.target_runtime(),
                    SeoTargetBulkListRequest {
                        tenant_id: tenant.id,
                        default_locale: tenant.default_locale.as_str(),
                        locale: locale.as_str(),
                    },
                )
                .await
                .map_err(|error| {
                    SeoError::validation(format!(
                        "SEO target provider `{}` failed to list bulk summaries: {error}",
                        provider.slug().as_str()
                    ))
                })?;

            for summary in summaries {
                total_targets += 1;
                let Some(meta) = self
                    .seo_meta(
                        tenant,
                        summary.target_kind.clone(),
                        summary.target_id,
                        Some(locale.as_str()),
                    )
                    .await?
                else {
                    continue;
                };

                match meta.effective_state.title.source {
                    SeoFieldSource::Explicit => explicit_count += 1,
                    SeoFieldSource::Generated => generated_count += 1,
                    SeoFieldSource::Fallback => fallback_count += 1,
                }

                if let Some(canonical_url) = meta.canonical_url.clone() {
                    canonical_usage.entry(canonical_url).or_default().push((
                        summary.target_kind.clone(),
                        summary.target_id,
                        meta.source.clone(),
                    ));
                }

                if meta
                    .translation
                    .title
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(|value| value.is_empty())
                {
                    issues.push(issue(
                        "missing_title",
                        SeoDiagnosticSeverity::Error,
                        &summary,
                        "Effective SEO title is missing.",
                        meta.canonical_url.clone(),
                        meta.source.clone(),
                        locale.as_str(),
                    ));
                }

                if meta
                    .translation
                    .description
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(|value| value.is_empty())
                {
                    issues.push(issue(
                        "missing_description",
                        SeoDiagnosticSeverity::Warning,
                        &summary,
                        "Effective SEO description is missing.",
                        meta.canonical_url.clone(),
                        meta.source.clone(),
                        locale.as_str(),
                    ));
                }

                if matches!(meta.effective_state.title.source, SeoFieldSource::Fallback) {
                    issues.push(issue(
                        "fallback_only",
                        SeoDiagnosticSeverity::Info,
                        &summary,
                        "Target still resolves through entity fallback instead of explicit or template SEO.",
                        meta.canonical_url.clone(),
                        meta.source.clone(),
                        locale.as_str(),
                    ));
                }

                if meta.structured_data.is_none() {
                    issues.push(issue(
                        "missing_schema",
                        SeoDiagnosticSeverity::Warning,
                        &summary,
                        "Structured data is missing for the effective SEO document.",
                        meta.canonical_url.clone(),
                        meta.source.clone(),
                        locale.as_str(),
                    ));
                }

                if meta.noindex && meta.canonical_url.is_some() {
                    issues.push(issue(
                        "noindex_canonical_conflict",
                        SeoDiagnosticSeverity::Warning,
                        &summary,
                        "Target combines an explicit canonical URL with noindex.",
                        meta.canonical_url.clone(),
                        meta.source.clone(),
                        locale.as_str(),
                    ));
                }

                if provider.capabilities().sitemaps
                    && !sitemap_targets.contains(&(summary.target_kind.clone(), summary.target_id))
                {
                    issues.push(issue(
                        "missing_sitemap_entry",
                        SeoDiagnosticSeverity::Warning,
                        &summary,
                        "Target is missing from the sitemap candidate set.",
                        meta.canonical_url.clone(),
                        meta.source.clone(),
                        locale.as_str(),
                    ));
                }
            }
        }

        for (canonical_url, entries) in canonical_usage {
            if entries.len() < 2 {
                continue;
            }
            for (target_kind, target_id, source) in entries {
                issues.push(SeoDiagnosticIssueRecord {
                    code: "duplicate_canonical".to_string(),
                    severity: SeoDiagnosticSeverity::Error,
                    target_kind,
                    target_id,
                    locale: locale.clone(),
                    message: format!(
                        "Canonical URL `{canonical_url}` is used by multiple targets."
                    ),
                    canonical_url: Some(canonical_url.clone()),
                    source,
                });
            }
        }

        let error_count = issues
            .iter()
            .filter(|issue| issue.severity == SeoDiagnosticSeverity::Error)
            .count() as i32;
        let warning_count = issues
            .iter()
            .filter(|issue| issue.severity == SeoDiagnosticSeverity::Warning)
            .count() as i32;
        let total_targets = total_targets.max(0);
        let readiness_score = if total_targets == 0 {
            100
        } else {
            let info_count = issues.len() as i32 - error_count - warning_count;
            let weighted_issues = (error_count * 3) + (warning_count * 2) + info_count;
            let max_weight = (total_targets * 6).max(1);
            let penalty = ((weighted_issues * 100) / max_weight).min(100);
            100 - penalty
        };

        Ok(SeoDiagnosticsSummaryRecord {
            locale,
            total_targets,
            readiness_score,
            issue_count: issues.len() as i32,
            error_count,
            warning_count,
            generated_count,
            explicit_count,
            fallback_count,
            issues: issues.into_iter().take(MAX_EXPOSED_ISSUES).collect(),
        })
    }
}

fn issue(
    code: &str,
    severity: SeoDiagnosticSeverity,
    summary: &rustok_seo_targets::SeoBulkSummaryRecord,
    message: &str,
    canonical_url: Option<String>,
    source: String,
    locale: &str,
) -> SeoDiagnosticIssueRecord {
    SeoDiagnosticIssueRecord {
        code: code.to_string(),
        severity,
        target_kind: summary.target_kind.clone(),
        target_id: summary.target_id,
        locale: locale.to_string(),
        message: message.to_string(),
        canonical_url,
        source,
    }
}

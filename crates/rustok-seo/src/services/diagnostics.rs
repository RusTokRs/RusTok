use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rustok_api::TenantContext;
use rustok_seo_targets::{SeoTargetBulkListRequest, SeoTargetCapabilityKind};
use url::Url;

use crate::dto::{
    SeoDiagnosticCountRecord, SeoDiagnosticIssueRecord, SeoDiagnosticSeverity,
    SeoDiagnosticsSummaryRecord, SeoFieldSource, SeoRedirectMatchType,
};
use crate::{SeoError, SeoResult};

use super::robots::schema_blocks_from_value;
use super::SeoService;

const MAX_EXPOSED_ISSUES: usize = 50;
type CanonicalUsageEntry = (
    rustok_seo_targets::SeoTargetSlug,
    uuid::Uuid,
    String,
    String,
    String,
);
type CanonicalUsageMap = HashMap<String, Vec<CanonicalUsageEntry>>;

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
        let mut canonical_usage: CanonicalUsageMap = HashMap::new();
        let mut sitemap_targets = BTreeSet::new();
        let redirect_graph = self.redirect_graph(tenant.id).await?;
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

                let page_context = self
                    .resolve_page_context(tenant, locale.as_str(), summary.route.as_str())
                    .await?;
                let effective_canonical = page_context
                    .as_ref()
                    .map(|context| context.route.canonical_url.clone())
                    .or_else(|| meta.canonical_url.clone());

                match meta.effective_state.title.source {
                    SeoFieldSource::Explicit => explicit_count += 1,
                    SeoFieldSource::Generated => generated_count += 1,
                    SeoFieldSource::Fallback => fallback_count += 1,
                }

                if let Some(canonical_url) = effective_canonical.clone() {
                    canonical_usage.entry(canonical_url).or_default().push((
                        summary.target_kind.clone(),
                        summary.target_id,
                        meta.source.clone(),
                        summary.label.clone(),
                        summary.route.clone(),
                    ));
                }

                if let Some(canonical_url) = effective_canonical.as_deref() {
                    if let Some(route) = redirect_lookup_route(canonical_url) {
                        if let Some(trace) = trace_redirects(route.as_str(), &redirect_graph) {
                            issues.push(issue(
                                trace.code(),
                                trace.severity(),
                                &summary,
                                trace.message().as_str(),
                                effective_canonical.clone(),
                                meta.source.clone(),
                                locale.as_str(),
                            ));
                        }
                    }
                }

                if let Some(context) = page_context.as_ref() {
                    let alternate_locales = context
                        .route
                        .alternates
                        .iter()
                        .map(|alternate| alternate.locale.as_str())
                        .collect::<HashSet<_>>();
                    let expected_locales = meta
                        .available_locales
                        .iter()
                        .map(String::as_str)
                        .filter(|value| !value.trim().is_empty())
                        .collect::<BTreeSet<_>>();
                    if expected_locales.len() > 1 {
                        let missing = expected_locales
                            .iter()
                            .filter(|locale| !alternate_locales.contains(**locale))
                            .copied()
                            .collect::<Vec<_>>();
                        if !missing.is_empty() {
                            issues.push(issue(
                                "missing_hreflang_pair",
                                SeoDiagnosticSeverity::Warning,
                                &summary,
                                format!("Missing hreflang alternates for {}.", missing.join(", "))
                                    .as_str(),
                                effective_canonical.clone(),
                                meta.source.clone(),
                                locale.as_str(),
                            ));
                        }
                        if !alternate_locales.contains("x-default") {
                            issues.push(issue(
                                "missing_hreflang_x_default",
                                SeoDiagnosticSeverity::Info,
                                &summary,
                                "Localized target alternates do not expose x-default.",
                                effective_canonical.clone(),
                                meta.source.clone(),
                                locale.as_str(),
                            ));
                        }
                    }
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
                        effective_canonical.clone(),
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
                        effective_canonical.clone(),
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
                        effective_canonical.clone(),
                        meta.source.clone(),
                        locale.as_str(),
                    ));
                }

                let schema_blocks = meta
                    .structured_data
                    .as_ref()
                    .map(|value| {
                        schema_blocks_from_value(
                            value.0.clone(),
                            meta.effective_state.structured_data.source,
                        )
                    })
                    .unwrap_or_default();
                if schema_blocks.is_empty() {
                    issues.push(issue(
                        "missing_schema",
                        SeoDiagnosticSeverity::Warning,
                        &summary,
                        "Typed structured data blocks are missing for the effective SEO document.",
                        effective_canonical.clone(),
                        meta.source.clone(),
                        locale.as_str(),
                    ));
                } else if schema_blocks
                    .iter()
                    .any(|block| block.schema_kind == crate::dto::SeoSchemaBlockKind::Unknown)
                {
                    issues.push(issue(
                        "unknown_schema_type",
                        SeoDiagnosticSeverity::Info,
                        &summary,
                        "Structured data is present but at least one block has no recognized schema.org type.",
                        effective_canonical.clone(),
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
                        effective_canonical.clone(),
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
                        effective_canonical.clone(),
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
            for (target_kind, target_id, source, target_label, route) in entries {
                issues.push(SeoDiagnosticIssueRecord {
                    code: "duplicate_canonical".to_string(),
                    severity: SeoDiagnosticSeverity::Error,
                    target_kind,
                    target_id,
                    target_label,
                    route,
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
        let issue_counts_by_code = count_by_key(issues.iter().map(|issue| issue.code.as_str()));
        let issue_counts_by_target_kind =
            count_by_key(issues.iter().map(|issue| issue.target_kind.as_str()));
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
            issue_counts_by_code,
            issue_counts_by_target_kind,
            issues: issues.into_iter().take(MAX_EXPOSED_ISSUES).collect(),
        })
    }

    async fn redirect_graph(&self, tenant_id: uuid::Uuid) -> SeoResult<HashMap<String, String>> {
        let redirects = self.list_redirects(tenant_id).await?;
        Ok(redirects
            .into_iter()
            .filter(|redirect| {
                redirect.is_active && redirect.match_type == SeoRedirectMatchType::Exact
            })
            .filter_map(|redirect| {
                let source = redirect_lookup_route(redirect.source_pattern.as_str())?;
                let target = redirect_lookup_route(redirect.target_url.as_str())?;
                Some((source, target))
            })
            .collect())
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
        target_label: summary.label.clone(),
        route: summary.route.clone(),
        locale: locale.to_string(),
        message: message.to_string(),
        canonical_url,
        source,
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum RedirectTrace {
    Single { target: String },
    Chain { final_target: String, hops: usize },
    Loop { at: String },
}

impl RedirectTrace {
    fn code(&self) -> &'static str {
        match self {
            Self::Single { .. } => "canonical_redirect_target",
            Self::Chain { .. } => "canonical_redirect_chain",
            Self::Loop { .. } => "canonical_redirect_loop",
        }
    }

    fn severity(&self) -> SeoDiagnosticSeverity {
        match self {
            Self::Loop { .. } => SeoDiagnosticSeverity::Error,
            Self::Single { .. } | Self::Chain { .. } => SeoDiagnosticSeverity::Warning,
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Single { target } => {
                format!("Effective canonical URL redirects to `{target}`.")
            }
            Self::Chain { final_target, hops } => {
                format!(
                    "Effective canonical URL redirects through {hops} hops to `{final_target}`."
                )
            }
            Self::Loop { at } => {
                format!("Effective canonical URL participates in a redirect loop at `{at}`.")
            }
        }
    }
}

fn redirect_lookup_route(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('/') {
        return Some(value.to_string());
    }
    let parsed = Url::parse(value).ok()?;
    let mut route = parsed.path().to_string();
    if let Some(query) = parsed.query() {
        route.push('?');
        route.push_str(query);
    }
    Some(route)
}

fn trace_redirects(route: &str, graph: &HashMap<String, String>) -> Option<RedirectTrace> {
    let first = graph.get(route)?;
    let mut visited = HashSet::from([route.to_string()]);
    let mut current = first.clone();
    let mut hops = 1_usize;

    loop {
        if !visited.insert(current.clone()) {
            return Some(RedirectTrace::Loop { at: current });
        }
        let Some(next) = graph.get(current.as_str()) else {
            return if hops == 1 {
                Some(RedirectTrace::Single { target: current })
            } else {
                Some(RedirectTrace::Chain {
                    final_target: current,
                    hops,
                })
            };
        };
        current = next.clone();
        hops += 1;
    }
}

fn count_by_key<'a>(keys: impl Iterator<Item = &'a str>) -> Vec<SeoDiagnosticCountRecord> {
    let mut counts = BTreeMap::<String, i32>::new();
    for key in keys {
        *counts.entry(key.to_string()).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(key, count)| SeoDiagnosticCountRecord { key, count })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{redirect_lookup_route, trace_redirects, RedirectTrace};
    use std::collections::HashMap;

    #[test]
    fn redirect_lookup_route_keeps_relative_path_and_query() {
        assert_eq!(
            redirect_lookup_route("https://example.test/en/page?x=1").as_deref(),
            Some("/en/page?x=1")
        );
        assert_eq!(
            redirect_lookup_route("/en/page").as_deref(),
            Some("/en/page")
        );
    }

    #[test]
    fn trace_redirects_classifies_single_chain_and_loop() {
        let single = HashMap::from([("/a".to_string(), "/b".to_string())]);
        assert_eq!(
            trace_redirects("/a", &single),
            Some(RedirectTrace::Single {
                target: "/b".to_string()
            })
        );

        let chain = HashMap::from([
            ("/a".to_string(), "/b".to_string()),
            ("/b".to_string(), "/c".to_string()),
        ]);
        assert_eq!(
            trace_redirects("/a", &chain),
            Some(RedirectTrace::Chain {
                final_target: "/c".to_string(),
                hops: 2,
            })
        );

        let looped = HashMap::from([
            ("/a".to_string(), "/b".to_string()),
            ("/b".to_string(), "/a".to_string()),
        ]);
        assert_eq!(
            trace_redirects("/a", &looped),
            Some(RedirectTrace::Loop {
                at: "/a".to_string(),
            })
        );
    }
}

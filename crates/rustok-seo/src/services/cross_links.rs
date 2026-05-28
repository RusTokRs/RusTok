use std::collections::{BTreeMap, BTreeSet};

use rustok_api::TenantContext;
use rustok_seo_targets::{SeoBulkSummaryRecord, SeoTargetBulkListRequest, SeoTargetCapabilityKind};

use crate::dto::SeoCrossLinkSuggestionRecord;
use crate::{SeoError, SeoResult};

use super::{normalize_effective_locale, SeoService};

const DEFAULT_SUGGESTIONS_PER_TARGET: usize = 3;
const MAX_SUGGESTIONS_PER_TARGET: usize = 10;
const CROSS_LINK_SUGGESTION_SOURCE: &str = "bulk_overlap_v1";

impl SeoService {
    pub async fn cross_link_suggestions(
        &self,
        tenant: &TenantContext,
        locale: Option<&str>,
        per_target_limit: Option<usize>,
    ) -> SeoResult<Vec<SeoCrossLinkSuggestionRecord>> {
        let locale = normalize_effective_locale(
            locale.unwrap_or(tenant.default_locale.as_str()),
            tenant.default_locale.as_str(),
        )?;
        let summaries = self.bulk_summaries_for_locale(tenant, locale.as_str()).await?;
        Ok(build_cross_link_suggestions(
            summaries.as_slice(),
            normalize_per_target_limit(per_target_limit),
        ))
    }

    pub(super) async fn cross_link_suggestions_by_target(
        &self,
        tenant: &TenantContext,
        locale: &str,
        per_target_limit: Option<usize>,
    ) -> SeoResult<BTreeMap<(rustok_seo_targets::SeoTargetSlug, uuid::Uuid), usize>> {
        let summaries = self.bulk_summaries_for_locale(tenant, locale).await?;
        Ok(build_cross_link_suggestions(summaries.as_slice(), normalize_per_target_limit(per_target_limit))
            .into_iter()
            .fold(BTreeMap::new(), |mut acc, suggestion| {
                *acc.entry((suggestion.target_kind, suggestion.target_id))
                    .or_default() += 1;
                acc
            }))
    }

    pub(super) async fn bulk_summaries_for_locale(
        &self,
        tenant: &TenantContext,
        locale: &str,
    ) -> SeoResult<Vec<SeoBulkSummaryRecord>> {
        let mut summaries = Vec::new();
        for provider in self
            .registry
            .providers_with_capability(SeoTargetCapabilityKind::Bulk)
        {
            let provider_summaries = provider
                .list_bulk_summaries(
                    &self.target_runtime(),
                    SeoTargetBulkListRequest {
                        tenant_id: tenant.id,
                        default_locale: tenant.default_locale.as_str(),
                        locale,
                    },
                )
                .await
                .map_err(|error| {
                    SeoError::validation(format!(
                        "SEO target provider `{}` failed to list bulk summaries: {error}",
                        provider.slug().as_str()
                    ))
                })?;
            summaries.extend(provider_summaries);
        }

        summaries.sort_by(|left, right| {
            left.target_kind
                .as_str()
                .cmp(right.target_kind.as_str())
                .then(left.route.cmp(&right.route))
                .then(left.label.cmp(&right.label))
                .then(left.target_id.cmp(&right.target_id))
        });
        summaries.dedup_by(|left, right| {
            left.target_kind == right.target_kind
                && left.target_id == right.target_id
                && left.route == right.route
                && left.label == right.label
        });

        Ok(summaries)
    }
}

fn normalize_per_target_limit(value: Option<usize>) -> usize {
    value
        .unwrap_or(DEFAULT_SUGGESTIONS_PER_TARGET)
        .clamp(1, MAX_SUGGESTIONS_PER_TARGET)
}

fn build_cross_link_suggestions(
    summaries: &[SeoBulkSummaryRecord],
    per_target_limit: usize,
) -> Vec<SeoCrossLinkSuggestionRecord> {
    if summaries.is_empty() {
        return Vec::new();
    }

    let tokenized = summaries
        .iter()
        .map(tokenize_summary)
        .collect::<Vec<BTreeSet<String>>>();

    let mut suggestions = Vec::new();
    for (source_index, source_summary) in summaries.iter().enumerate() {
        let mut candidates = Vec::new();
        for (destination_index, destination_summary) in summaries.iter().enumerate() {
            if source_index == destination_index {
                continue;
            }
            if source_summary.route == destination_summary.route {
                continue;
            }
            let overlap = tokenized[source_index]
                .intersection(&tokenized[destination_index])
                .count() as i32;
            if overlap == 0 {
                continue;
            }
            let mut confidence = 35 + (overlap * 15);
            if source_summary.target_kind == destination_summary.target_kind {
                confidence += 10;
            }
            confidence = confidence.clamp(0, 95);
            candidates.push((destination_summary, confidence));
        }

        candidates.sort_by(|(left_summary, left_confidence), (right_summary, right_confidence)| {
            right_confidence
                .cmp(left_confidence)
                .then(left_summary.route.cmp(&right_summary.route))
                .then(left_summary.label.cmp(&right_summary.label))
                .then(left_summary.target_kind.as_str().cmp(right_summary.target_kind.as_str()))
                .then(left_summary.target_id.cmp(&right_summary.target_id))
        });

        for (destination_summary, confidence) in candidates.into_iter().take(per_target_limit) {
            suggestions.push(SeoCrossLinkSuggestionRecord {
                target_kind: source_summary.target_kind.clone(),
                target_id: source_summary.target_id,
                target_route: source_summary.route.clone(),
                anchor_hint: normalized_anchor_hint(destination_summary),
                destination_route: destination_summary.route.clone(),
                confidence,
                source: CROSS_LINK_SUGGESTION_SOURCE.to_string(),
            });
        }
    }

    suggestions.sort_by(|left, right| {
        left.target_kind
            .as_str()
            .cmp(right.target_kind.as_str())
            .then(left.target_route.cmp(&right.target_route))
            .then(left.destination_route.cmp(&right.destination_route))
            .then(right.confidence.cmp(&left.confidence))
            .then(left.target_id.cmp(&right.target_id))
    });
    suggestions
}

fn tokenize_summary(summary: &SeoBulkSummaryRecord) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    tokens.extend(tokenize_value(summary.label.as_str()));
    tokens.extend(tokenize_value(summary.route.as_str()));
    tokens
}

fn tokenize_value(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::trim)
        .filter(|token| token.len() >= 3)
        .map(|token| token.to_ascii_lowercase())
}

fn normalized_anchor_hint(summary: &SeoBulkSummaryRecord) -> String {
    let label = summary.label.trim();
    if !label.is_empty() {
        return label.to_string();
    }
    summary.route.clone()
}

#[cfg(test)]
mod tests {
    use rustok_seo_targets::SeoTargetSlug;

    use super::build_cross_link_suggestions;

    fn summary(kind: &str, route: &str, label: &str, id_suffix: u128) -> rustok_seo_targets::SeoBulkSummaryRecord {
        rustok_seo_targets::SeoBulkSummaryRecord {
            target_kind: SeoTargetSlug::new(kind).expect("valid target slug"),
            target_id: uuid::Uuid::from_u128(id_suffix),
            effective_locale: "en".to_string(),
            label: label.to_string(),
            route: route.to_string(),
        }
    }

    #[test]
    fn cross_link_suggestions_are_deterministic_and_scored() {
        let suggestions = build_cross_link_suggestions(
            &[
                summary("page", "/guides/rust-seo", "Rust SEO Guide", 1),
                summary("blog_post", "/blog/rust-seo-checklist", "Rust SEO Checklist", 2),
                summary("product", "/products/coffee", "Coffee Beans", 3),
            ],
            2,
        );

        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].target_route, "/blog/rust-seo-checklist");
        assert_eq!(suggestions[0].destination_route, "/guides/rust-seo");
        assert_eq!(suggestions[0].anchor_hint, "Rust SEO Guide");
        assert!(suggestions[0].confidence >= 65);

        assert_eq!(suggestions[1].target_route, "/guides/rust-seo");
        assert_eq!(suggestions[1].destination_route, "/blog/rust-seo-checklist");
        assert_eq!(suggestions[1].anchor_hint, "Rust SEO Checklist");
        assert!(suggestions[1].confidence >= 65);
    }

    #[test]
    fn cross_link_suggestions_skip_unrelated_targets() {
        let suggestions = build_cross_link_suggestions(
            &[
                summary("page", "/guides/rust-seo", "Rust SEO Guide", 1),
                summary("product", "/products/coffee", "Coffee Beans", 2),
            ],
            3,
        );

        assert!(suggestions.is_empty());
    }
}

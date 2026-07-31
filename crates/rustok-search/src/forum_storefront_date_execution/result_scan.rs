async fn execute_date_window_result_eligible_search(
    db: &DatabaseConnection,
    query: &SearchQuery,
    eligibility_port: Option<SharedStorefrontSearchResultEligibilityPort>,
    effective_locale: String,
    auth: Option<AuthContext>,
    request_context: Option<RequestContext>,
    trusted_channel: &TrustedStorefrontChannel,
    document_filters: &ForumStorefrontDocumentFilters,
    locale_date_filters: &ForumStorefrontLocaleDateFilters,
    transport: StorefrontSearchTransport,
) -> Result<SearchResult, ForumStorefrontSearchExecutionError> {
    let started_at = Instant::now();
    let engine = PgSearchEngine::new(db.clone());
    let mut scan_query = query.clone();
    scan_query.limit = FORUM_RESULT_SCAN_PAGE_SIZE;
    scan_query.offset = 0;

    let first_page = engine
        .search_storefront(scan_query.clone(), trusted_channel)
        .await?;
    let raw_total = usize::try_from(first_page.total).map_err(|_| {
        ForumStorefrontSearchExecutionError::Validation(
            "Forum storefront Search candidate count is too large".to_string(),
        )
    })?;
    if raw_total > MAX_FORUM_SEARCH_RESULT_CANDIDATES {
        return validation(format!(
            "Forum storefront Search matched more than {MAX_FORUM_SEARCH_RESULT_CANDIDATES} candidates; narrow the query or category scope"
        ));
    }

    let engine_kind = first_page.engine;
    let ranking_profile = first_page.ranking_profile;
    let mut all_items = first_page.items;
    let mut raw_rows = HashSet::with_capacity(raw_total);
    register_date_window_raw_rows(&mut raw_rows, &all_items)?;
    while all_items.len() < raw_total {
        scan_query.offset = all_items.len();
        let page = engine
            .search_storefront(scan_query.clone(), trusted_channel)
            .await?;
        if page.total != raw_total as u64 || page.items.is_empty() {
            return Err(ForumStorefrontSearchExecutionError::Invariant(
                "Forum storefront Search candidate snapshot changed during bounded eligibility evaluation",
            ));
        }
        register_date_window_raw_rows(&mut raw_rows, &page.items)?;
        all_items.extend(page.items);
        if all_items.len() > raw_total {
            return Err(ForumStorefrontSearchExecutionError::Invariant(
                "Forum storefront Search candidate scan exceeded its initial bounded total",
            ));
        }
    }
    if all_items.len() != raw_total || raw_rows.len() != raw_total {
        return Err(ForumStorefrontSearchExecutionError::Invariant(
            "Forum storefront Search candidate scan did not resolve one unique row per result",
        ));
    }

    all_items.retain(|item| locale_date_filters.matches(item) && document_filters.matches(item));

    let mut seen_candidates = HashSet::new();
    let candidates = all_items
        .iter()
        .filter_map(date_window_result_candidate)
        .filter(|candidate| seen_candidates.insert(*candidate))
        .collect::<Vec<_>>();
    let allowed = resolve_storefront_search_result_candidates(
        eligibility_port,
        StorefrontSearchResultEligibilityRequest {
            tenant_id: query.tenant_id.expect("validated Forum Search tenant"),
            locale: effective_locale,
            candidates,
            auth,
            request_context,
            transport,
        },
    )
    .await?;
    let allowed = allowed.into_iter().collect::<HashSet<_>>();

    let visible_items = all_items
        .into_iter()
        .filter(|item| match item.entity_type.as_str() {
            "forum_category" => true,
            "forum_topic" | "forum_reply" => date_window_result_candidate(item)
                .is_some_and(|candidate| allowed.contains(&candidate)),
            _ => false,
        })
        .collect::<Vec<_>>();
    let total = visible_items.len() as u64;
    let facets = build_date_window_forum_result_facets(&visible_items);
    let items = visible_items
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect();

    Ok(SearchResult {
        items,
        total,
        took_ms: started_at.elapsed().as_millis() as u64,
        engine: engine_kind,
        ranking_profile,
        facets,
    })
}

fn register_date_window_raw_rows(
    seen: &mut HashSet<(String, String, Uuid, Option<String>)>,
    items: &[SearchResultItem],
) -> Result<(), ForumStorefrontSearchExecutionError> {
    for item in items {
        if !seen.insert((
            item.source_module.clone(),
            item.entity_type.clone(),
            item.id,
            item.locale.clone(),
        )) {
            return Err(ForumStorefrontSearchExecutionError::Invariant(
                "Forum storefront Search candidate scan returned a duplicate raw row",
            ));
        }
    }
    Ok(())
}

fn date_window_result_candidate(
    item: &SearchResultItem,
) -> Option<StorefrontSearchResultCandidate> {
    StorefrontSearchResultCandidateKind::from_entity_type(&item.entity_type).map(|kind| {
        StorefrontSearchResultCandidate {
            document_id: item.id,
            kind,
        }
    })
}

fn build_date_window_forum_result_facets(items: &[SearchResultItem]) -> Vec<SearchFacetGroup> {
    let mut entity_types = BTreeMap::<String, u64>::new();
    let mut source_modules = BTreeMap::<String, u64>::new();
    let mut statuses = BTreeMap::<String, u64>::new();
    for item in items {
        *entity_types.entry(item.entity_type.clone()).or_default() += 1;
        *source_modules
            .entry(item.source_module.clone())
            .or_default() += 1;
        if let Some(status) = date_window_forum_visible_status(&item.entity_type) {
            *statuses.entry(status.to_string()).or_default() += 1;
        }
    }
    vec![
        SearchFacetGroup {
            name: "entity_type".to_string(),
            buckets: date_window_facet_buckets(entity_types),
        },
        SearchFacetGroup {
            name: "source_module".to_string(),
            buckets: date_window_facet_buckets(source_modules),
        },
        SearchFacetGroup {
            name: "status".to_string(),
            buckets: date_window_facet_buckets(statuses),
        },
    ]
}

fn date_window_facet_buckets(values: BTreeMap<String, u64>) -> Vec<SearchFacetBucket> {
    let mut buckets = values
        .into_iter()
        .map(|(value, count)| SearchFacetBucket {
            value,
            label: None,
            count,
        })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.value.cmp(&right.value))
    });
    buckets
}

fn date_window_forum_visible_status(entity_type: &str) -> Option<&'static str> {
    match entity_type {
        "forum_category" => Some("public"),
        "forum_topic" => Some("open"),
        "forum_reply" => Some("approved"),
        _ => None,
    }
}

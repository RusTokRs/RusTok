/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use rustok_api::{AuthContext, PortError, PortErrorKind, RequestContext};
use rustok_core::{Error, Result as CoreResult};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dictionaries::SearchDictionaryService;
use crate::engine::{
    SearchEngineKind, SearchFacetBucket, SearchFacetGroup, SearchQuery, SearchResult,
    SearchResultItem,
};
use crate::pg_engine::{PgSearchEngine, SearchAudienceDocumentScope};
use crate::storefront_category_scope::{
    SharedStorefrontSearchCategoryScopePort, StorefrontSearchTransport,
};
use crate::storefront_channel_authority::TrustedStorefrontChannel;

/// Maximum number of candidate documents a single audience policy call receives.
///
/// The bound is per batch, never per request: the executor splits larger candidate
/// sets into bounded batches, so a broad query is paginated instead of rejected.
pub const MAX_SEARCH_AUDIENCE_CANDIDATES: usize = 100;
/// Page size used while collecting audience-restricted candidates from PostgreSQL.
pub const AUDIENCE_CANDIDATE_SCAN_PAGE_SIZE: usize = 50;
/// Maximum number of audience-restricted (gated) matching documents one storefront
/// request may scan and post-authorize.
///
/// Every scan page is an extra owner round-trip, so the bound keeps a single public
/// request from fanning out without limit. Exceeding it is a caller-visible
/// `Validation` error asking for a narrower query; public modules are never subject
/// to this bound because PostgreSQL authorizes and counts them directly.
pub const MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS: usize = 1_000;

/// Neutral candidate descriptor representing one document returned by search engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SearchCandidateDescriptor {
    pub document_id: Uuid,
    pub source_module: String,
    pub entity_type: String,
}

impl SearchCandidateDescriptor {
    pub fn new(
        document_id: Uuid,
        source_module: impl Into<String>,
        entity_type: impl Into<String>,
    ) -> Self {
        Self {
            document_id,
            source_module: source_module.into(),
            entity_type: entity_type.into(),
        }
    }
}

/// Request passed to a domain owner to evaluate real-time candidate eligibility.
#[derive(Clone)]
pub struct SearchAudienceRequest {
    pub tenant_id: Uuid,
    pub locale: String,
    pub candidates: Vec<SearchCandidateDescriptor>,
    pub auth: Option<AuthContext>,
    pub request_context: Option<RequestContext>,
    pub transport: StorefrontSearchTransport,
}

/// Declared audience class of one source module inside the Search corpus.
///
/// Every source module that writes `search_documents` must be declared exactly once.
/// Undeclared modules are never publicly discoverable: Search denies them instead of
/// assuming that unregistered content is public.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchAudienceClass {
    /// Documents are public discovery material. PostgreSQL visibility predicates
    /// (`published_only`, status, channel, locale) remain the only gate.
    Public,
    /// Documents require a module-owned real-time policy before they are exposed.
    Gated,
}

/// Generic Service Provider Interface (SPI) for module-owned search audience security.
///
/// Any domain module with audience, private categories, or dynamic visibility
/// implements this port without requiring changes to `rustok-search`.
///
/// Error contract (fail-closed):
/// - `Ok(false)` / `NotFound` / `Forbidden` mean "this module is unavailable to the
///   caller": its documents are excluded from the response, the request itself
///   succeeds.
/// - `Unavailable` / `Timeout` / `InvariantViolation` mean the answer is unknown:
///   the request fails instead of returning partially authorized results.
#[async_trait]
pub trait SearchAudiencePolicyPort: Send + Sync {
    /// The source module this audience policy applies to (e.g. "forum", "events", "courses").
    fn source_module(&self) -> &'static str;

    /// 1. Preflight discovery gate.
    ///
    /// Evaluated before full-text search. If false, public search skips this source
    /// module before touching the index, preventing timing and facet oracle leaks.
    /// Implementations must be explicit: there is no permissive default.
    async fn public_discovery_allowed(&self, tenant_id: Uuid) -> Result<bool, PortError>;

    /// 2. Real-time batch candidate post-authorization.
    ///
    /// Evaluates raw candidate IDs against current domain business rules and returns
    /// the subset of allowed document IDs. `request.candidates` never exceeds
    /// [`MAX_SEARCH_AUDIENCE_CANDIDATES`] entries.
    async fn filter_eligible_candidates(
        &self,
        request: SearchAudienceRequest,
    ) -> Result<Vec<Uuid>, PortError>;
}

pub type SharedSearchAudiencePolicyPort = Arc<dyn SearchAudiencePolicyPort>;

/// Registry holding the declared audience class of every Search source module.
///
/// The registry is the single source of truth for the public Search corpus:
/// - [`SearchAudienceRegistry::declare_public`] admits a module as public discovery,
/// - [`SearchAudienceRegistry::register`] admits a module as gated by its policy,
/// - modules absent from the registry are denied (never silently public).
#[derive(Clone, Default)]
pub struct SearchAudienceRegistry {
    gated: BTreeMap<String, SharedSearchAudiencePolicyPort>,
    public: BTreeSet<String>,
}

impl SearchAudienceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares a module as gated and registers its audience policy.
    ///
    /// A module may be declared exactly once. Re-declaring it — as gated or as
    /// public — is an invariant error instead of a silent overwrite.
    pub fn register<P>(&mut self, policy: P) -> Result<(), PortError>
    where
        P: SearchAudiencePolicyPort + 'static,
    {
        self.register_shared(Arc::new(policy))
    }

    /// Declares a shared gated policy. See [`SearchAudienceRegistry::register`].
    pub fn register_shared(
        &mut self,
        policy: SharedSearchAudiencePolicyPort,
    ) -> Result<(), PortError> {
        let source_module = normalize_source_module(policy.source_module())?;
        if self.is_declared(&source_module) {
            return Err(duplicate_declaration_error(&source_module));
        }
        self.gated.insert(source_module, policy);
        Ok(())
    }

    /// Declares a module whose Search documents are public discovery material.
    pub fn declare_public(&mut self, source_module: &str) -> Result<(), PortError> {
        let source_module = normalize_source_module(source_module)?;
        if self.is_declared(&source_module) {
            return Err(duplicate_declaration_error(&source_module));
        }
        self.public.insert(source_module);
        Ok(())
    }

    /// Declares a set of public modules. See [`SearchAudienceRegistry::declare_public`].
    pub fn declare_public_modules<I, S>(&mut self, source_modules: I) -> Result<(), PortError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for source_module in source_modules {
            self.declare_public(source_module.as_ref())?;
        }
        Ok(())
    }

    pub fn get(&self, source_module: &str) -> Option<SharedSearchAudiencePolicyPort> {
        let source_module = normalize_source_module(source_module).ok()?;
        self.gated.get(&source_module).cloned()
    }

    /// Resolves the declared audience class of a module.
    ///
    /// `None` means the host never declared the module; callers must deny it.
    pub fn audience_class(&self, source_module: &str) -> Option<SearchAudienceClass> {
        let source_module = normalize_source_module(source_module).ok()?;
        if self.gated.contains_key(&source_module) {
            Some(SearchAudienceClass::Gated)
        } else if self.public.contains(&source_module) {
            Some(SearchAudienceClass::Public)
        } else {
            None
        }
    }

    /// Returns true when the module is declared and requires a policy.
    pub fn is_gated(&self, source_module: &str) -> bool {
        self.audience_class(source_module) == Some(SearchAudienceClass::Gated)
    }

    pub fn is_declared(&self, source_module: &str) -> bool {
        self.audience_class(source_module).is_some()
    }

    /// All declared modules (public and gated), sorted and deduplicated.
    pub fn declared_source_modules(&self) -> Vec<String> {
        self.public
            .iter()
            .chain(self.gated.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Declared modules that require real-time candidate authorization.
    pub fn gated_source_modules(&self) -> Vec<String> {
        self.gated.keys().cloned().collect()
    }

    /// Declared modules whose documents are public discovery material.
    pub fn public_source_modules(&self) -> Vec<String> {
        self.public.iter().cloned().collect()
    }

    /// Alias of [`SearchAudienceRegistry::declared_source_modules`].
    pub fn source_modules(&self) -> Vec<String> {
        self.declared_source_modules()
    }

    pub fn is_empty(&self) -> bool {
        self.public.is_empty() && self.gated.is_empty()
    }

    /// Number of declared modules (public and gated).
    pub fn len(&self) -> usize {
        self.public.len() + self.gated.len()
    }

    /// Public discovery gate for one module.
    ///
    /// Gated modules delegate to their policy; public modules are discoverable;
    /// undeclared modules are never discoverable.
    pub async fn public_discovery_allowed(
        &self,
        source_module: &str,
        tenant_id: Uuid,
    ) -> Result<bool, PortError> {
        match self.audience_class(source_module) {
            Some(SearchAudienceClass::Public) => Ok(true),
            Some(SearchAudienceClass::Gated) => match self.get(source_module) {
                Some(policy) => policy.public_discovery_allowed(tenant_id).await,
                None => Ok(false),
            },
            None => Ok(false),
        }
    }
}

/// Evaluates candidates across all declared audience classes.
///
/// Public modules are admitted, gated modules are authorized by their policy in
/// bounded batches, and modules that the host never declared are denied so a
/// forgotten declaration can never turn into public discovery.
pub async fn resolve_audience_eligible_candidates(
    registry: &SearchAudienceRegistry,
    tenant_id: Uuid,
    locale: &str,
    candidates: &[SearchCandidateDescriptor],
    auth: Option<AuthContext>,
    request_context: Option<RequestContext>,
    transport: StorefrontSearchTransport,
) -> Result<HashSet<Uuid>, PortError> {
    if candidates.is_empty() {
        return Ok(HashSet::new());
    }
    if tenant_id.is_nil() {
        return Err(PortError::validation(
            "search.audience.tenant_required",
            "Search audience candidate resolution requires a valid tenant ID",
        ));
    }
    let effective_locale = if locale.trim().is_empty() {
        "en"
    } else {
        locale.trim()
    };

    if candidates.iter().any(|candidate| candidate.document_id.is_nil()) {
        return Err(PortError::validation(
            "search.audience.document_id_required",
            "Search audience candidate resolution requires non-nil document identifiers",
        ));
    }

    let unique = unique_candidate_descriptors(candidates)?;
    let mut allowed_ids = HashSet::with_capacity(unique.len());
    let mut partitioned: HashMap<&str, Vec<&SearchCandidateDescriptor>> = HashMap::new();

    for candidate in unique.values() {
        partitioned
            .entry(candidate.source_module.as_str())
            .or_default()
            .push(candidate);
    }

    for (source_module, module_candidates) in partitioned {
        match registry.audience_class(source_module) {
            Some(SearchAudienceClass::Public) => {
                allowed_ids.extend(module_candidates.iter().map(|candidate| candidate.document_id));
            }
            Some(SearchAudienceClass::Gated) => {
                let Some(policy) = registry.get(source_module) else {
                    tracing::error!(
                        source_module = %source_module,
                        "Search audience registry lost a gated policy between class resolution and dispatch"
                    );
                    return Err(PortError::invariant_violation(
                        "search.audience.registry_inconsistent",
                        format!("Search audience policy `{source_module}` disappeared from the registry"),
                    ));
                };

                for batch in module_candidates.chunks(MAX_SEARCH_AUDIENCE_CANDIDATES) {
                    let requested_ids = batch
                        .iter()
                        .map(|candidate| candidate.document_id)
                        .collect::<HashSet<_>>();

                    let request = SearchAudienceRequest {
                        tenant_id,
                        locale: effective_locale.to_string(),
                        candidates: batch
                            .iter()
                            .map(|candidate| (**candidate).clone())
                            .collect(),
                        auth: auth.clone(),
                        request_context: request_context.clone(),
                        transport,
                    };

                    let module_allowed = match policy.filter_eligible_candidates(request).await {
                        Ok(allowed) => allowed,
                        Err(error) if module_is_unavailable(&error) => {
                            tracing::warn!(
                                source_module = %source_module,
                                code = %error.code,
                                "Search audience module is unavailable to this caller; excluding its documents"
                            );
                            continue;
                        }
                        Err(error) => return Err(error),
                    };

                    if module_allowed.len() > requested_ids.len() {
                        return Err(PortError::invariant_violation(
                            "search.audience.owner_limit_exceeded",
                            format!(
                                "Search audience policy for `{source_module}` returned more candidates than requested"
                            ),
                        ));
                    }

                    let mut seen_allowed = HashSet::with_capacity(module_allowed.len());
                    for allowed_id in module_allowed {
                        if !seen_allowed.insert(allowed_id) {
                            return Err(PortError::invariant_violation(
                                "search.audience.duplicate_candidate",
                                format!(
                                    "Search audience policy for `{source_module}` returned duplicate candidate ID {allowed_id}"
                                ),
                            ));
                        }
                        if !requested_ids.contains(&allowed_id) {
                            return Err(PortError::invariant_violation(
                                "search.audience.owner_scope_invalid",
                                format!(
                                    "Search audience policy for `{source_module}` returned an unrequested candidate ID"
                                ),
                            ));
                        }
                        allowed_ids.insert(allowed_id);
                    }
                }
            }
            None => {
                tracing::warn!(
                    source_module = %source_module,
                    "Search audience registry has no declared audience class for a matching source module; denying its documents"
                );
            }
        }
    }

    Ok(allowed_ids)
}

/// Everything a storefront read must carry into the audience-aware executor.
///
/// The context is owned by the transport boundary (`apps/server` GraphQL data, the
/// native SSR adapter, or a test): Search itself never resolves these values, because
/// doing so would silently reintroduce an implicit corpus.
pub struct StorefrontSearchAudienceContext<'a> {
    /// The host-composed declaration of the public and gated Search corpus.
    pub registry: &'a SearchAudienceRegistry,
    /// The owner capability that expands storefront category roots into the exact
    /// document category scope. Applied only for the single-module scope the owner
    /// declares; absent means exact category semantics (owner not composed).
    pub category_scope: Option<SharedStorefrontSearchCategoryScopePort>,
    /// Present when the caller is authenticated.
    pub auth: Option<AuthContext>,
    pub request_context: Option<RequestContext>,
    pub transport: StorefrontSearchTransport,
}

/// Executes unified storefront search across public and audience-gated source modules.
///
/// Execution model:
/// 1. resolve the request scope against the declared corpus (undeclared modules denied),
/// 2. run the public discovery preflight for anonymous requests,
/// 3. scan and post-authorize gated candidates in bounded batches,
/// 4. execute exactly one scoped PostgreSQL query, so totals, facets, ordering and
///    pagination are computed over the authorized row set by the database itself,
/// 5. apply storefront query rules without letting them inject documents that
///    skipped authorization.
pub async fn execute_audience_eligible_search(
    db: &DatabaseConnection,
    engine: &PgSearchEngine,
    mut search_query: SearchQuery,
    channel: &TrustedStorefrontChannel,
    context: StorefrontSearchAudienceContext<'_>,
) -> CoreResult<SearchResult> {
    let StorefrontSearchAudienceContext {
        registry,
        category_scope,
        auth,
        request_context,
        transport,
    } = context;
    let started_at = Instant::now();
    let tenant_id = search_query.tenant_id.filter(|tenant| !tenant.is_nil()).ok_or_else(|| {
        Error::Validation("storefront search requires a valid tenant ID".to_string())
    })?;

    if search_query.query.trim().is_empty() {
        return Ok(empty_storefront_result(
            search_query.ranking_profile,
            started_at.elapsed().as_millis() as u64,
        ));
    }

    let declared_modules = registry.declared_source_modules();
    if declared_modules.is_empty() {
        tracing::error!(
            tenant_id = %tenant_id,
            "Search storefront request rejected because the host declared no Search audience modules"
        );
        return Ok(empty_storefront_result(
            search_query.ranking_profile,
            started_at.elapsed().as_millis() as u64,
        ));
    }

    // 1. Resolve the request scope against the declared corpus. Undeclared modules are
    //    denied rather than assumed public.
    let mut scope_modules = resolve_scope_modules(registry, &search_query.source_modules, &declared_modules, tenant_id);
    if scope_modules.is_empty() {
        return Ok(empty_storefront_result(
            search_query.ranking_profile,
            started_at.elapsed().as_millis() as u64,
        ));
    }

    // 2. Public discovery preflight. Anonymous callers must not reach the index for a
    //    module their owner closed to public discovery; authenticated callers are
    //    authorized per candidate in step 3 because discovery gating is a public-surface
    //    concept, not a permission.
    if auth.is_none() {
        let mut allowed_modules = Vec::with_capacity(scope_modules.len());
        for module in &scope_modules {
            match registry.public_discovery_allowed(module, tenant_id).await {
                Ok(true) => allowed_modules.push(module.clone()),
                Ok(false) => {
                    tracing::debug!(
                        source_module = %module,
                        tenant_id = %tenant_id,
                        "Search audience preflight closed module to public discovery"
                    );
                }
                Err(error) if module_is_unavailable(&error) => {
                    tracing::warn!(
                        source_module = %module,
                        code = %error.code,
                        "Search audience preflight found module unavailable; excluding it"
                    );
                }
                Err(error) => {
                    return Err(search_audience_error(error, "preflight discovery check"));
                }
            }
        }
        scope_modules = allowed_modules;
        if scope_modules.is_empty() {
            return Ok(empty_storefront_result(
                search_query.ranking_profile,
                started_at.elapsed().as_millis() as u64,
            ));
        }
    }

    // 3. Resolve storefront category roots through the owner capability. The owner
    //    declares which source module it owns, so a mixed or unrelated scope keeps the
    //    exact-category semantics and Search never guesses a module's category tree.
    //    This runs after the preflight so a module closed to public discovery never
    //    reaches any owner port.
    if !search_query.category_ids.is_empty() {
        // The locale the request actually runs in: the explicit query locale, else the
        // locale the host already resolved for this request. Search invents no default,
        // because a made-up locale would expand a tree in the wrong translation.
        let effective_locale = search_query
            .locale
            .as_deref()
            .map(str::trim)
            .filter(|locale| !locale.is_empty())
            .map(str::to_string)
            .or_else(|| {
                request_context
                    .as_ref()
                    .map(|context| context.locale.trim().to_string())
                    .filter(|locale| !locale.is_empty())
            });
        if let Some(effective_locale) = effective_locale {
            search_query.category_ids = crate::storefront_category_scope::resolve_storefront_search_category_ids(
                category_scope,
                crate::storefront_category_scope::StorefrontSearchCategoryScopeRequest {
                    tenant_id,
                    locale: effective_locale,
                    // The generic storefront request carries no separate fallback locale;
                    // the host resolved `effective_locale` from the request context, so the
                    // owner sees the same locale the result rows will be read in.
                    fallback_locale: None,
                    source_modules: scope_modules.clone(),
                    category_ids: search_query.category_ids.clone(),
                    auth: auth.clone(),
                    request_context: request_context.clone(),
                    transport,
                },
            )
            .await
            .map_err(|error| search_audience_error(error, "category scope resolution"))?;

            // An owner that resolved a requested category root to nothing has authorized
            // no category. Returning the unfiltered scope here would turn "this category
            // is not visible to you" into "search everything", so the request short
            // circuits the same way an empty module scope does.
            if search_query.category_ids.is_empty() {
                tracing::debug!(
                    tenant_id = %tenant_id,
                    "Search storefront request resolved no visible category in scope"
                );
                return Ok(empty_storefront_result(
                    search_query.ranking_profile,
                    started_at.elapsed().as_millis() as u64,
                ));
            }
        }
    }

    // 4. Bounded, batched post-authorization for gated modules only.
    let gated_scope = scope_modules
        .iter()
        .filter(|module| registry.is_gated(module))
        .cloned()
        .collect::<Vec<_>>();

    let authorized_ids = if gated_scope.is_empty() {
        Vec::new()
    } else {
        authorize_gated_candidates(
            engine,
            &search_query,
            channel,
            &gated_scope,
            registry,
            tenant_id,
            auth.clone(),
            request_context.clone(),
            transport,
        )
        .await?
    };

    // 5. One scoped SQL execution: totals, facets, ordering and pagination are exact.
    let audience_scope = SearchAudienceDocumentScope::new(gated_scope.clone(), authorized_ids)
        .map_err(|error| Error::Internal(format!("invalid Search audience scope: {error}")))?;
    search_query.source_modules = scope_modules;

    let result = engine
        .search_storefront_scoped(search_query.clone(), channel, &audience_scope)
        .await?;

    // 6. Query rules may reorder the authorized page. They may only inject a pinned
    //    document when no gated module is in scope, because a pin has no owner
    //    authorization.
    SearchDictionaryService::apply_storefront_query_rules(
        db,
        &search_query,
        result,
        channel,
        audience_scope.is_unrestricted(),
    )
    .await
}

/// Builds the canonical empty storefront result, including the canonical facet groups.
pub fn empty_storefront_result(
    ranking_profile: crate::ranking::SearchRankingProfile,
    took_ms: u64,
) -> SearchResult {
    SearchResult {
        items: Vec::new(),
        total: 0,
        took_ms,
        engine: SearchEngineKind::Postgres,
        ranking_profile,
        facets: vec![
            empty_facet_group("entity_type"),
            empty_facet_group("source_module"),
            empty_facet_group("status"),
        ],
    }
}

fn empty_facet_group(name: &str) -> SearchFacetGroup {
    SearchFacetGroup {
        name: name.to_string(),
        buckets: Vec::<SearchFacetBucket>::new(),
    }
}

fn resolve_scope_modules(
    registry: &SearchAudienceRegistry,
    requested_modules: &[String],
    declared_modules: &[String],
    tenant_id: Uuid,
) -> Vec<String> {
    if requested_modules.is_empty() {
        return declared_modules.to_vec();
    }

    let mut scope = BTreeSet::new();
    for module in requested_modules {
        match registry.audience_class(module) {
            Some(_) => {
                scope.insert(module.trim().to_ascii_lowercase());
            }
            None => {
                tracing::warn!(
                    source_module = %module,
                    tenant_id = %tenant_id,
                    "Search request asked for a source module with no declared audience class; denying it"
                );
            }
        }
    }
    scope.into_iter().collect()
}

#[allow(clippy::too_many_arguments)]
async fn authorize_gated_candidates(
    engine: &PgSearchEngine,
    search_query: &SearchQuery,
    channel: &TrustedStorefrontChannel,
    gated_scope: &[String],
    registry: &SearchAudienceRegistry,
    tenant_id: Uuid,
    auth: Option<AuthContext>,
    request_context: Option<RequestContext>,
    transport: StorefrontSearchTransport,
) -> CoreResult<Vec<Uuid>> {
    let mut scan_query = search_query.clone();
    scan_query.source_modules = gated_scope.to_vec();
    scan_query.offset = 0;
    scan_query.limit = AUDIENCE_CANDIDATE_SCAN_PAGE_SIZE;

    let mut items: Vec<SearchResultItem> = Vec::new();
    let mut expected_total: Option<u64> = None;
    let mut seen_rows = HashSet::new();

    loop {
        scan_query.offset = items.len();
        let page = engine.search_storefront(scan_query.clone(), channel).await?;

        let total = match expected_total {
            Some(expected) if expected != page.total => {
                return Err(Error::Internal(
                    "Storefront Search candidate snapshot changed during bounded eligibility evaluation"
                        .to_string(),
                ));
            }
            Some(expected) => expected,
            None => {
                if page.total > MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS as u64 {
                    return Err(Error::Validation(format!(
                        "Storefront Search matched more than {MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS} audience-restricted documents; narrow the query or filter scope"
                    )));
                }
                expected_total = Some(page.total);
                page.total
            }
        };

        if page.items.is_empty() {
            break;
        }

        for item in page.items {
            if !seen_rows.insert((
                item.source_module.clone(),
                item.entity_type.clone(),
                item.id,
                item.locale.clone(),
            )) {
                return Err(Error::Internal(
                    "Storefront Search candidate scan returned a duplicate raw row".to_string(),
                ));
            }
            items.push(item);
        }

        if items.len() as u64 >= total {
            break;
        }
        if items.len() > MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS {
            return Err(Error::Internal(
                "Storefront Search candidate scan exceeded its bounded total".to_string(),
            ));
        }
    }

    if items.len() as u64 != total {
        return Err(Error::Internal(
            "Storefront Search candidate scan did not resolve one unique row per result".to_string(),
        ));
    }

    let descriptors = items
        .iter()
        .map(|item| {
            SearchCandidateDescriptor::new(item.id, &item.source_module, &item.entity_type)
        })
        .collect::<Vec<_>>();

    let effective_locale = scan_query
        .locale
        .as_deref()
        .map(str::trim)
        .filter(|locale| !locale.is_empty())
        .unwrap_or("en");

    let allowed_ids = resolve_audience_eligible_candidates(
        registry,
        tenant_id,
        effective_locale,
        &descriptors,
        auth,
        request_context,
        transport,
    )
    .await
    .map_err(|error| search_audience_error(error, "candidate authorization"))?;

    let mut authorized = allowed_ids.into_iter().collect::<Vec<_>>();
    authorized.sort_unstable();
    Ok(authorized)
}

fn unique_candidate_descriptors(
    candidates: &[SearchCandidateDescriptor],
) -> Result<BTreeMap<Uuid, SearchCandidateDescriptor>, PortError> {
    let mut unique = BTreeMap::new();
    for candidate in candidates {
        if let Some(existing) = unique.get(&candidate.document_id) {
            if existing.source_module != candidate.source_module
                || existing.entity_type != candidate.entity_type
            {
                return Err(PortError::invariant_violation(
                    "search.audience.candidate_conflict",
                    format!(
                        "Search audience candidate {} was reported with conflicting source module or entity type",
                        candidate.document_id
                    ),
                ));
            }
            continue;
        }
        unique.insert(candidate.document_id, candidate.clone());
    }
    Ok(unique)
}

/// True when a policy answered "this module is not available to this caller", which
/// excludes its documents but keeps the request successful.
fn module_is_unavailable(error: &PortError) -> bool {
    matches!(
        error.kind,
        PortErrorKind::NotFound | PortErrorKind::Forbidden
    )
}

/// Maps an audience-policy failure onto the Search error taxonomy.
///
/// Validation, not-found and forbidden keep their public semantics; unavailable and
/// timeout stay operational failures (never caller validation errors), and invariant
/// violations stay internal errors.
pub(crate) fn search_audience_error(error: PortError, stage: &str) -> Error {
    let message = format!(
        "Search audience {stage} failed [{}]: {}",
        error.code, error.message
    );
    match error.kind {
        PortErrorKind::Validation => Error::Validation(message),
        PortErrorKind::NotFound => Error::NotFound(message),
        PortErrorKind::Conflict => Error::Validation(message),
        PortErrorKind::Forbidden => Error::Forbidden(message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => Error::External(message),
        PortErrorKind::InvariantViolation => Error::Internal(message),
    }
}

fn duplicate_declaration_error(source_module: &str) -> PortError {
    PortError::conflict(
        "search.audience_registry.duplicate_source_module",
        format!("Search source module `{source_module}` is already declared"),
    )
}

fn normalize_source_module(value: &str) -> Result<String, PortError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 64
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_'))
    {
        return Err(PortError::validation(
            "search.audience_registry.invalid_source_module",
            format!("Invalid Search audience source module `{value}`"),
        ));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public_blog_registry() -> SearchAudienceRegistry {
        let mut registry = SearchAudienceRegistry::new();
        registry
            .declare_public_modules(["blog", "pages", "docs", "content", "commerce"])
            .expect("public modules");
        registry
    }

    struct MockEventsPolicy {
        allowed_id: Uuid,
    }

    #[async_trait]
    impl SearchAudiencePolicyPort for MockEventsPolicy {
        fn source_module(&self) -> &'static str {
            "events"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Ok(true)
        }

        async fn filter_eligible_candidates(
            &self,
            request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            Ok(request
                .candidates
                .into_iter()
                .filter(|c| c.document_id == self.allowed_id)
                .map(|c| c.document_id)
                .collect())
        }
    }

    #[tokio::test]
    async fn multi_module_candidate_authorization() {
        let allowed_event = Uuid::new_v4();
        let denied_event = Uuid::new_v4();
        let blog_post = Uuid::new_v4();

        let mut registry = public_blog_registry();
        registry
            .register(MockEventsPolicy {
                allowed_id: allowed_event,
            })
            .unwrap();

        let candidates = vec![
            SearchCandidateDescriptor::new(allowed_event, "events", "event_announcement"),
            SearchCandidateDescriptor::new(denied_event, "events", "event_announcement"),
            SearchCandidateDescriptor::new(blog_post, "blog", "blog_post"),
        ];

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &candidates,
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("authorized candidates");

        assert!(allowed.contains(&allowed_event));
        assert!(!allowed.contains(&denied_event));
        assert!(allowed.contains(&blog_post)); // Blog is declared public
    }

    #[tokio::test]
    async fn undeclared_source_module_is_denied() {
        let registry = public_blog_registry();
        let candidate = Uuid::new_v4();

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &[SearchCandidateDescriptor::new(candidate, "secret_lab", "note")],
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("declared candidates resolved");

        assert!(
            allowed.is_empty(),
            "an undeclared module must never be publicly discoverable"
        );
        assert_eq!(
            registry.audience_class("secret_lab"),
            None,
            "undeclared modules have no audience class"
        );
        assert!(!registry.public_discovery_allowed("secret_lab", Uuid::new_v4()).await.unwrap());
    }

    struct CountingEventsPolicy {
        batch_sizes: Arc<std::sync::Mutex<Vec<usize>>>,
    }

    #[async_trait]
    impl SearchAudiencePolicyPort for CountingEventsPolicy {
        fn source_module(&self) -> &'static str {
            "events"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Ok(true)
        }

        async fn filter_eligible_candidates(
            &self,
            request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            self.batch_sizes
                .lock()
                .expect("batch size recorder")
                .push(request.candidates.len());
            Ok(request
                .candidates
                .into_iter()
                .map(|candidate| candidate.document_id)
                .collect())
        }
    }

    #[tokio::test]
    async fn gated_candidates_are_authorized_in_bounded_batches() {
        // A broad gated query must be paginated through the owner port, never rejected
        // because the candidate set is larger than one policy call.
        let batch_sizes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut registry = public_blog_registry();
        registry
            .register(CountingEventsPolicy {
                batch_sizes: batch_sizes.clone(),
            })
            .expect("gated module registration");

        let candidates = (0..(MAX_SEARCH_AUDIENCE_CANDIDATES * 2 + 5))
            .map(|_| {
                SearchCandidateDescriptor::new(Uuid::new_v4(), "events", "event_announcement")
            })
            .collect::<Vec<_>>();

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &candidates,
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("batched authorization");

        assert_eq!(allowed.len(), candidates.len());
        assert_eq!(
            batch_sizes.lock().expect("batch size recorder").clone(),
            vec![MAX_SEARCH_AUDIENCE_CANDIDATES, MAX_SEARCH_AUDIENCE_CANDIDATES, 5],
            "every policy call must stay within the documented batch bound"
        );
    }

    #[tokio::test]
    async fn multi_locale_candidates_are_authorized_once() {
        // One document can match in several locales; authorization is per document, so
        // the owner must not see duplicate ids and must not be asked twice.
        let batch_sizes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut registry = public_blog_registry();
        registry
            .register(CountingEventsPolicy {
                batch_sizes: batch_sizes.clone(),
            })
            .expect("gated module registration");

        let document_id = Uuid::new_v4();
        let candidates = vec![
            SearchCandidateDescriptor::new(document_id, "events", "event_announcement"),
            SearchCandidateDescriptor::new(document_id, "events", "event_announcement"),
        ];

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &candidates,
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("deduplicated authorization");

        assert_eq!(allowed, HashSet::from([document_id]));
        assert_eq!(
            batch_sizes.lock().expect("batch size recorder").clone(),
            vec![1]
        );
    }

    #[test]
    fn audience_error_taxonomy_is_explicit() {
        let unavailable = search_audience_error(
            PortError::unavailable("search.audience.unavailable", "owner offline"),
            "preflight",
        );
        assert!(
            matches!(unavailable, Error::External(_)),
            "an unavailable owner must stay an operational failure"
        );

        let timeout = search_audience_error(
            PortError::timeout("search.audience.timeout", "owner timeout"),
            "preflight",
        );
        assert!(matches!(timeout, Error::External(_)));

        let validation = search_audience_error(
            PortError::validation("search.audience.invalid", "bad request"),
            "preflight",
        );
        assert!(matches!(validation, Error::Validation(_)));

        let forbidden = search_audience_error(
            PortError::forbidden("search.audience.forbidden", "no access"),
            "preflight",
        );
        assert!(matches!(forbidden, Error::Forbidden(_)));

        let invariant = search_audience_error(
            PortError::invariant_violation("search.audience.invariant", "impossible"),
            "preflight",
        );
        assert!(matches!(invariant, Error::Internal(_)));
    }

    #[test]
    fn duplicate_declarations_are_rejected() {
        let mut registry = public_blog_registry();
        assert!(
            registry.declare_public("blog").is_err(),
            "public module re-declaration must fail"
        );
        assert!(
            registry
                .register(MockEventsPolicy {
                    allowed_id: Uuid::new_v4()
                })
                .is_ok()
        );
        assert!(
            registry
                .register(MockEventsPolicy {
                    allowed_id: Uuid::new_v4()
                })
                .is_err(),
            "gated policy re-registration must fail instead of overwriting"
        );
        assert!(
            registry.declare_public("events").is_err(),
            "a gated module cannot be re-declared as public"
        );
    }

    struct RoguePolicy;

    #[async_trait]
    impl SearchAudiencePolicyPort for RoguePolicy {
        fn source_module(&self) -> &'static str {
            "rogue"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Ok(true)
        }

        async fn filter_eligible_candidates(
            &self,
            _request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            // Returns an unrequested UUID
            Ok(vec![Uuid::new_v4()])
        }
    }

    #[tokio::test]
    async fn unrequested_candidate_id_fails_closed() {
        let requested_id = Uuid::new_v4();
        let mut registry = SearchAudienceRegistry::new();
        registry.register(RoguePolicy).unwrap();

        let candidates = vec![SearchCandidateDescriptor::new(
            requested_id,
            "rogue",
            "item",
        )];

        let result = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &candidates,
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await;

        assert!(result.is_err());
    }

    struct BatchRecordingPolicy {
        expected_max_batch: usize,
    }

    #[async_trait]
    impl SearchAudiencePolicyPort for BatchRecordingPolicy {
        fn source_module(&self) -> &'static str {
            "events"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Ok(true)
        }

        async fn filter_eligible_candidates(
            &self,
            request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            assert!(
                request.candidates.len() <= self.expected_max_batch,
                "audience batches must stay bounded"
            );
            Ok(request
                .candidates
                .into_iter()
                .map(|candidate| candidate.document_id)
                .collect())
        }
    }

    #[tokio::test]
    async fn large_candidate_sets_are_batched_instead_of_rejected() {
        let mut registry = SearchAudienceRegistry::new();
        registry
            .register(BatchRecordingPolicy {
                expected_max_batch: MAX_SEARCH_AUDIENCE_CANDIDATES,
            })
            .unwrap();

        let candidates = (0..(MAX_SEARCH_AUDIENCE_CANDIDATES * 2 + 7))
            .map(|_| {
                SearchCandidateDescriptor::new(Uuid::new_v4(), "events", "event_announcement")
            })
            .collect::<Vec<_>>();

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &candidates,
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("batched authorization");

        assert_eq!(allowed.len(), candidates.len());
    }

    #[tokio::test]
    async fn duplicate_candidates_across_locales_are_authorized_once() {
        let mut registry = SearchAudienceRegistry::new();
        registry
            .register(BatchRecordingPolicy {
                expected_max_batch: MAX_SEARCH_AUDIENCE_CANDIDATES,
            })
            .unwrap();

        let document_id = Uuid::new_v4();
        let candidates = vec![
            SearchCandidateDescriptor::new(document_id, "events", "event_announcement"),
            SearchCandidateDescriptor::new(document_id, "events", "event_announcement"),
        ];

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &candidates,
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("deduplicated authorization");

        assert_eq!(allowed.len(), 1);
    }

    struct UnavailablePolicy;

    #[async_trait]
    impl SearchAudiencePolicyPort for UnavailablePolicy {
        fn source_module(&self) -> &'static str {
            "events"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Err(PortError::unavailable(
                "events.audience.unavailable",
                "Events audience is temporarily unavailable",
            ))
        }

        async fn filter_eligible_candidates(
            &self,
            _request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            Err(PortError::unavailable(
                "events.audience.unavailable",
                "Events audience is temporarily unavailable",
            ))
        }
    }

    struct DisabledModulePolicy;

    #[async_trait]
    impl SearchAudiencePolicyPort for DisabledModulePolicy {
        fn source_module(&self) -> &'static str {
            "events"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Err(PortError::not_found(
                "events.unavailable",
                "Events module is disabled",
            ))
        }

        async fn filter_eligible_candidates(
            &self,
            _request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            Err(PortError::not_found(
                "events.unavailable",
                "Events module is disabled",
            ))
        }
    }

    #[tokio::test]
    async fn disabled_module_is_excluded_instead_of_failing_the_request() {
        let mut registry = public_blog_registry();
        registry.register(DisabledModulePolicy).unwrap();
        let blog_post = Uuid::new_v4();

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &[SearchCandidateDescriptor::new(blog_post, "blog", "blog_post")],
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("public module resolution");

        assert!(allowed.contains(&blog_post));
    }

    #[tokio::test]
    async fn unavailable_module_fails_the_request_instead_of_leaking() {
        let mut registry = SearchAudienceRegistry::new();
        registry.register(UnavailablePolicy).unwrap();

        let error = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &[SearchCandidateDescriptor::new(
                Uuid::new_v4(),
                "events",
                "event_announcement",
            )],
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect_err("unavailable owner must fail closed");

        assert_eq!(error.kind, PortErrorKind::Unavailable);
    }

    #[test]
    fn port_error_kinds_map_onto_the_search_error_taxonomy() {
        assert!(matches!(
            search_audience_error(
                PortError::unavailable("x", "temporarily unavailable"),
                "preflight"
            ),
            Error::External(_)
        ));
        assert!(matches!(
            search_audience_error(
                PortError::invariant_violation("x", "invariant"),
                "preflight"
            ),
            Error::Internal(_)
        ));
        assert!(matches!(
            search_audience_error(PortError::validation("x", "invalid"), "preflight"),
            Error::Validation(_)
        ));
        assert!(matches!(
            search_audience_error(PortError::forbidden("x", "forbidden"), "preflight"),
            Error::Forbidden(_)
        ));
    }

    struct MockDirectForumPolicy {
        allowed_topic_id: Uuid,
    }

    #[async_trait]
    impl SearchAudiencePolicyPort for MockDirectForumPolicy {
        fn source_module(&self) -> &'static str {
            "forum"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Ok(true)
        }

        async fn filter_eligible_candidates(
            &self,
            request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            Ok(request
                .candidates
                .into_iter()
                .filter(|c| c.document_id == self.allowed_topic_id)
                .map(|c| c.document_id)
                .collect())
        }
    }

    #[tokio::test]
    async fn direct_forum_policy_registration_and_evaluation() {
        let allowed_topic = Uuid::new_v4();
        let denied_topic = Uuid::new_v4();

        let policy: SharedSearchAudiencePolicyPort = Arc::new(MockDirectForumPolicy {
            allowed_topic_id: allowed_topic,
        });

        let mut registry = SearchAudienceRegistry::new();
        registry.register_shared(policy).unwrap();

        assert!(
            registry
                .public_discovery_allowed("forum", Uuid::new_v4())
                .await
                .unwrap()
        );

        let candidates = vec![
            SearchCandidateDescriptor::new(allowed_topic, "forum", "forum_topic"),
            SearchCandidateDescriptor::new(denied_topic, "forum", "forum_topic"),
        ];

        let allowed = resolve_audience_eligible_candidates(
            &registry,
            Uuid::new_v4(),
            "en",
            &candidates,
            None,
            None,
            StorefrontSearchTransport::Graphql,
        )
        .await
        .expect("authorized candidates");

        assert!(allowed.contains(&allowed_topic));
        assert!(!allowed.contains(&denied_topic));
    }

    struct ClosedAudiencePolicy;

    #[async_trait]
    impl SearchAudiencePolicyPort for ClosedAudiencePolicy {
        fn source_module(&self) -> &'static str {
            "private_docs"
        }

        async fn public_discovery_allowed(&self, _tenant_id: Uuid) -> Result<bool, PortError> {
            Ok(false)
        }

        async fn filter_eligible_candidates(
            &self,
            _request: SearchAudienceRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            Ok(Vec::new())
        }
    }

    fn storefront_query(query: &str, source_modules: Vec<String>) -> SearchQuery {
        SearchQuery {
            tenant_id: Some(Uuid::new_v4()),
            locale: Some("en".to_string()),
            channel_id: None,
            original_query: query.to_string(),
            query: query.to_string(),
            ranking_profile: crate::ranking::SearchRankingProfile::Balanced,
            preset_key: None,
            limit: 10,
            offset: 0,
            published_only: true,
            entity_types: Vec::new(),
            source_modules,
            statuses: Vec::new(),
            category_ids: Vec::new(),
            attribute_filters: Vec::new(),
            sort_attribute_code: None,
            sort_desc: false,
        }
    }

    fn untrusted_channel() -> TrustedStorefrontChannel {
        TrustedStorefrontChannel {
            channel_id: None,
            channel_slug: None,
        }
    }

    #[tokio::test]
    async fn preflight_discovery_blocked_skips_execution() {
        let mut registry = SearchAudienceRegistry::new();
        registry.register(ClosedAudiencePolicy).unwrap();

        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let engine = PgSearchEngine::new(db.clone());
        let channel = untrusted_channel();

        let result = execute_audience_eligible_search(
            &db,
            &engine,
            storefront_query("confidential roadmap", vec!["private_docs".to_string()]),
            &channel,
            StorefrontSearchAudienceContext {
                registry: &registry,
                category_scope: None,
                auth: None,
                request_context: None,
                transport: StorefrontSearchTransport::Graphql,
            },
        )
        .await
        .expect("empty public search");

        assert!(result.items.is_empty());
        assert_eq!(result.total, 0);
        assert!(result.facets.iter().all(|f| f.buckets.is_empty()));
    }

    #[tokio::test]
    async fn undeclared_requested_module_returns_no_results() {
        let registry = public_blog_registry();

        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let engine = PgSearchEngine::new(db.clone());
        let channel = untrusted_channel();

        let result = execute_audience_eligible_search(
            &db,
            &engine,
            storefront_query("anything", vec!["secret_lab".to_string()]),
            &channel,
            StorefrontSearchAudienceContext {
                registry: &registry,
                category_scope: None,
                auth: None,
                request_context: None,
                transport: StorefrontSearchTransport::Graphql,
            },
        )
        .await
        .expect("denied module produces an empty result");

        assert!(result.items.is_empty());
        assert_eq!(result.total, 0);
    }

    #[tokio::test]
    async fn empty_query_returns_empty_result() {
        let registry = SearchAudienceRegistry::new();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let engine = PgSearchEngine::new(db.clone());
        let channel = untrusted_channel();

        let result = execute_audience_eligible_search(
            &db,
            &engine,
            storefront_query("   ", Vec::new()),
            &channel,
            StorefrontSearchAudienceContext {
                registry: &registry,
                category_scope: None,
                auth: None,
                request_context: None,
                transport: StorefrontSearchTransport::Graphql,
            },
        )
        .await
        .expect("empty query result");

        assert!(result.items.is_empty());
        assert_eq!(result.total, 0);
    }

    /// Owner category capability that reports "no visible category" for every root.
    struct EmptyCategoryScopePort;

    #[async_trait]
    impl crate::storefront_category_scope::StorefrontSearchCategoryScopePort for EmptyCategoryScopePort {
        fn source_module(&self) -> &'static str {
            "forum"
        }

        async fn expand_category_scope(
            &self,
            _request: crate::storefront_category_scope::StorefrontSearchCategoryScopeRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn category_scope_resolved_to_nothing_returns_empty_result() {
        let mut registry = SearchAudienceRegistry::new();
        registry
            .declare_public_modules(["forum"])
            .expect("public modules");
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let engine = PgSearchEngine::new(db.clone());
        let channel = untrusted_channel();
        let category_scope: crate::SharedStorefrontSearchCategoryScopePort =
            std::sync::Arc::new(EmptyCategoryScopePort);
        let mut query = storefront_query("anything", vec!["forum".to_string()]);
        query.category_ids = vec![Uuid::new_v4()];

        let result = execute_audience_eligible_search(
            &db,
            &engine,
            query,
            &channel,
            StorefrontSearchAudienceContext {
                registry: &registry,
                category_scope: Some(category_scope),
                auth: None,
                request_context: None,
                transport: StorefrontSearchTransport::Graphql,
            },
        )
        .await
        .expect("a category with nothing visible must not widen the search");

        assert!(result.items.is_empty());
        assert_eq!(result.total, 0);
    }

    #[tokio::test]
    async fn registry_without_declarations_returns_empty_result() {
        let registry = SearchAudienceRegistry::new();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let engine = PgSearchEngine::new(db.clone());
        let channel = untrusted_channel();

        let result = execute_audience_eligible_search(
            &db,
            &engine,
            storefront_query("anything", Vec::new()),
            &channel,
            StorefrontSearchAudienceContext {
                registry: &registry,
                category_scope: None,
                auth: None,
                request_context: None,
                transport: StorefrontSearchTransport::Graphql,
            },
        )
        .await
        .expect("undeclared corpus is not publicly discoverable");

        assert!(result.items.is_empty());
        assert_eq!(result.total, 0);
    }
}

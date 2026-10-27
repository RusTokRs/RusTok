# Declared Search Audience Corpus with Explainable Fail-Closed Defaults

- Date: 2026-10-25
- Decision status: Accepted
- Implementation status: Implemented
- Owners: platform / search team
- Extends: `2026-10-16-generic-search-audience-security-spi.md`
- Supersedes: `2026-10-16-generic-search-audience-security-spi.md`
- Superseded by: None

## Context

The generic Search audience SPI (`2026-10-16-generic-search-audience-security-spi.md`)
introduced module-owned `SearchAudiencePolicyPort` implementations and a
`SearchAudienceRegistry` registered through module runtime extensions. A mechanism
audit against the "safe for Blog and for every other module" requirement found that the
SPI was structurally module-agnostic but defaulted open in four independent places:

1. both storefront boundaries substituted an empty registry when the host had not
   composed one (`unwrap_or(&default_registry)`), which erased every gated module and
   published its documents;
2. `resolve_audience_eligible_candidates` admitted candidates whose `source_module` had
   no registered policy, so a module that simply forgot a policy became public;
3. `apps/server` discarded the `register` result with `let _ =`, so a duplicate or
   invalid policy registration was invisible;
4. `SearchAudiencePolicyPort::public_discovery_allowed` defaulted to `Ok(true)`.

The same audit found availability and taxonomy defects that made the "ideal" claim
untrue even when registration was correct:

- the executor refused any storefront query that matched more than 100 documents in
  total, including documents of public modules, with a caller-facing `Validation`
  error; a broad query over Blog failed as soon as Forum registered a policy;
- `PortError` from a policy was flattened to `Error::Validation`, so `unavailable` and
  `invariant_violation` were reported as caller mistakes (400 instead of 5xx), while
  `not_found` from a disabled module failed the whole request instead of excluding that
  module;
- facets were re-aggregated in Rust with a hardcoded `forum_visible_status` fallback,
  producing a different facet vocabulary from the SQL fast path and coupling the
  generic SPI to the deprecated Forum pipeline;
- storefront suggestions bypassed the audience mechanism entirely and hardcoded
  `source_module <> 'forum'`;
- the generic engine hardcoded Forum (`source_module <> 'forum'`, `exclude_forum`,
  a `"forum"` special case in the exclusion loop) as the only gated module.

## Decision

1. **Audience class is declared, never inferred.** `SearchAudienceRegistry` holds
   exactly one declared class per source module: `SearchAudienceClass::Public`
   (`declare_public` / `declare_public_modules`) or `SearchAudienceClass::Gated`
   (`register` / `register_shared`). Declaring a module twice — in either class — is a
   `conflict` error, not a silent overwrite.
2. **Undeclared modules are denied.** `resolve_audience_eligible_candidates` denies
   candidates of a module with no declared class (with a `tracing::warn!` for the
   operator) and `public_discovery_allowed` returns `false` for it. The host, not
   `rustok-search`, owns the corpus inventory; `apps/server` declares
   `search_public_source_modules()` and registers gated policies.
3. **Every storefront boundary fails closed without a registry.** GraphQL
   `storefrontSearch` / `storefrontSearchSuggestions` and the native SSR adapter return
   an explicit error when the `SearchAudienceRegistry` is missing instead of degrading
   to "nothing is gated". Host composition propagates registration failures instead of
   discarding them.
4. **One SQL query decides totals, facets, ordering and pagination.** Gated modules are
   scanned in pages of `AUDIENCE_CANDIDATE_SCAN_PAGE_SIZE` and authorized in batches of
   at most `MAX_SEARCH_AUDIENCE_CANDIDATES`; the authorized ids are passed to PostgreSQL
   as `(source_module NOT IN (gated) OR document_id IN (authorized))` inside the normal
   visibility filter. Public modules are never bounded by the audience scan. The scan
   itself is bounded by `MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS` and exceeding it is a
   documented `Validation` error.
5. **SPI error taxonomy is explicit.** Policy `NotFound`/`Forbidden` means "module
   unavailable to this caller": its documents are excluded and the request succeeds.
   `Validation`/`Conflict` map to caller errors, `Forbidden` to a permission error,
   `Unavailable`/`Timeout` to an operational error, and `InvariantViolation` to an
   internal error. A disabled module can no longer fail an unrelated storefront search.
6. **Suggestions obey the same corpus.** Storefront suggestions take a
   `StorefrontSuggestionAudience` (registry, auth, request context, transport), restrict
   the SQL by the declared module scope, run the same public-discovery preflight for
   anonymous callers, and post-authorize gated document suggestions through
   `filter_eligible_candidates`.
7. **Query rules may not resurrect unauthorized documents.** Pinned rules can always
   reorder the authorized page; they may inject an extra document only when no gated
   module is in the scope, and injected documents are checked for payload channel
   visibility for every entity type, not only products.
8. **The generic engine carries no module knowledge.** `exclude_forum` and
   `source_module <> 'forum'` are removed from `pg_engine`; the only module-level input
   is the neutral `SearchAudienceDocumentScope`.

## Sources of truth and ownership

- `rustok-search` owns the registry, the declared-class vocabulary, the bounded scan,
  the SQL audience predicate, the SPI error taxonomy, and the storefront suggestion
  parity.
- The host (`apps/server`) owns the corpus inventory: it declares
  `search_public_source_modules()` and registers each gated module policy, and it must
  fail startup composition when a declaration or registration is rejected.
- Domain modules own the real-time answer (`public_discovery_allowed`,
  `filter_eligible_candidates`) and never depend on Search internals.
- `DECISIONS/2026-10-16-generic-search-audience-security-spi.md` remains the historical
  record of the SPI introduction; this ADR is the active decision for its defaults.

## Invariants

### Allowed states

- Every source module that writes `search_documents` is declared exactly once, as public
  or as gated.
- A gated module answers both SPI phases; a public module is admitted by PostgreSQL
  visibility predicates only.
- Multiple gated modules can participate in one request; authorization runs per module
  in bounded batches.
- Totals, facets and pagination are computed by PostgreSQL over the authorized row set.

### Forbidden states

- An undeclared source module being returned by a public storefront read.
- A storefront boundary running with an empty or missing registry.
- A silent duplicate or discarded registration result at host composition.
- A caller-facing validation error caused by a module being disabled or temporarily
  unavailable.
- Facet or total computation over rows that have not been authorized.
- A pinned query rule injecting a document that skipped audience authorization.
- Module-specific hardcoding inside the generic storefront engine.

## Non-goals

- Storing per-user ACLs inside the FTS index. The audience predicate is evaluated per
  request; only authorized document ids are bound into SQL.
- Replacing `SearchProjectionSource` or the projection registry.
- Removing the deprecated Forum bridge files in this change; they are retained as
  `#[deprecated]` adapters and are no longer referenced by the generic SPI.

## Data, transaction, and concurrency boundary

The candidate scan is read-only and runs on the request connection. Authorization is a
per-request computation; no intermediate authorization state is persisted. The final
query is one statement pair (count/facets/items) against the same snapshot semantics as
any other Search read. A snapshot change between scan pages aborts with an internal
error instead of returning partially authorized results.

## Context dimensions

- **Tenant:** every query, policy call and scan is tenant-scoped; a nil tenant is a
  validation error.
- **Auth & request context:** passed unchanged into `SearchAudienceRequest`; the public
  discovery preflight runs for anonymous requests only, because it is a public-surface
  discovery gate rather than a permission.
- **Locale:** normalized to a non-empty locale (`en` fallback) before policy dispatch.

## Events and projections

No new events. Audience declarations are composition-time values in
`ModuleRuntimeExtensions`; they are rebuilt on process start and never mutated at
runtime.

## Failure semantics

- Missing registry at a storefront boundary → error (`FieldError` / `ServerFnError`),
  never an unfiltered read.
- Undeclared module in results → documents denied, warning emitted.
- Policy `NotFound`/`Forbidden` → module excluded, request succeeds.
- Policy `Unavailable`/`Timeout` → operational error, no partial results.
- Policy `InvariantViolation` (unrequested id, duplicate id, more ids than requested) →
  internal error.
- Scan above `MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS` → `Validation` asking for a narrower
  query.

## Migration and cutover

Existing deployments must declare their corpus. `apps/server` declares `blog`,
`commerce`, `docs`, `pages`, the Content node kinds and registers Forum as gated in the
same composition step, so the shipped host keeps its previous public behavior while
becoming fail-closed for unknown modules.

Content is the one module whose documents are keyed by kind: `projector_core` writes
`source_module = node.kind`, so the Content contribution to the corpus is the
Content-owned `CONTENT_NODE_KINDS` list (the same closed list that validates writes). The
host therefore never mirrors that vocabulary by hand, and a node kind that is not in the
list is rejected on write instead of being indexed and silently denied. The host also
declares `content`, the projector's fallback bucket for a node whose stored kind is empty
(`COALESCE(NULLIF(kind, ''), 'content')`): the corpus must remain a superset of every
value the projector can emit, otherwise fail-closed Search would silently hide a damaged
published row instead of showing it.

A host that composes `rustok-search` without `apps/server` must declare its own modules;
otherwise storefront search returns empty results and logs a composition error.

## Alternatives considered

- **Keep the permissive default and add a linter.** Rejected: the failure mode is a
  silent cross-module leak, which no lint can reliably catch at the SPI boundary.
- **Deny by default without a public class.** Rejected: every public module would need a
  no-op policy, which is a stub with no owner semantics.
- **Keep in-Rust facet re-aggregation.** Rejected: two facet vocabularies and a second
  pagination path cannot stay consistent with SQL ordering.
- **Keep the 100-candidate request cap.** Rejected: it converts a broad but legitimate
  public query into a client error as soon as any module is gated.

## Verification

- `cargo test -p rustok-search` (audience modules: declared classes, batching,
  undeclared denial, unavailable vs disabled modules, port error mapping).
- `npm run verify:search:audience` — source guard for declared classes, fail-closed
  boundaries, neutral engine scope, suggestion parity, and host composition.
- `npm run verify:forum-search:product-channel-visibility` (existing channel predicate
  contract) and the Forum source guards that reference the Search engine.

## Consequences

- A forgotten declaration or registration now hides documents instead of publishing
  them; the host composition path fails loudly for duplicate declarations.
- Broad storefront queries work with any number of gated modules, bounded only by the
  documented scan ceiling.
- The deprecated Forum bridge files remain, but the generic path no longer depends on
  them, which unblocks their physical removal.

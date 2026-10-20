# SEO canonical, hreflang, sitemap, and redirect route-family contract

- Date: 2026-10-18
- Decision status: Proposed
- Implementation status: Not started
- Owners: `rustok-seo` (page-context contract and the single SEO output adapter); owner modules declare their route family (`rustok-blog`, `rustok-product`, `rustok-pages`, `rustok-forum`, `rustok-taxonomy`); storefront hosts serve the declared families (`apps/next-frontend`, `apps/storefront`)
- Extends: `DECISIONS/2026-03-28-multilingual-content-contract.md`, `DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md`, `DECISIONS/2026-10-08-owner-routes-and-batched-conversion.md`
- Supersedes: None
- Superseded by: None

## Context

The 2026-10-18 SEO product-logic audit (finding P1-2, `docs/audits/seo-module-product-logic-audit-2026-10-18.md`)
found that SEO outputs disagree about which URL family represents a page:

1. Owner providers emit heterogeneous canonical families:
   blog `/modules/blog?slug={slug}` (pinned by the Accepted blog URL ADR),
   product `/modules/product?handle={handle}`, pages `/{locale}/modules/pages?slug={slug}`,
   forum `/{locale}/forum/t|c/...` (native public routes), taxonomy category `/categories/{slug}` (no locale prefix).
2. The Next storefront serves only `app/[locale]/modules/forum/` for module routes; head tags publish
   `context.route.canonicalUrl` and `context.route.alternates` verbatim, so `rel=canonical`/`hreflang`
   land on 404 for blog/product/pages module routes, while `adaptRuntimeSitemapUrl` **translates**
   `/modules/...` to public paths for the sitemap only — contradictory signals for the same page.
3. Blog alternates are identical for every locale (the provider stores `canonical_route` for each
   locale) and product alternates lack locale prefixes, so the hreflang cluster degenerates to one href.
4. The runtime-parity fixtures (`apps/next-frontend/contracts/seo/runtime-parity-fixtures.json`) already
   record both a module pattern and a public pattern per owner and bless the Rust storefront module
   route, but no rule says which family any given output must use, so each consumer improvises.

Two Accepted decisions constrain the solution space: the blog route is `/modules/blog?slug=`
(`2026-10-08-blog-post-url-and-publication-contract.md`), and owner-held routes derive canonical
routes in the same module shape (`2026-10-08-owner-routes-and-batched-conversion.md`). The contradiction
is therefore not "which ADR wins" but "which family is emitted where, by whom, and who must serve it".

## Decision

1. **Every owner declares exactly one canonical route family** for each target kind it owns, in the
   route-ownership contract (`routeOwnership` in the runtime-parity fixtures). The declared family is
   the only family that SEO outputs emit. Current declarations:

   | Owner | Target kinds | Canonical family |
   |---|---|---|
   | `rustok-blog` | `blog_post` | `/{locale}/modules/blog?slug={slug}` (locale-qualified form of the Accepted blog route) |
   | `rustok-product` | `product` | `/{locale}/modules/product?handle={handle}` |
   | `rustok-pages` | `page` | `/{locale}/modules/pages?slug={slug}` |
   | `rustok-forum` | `forum_category`, `forum_topic` | native public routes `/{locale}/forum/...` |
   | `rustok-taxonomy` | `category` | `/{locale}/categories/{slug}` |

2. **Every emitted public href is locale-qualified** with the href's own locale
   (`/{locale}/...`), so hreflang alternates are unique per locale by construction.

3. **One adapter emits all SEO outputs.** Canonical URLs, hreflang alternates, redirect targets,
   and sitemap entries are derived from the declared family through the shared route-family
   adapter in `rustok-seo` (`canonical_url_for_locale` / `locale_prefixed_path` plus the ownership
   matrix). Output-specific translation — today's sitemap-only `adaptRuntimeSitemapUrl` — is forbidden;
   during migration the adapter may be the only translator, and it translates **once** for every output.

4. **Hosts serve their owners' declared families.** A storefront that cannot serve a declared
   canonical family (Next has no `/{locale}/modules/blog|product|pages` routes) has a route-ownership
   bug, not a reason to redirect signals elsewhere. The existing owner public patterns
   (`/blog/:slug`, `/products/:slug`, `/{locale}/forum/...`) remain owner-served aliases; they may
   return 200 with `rel=canonical` pointing at the canonical family.

## Sources of truth and ownership

- Canonical owner of the page-context contract and the adapter: `rustok-seo`.
- Canonical owner of each route family: the owning module (blog/product/pages/forum/taxonomy), which
  derives its canonical route from its own identity per the owner-routes ADR.
- Authoritative declaration of family per owner: `apps/next-frontend/contracts/seo/runtime-parity-fixtures.json`
  `routeOwnership` (updated in the same change as any owner route change).
- Derived projections: head tags, sitemap XML, robots `Sitemap:` URL, redirect decisions — all derived,
  none authoritative.
- Dependency direction: owners expose routes; `rustok-seo` consumes and emits; hosts serve. Hosts never
  invent route families, and `rustok-seo` never hardcodes an owner's family.

## Invariants

### Allowed states

- For one target and locale: canonical == the locale-qualified href that sitemap lists == the href
  every hreflang entry resolves (HTTP 200) on the serving host.
- Every hreflang alternate href differs by locale (no duplicate hrefs inside one alternates cluster).
- An owner's public alias may 200 with a canonical pointing at the canonical family, or 308 to it;
  both are conforming as long as the canonical family href itself resolves 200.
- Outputs of one page produced through different transports (GraphQL, REST, Leptos SSR, Next SSR)
  are byte-identical for canonical/alternates/sitemap URLs.

### Forbidden states

- Emitting a family in one output that another output for the same page does not resolve
  (canonical 404s while sitemap 200s, or vice versa).
- Locale-less public hrefs for targets that carry locales, or duplicate hrefs across hreflang entries.
- Host-specific re-translation of family (sitemap-only adaptation) after the adapter migration completes.
- A new owner registering targets without declaring its family in `routeOwnership`.

## Non-goals

- Redesigning URL shapes (slug normalization, query vs path parameters) — owned by the owner ADRs.
- SEO ranking heuristics, sitemap budgets, and index-repair policy.
- The blog publication timestamp/featured-image parts of the blog ADR, which this ADR extends but does not touch.
- Routing of authenticated/non-public surfaces.

## Data, transaction, and concurrency boundary

Not applicable to persisted state: this decision constrains derived read outputs only. Route storage
stays with its current owners (owner-routes ADR governs storage and writers). No new tables, no
transaction coupling; fixture JSON is configuration and changes follow the normal review path.

## Context dimensions

- **Tenant:** the ownership matrix and route families are tenant-independent; tenant affects only
  which modules are enabled.
- **Locale:** first-class — every emitted href is qualified with the href's locale; `x-default`
  resolves through `settings.x_default_locale` as today.
- **Channel:** forum visibility/channel restrictions gate whether a target appears at all; they do
  not change the family.
- **Principal/auth, policy, trace, timezone, currency:** not applicable (public output contract).

## Events and projections

No new events. Existing redirect transitions (`SeoRedirectUpserted/Disabled`) and sitemap/index
delivery rows remain projections of their current writers. Because outputs are derived, the
reconciliation rule is: any stored redirect or sitemap artifact whose URL does not resolve on the
serving host is stale derived state and must be rebuilt (the existing `public_sitemap_requires_refresh`
live-rebuild path already covers owner-visibility drift).

## Failure semantics

- If the ownership declaration is missing for a registered target, the adapter fails closed:
  the page context renders without canonical/alternates rather than guessing a family (missing
  noindex-class behavior beats emitting a 404 href).
- If a host cannot serve a declared family, the host build/verification fails (fixture contract),
  not the public render.
- Transport parity failures surface through the existing FBA/parity verifiers as build-time errors.

## Migration and cutover

1. **Host routes first:** add Next `/{locale}/modules/blog|product|pages` route handlers (or
   equivalent rewrites) so the already-emitted canonical family resolves; extend
   `routeOwnership.nextSmokeRoute` coverage accordingly.
2. **Locale-qualify outputs:** fix owner alternates (blog identical hrefs, product missing locale
   prefix) and taxonomy canonical to the declared family.
3. **Unify the adapter:** remove sitemap-only translation once host routes land; every output reads
   the same adapter result (head, hreflang, redirect target, sitemap).
4. Update `crates/modules/rustok-seo/README.md` and the audit report status; keep
   `2026-10-08-blog-post-url-and-publication-contract.md` relations intact (this ADR extends, does
   not supersede, the blog route decision — the route shape is preserved and only locale qualification
   and emission rules are added).

Zero-legacy: no dual-emission period where one output translates and another does not; the
sitemap-only translator is deleted in the same change that flips the adapter.

## Alternatives considered

- **Public native routes become canonical everywhere (`/blog/:slug`, `/products/:slug`), module
  routes become 308 aliases.** Rejected: it would partially overturn two Accepted ADRs, requires
  inventing a public family for pages (none exists), churns fixtures and content-orchestration
  writers, and forum already proves native routes work — but blog/product canonical shapes are
  deliberate Accepted decisions.
- **Per-host adaptation (keep head raw, adapt per consumer).** Rejected: it is exactly today's bug —
  N adapters drift; sitemap and head already disagree.
- **Owner decides per output.** Rejected: no single page-level invariant, diagnostics cannot verify.

## Verification

- Extended `scripts/verify/verify-seo-fba.mjs` and runtime-parity fixtures: for each owner/target
  fixture, canonical, every hreflang href, and the sitemap entry resolve HTTP 200 on **both** hosts
  and are byte-equal across GraphQL/REST/Leptos/Next transports.
- New invariant check: hreflang cluster hrefs are unique per locale (catches the blog degenerate cluster).
- Host route-ownership check: every `routePatterns` canonical family entry has a serving route on
  each storefront (Next smoke routes extended to module families).
- Existing static verifiers (`verify-seo-admin-boundary`, `verify-seo-fba`, forum/product seam
  verifiers) must stay green; Rust-side tests added with the implementation.

## Consequences

- Next must add module-family routes (or rewrites) — new host work tracked under the audit's
  remediation plan (replaces the "one shared `withSeoRedirect` hook" framing for canonical parity;
  redirect application parity P2-13 remains separate host work).
- Owner providers get a small, mechanical alternates/canonical change; no storage migrations.
- Sitemap stops translating families — URL counts on Next change from public paths to canonical
  family paths (search engines re-observe; both resolve during and after cutover because host routes
  land first).
- Diagnostics (P2-5 follow-up) can verify one invariant ("one href per locale per page") instead of
  per-consumer heuristics.
- Until acceptance and implementation, P1-2 stays `NEEDS ADR → implementation blocked` in the audit
  report; this file is `Proposed` and not yet authoritative.

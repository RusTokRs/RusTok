# @rustok/seo-frontend API Specification & Contract

## 1. Package Overview & Role
`@rustok/seo-frontend` (`apps/next-frontend/packages/rustok-seo`) is the modular Next.js storefront package for RusToK SEO integration. It manages typed metadata synthesis, structured data JSON-LD serialization, sitemap and robots resolution, HTTP redirect application, and dual-transport (`REST` first with `GraphQL` fallback) data fetching for `SeoPageContext`.

- **Package Name**: `@rustok/seo-frontend`
- **Module Directory**: `apps/next-frontend/packages/rustok-seo`
- **Type**: ESM Package (`"type": "module"`)
- **Key Invariants**:
  - Deterministic route rewriting across all standard modules (`/products/*`, `/blog/*`, `/pages/*`, `/modules/*`).
  - Strict XSS and HTML character sanitization (`\u003c`, `\u003e`, `\u0026`, `\u2028`, `\u2029`) in structured data JSON-LD scripts.
  - Zero domain logic in presentation adapters; pure functional metadata construction.
  - Parity with Leptos storefront (`crates/modules/rustok-seo/storefront`) and backend contracts (`crates/modules/rustok-seo`).

---

## 2. Exported Symbols & Contracts

### Pure Helpers & Utility Functions (`src/utils.ts` & `src/site.ts`)
- `buildDeterministicSeoRoute(input: ResolveSeoRouteInput): string`: Normalizes and canonicalizes route targets and query parameters into deterministic internal SEO query paths.
- `serializeStructuredData(payload: unknown): string | null`: Safely serializes arbitrary structured data blocks to JSON with XSS character escapes.
- `normalizeRobots(robots?: SeoRobots | null): Metadata["robots"]`: Converts backend `SeoRobots` contracts into Next.js App Router metadata robots objects.
- `buildVerification(verification?: SeoVerification | null): Metadata["verification"]`: Formats search engine verification codes (Google, Yandex, Yahoo, custom tags).
- `getSiteUrl(): string`: Resolves base storefront URL from environment variables (`NEXT_PUBLIC_SITE_URL`, `NEXT_PUBLIC_STOREFRONT_URL`) with fallback.
- `localizedPath(locale: string, path?: string): string`: Computes localized paths while avoiding duplicate locale prefixes.
- `resolveLocale(value?: string | null): string`: Resolves and normalizes locale identifiers against supported platform locales (`["en", "ru"]`).

### Metadata Generators (`src/metadata.ts`)
- `buildSeoMetadata(options?: BuildSeoMetadataOptions): Metadata`: Constructs complete Next.js `Metadata` objects containing metadataBase, title, description, alternates, robots, openGraph, twitter, and verification.
- `buildAlternates(locale: string, canonicalUrl: string, alternates?: SeoAlternateLink[] | null): Metadata["alternates"]`: Generates hreflang alternate link maps with `x-default` handling.
- `buildOpenGraph(locale: string, canonicalUrl: string, openGraph?: SeoOpenGraph | null, ...)`: Generates OpenGraph metadata.
- `buildTwitter(twitter?: SeoTwitterCard | null, ...)`: Generates Twitter card metadata.
- `buildSeoStructuredDataScripts(context?: SeoPageContext | null): SeoStructuredDataScript[]`: Extracts JSON-LD scripts from `document.structuredDataBlocks`.

### Transports & Runtime (`src/transport.ts` & `src/runtime.ts`)
- `fetchSeoPageContext(options: SeoTransportOptions): Promise<SeoTransportResult>`: Executes REST GET `/api/seo/page-context`, falling back to GraphQL `seoPageContext` query on non-terminal errors.
- `resolveSeoPageContextForRoute(input: { locale?, route?, routeSegment?, query? }): Promise<SeoPageContextResolution>`: Cached per-request SEO resolution that applies HTTP redirects automatically via `applySeoRouteRedirect`.
- `resolveRobotsMetadata(): Promise<MetadataRoute.Robots>`: Fetches and parses `/robots.txt` directives into Next.js `MetadataRoute.Robots`.
- `resolveSitemapMetadata(): Promise<MetadataRoute.Sitemap>`: Fetches and parses `/sitemap.xml` entries into Next.js `MetadataRoute.Sitemap`.
- `SeoTransportError`: Typed transport error carrying `code: SeoTransportErrorCode`.

---

## 3. Architecture & FFA Placement
- Follows **Fluid Frontend Architecture (FFA)**:
  - Framework-agnostic pure logic lives in `utils.ts` and `site.ts`.
  - Next.js-specific bindings live in `metadata.ts` (`next/types`) and `runtime.ts` (`next/navigation`).
  - Storefront pages (`apps/next-frontend/src/app/**`) consume SEO metadata and page contexts cleanly without direct backend coupling.

---

## 4. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - `buildDeterministicSeoRoute`: trims whitespace, guarantees leading slash, filters out empty query params, and strips `lang` parameter to avoid duplicate locale state.
   - `serializeStructuredData`: guards against `null`, `undefined`, and non-object values before serialization.
   - `normalizeRobots`: validates `maxImagePreview` enum against permitted Next.js values (`"none"`, `"standard"`, `"large"`), dropping unsupported strings.
   - `buildVerification`: strips blank strings and filters out empty name/value pairs in custom verification tags.
2. **Branch Completeness & Anti-Swallowing**:
   - `fetchSeoPageContextRest`: parses error envelopes and converts HTTP/GraphQL error structures into typed `SeoTransportError` instances without silent swallows.
   - `fetchSeoPageContextGraphql`: validates `payload.errors` and throws explicit `SeoTransportError` with accurate error codes.
   - Terminal errors (`BAD_USER_INPUT`) are never masked by fake fallbacks.
3. **State Transition & Fallbacks**:
   - Graceful fallback: when SEO backend module is disabled or unreachable, runtime yields `source: "fallback_static"` with typed reason (`module_disabled`, `transport_failure`, `not_found`, `permission_denied`).
   - Redirect handling: `applySeoRouteRedirect` enforces HTTP redirects (301/308 permanent or 307/302 temporary) through `next/navigation`.
4. **Side-Effect Completeness**:
   - Zero side-effects in metadata builders; deterministic pure output for identical inputs.
5. **Algorithmic Correctness**:
   - Path localization correctly handles supported locale swapping without producing doubled prefixes like `/en/en/...`.
   - Robots directive parser accumulates rules per user agent, handles comments, and respects crawl delay integers.
   - XML location entity decoder correctly unescapes `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`.
6. **Order & Concurrency**:
   - Route resolution is wrapped with React `cache` for per-request deduplication across layout and page server components.

---

## 5. Security & Anti-Defect Guarantees
- **XSS Prevention in JSON-LD**: All structured data payload strings have angle brackets `< >`, ampersands `&`, and line/paragraph separators `\u2028 \u2029` unicode-escaped before script injection.
- **SSRF Prevention in Document Fetching**: `fetchSeoTextDocument` strictly verifies target origin against `allowedOrigins` allowlist before issuing HTTP requests.
- **Contract Unit Tests**: Verified by `packages/rustok-seo/src/seo-frontend.test.mjs` (7 tests, 100% pass) and `apps/next-frontend/scripts/verify-seo-runtime-fixtures.mjs` (10 D8 gates, 4 fallback cases, 4 route rows, 2 smoke routes).


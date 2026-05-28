# План реализации `rustok-seo`

Статус: SEO Suite v1 собран как optional platform module. Текущий product track движется от foundation к Amasty-class SEO suite через templates first, затем bulk remediation, diagnostics, schema breadth, linking и external integrations.

## Execution checkpoint

- Current phase: phase_c2_execution
- Last checkpoint: C1 закрыт полностью: sitemap submission orchestration переведён на telemetry-friendly per-endpoint aggregation (`success/failure/timeout/invalid_endpoint`) с bounded partial-failure summary (`max_errors`, `max_timeout_details`) и deterministic ordering по endpoint без изменения public `SeoSitemapStatusRecord`; regression matrix для adapter path покрывает all-success, partial-failure, invalid-endpoint-skip и timeout/failure truncation.
- Next step: Продолжать C2 (cross-linking foundation) от текущего read-only baseline: расширять suggestion heuristics, remediation UX и owner-module integration без автоматических HTML mutation.
- Open blockers: Для полноценных provider-specific adapters (Google Indexing API и др.) нужен отдельный tenant-secret contract и policy для secret rotation; для production-grade cross-link relevance понадобятся owner-module signals beyond bulk summaries (вне текущего C2 foundation scope).
- Hand-off notes for next agent: C1 verify/doc gates уже синхронизированы; в C2 держать read-only contract (`seoCrossLinkSuggestions` + `/api/seo/cross-link-suggestions`) и diagnostics `cross_link_gap` aggregates, не добавляя auto-link writer до отдельного quality track.
- Last updated at (UTC): 2026-05-28T21:45:00Z

## FFA/FBA status block

- FFA status: `in_progress`
- FBA status: `in_progress`
- Last verification evidence:
  - `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
  - `cargo test -p rustok-seo --lib sitemaps`
  - `cargo check -p rustok-seo-admin --features ssr --config profile.dev.debug=0`
  - `cargo check -p rustok-server --lib --config profile.dev.debug=0`
  - `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- Scope note: module-owned UI остаётся infrastructure control-plane (`rustok-seo-admin` + owner-side SEO panels в `pages/product/blog/forum`); transport boundary развивается через GraphQL + REST `/api/seo/page-context` и read-only cross-link contract (`seoCrossLinkSuggestions` + `/api/seo/cross-link-suggestions`).

## Область работ

- держать `rustok-seo` единым tenant-aware SEO runtime вместо набора разрозненных SEO модулей;
- синхронизировать metadata precedence, redirects, sitemap/robots и storefront SEO contract между server и frontend hosts;
- оставить entity SEO authoring в owner modules, а `rustok-seo-admin` использовать только как cross-cutting infrastructure control-plane;
- не допускать raw HTML/JSON template context, raw schema blobs и silent host-local target mappings;
- строить merchant-facing automation поверх typed target descriptors из `rustok-seo-targets`.

## Текущее состояние

- module bootstrap, manifest wiring, migrations, permissions и local docs подключены;
- core storage использует `meta` / `meta_translations`, `seo_redirects`, `seo_revisions`, `seo_sitemap_jobs`, `seo_sitemap_files`, `seo_bulk_jobs`, `seo_bulk_job_items`, `seo_bulk_job_artifacts`;
- locale columns для SEO-related tables расширены до `VARCHAR(32)`, rollback остаётся forward-only и не сужает locale;
- `SeoModuleSettings` уже включает typed `sitemap_submission_endpoints` с server-side normalization
  (`http/https`, trim, dedupe, strip fragment) как foundation для внешних sitemap ping adapters;
- storefront SEO read-side живёт на permanent contract `SeoPageContext = route + document`;
- Rust-side SSR head rendering вынесен в `rustok-seo-render`;
- `rustok-seo-admin` разбит на `lib/component/model/api/i18n/sections` и больше не содержит central entity metadata editor;
- owner-side SEO panels встроены в `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- target extensibility идёт через `rustok-seo-targets` и runtime registration providers;
- tenant templates и diagnostics уже являются first-class read/control-plane слоями; diagnostics покрывает issue aggregates, canonical redirect chains/loops, hreflang gaps и `cross_link_gap` remediation hints;
- read-only cross-link contract добавлен как foundation surface (`seoCrossLinkSuggestions` + `/api/seo/cross-link-suggestions`) с tenant/RBAC parity;
- `SeoDocument.structured_data_blocks` больше не является raw JSON passthrough: JSON-LD нормализуется в typed schema blocks с `schema_kind`, `schema_type`, legacy `kind`, `source` и payload.

## Итог последней exploration-сессии

- baseline runtime и control-plane для templates/bulk/diagnostics подтверждён как завершённый;
- C1 закрыт: sitemap submit имеет provider seam + telemetry-friendly per-endpoint aggregation и deterministic bounded partial-failure summary;
- C2 foundation закрыт: read-only cross-link suggestions доступны через GraphQL/REST, diagnostics включает `cross_link_gap` issue code и remediation entrypoint в текущем SEO control-plane;
- следующая крупная итерация Phase C — C3 (image SEO hooks), дополнительные SEO surface-расширения для Next/storefront не должны опережать реальное появление route ownership в host-приложениях.

## Этапы

### Закрыто

#### Core runtime

- [x] Module bootstrap и manifest-driven wiring.
- [x] Canonical target contract через `SeoTargetSlug` и registry-backed providers.
- [x] Metadata precedence: explicit SEO > template-generated SEO > domain/entity fallback.
- [x] Locale storage widening до `VARCHAR(32)`.
- [x] Internal `SeoService` split на `meta`, `routing`, `redirects`, `sitemaps`, `robots`, `targets`, `templates`, `diagnostics`, `bulk`.

#### Public surfaces

- [x] GraphQL contract для metadata, redirects, sitemap lifecycle, target registry и diagnostics.
- [x] HTTP endpoints `/robots.txt`, `/sitemap.xml`, `/sitemaps/{name}`.
- [x] Headless REST read path `GET /api/seo/page-context?route=...`.
- [x] Forum topic SEO resolution учитывает host-provided request channel slug.
- [x] Leptos `#[server]` functions для module-owned admin flows.
- [x] Nested storefront contract `route + document` с typed robots/OG/Twitter/verification/JSON-LD blocks.
- [x] Admin route использует URL-owned `tab` query key.

#### Templates first

- [x] `SeoModuleSettings` держит `template_defaults` и `template_overrides`.
- [x] `rustok-seo-targets` отдаёт typed `template_fields`.
- [x] Owner modules не рендерят templates сами и не передают сырой HTML/JSON в template runtime.
- [x] `SeoPageContext.document.effective_state` и `seoMeta.effective_state` показывают `explicit` / `generated` / `fallback`.
- [x] `rustok-seo-admin` имеет defaults/template control-plane.

#### Bulk remediation

- [x] Bulk editor работает по одному `SeoTargetSlug` и одному locale на job.
- [x] Async apply/export/import flow идёт через `seo_bulk_jobs`, `seo_bulk_job_items`, `seo_bulk_job_artifacts`.
- [x] Bulk source filter различает `explicit`, `generated`, `fallback`, `any`.
- [x] Apply mode contract реализован: `preview_only`, `apply_missing_only`, `overwrite_generated_only`, `force_overwrite_explicit`.
- [x] `preview_only` создаёт preview artifact без записи SEO records.
- [x] `apply_missing_only` не перетирает explicit SEO.
- [x] `overwrite_generated_only` пишет только targets с текущим source `generated`.
- [x] `force_overwrite_explicit` требует явный patch delta.

#### Diagnostics

- [x] `seoDiagnostics` возвращает readiness score, issue list и source counts.
- [x] `seoDiagnostics` возвращает counts by issue code и target kind для admin filters/remediation entrypoints.
- [x] Diagnostics ловит canonical redirect targets/chains/loops.
- [x] Diagnostics ловит missing hreflang alternates и missing `x-default` для localized targets.
- [x] Diagnostics различает отсутствующие typed schema blocks и present-but-unknown schema.org type.
- [x] Admin diagnostics pane показывает tenant SEO health без переноса entity editors в SEO module.

#### Rich snippets foundation

- [x] `SeoSchemaBlockKind` фиксирует canonical typed kinds для Product, Offer, AggregateRating, BreadcrumbList, ItemList, Organization, LocalBusiness, WebSite/SearchAction, Article/BlogPosting, FAQ/HowTo, media objects и forum/discussion shapes.
- [x] `SeoStructuredDataBlock` отдаёт `schema_kind`, `schema_type`, legacy `kind`, `source` и JSON-LD payload без изменения storage schema.
- [x] JSON-LD `@graph` разворачивается в отдельные typed blocks с наследованием `@context`.
- [x] `rustok-seo-render`, Leptos storefront GraphQL/server-function contract и Next shared SEO type знают о typed schema-block metadata.
- [x] Explicit `structured_data` writes через metadata и bulk paths валидируют JSON-LD shape: object/array/`@graph` минимум с одним непустым `@type`; будущие schema.org types остаются допустимыми как `other`.
- [x] Built-in owner providers (`pages/product/blog/forum`) используют `rustok-seo-targets::schema` builders для fallback JSON-LD вместо hand-rolled raw `json!` blobs.

### Следующий scope

#### Phase A — typed schema authoring parity

- [x] Нарастить schema builders до Product Offer/Review, BreadcrumbList, ItemList, Organization/LocalBusiness, FAQ/HowTo и расширенных forum-specific schema.
- [x] Добавить typed schema input contract в `rustok-seo-admin-support`, чтобы owner-module panels писали schema blocks без raw JSON textarea.
- [x] Переключить owner-side SEO panels (`pages/product/blog/forum`) на typed schema input с сохранением GraphQL parity.
- [x] Зафиксировать server-side validation matrix для typed schema input: обязательные поля, unknown `@type` fallback в `other`, deterministic normalization.

#### Phase B — operator UX и remediation

- [x] Rich-snippet preview/validation UI в owner-module panels и diagnostics remediation.
- [x] Добавить diagnostics issue codes для schema completeness (missing required fields, invalid array/object shape, unsupported source payload).
- [x] Добавить bulk-safe remediation actions для schema issues без implicit overwrite explicit SEO.

#### Phase C — indexing и linking automation

- [x] **Iteration C1 — external submission adapters (runtime seam + hardening)**
  - [x] C1.0 Зафиксировать runtime interface `submit_sitemap_index` (trait/adapter seam) и default HTTP adapter wiring без breaking changes в существующем orchestrator flow.
  - [x] Вынести текущий sitemap submit flow в typed adapter contract (`submit_sitemap_index`) с default HTTP adapter поверх уже существующих `sitemap_submission_endpoints`.
  - [x] C1.1 Ввести telemetry-friendly aggregation model (per-endpoint status + bounded error summary) и адаптировать внутренний статус sitemap job без изменения public `SeoSitemapStatusRecord`.
  - [x] Добавить per-endpoint result aggregation (success/failure count + bounded error summary) без изменения существующего `SeoSitemapStatusRecord` public shape.
  - [x] C1.2 Добавить regression test matrix для endpoint fan-out и ограничить объём ошибок/timeout details deterministic truncation-правилом.
  - [x] Покрыть adapter path regression tests: all-success, partial-failure, invalid endpoint skip, timeout/failure truncation.
  - [x] C1.3 Обновить docs/verification evidence для sitemap submit orchestration (что именно считается pass/fail по partial failures).
  - Tactical rollout (выполнено):
    1. Добавлен internal aggregation DTO + mapping в существующий orchestration flow без изменения public shape `SeoSitemapStatusRecord`.
    2. Зафиксирована bounded truncation policy (`max_errors`, `max_timeout_details`) с deterministic ordering по endpoint.
    3. Добавлены regression tests для fan-out, partial failure, invalid endpoint skip и timeout/failure truncation.
    4. Синхронизированы verification gates и локальные docs (`README.md`, `docs/README.md`, этот план).
  - Проверка инкремента:
    - `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
    - `cargo test -p rustok-seo --lib sitemaps`

- [x] **Iteration C2 — cross-linking foundation (read-only suggestions first)**
  - [x] Добавить typed cross-link suggestions read model (target, anchor hint, destination route, confidence/source), не выполняя автоматических HTML mutation.
  - [x] Включить cross-link gaps в diagnostics (issue codes + aggregates) и дать remediation entrypoint через существующий SEO control-plane.
  - [x] Добавить GraphQL/REST read contract для suggestions с tenant/RBAC guard parity.
  - Проверка инкремента:
    - `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
    - `cargo check -p rustok-seo-admin --features ssr --config profile.dev.debug=0`
    - `cargo check -p rustok-server --lib --config profile.dev.debug=0`

- [ ] **Iteration C3 — image SEO hooks через `rustok-media`**
  - [ ] Зафиксировать module boundary contract: `rustok-media` отдаёт typed image descriptors (url/alt/size/mime), `rustok-seo` только потребляет их для OG/Twitter/schema fallback.
  - [ ] Обновить built-in owner providers (`pages/product/blog/forum`) для заполнения image-aware template/schema fields без raw blob glue.
  - [ ] Добавить diagnostics checks для missing image alt/size в SEO-critical targets.
  - Проверка инкремента:
    - `cargo check -p rustok-media --tests --config profile.dev.debug=0`
    - `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
    - `cargo check -p rustok-storefront --config profile.dev.debug=0`

- [ ] Расширять Next route coverage только вместе с появлением реальных storefront routes и после фиксации C1–C3 baseline.


## Осталось сделать (оценка на 2026-05-28)

- **Phase C — indexing и linking automation**: 1/3 итерации в статусе open (`C3`), `C1` и `C2` закрыты.
- **Незавершённые checklist-пункты в Phase C**: **4**
  - C3: 3 пункта
  - Next coverage guardrail (расширение Next routes только после C1–C3): 1 пункт
- **Quality backlog**: 0 open пунктов (tests/docs и verification gates синхронизированы в рамках C1.3/C2 foundation).
- **Итого open пунктов в документе**: **4**.

Приоритет исполнения: завершить C3 (image SEO hooks через `rustok-media`), после чего открывать расширение Next route coverage.

## Проверка

- `cargo xtask module validate seo`
- `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin --features ssr --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- `cargo check -p rustok-admin --lib --config profile.dev.debug=0`
- `cargo check -p rustok-storefront --config profile.dev.debug=0`
- `cargo check -p rustok-server --lib --config profile.dev.debug=0`

## Правила обновления

1. При изменении SEO runtime contract сначала обновлять этот файл.
2. При изменении public/storefront surfaces синхронизировать root `README.md`, local `docs/README.md` и host docs.
3. При изменении module wiring, permissions или UI classification синхронизировать `rustok-module.toml`, `modules.toml` и central docs.
4. При изменении multilingual fallback semantics синхронизировать SEO docs с `docs/architecture/i18n.md` и storefront host docs.


## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [x] Проверить полноту и актуальность `README.md` и локальных docs.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля (перенесено в C1.3 tactical track).

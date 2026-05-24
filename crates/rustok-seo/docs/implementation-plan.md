# План реализации `rustok-seo`

Статус: SEO Suite v1 собран как optional platform module. Текущий product track движется от foundation к Amasty-class SEO suite через templates first, затем bulk remediation, diagnostics, schema breadth, linking и external integrations.

## Execution checkpoint

- Current phase: phase_c1_execution_prep
- Last checkpoint: Выполнен plan-sync с фактическим кодом `rustok-seo`: подтверждены typed schema input/write paths, diagnostics remediation flow и runtime foundation для sitemap submission endpoints (`sitemap_submission_endpoints` + bounded best-effort submit в `generate_sitemaps`).
- Next step: Реализовать Iteration C1 — зафиксировать typed adapter seam `submit_sitemap_index` + endpoint aggregation и закрыть regression coverage для success/failure endpoint fan-out.
- Open blockers: Для полноценного Google Indexing API/поисковых провайдеров нужен отдельный tenant-secret contract (вне текущего scope C1).
- Hand-off notes for next agent: Не расширять C1 до cross-linking/media; сначала зафиксировать adapter seam + tests, затем переходить к C2/C3.
- Last updated at (UTC): 2026-05-24T17:07:32Z

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
- tenant templates и diagnostics уже являются first-class read/control-plane слоями; diagnostics покрывает issue aggregates, canonical redirect chains/loops и hreflang gaps;
- `SeoDocument.structured_data_blocks` больше не является raw JSON passthrough: JSON-LD нормализуется в typed schema blocks с `schema_kind`, `schema_type`, legacy `kind`, `source` и payload.

## Итог последней exploration-сессии

- baseline runtime и control-plane для templates/bulk/diagnostics подтверждён как завершённый;
- Phase C уже имеет production foundation для sitemap submit, но пока без явного provider seam и без расширенной telemetry/analytics детализации;
- cross-linking и image SEO остаются следующими крупными инкрементами, требующими отдельного typed read contract и diagnostics coverage;
- дополнительные SEO surface-расширения для Next/storefront не должны опережать реальное появление route ownership в host-приложениях.

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

- [ ] **Iteration C1 — external submission adapters (runtime seam + hardening)**
  - [ ] Вынести текущий sitemap submit flow в typed adapter contract (`submit_sitemap_index`) с default HTTP adapter поверх уже существующих `sitemap_submission_endpoints`.
  - [ ] Добавить per-endpoint result aggregation (success/failure count + bounded error summary) без изменения существующего `SeoSitemapStatusRecord` public shape.
  - [ ] Покрыть adapter path regression tests: all-success, partial-failure, invalid endpoint skip, timeout/failure truncation.
  - Проверка инкремента:
    - `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
    - `cargo test -p rustok-seo --lib sitemaps`

- [ ] **Iteration C2 — cross-linking foundation (read-only suggestions first)**
  - [ ] Добавить typed cross-link suggestions read model (target, anchor hint, destination route, confidence/source), не выполняя автоматических HTML mutation.
  - [ ] Включить cross-link gaps в diagnostics (issue codes + aggregates) и дать remediation entrypoint через существующий SEO control-plane.
  - [ ] Добавить GraphQL/REST read contract для suggestions с tenant/RBAC guard parity.
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


## Осталось сделать (оценка на 2026-05-24)

- **Phase C — indexing и linking automation**: 3/3 итерации в статусе open (`C1`, `C2`, `C3`).
- **Незавершённые checklist-пункты в Phase C**: **10**
  - C1: 3 пункта
  - C2: 3 пункта
  - C3: 3 пункта
  - Next coverage guardrail (расширение Next routes только после C1–C3): 1 пункт
- **Quality backlog**: 3 open пункта (tests/docs/verification gates).
- **Итого open пунктов в документе**: **13** (Phase C + Quality backlog).

Приоритет исполнения: сначала C1 (adapter seam + tests), затем C2 (cross-link suggestions + diagnostics), затем C3 (image SEO hooks через `rustok-media`).

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

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.

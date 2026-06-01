# План реализации `rustok-seo`

Статус: SEO Suite v1 собран как optional platform module. Phase A–C (templates/bulk/diagnostics/schema/cross-link/image boundary) закрыты по baseline. Следующий execution wave — **Phase D: Productionization & Integration Parity**.

## Execution checkpoint

- Current phase: `phase_d4_rest_parity_hardening`
- Last checkpoint: Закрыт Batch D2: publish path пишет `seo_event_deliveries`, связывает delivery с outbox envelope id (`sys_events.id`) и блокирует duplicate emission по deterministic idempotency key; добавлены integration tests на duplicate guard для bulk terminal events. Параллельно продвинут D4: добавлены REST endpoints `/api/seo/diagnostics`, `/api/seo/sitemaps/status`, `/api/seo/sitemaps/jobs`, `/api/seo/sitemaps/jobs/{job_id}`, `/api/seo/bulk/jobs`, `/api/seo/bulk/jobs/{job_id}` и GraphQL parity fields `seoSitemapJobs`/`seoSitemapJob`.
- Next step: Закрыть D4 error-envelope unification и перейти к D3 SEO->index consumer seam (tenant/kind scoped reindex + retry/DLQ policy).
- Open blockers:
  - Для D3 нужен синхронный contract sign-off между владельцами `rustok-seo`, `rustok-outbox` и `rustok-index`.
- Hand-off notes for next agent:
  - Не обходить boundary `MediaImageDescriptor` и existing `SeoPageContext` contract.
  - REST/GraphQL расширять только additive-изменениями в стабильном `v1`.
  - Для delivery tracker держать invariant: один idempotency key = один фактический state transition.
- Last updated at (UTC): 2026-05-30T12:00:00Z

## FFA/FBA status block

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `docs_boundary`
- Last verification evidence:
  - `cargo xtask module validate seo` *(pass, 2026-05-30)*
  - `cargo check -p rustok-seo --tests --config profile.dev.debug=0` *(pass, 2026-05-30)*
  - `cargo test -p rustok-seo services::events::tests --config profile.dev.debug=0` *(pass, 2026-05-30)*
  - `cargo test -p rustok-events --test canonical_contracts --config profile.dev.debug=0` *(pass, 2026-05-30)*
  - `cargo fmt --all -- --check` *(warning: repository-wide pre-existing rustfmt drift outside this SEO D2 patch remains)*
- Scope note: module-owned UI остаётся infrastructure control-plane (`rustok-seo-admin` + owner-side SEO panels в `pages/product/blog/forum`); transport boundary развивается через GraphQL + REST `/api/seo/page-context`, `/api/seo/cross-link-suggestions` и planned parity expansion в рамках Phase D.

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
- `SeoModuleSettings` включает typed `sitemap_submission_endpoints` с server-side normalization (`http/https`, trim, dedupe, strip fragment);
- storefront SEO read-side живёт на permanent contract `SeoPageContext = route + document`;
- Rust-side SSR head rendering вынесен в `rustok-seo-render`;
- `rustok-seo-admin` разбит на `lib/component/model/api/i18n/sections` и не является universal entity editor;
- owner-side SEO panels встроены в `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- target extensibility идёт через `rustok-seo-targets` и runtime registration providers;
- tenant templates и diagnostics уже first-class read/control-plane слой; diagnostics покрывает issue aggregates, canonical redirect chains/loops, hreflang gaps, `cross_link_gap`, `missing_image_alt`, `missing_image_size`;
- read-only cross-link contract добавлен (`seoCrossLinkSuggestions` + `/api/seo/cross-link-suggestions`) с tenant/RBAC parity;
- `SeoDocument.structured_data_blocks` больше не raw JSON passthrough: JSON-LD нормализуется в typed schema blocks (`schema_kind`, `schema_type`, legacy `kind`, `source`, payload);
- boundary contract C3 закреплён через `rustok-media::MediaImageDescriptor` -> `rustok-seo-targets::SeoTargetImageRecord`;
- **open productionization gaps (Phase D):**
  - D2 закрыт: typed SEO event model и delivery/idempotency tracking live (`seo_event_deliveries` + outbox envelope linkage + duplicate guard);
  - direct SEO -> `rustok-index` consumer seam и retry/DLQ policy не доведены;
  - REST control-plane parity в основном закрыта, но остаётся унификация error envelope между GraphQL/REST;
  - `apps/next-admin` API helper расширен до control-plane read surfaces, но host UI observability/remediation widgets и Next storefront runtime parity остаются открытыми.

## Итог последней exploration-сессии

- baseline runtime и control-plane для templates/bulk/diagnostics подтверждён как завершённый;
- C1 закрыт: sitemap submit имеет adapter seam + telemetry-friendly per-endpoint aggregation;
- C2 закрыт: read-only cross-link suggestions доступны через GraphQL/REST, diagnostics включает `cross_link_gap`;
- C3 закрыт: `rustok-media` ↔ `rustok-seo` image boundary переведён на typed descriptors;
- D2 закрыт: publish path пишет delivery tracker (`seo_event_deliveries`), связывает с outbox envelope id и держит duplicate guard по idempotency key;
- D4 частично закрыт: REST control-plane parity endpoints и GraphQL `seoSitemapJobs`/`seoSitemapJob` live, open только error-envelope unification;
- guardrail по Next route coverage остаётся deferred до появления реального route ownership surface;
- для следующего execution wave приоритет: D4 error envelope -> D3 index seam -> D5 migration/backfill policy.

## Контракт совместимости (Phase D freeze)

### Breaking vs non-breaking

- **Non-breaking (разрешено в `v1`)**
  - additive поля в GraphQL/REST DTO;
  - новые REST endpoints под `/api/seo/*` без изменения существующих response shapes;
  - новые diagnostics issue codes и агрегаты;
  - внутренние migration/table additions без изменения текущих API payload contracts.
- **Breaking (запрещено в текущем wave)**
  - удаление/переименование текущих GraphQL полей и REST endpoint-ов;
  - изменение meaning/semantics существующих enum values и source precedence;
  - изменение shape `SeoPageContext` и `SeoStructuredDataBlock` без отдельного versioned contract.

### Versioning стратегия

- REST/GraphQL идут как стабильный `v1`.
- Все расширения делаются additive.
- Если появится необходимость несовместимого изменения — отдельный `v2` трек с parallel compatibility window.

### Rollout flags (draft)

- `seo_events_enabled` — включает typed SEO event emission.
- `seo_outbox_enabled` — включает outbox relay path для SEO events.
- `seo_index_consumer_enabled` — включает SEO->index consumer адаптер.
- `seo_rest_parity_enabled` — включает новые REST control-plane endpoints.
- `seo_next_runtime_sitemap_enabled` — включает runtime-driven sitemap/robots в Next host.

Все флаги tenant-aware, по умолчанию `false` для безопасного staged rollout.

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

#### Diagnostics

- [x] `seoDiagnostics` возвращает readiness score, issue list и source counts.
- [x] `seoDiagnostics` возвращает counts by issue code и target kind для admin filters/remediation entrypoints.
- [x] Diagnostics ловит canonical redirect targets/chains/loops.
- [x] Diagnostics ловит missing hreflang alternates и missing `x-default` для localized targets.
- [x] Diagnostics различает отсутствующие typed schema blocks и present-but-unknown schema.org type.
- [x] Admin diagnostics pane показывает tenant SEO health без переноса entity editors в SEO module.

#### Rich snippets foundation

- [x] `SeoSchemaBlockKind` фиксирует canonical typed kinds для Product/Offer/Rating/Breadcrumb/ItemList/Organization/LocalBusiness/WebSite/SearchAction/Article/FAQ/HowTo/media/forum shapes.
- [x] `SeoStructuredDataBlock` отдаёт `schema_kind`, `schema_type`, legacy `kind`, `source` и JSON-LD payload без изменения storage schema.
- [x] JSON-LD `@graph` разворачивается в отдельные typed blocks с наследованием `@context`.
- [x] `rustok-seo-render`, Leptos storefront GraphQL/server-function contract и Next shared SEO type знают о typed schema-block metadata.
- [x] Explicit `structured_data` writes валидируют JSON-LD shape: object/array/`@graph` минимум с одним непустым `@type`.
- [x] Built-in owner providers (`pages/product/blog/forum`) используют `rustok-seo-targets::schema` builders для fallback JSON-LD.

### Phase C — indexing и linking automation (закрыто)

- [x] C1 external submission adapters.
- [x] C2 cross-linking foundation (read-only suggestions).
- [x] C3 image SEO hooks через `rustok-media`.
- [ ] Next route coverage расширять только при появлении реальных Next storefront route owners (guardrail остаётся deferred-with-reason).

### Phase D — Productionization & Integration Parity

- [x] **Batch D1 — Contract freeze + scope gate**
  - [x] Зафиксировать Phase D (`D1..D9`) и execution order.
  - [x] Явно выделить breaking/non-breaking policy для GraphQL/REST/DTO.
  - [x] Зафиксировать rollout-флаги для event/outbox/index/API/Next parity.

- [x] **Batch D2 — Backend domain: SEO events + outbox foundation**
  - [x] Ввести typed events для: meta upsert/publish/rollback, redirect upsert/disable, sitemap generated/submitted, bulk completed/partial/failed.
  - [x] Добавить deterministic idempotency key (`tenant_id + target_kind + target_id + revision_or_job_id`) и scope-sensitive keys для terminal bulk states.
  - [x] Интегрировать emission path с `rustok-outbox` без duplicate emission в bulk loops: publish path пишет `seo_event_deliveries`, связывает delivery с outbox envelope id и фиксирует duplicate guard integration tests для bulk terminal events.

- [ ] **Batch D3 — Indexing integration seam (SEO -> rustok-index)**
  - [ ] Добавить consumer/adapter contract для selective invalidate/rebuild index documents.
  - [ ] Добавить tenant/kind-scoped reindex trigger.
  - [ ] Зафиксировать bounded retry + dead-letter policy для indexing failures.

- [ ] **Batch D4 — GraphQL/REST parity completion**
  - [x] Добавить REST parity для diagnostics summary/filtering.
  - [x] Добавить REST для sitemap status/job detail.
  - [x] Добавить REST для bulk jobs list/detail/status (и preview endpoint при необходимости).
  - [ ] Унифицировать error envelope между GraphQL/REST (validation/config/not_found/permission).

- [ ] **Batch D5 — Миграции и backfill**
  - [ ] Добавить schema changes для event/outbox/index tracking.
  - [ ] Подготовить backfill/repair path: initial cursor/high-water mark.
  - [ ] Подготовить optional replay mode для исторических SEO changes.
  - [ ] Зафиксировать forward-only rollback policy.

- [ ] **Batch D6 — Admin integrations (Leptos admin + next-admin)**
  - [ ] Расширить `rustok-seo/admin` observability блоком (events, delivery status, reindex actions, failure drilldown).
  - [ ] Расширить `rustok-seo-admin-support` reusable cards под diagnostics remediation и event status hints.
  - [x] Расширить `apps/next-admin/src/shared/api/seo.ts` до полного control-plane API (REST-first + GraphQL fallback для targets/diagnostics/sitemaps/bulk jobs).

- [ ] **Batch D7 — Storefront + Next frontend integrations**
  - [ ] Довести Rust storefront `SeoPageContext` consume flow до uniform режима (`#[server]` + GraphQL fallback + telemetry).
  - [ ] Перевести `apps/next-frontend` с static `robots.ts`/`sitemap.ts` на runtime-driven SEO данные.
  - [ ] Закрыть текущий guardrail по Next route expansion только при появлении реальных route owners beyond home.

- [ ] **Batch D8 — Verification matrix и quality gates**
  - [ ] Unit: DTO normalization, schema validation, event payload mapping, idempotency keys.
  - [ ] Integration: GraphQL/REST parity, outbox emission, index consumer pipeline, tenant/module gating.
  - [ ] E2E: admin remediation flow, storefront canonical/alternates/robots, Next metadata parity.
  - [ ] Contract tests: parity `SeoPageContext` между Rust storefront и Next adapter.

- [ ] **Batch D9 — Docs / runbooks / DoD**
  - [ ] Обновить `README.md`/`docs/README.md` у `rustok-seo`, `rustok-seo-admin-support`, `rustok-seo-render` и host docs (`storefront`, `next-frontend`).
  - [ ] Добавить runbooks: `SEO event backlog stuck`, `partial indexing failures`, `replay/reindex procedures`.
  - [ ] Зафиксировать Definition of Ready/Done для следующего execution wave.

## Осталось сделать (оценка на 2026-05-30)

- **Phase C**: технически закрыт, guardrail по Next route coverage перенесён в D7 rollout criteria.
- **Phase D**: D1 и D2 закрыты; D4 частично закрыт (REST parity + GraphQL sitemap jobs), D3 и D5–D9 открыты.
- **Незавершённые batch-пункты в Phase D**: **7** (открыты D3, D4, D5, D6, D7, D8, D9).
- **Quality backlog**: закрыть D4 error-envelope unification и получить cross-module evidence по D3 index seam.
- **Итого open batch-пунктов в документе**: **7**.

Приоритет исполнения: D4 (error envelope) -> D3 -> D5 как backend/control-plane foundation, затем D6/D7 host parity, затем D8/D9 verification + docs closeout.

## Проверка

- `cargo xtask module validate seo`
- `cargo check -p rustok-seo --tests --config profile.dev.debug=0`
- `cargo check -p rustok-outbox --tests --config profile.dev.debug=0`
- `cargo check -p rustok-index --tests --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin --features ssr --config profile.dev.debug=0`
- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- `cargo check -p rustok-storefront --config profile.dev.debug=0`
- `cargo check -p rustok-server --lib --config profile.dev.debug=0`
- `npm --prefix apps/next-admin run lint && npm --prefix apps/next-admin run typecheck`
- `npm --prefix apps/next-frontend run lint && npm --prefix apps/next-frontend run typecheck`

## Правила обновления

1. При изменении SEO runtime contract сначала обновлять этот файл.
2. При изменении public/storefront surfaces синхронизировать root `README.md`, local `docs/README.md` и host docs.
3. При изменении module wiring, permissions или UI classification синхронизировать `rustok-module.toml`, `modules.toml` и central docs.
4. При изменении multilingual fallback semantics синхронизировать SEO docs с `docs/architecture/i18n.md` и storefront host docs.
5. Если меняется FFA/FBA status block, в том же изменении обновлять central readiness board `docs/modules/registry.md`.

## Quality backlog

- [ ] Закрыть D4 error-envelope unification и зафиксировать parity evidence GraphQL/REST.
- [ ] Закрыть D8 verification matrix с реальным CI evidence packet.
- [ ] Зафиксировать D9 runbooks и operational remediation playbooks.
- [ ] Обновлять execution checkpoint после каждого batch-инкремента Phase D.

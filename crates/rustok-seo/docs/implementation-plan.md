# План реализации `rustok-seo`

Статус: SEO Suite v1 собран как optional platform module. Phase A–C (templates/bulk/diagnostics/schema/cross-link/image boundary) закрыты по baseline. Phase D перешла в execution wave **D6–D9 (host integrations + verification + runbooks)** после закрытия D5 replay/control-plane foundation.

## Execution checkpoint

- Current phase: `phase_d6_admin_integrations_observability`
- Last checkpoint: Закрыт grouped batch D6.1+D6.3+D6.5 (v1 additive): `rustok-seo-admin` получил index delivery observability pane (summary/cursor timeline/failure drilldown + explicit repair/replay confirmations), Leptos server-function transport для index tracking/replay, DTO расширен `failure_samples`, `apps/next-admin` получил operator route `/dashboard/seo` с replay actions, semantic error handling (`BAD_USER_INPUT`/`PERMISSION_DENIED`/`NOT_FOUND`) и telemetry hooks (`seo-operator-action`).
- Next step: Закрыть D6.2/D6.4 как единый execution пакет — reusable remediation widgets в `rustok-seo-admin-support` + owner-module wiring (`pages/product/blog/forum`) без переноса ownership экранов в central SEO hub.
- Open blockers:
  - Нужен согласованный UX flow replay/remediation между `rustok-seo-admin`, `rustok-seo-admin-support` и owner-module admin surfaces (`pages/product/blog/forum`).
  - Для D7 route parity в `apps/next-frontend` нужен явный route ownership список beyond home route.
- Hand-off notes for next agent:
  - Не обходить boundary `MediaImageDescriptor` и existing `SeoPageContext` contract.
  - REST/GraphQL расширять только additive-изменениями в стабильном `v1`.
  - Для delivery tracker держать invariant: один idempotency key = один фактический state transition.
  - Для replay mode сохранять forward-only semantics (`not_started -> repair_only -> replay_requested -> replaying -> replay_completed`) без backward transitions.
  - Для REST parity fallback в Next admin не возвращаться к blanket `catch {}`: semantic ошибки (`BAD_USER_INPUT`/`PERMISSION_DENIED`) должны пробрасываться в UI.
- Last updated at (UTC): 2026-06-02T16:30:00Z

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
- Scope note: module-owned UI остаётся infrastructure control-plane (`rustok-seo-admin` + owner-side SEO panels в `pages/product/blog/forum`); transport boundary развивается через GraphQL + REST `/api/seo/page-context`, `/api/seo/cross-link-suggestions`, control-plane parity endpoints и унифицированный GraphQL-compatible REST error envelope в рамках Phase D.

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
  - D3 закрыт: SEO->index adapter seam live (`Seo*` events -> `index.reindex_requested`), tenant/kind-scoped triggers и bounded retry/dead-letter tracking в `seo_index_deliveries`;
  - D4 закрыт: REST control-plane parity завершён, включая GraphQL-compatible error envelope для validation/config/not_found/permission сценариев;
  - D5 закрыт: schema/index cursor foundation, repair path и historical replay mode завершены (`run_index_repair_replay` + forward-only cursor replay transitions);
  - `apps/next-admin` API helper расширен до control-plane read/write surfaces (включая index tracking/replay) и error mapping parity, но host UI observability/remediation widgets и Next storefront runtime parity остаются открытыми.

## Итог последней exploration-сессии

- baseline runtime и control-plane для templates/bulk/diagnostics подтверждён как завершённый;
- C1 закрыт: sitemap submit имеет adapter seam + telemetry-friendly per-endpoint aggregation;
- C2 закрыт: read-only cross-link suggestions доступны через GraphQL/REST, diagnostics включает `cross_link_gap`;
- C3 закрыт: `rustok-media` ↔ `rustok-seo` image boundary переведён на typed descriptors;
- D2 закрыт: publish path пишет delivery tracker (`seo_event_deliveries`), связывает с outbox envelope id и держит duplicate guard по idempotency key;
- D3 закрыт: SEO->index seam публикует selective `index.reindex_requested` triggers и пишет retry/DLQ tracking в `seo_index_deliveries` с cursor/high-water state в `seo_index_cursors`;
- D4 закрыт: REST control-plane parity endpoints и GraphQL surfaces дополнены унифицированным error envelope (`errors[].extensions.code`) и Next admin error mapping parity;
- D5 закрыт: operator-triggered replay path и control-plane surface уже live (`runSeoIndexRepairReplay`, `/api/seo/index/repair-replay`), cursor replay mode удерживает forward-only transitions и покрыт targeted tests;
- `apps/next-admin` transport helper уже умеет index tracking/replay (REST-first + GraphQL fallback), значит D6 фокус смещён на UI observability/remediation widgets, а не на transport;
- guardrail по Next route coverage остаётся deferred до появления реального route ownership surface beyond home route;
- для следующего execution wave приоритет: D6 admin/operator integrations -> D7 storefront parity -> D8 verification matrix -> D9 runbooks/docs closeout.

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

- [x] **Batch D3 — Indexing integration seam (SEO -> rustok-index)**
  - [x] Добавить consumer/adapter contract для selective invalidate/rebuild index documents.
  - [x] Добавить tenant/kind-scoped reindex trigger.
  - [x] Зафиксировать bounded retry + dead-letter policy для indexing failures.

- [x] **Batch D4 — GraphQL/REST parity completion**
  - [x] Добавить REST parity для diagnostics summary/filtering.
  - [x] Добавить REST для sitemap status/job detail.
  - [x] Добавить REST для bulk jobs list/detail/status (и preview endpoint при необходимости).
  - [x] Унифицировать error envelope между GraphQL/REST (validation/config/not_found/permission).

- [x] **Batch D5 — Миграции, backfill и replay foundation**
  - [x] Добавить schema changes для event/outbox/index tracking.
  - [x] Подготовить backfill/repair path: initial cursor/high-water mark.
  - [x] Подготовить optional replay mode для исторических SEO changes (`run_index_repair_replay`, `replay_historical`).
  - [x] Зафиксировать forward-only replay policy (cursor state machine + tests).
  - [x] Добавить control-plane transport parity для tracking/replay: GraphQL + REST.

- [ ] **Batch D6 — Admin integrations (Leptos admin + next-admin + owner panels)**
  - [x] **D6.1 `rustok-seo/admin` observability surface**
    - [x] Добавить index delivery summary card (`pending/sent/retry/failed/dead_letter`) с tenant/target filter.
    - [x] Добавить cursor timeline card (`initial/high_water/last_repair/replay timestamps`) с forward-only replay badge.
    - [x] Добавить operator actions: `repair_only` и `repair+historical_replay` с explicit confirmation UX.
    - [x] Добавить failure drilldown (last_error sample + retry counters + dead-letter hints).
  - [ ] **D6.2 `rustok-seo-admin-support` reusable widgets**

    - [ ] Добавить typed remediation mapping (`issue_code -> action`), включая `run_reindex` и `open_bulk_job`.
    - [ ] Добавить общий error/permission/empty-state contract для SEO control-plane виджетов.
  - [x] **D6.3 host wiring (`apps/next-admin`)**
    - [x] Подключить index tracking/replay API в operator UI (используя существующий REST-first helper).
    - [x] Добавить semantic error handling для replay flows (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`).
    - [x] Добавить telemetry hooks (action started/success/failure) для replay/remediation операций.
  - [ ] **D6.4 owner-module wiring (`pages/product/blog/forum`)**

    - [ ] Проверить locale contract: использовать host effective locale, без package-local fallback цепочек.
  - [x] **D6.5 transport/contract hardening**
    - [x] Зафиксировать DTO limits/validation для replay input (`limit 1..500`, `target_type content|product`).
    - [x] Добавить anti-regression checks на idempotency key invariants после operator replay.

- [ ] **Batch D7 — Storefront + Next frontend runtime parity**
  - [ ] **D7.1 Rust storefront uniform consume flow**
    - [ ] Довести `SeoPageContext` consume path до uniform режима (`#[server]` + GraphQL fallback + telemetry labels).
    - [ ] Зафиксировать deterministic fallback order и отказоустойчивость при SEO module disable.
  - [ ] **D7.2 Next runtime SEO migration**
    - [ ] Перевести `apps/next-frontend/src/app/robots.ts` на runtime-driven source (вместо static rules).
    - [ ] Перевести `apps/next-frontend/src/app/sitemap.ts` на runtime-driven SEO data source.
    - [ ] Добавить shared adapter слой для reuse `SeoPageContext` semantics в Next metadata generation.
  - [ ] **D7.3 Route ownership guardrail closure**
    - [ ] Зафиксировать route ownership matrix beyond home route (кто владеет route и target_kind).
    - [ ] Закрыть deferred guardrail только после появления реальных route owners и tests.
  - [ ] **D7.4 Cross-host parity fixtures**
    - [ ] Добавить fixture-set для canonical/hreflang/robots/schema parity Rust vs Next.
    - [ ] Явно зафиксировать допустимые расхождения long-tail metadata.

- [ ] **Batch D8 — Verification matrix и quality gates**
  - [ ] **D8.1 Unit coverage**
    - [ ] DTO normalization и validation bounds (`target_type`, `limit`, locale canonicalization).
    - [ ] Replay mode state transitions (forward-only) и helper функции (`advance_replay_mode`).
    - [ ] Event payload mapping + idempotency keys (meta/revision/redirect/bulk).
  - [ ] **D8.2 Integration coverage (`rustok-seo`)**
    - [ ] GraphQL/REST parity для diagnostics, sitemaps, bulk jobs, index tracking/replay.
    - [ ] Outbox emission + index consumer pipeline + retry/dead-letter replay scenario.
    - [ ] Tenant/module gating parity (`module disabled`, RBAC matrix).
  - [ ] **D8.3 Host integration coverage**
    - [ ] `apps/next-admin` API/UI integration tests для REST-first + GraphQL fallback.
    - [ ] `apps/storefront` SSR SEO smoke tests (canonical/alternates/robots/JSON-LD).
    - [ ] `apps/next-frontend` metadata parity smoke tests после runtime migration.
  - [ ] **D8.4 Contract parity tests**
    - [ ] Parity `SeoPageContext` между Rust storefront renderer и Next adapter на одном fixture set.
    - [ ] Regression tests для structured data blocks (`schema_kind`, `schema_type`, `source`, payload).

- [ ] **Batch D9 — Docs / runbooks / readiness closeout**
  - [ ] **D9.1 Docs sync**
    - [ ] Обновить `README.md`/`docs/README.md` у `rustok-seo`, `rustok-seo-admin-support`, `rustok-seo-render`.
    - [ ] Синхронизировать host docs (`apps/storefront/docs`, `apps/next-frontend/docs`, `apps/next-admin/docs`).
    - [ ] Обновить central docs (`docs/modules/registry.md`, `docs/index.md`) при изменении surface карты.
  - [ ] **D9.2 Runbooks**
    - [ ] `SEO event backlog stuck` (diagnose -> repair -> replay -> verify).
    - [ ] `Partial indexing failures` (retry budget, DLQ handling, escalation).
    - [ ] `Replay/Reindex procedures` (tenant-safe execution + rollback/stop criteria).
  - [ ] **D9.3 Operational readiness / DoD**
    - [ ] Зафиксировать Definition of Ready/Done для следующего execution wave.
    - [ ] Сформировать evidence packet (tests + smoke + observability snapshots + owner sign-off).

## Осталось сделать (оценка на 2026-06-02)

- **Phase C**: технически закрыт, guardrail по Next route coverage перенесён в D7 rollout criteria.
- **Phase D**: D1–D5 закрыты; открыты D6–D9.
- **Незавершённые batch-пункты в Phase D**: **4** (D6, D7, D8, D9).
- **Execution focus**: сначала D6 operator/admin parity, затем D7 runtime parity, после этого D8 verification и D9 docs/runbooks closeout.
- **Итого open batch-пунктов в документе**: **4**.

## Детализированный execution план на текущую итерацию (максимальный scope)

### Iteration package P0 (must-have, backend already ready -> host/operator delivery)

- [ ] P0.1 Закрыть D6.1/D6.2 UI widgets package (`rustok-seo/admin` + `rustok-seo-admin-support`).
- [ ] P0.2 Подключить P0 widgets минимум в один owner module (`pages` как reference), затем распространить на `product/blog/forum`.
- [ ] P0.3 Закрыть Next admin operator flow для replay/remediation поверх уже существующих API helpers.
- [ ] P0.4 Добавить targeted tests: widget states + API error mapping + replay action success/failure paths.

### Iteration package P1 (runtime parity)

- [ ] P1.1 Мигрировать Next `robots.ts`/`sitemap.ts` на runtime SEO source.
- [ ] P1.2 Выравнять Rust storefront и Next metadata adapter по единому fixture set.
- [ ] P1.3 Закрыть route ownership guardrail (документ + tests) для non-home routes.

### Iteration package P2 (verification + operability closeout)

- [ ] P2.1 Прогнать D8 verification matrix и собрать evidence packet.
- [ ] P2.2 Добавить D9 runbooks и operational checklists.
- [ ] P2.3 Обновить central/local docs и checkpoint-блоки смежных SEO crates.

### Cut line (если не влезает весь scope)

1. Минимум для завершения итерации: P0.1 + P0.3 + хотя бы часть P0.4 (API error mapping tests).
2. Если остаётся время: P0.2 и начало P1.1.
3. P1.2/P1.3/P2 допускаются как follow-up, но должны быть заранее занесены в checkpoint следующей итерации.

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

- [ ] Закрыть D6 UI observability/remediation coverage в `rustok-seo-admin` и `rustok-seo-admin-support` с reusable contract tests.
- [ ] Закрыть D7 runtime parity доказательствами Rust storefront vs Next metadata adapter.
- [ ] Закрыть D8 verification matrix с реальным CI evidence packet.
- [ ] Зафиксировать D9 runbooks и operational remediation playbooks.
- [ ] Обновлять execution checkpoint после каждого batch-инкремента Phase D.

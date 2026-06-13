# План реализации `rustok-seo`

Статус: SEO Suite v1 собран как optional platform module. Phase A–C (templates/bulk/diagnostics/schema/cross-link/image boundary) закрыты по baseline. В Phase D закрыт полный D6 execution пакет; активный фокус смещён на **D7–D9 (storefront parity + verification + runbooks)**.

## Execution checkpoint

- Current phase: `phase_d7c_route_ownership_fixture_parity`
- Last checkpoint: D7 Milestones A–C закрыты как fixture/evidence baseline: добавлен `apps/next-frontend/contracts/seo/runtime-parity-fixtures.json` с fallback behavior cases, route ownership matrix, двумя non-home metadata smoke routes и allowlist допустимых long-tail differences; добавлен быстрый verifier без компиляции.
- Next step: перейти к D8 lightweight verification matrix без долгих компиляций: прогнать fixture verifier, Next lint/typecheck по возможности и точечные SEO unit checks.
- Open blockers:
  - Для D8 остаётся получить живой CI/runtime evidence packet против поднятого backend.
  - Для D9 остаётся финализировать runbooks backlog/index/replay.
- Hand-off notes for next agent:
  - Не обходить boundary `MediaImageDescriptor` и existing `SeoPageContext` contract.
  - REST/GraphQL расширять только additive-изменениями в стабильном `v1`.
  - Для delivery tracker держать invariant: один idempotency key = один фактический state transition.
  - Для replay mode сохранять forward-only semantics (`not_started -> repair_only -> replay_requested -> replaying -> replay_completed`) без backward transitions.
  - Для Next runtime adapter сохранять semantic error mapping (`BAD_USER_INPUT` / `PERMISSION_DENIED` / `NOT_FOUND` / transport failures) и не возвращаться к blanket `catch {}`.
- Last updated at (UTC): 2026-06-13T00:00:00Z

## FFA/FBA status block

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Last verification evidence:
  - `cargo fmt --all -- --check` *(pass, 2026-06-07)*
  - `cargo check -p rustok-seo-admin --config profile.dev.debug=0` *(pass, 2026-06-07)*
  - `cargo test -p rustok-seo-admin --lib --config profile.dev.debug=0` *(pass, 2026-06-07; 12 pure-core tests)*
- Scope note: module-owned UI остаётся infrastructure control-plane (`rustok-seo-admin` + owner-side SEO panels в `pages/product/blog/forum`); `rustok-seo-admin` теперь имеет явный `core/transport/ui` FFA split, а transport boundary продолжает развиваться через GraphQL + REST `/api/seo/page-context`, `/api/seo/cross-link-suggestions`, control-plane parity endpoints и унифицированный GraphQL-compatible REST error envelope в рамках Phase D.

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
- `rustok-seo-admin` разбит на FFA-слои `core/transport/ui/leptos/sections/i18n` и не является universal entity editor;
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
  - `apps/next-admin` API helper расширен до control-plane read/write surfaces (включая index tracking/replay) и error mapping parity; owner-side observability/remediation widgets через `rustok-seo-admin-support` и wiring в `pages/product/blog/forum` закрыты, open focus смещён на Next storefront runtime parity.

## Итог последней exploration-сессии

- baseline runtime и control-plane для templates/bulk/diagnostics подтверждён как завершённый;
- C1 закрыт: sitemap submit имеет adapter seam + telemetry-friendly per-endpoint aggregation;
- C2 закрыт: read-only cross-link suggestions доступны через GraphQL/REST, diagnostics включает `cross_link_gap`;
- C3 закрыт: `rustok-media` ↔ `rustok-seo` image boundary переведён на typed descriptors;
- D2 закрыт: publish path пишет delivery tracker (`seo_event_deliveries`), связывает с outbox envelope id и держит duplicate guard по idempotency key;
- D3 закрыт: SEO->index seam публикует selective `index.reindex_requested` triggers и пишет retry/DLQ tracking в `seo_index_deliveries` с cursor/high-water state в `seo_index_cursors`;
- D4 закрыт: REST control-plane parity endpoints и GraphQL surfaces дополнены унифицированным error envelope (`errors[].extensions.code`) и Next admin error mapping parity;
- D5 закрыт: operator-triggered replay path и control-plane surface уже live (`runSeoIndexRepairReplay`, `/api/seo/index/repair-replay`), cursor replay mode удерживает forward-only transitions и покрыт targeted tests;
- D6 закрыт end-to-end: owner-side reusable control-plane widgets + typed remediation mapping + host-locale wiring в `pages/product/blog/forum` + Next admin operator parity;
- guardrail по Next route coverage остаётся deferred до появления реального route ownership surface beyond home route;
- для следующего execution wave приоритет: D7 storefront parity -> D8 verification matrix -> D9 runbooks/docs closeout.

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

- [x] **Batch D6 — Admin integrations (Leptos admin + next-admin + owner panels)**
  - [x] **D6.1 `rustok-seo/admin` observability surface**
    - [x] Добавить index delivery summary card (`pending/sent/retry/failed/dead_letter`) с tenant/target filter.
    - [x] Добавить cursor timeline card (`initial/high_water/last_repair/replay timestamps`) с forward-only replay badge.
    - [x] Добавить operator actions: `repair_only` и `repair+historical_replay` с explicit confirmation UX.
    - [x] Добавить failure drilldown (last_error sample + retry counters + dead-letter hints).
  - [x] **D6.2 `rustok-seo-admin-support` reusable widgets**

    - [x] Добавить typed remediation mapping (`issue_code -> action`), включая `run_reindex` и `open_bulk_job`.
    - [x] Добавить общий error/permission/empty-state contract для SEO control-plane виджетов.
  - [x] **D6.3 host wiring (`apps/next-admin`)**
    - [x] Подключить index tracking/replay API в operator UI (используя существующий REST-first helper).
    - [x] Добавить semantic error handling для replay flows (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`).
    - [x] Добавить telemetry hooks (action started/success/failure) для replay/remediation операций.
  - [x] **D6.4 owner-module wiring (`pages/product/blog/forum`)**

    - [x] Проверить locale contract: использовать host effective locale, без package-local fallback цепочек.
  - [x] **D6.5 transport/contract hardening**
    - [x] Зафиксировать DTO limits/validation для replay input (`limit 1..500`, `target_type content|product`).
    - [x] Добавить anti-regression checks на idempotency key invariants после operator replay.

- [x] **Batch D7 — Storefront + Next frontend runtime parity (regrouped in Milestones A–C)**
  - [x] **Milestone A — Runtime SEO Data Plumbing (D7 foundation, large batch)**
    - [x] A.1 Добавить shared Next runtime adapter: REST-first + GraphQL fallback с typed semantic error mapping (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport failures).
    - [x] A.2 Зафиксировать deterministic locale/route/query normalization policy в Next adapter (`routeSegment -> /modules/*`, `lang` исключается, query keys сортируются).
    - [x] A.3 Унифицировать response->metadata mapping (`canonical/hreflang/robots/OG/Twitter/verification`) и добавить runtime JSON-LD script extraction из `structuredDataBlocks`.
    - [x] A.4 Зафиксировать fallback-behavior evidence (module disabled / not found / permission / transport) на fixture-наборе (`apps/next-frontend/contracts/seo/runtime-parity-fixtures.json`).
  - [x] **Milestone B — End-to-End Next Runtime Migration (D7 cutover, large batch)**
    - [x] B.1 Перевести `apps/next-frontend/src/app/robots.ts` на runtime-driven source с safe static fallback.
    - [x] B.2 Перевести `apps/next-frontend/src/app/sitemap.ts` на runtime-driven source с fallback на host-local static metadata.
    - [x] B.3 Перевести home `generateMetadata` на runtime `SeoPageContext` adapter semantics.
    - [x] B.4 Расширить metadata smoke минимум на два non-home owner route и зафиксировать parity evidence (`product`, `blog`).
  - [x] **Milestone C — Route ownership matrix + cross-host fixture parity (D7 guardrail closure)**
    - [x] C.1 Зафиксировать route ownership matrix: owner module -> route patterns -> `target_kind` (beyond home route).
    - [x] C.2 Добавить единый fixture-set Rust storefront vs Next host.
    - [x] C.3 Документировать explicit allowlist допустимых long-tail metadata differences.

- [ ] **Batch D8 — Verification matrix и quality gates (Milestone D, heavy QA batch)**
  - [ ] D.1 Прогнать unit coverage для normalization/validation/idempotency/replay transitions.
  - [ ] D.2 Прогнать `rustok-seo` integration matrix (GraphQL/REST parity + outbox/index pipeline + tenant/module gating).
  - [ ] D.3 Прогнать host integration matrix (`apps/storefront`, `apps/next-frontend`, `apps/next-admin`) с RBAC/module gating parity.
  - [ ] D.4 Собрать evidence packet (команды, логи, snapshots), закрыть high-severity parity defects.

- [ ] **Batch D9 — Docs / runbooks / readiness closeout (Milestone E, operational batch)**
  - [ ] E.1 Обновить docs в `rustok-seo`, `rustok-seo-admin-support`, `rustok-seo-render`, `apps/storefront`, `apps/next-frontend`, `apps/next-admin`, central docs registry/index.
  - [ ] E.2 Финализировать runbooks: `SEO event backlog stuck`, `Partial indexing failures`, `Replay/Reindex procedures` с rollback/stop criteria.
  - [ ] E.3 Зафиксировать owner sign-off checklist и DoD/DoR для следующего execution wave.

## Осталось сделать (оценка на 2026-06-08)

- **Phase C**: технически закрыт; route ownership guardrail формально ведётся в Milestone C.
- **Phase D**: D1–D6 закрыты; D7–D9 ведутся как крупные Milestones `A–E`.
- **Прогресс по Milestones**:
  - `A` — закрыт fixture evidence baseline;
  - `B` — закрыт runtime cutover + non-home smoke evidence baseline;
  - `C` — закрыт route ownership + fixture parity guardrail;
  - `D`, `E` — не начаты.
- **Execution focus**: D8 lightweight verification без долгих компиляций, затем D9 docs/runbooks closeout.
- **Итого open milestone-пакетов**: **2** (`D..E`).

## Детализированный execution план на текущую итерацию (крупные пакеты)

### Milestone A — Runtime SEO Data Plumbing

- [x] A.1 Shared Next runtime adapter (`REST-first + GraphQL fallback + typed errors`).
- [x] A.2 Deterministic locale/route/query normalization parity со storefront.
- [x] A.3 Unified metadata mapper + JSON-LD extraction helper.
- [x] A.4 Fallback fixtures/evidence: `module_disabled`, `not_found`, `permission_denied`, transport failures.

### Milestone B — End-to-End Next runtime migration

- [x] B.1 Runtime-driven `robots.ts`.
- [x] B.2 Runtime-driven `sitemap.ts`.
- [x] B.3 Home route `generateMetadata` + JSON-LD runtime rendering.
- [x] B.4 Minimum 2 non-home owner routes на runtime metadata adapter + smoke proof.

### Milestone C — Route ownership + cross-host fixtures

- [x] C.1 Route ownership matrix (owner -> patterns -> `target_kind`).
- [x] C.2 Unified fixture set Rust storefront vs Next host.
- [x] C.3 Explicit long-tail diff allowlist.

### Milestone D — Verification matrix execution

- [ ] D.1 Unit/integration/host matrix прогон.
- [ ] D.2 RBAC/module gating parity checks.
- [ ] D.3 Replay/index pipeline regression checks.
- [ ] D.4 Evidence packet + high-severity defect closure.

### Milestone E — Docs / runbooks / readiness closeout

- [ ] E.1 Docs sync (`rustok-seo*`, host docs, central docs).
- [ ] E.2 Operational runbooks (backlog stuck / partial indexing / replay-reindex).
- [ ] E.3 Owner sign-off checklist + DoD/DoR finalization.

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
- `npm --prefix apps/next-frontend run verify:seo-runtime-fixtures`
- `npm --prefix apps/next-frontend run lint && npm --prefix apps/next-frontend run typecheck`

## Правила обновления

1. При изменении SEO runtime contract сначала обновлять этот файл.
2. При изменении public/storefront surfaces синхронизировать root `README.md`, local `docs/README.md` и host docs.
3. При изменении module wiring, permissions или UI classification синхронизировать `rustok-module.toml`, `modules.toml` и central docs.
4. При изменении multilingual fallback semantics синхронизировать SEO docs с `docs/architecture/i18n.md` и storefront host docs.
5. Если меняется FFA/FBA status block, в том же изменении обновлять central readiness board `docs/modules/registry.md`.

## Quality backlog

- [ ] Закрыть Milestone D verification matrix с реальным CI evidence packet.
- [ ] Зафиксировать Milestone E runbooks и operational remediation playbooks.
- [ ] Обновлять execution checkpoint после каждого milestone-инкремента Phase D.

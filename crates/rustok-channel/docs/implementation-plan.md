# План реализации `rustok-channel`

Статус: experimental core capability; `v0 baseline complete`. Текущий фокус —
post-v0 rollout policy lifecycle, runtime integration parity, no-compile executable FBA fallback evidence и закрепление решения по built-in host fast-path.

## Текущее состояние

- План синхронизирован с текущей реализацией policy lifecycle: update/reorder/disable для rules уже присутствуют в domain/service и server transport.
- Rollout-решение зафиксировано: built-in host fast-path остаётся отдельным быстрым слоем между explicit selectors и typed policies, чтобы host-target lookup не деградировал в policy-only mode и сохранял совместимость с существующими каналами; canonical order: `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`.
- Дополнительный focus текущей итерации: стабилизация runtime facts parity (`locale`/`oauth_app_id`) и поддержание deterministic contract в tests/docs.

## Execution checkpoint

- Current phase: semantic_proof_points_guardrail
- Last checkpoint: semantic proof-points slice добавил no-compile guardrail `npm run verify:channel:proof-points`, который source-locks channel-aware интеграцию `rustok-pages`, `rustok-blog`, `rustok-commerce` и `rustok-forum`: host `ChannelContext`/`channel_module_bindings`, metadata `channelSlugs` visibility, commerce cart/pricing channel snapshot, forum topic/reply/SEO channel visibility и документацию proof-point modules.
- Next step: Собрать full Rust runtime contract evidence для `ChannelReadPort` и полный `cargo check`/`cargo test` evidence для `rustok-channel-admin`/server middleware в CI или в сессии без короткого execution limit; до Rust runtime evidence FBA остаётся `in_progress`, но fallback smoke profiles теперь закреплены dedicated no-compile executable verifier-ом, а resolution-order decision — быстрым source verifier-ом.
- Open blockers: по запросу итерации компиляции не запускались; compile evidence отсутствует, поэтому FFA/FBA status остаётся `in_progress`. Compile-free evidence проходит: channel admin boundary verifier, channel FBA verifier со static matrix + no-compile executable runtime fallback smoke, channel resolution contract verifier, channel proof-points verifier и channel boundary fixture suite 13/13.
- Hand-off notes for next agent: Держать вызовы channel admin UI за `transport`, а route-selection policy — в `core` или shared route helpers; не возвращать raw transport calls в `ui/leptos/`.
- Last updated at (UTC): 2026-06-26T00:00:00Z

## FFA/FBA readiness

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - Boundary readiness update: `crates/rustok-channel/contracts/channel-fba-registry.json`, `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` and `crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json` are locked by `npm run verify:channel:fba`; FBA status is `boundary_ready`, while full Rust runtime contract evidence remains the next step before `transport_verified`.
  - `crates/rustok-channel/admin/src/lib.rs` теперь является composition/re-export слоем для module-owned admin surface.
  - Runtime facts parity slice: `apps/server/src/middleware/channel.rs` builds `RequestFacts.locale` from `ResolvedRequestLocale.effective_locale` and `RequestFacts.oauth_app_id` from `AuthContextExtension.client_id`; `ChannelResolutionCacheKey` includes both fields to avoid cross-locale/cross-client policy cache reuse, and source-level middleware tests now cover `LocaleEquals`/`OAuthAppEquals` policy selection from real request extensions.
  - FBA provider slice: `crates/rustok-channel/src/ports.rs` declares `ChannelReadPort` / `channel.read_projection.v1` for channel/default/host-target read projection consumers with shared `rustok_api::PortContext`/`PortError`, tenant-scope preservation, inactive-channel degraded-mode filtering and `PortCallPolicy::read()` deadline semantics; `crates/rustok-channel/contracts/channel-fba-registry.json` plus `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` lock planned contract cases, and `crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json` locks fallback profiles under `npm run verify:channel:fba` через dedicated no-compile executable smoke verifier; Rust runtime execution remains the next step before `transport_verified`.
  - Resolution contract slice: built-in host fast-path остаётся отдельным слоем после header/query selectors и до typed policies; `npm run verify:channel:resolution-contract` фиксирует source order и docs sync для canonical order `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`.
  - Semantic proof-points slice: `npm run verify:channel:proof-points` source-locks `rustok-pages`, `rustok-blog`, `rustok-commerce` и `rustok-forum` как текущие channel-aware proof points: public REST/GraphQL gates используют resolved host `ChannelContext` и `channel_module_bindings`, page/blog publication visibility остаётся за metadata `channelSlugs`, commerce сохраняет channel snapshot в cart/order/pricing flows без второго sales-channel домена, а forum фиксирует topic/reply/SEO filtering через `forum_topic_channel_access` и request channel slug.
  - FBA provider slice: `crates/rustok-channel/src/ports.rs` declares `ChannelReadPort` / `channel.read_projection.v1` for channel/default/host-target read projection consumers with typed `PortContext`/`PortError`, tenant-scope preservation, inactive-channel degraded-mode filtering and read deadline semantics; `crates/rustok-channel/contracts/channel-fba-registry.json` plus `crates/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` lock planned contract cases, and `crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json` locks fallback profiles under `npm run verify:channel:fba` через dedicated no-compile executable smoke verifier; Rust runtime execution remains the next step before `transport_verified`.
  - `crates/rustok-channel/admin/src/core.rs` содержит Leptos-free selection policy для очистки URL-owned channel selection.
  - `ChannelPolicySelectionCleanup` / `channel_policy_selection_cleanup` централизуют trim, policy-set lookup и stale rule cleanup; Leptos route effect больше не владеет этой decision logic.
  - `PolicyRuleFormState` и create/edit builders владеют приоритетом по умолчанию, fallback action channel и predicate-to-form mapping; Leptos применяет подготовленное состояние только к signals.
  - `reorder_policy_rule_ids` владеет проверкой first/last boundary и перестановкой rule IDs; Leptos move-up/move-down handlers только отправляют подготовленный порядок в transport facade.
  - `PolicyRuleFormState::{create_payload,update_payload}` и `policy_rule_active_update_payload` владеют optional-text normalization и transport DTO construction для create/edit/toggle flows.
  - `crates/rustok-channel/admin/src/transport/mod.rs` содержит module-owned transport facade и fallback policy, `native_server_adapter.rs` содержит server-function endpoints, а `rest_adapter.rs` содержит REST fallback; Leptos adapter больше не импортирует pre-FFA модуль `api`.
  - `crates/rustok-channel/admin/src/ui/leptos/` является явным Leptos render adapter directory: `mod.rs` владеет `ChannelAdmin`/shared render glue, а runtime context, policy workbench, policy-set card и channel card изолированы в component files; channel operations вызывают только module-owned transport facade.
  - `scripts/verify/verify-channel-admin-boundary.mjs` закрепляет split без полной Rust-компиляции: обязательную структуру `ui/leptos/`, отсутствие `api.rs`/legacy `transport.rs`, отсутствие raw transport calls в UI, Leptos-free `core`, и разнесение `#[server]`/`reqwest` по adapter-файлам.
  - `scripts/verify/verify-channel-admin-boundary.test.mjs` добавляет fixture-based regression coverage для pass path, legacy `api.rs`, legacy flat `transport.rs`, raw adapter calls из UI, inline policy-selection lookup, Leptos-specific core regression, ошибочных `#[server]` endpoints в facade/REST adapter и raw REST calls вне `rest_adapter.rs`.
  - `npm run verify:ffa:ui:migration` теперь запускает channel admin boundary verifier как часть общего FFA verification pipeline.
- Compile-evidence note (2026-06-20): по запросу текущей итерации компиляции не запускались. Compile-free gates: `npm run verify:channel:admin-boundary`, `npm run verify:channel:fba` (registry + static matrix + no-compile executable fallback smoke), `npm run verify:channel:resolution-contract` (canonical order + built-in host fast-path docs sync), `npm run verify:channel:proof-points` (pages/blog/commerce/forum proof-point source/docs sync) и `node --test scripts/verify/verify-channel-admin-boundary.test.mjs` прошли; `cargo fmt -p rustok-server -- apps/server/src/middleware/channel.rs` применён только как форматирование без компиляции.
- Следующий parity step: собрать full Rust evidence (`cargo check`/`cargo test`) перед переводом строки channel admin в `phase_b_ready`.

## Область работ

- удерживать `rustok-channel` как domain-owned resolution module, а не host middleware bucket;
- синхронизировать channel runtime contract, admin UI и manifest metadata;
- развивать typed resolution policies без возврата к ad-hoc host logic.

## Сводка текущего exploration

- resolver precedence уже закреплён в `crates/rustok-channel/src/resolution.rs`:
  `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`;
- storage и domain слой для policy уже есть (`channel_resolution_policy_sets` +
  `channel_resolution_policy_rules`);
- server transport (`apps/server/src/controllers/channel.rs`) расширяется вместе с policy lifecycle;
- admin UI (`crates/rustok-channel/admin/src/ui/leptos/`) уже покрывает базовые operator flows и
  rollout rule-level lifecycle;
- middleware request facts (`apps/server/src/middleware/channel.rs`) пока передаёт
  `oauth_app_id = None` и `locale = None`, из-за чего часть typed predicates работает
  только в synthetic/tests сценариях.

## Необходимые изменения

### 1) Domain contract (`rustok-channel`)

- добавить DTO для update lifecycle policy set/rule (rename/active-toggle/rule update/reorder);
- расширить `ChannelService` методами:
  - `update_resolution_policy_set(...)`,
  - `update_resolution_rule(...)`,
  - `reorder_resolution_rules(...)` (bulk или single move);
- закрепить partial-update contract для `update_resolution_rule(...)`:
  - `priority/is_active/action_channel_id` optional: отсутствие в payload => поле не меняется;
  - `host_equals/host_suffix/oauth_app_id/surface/locale` optional patch fields:
    отсутствие => без изменений, пустая строка => удалить соответствующий predicate, непустое значение => заменить/установить predicate с обычной валидацией/нормализацией;
- зафиксировать инварианты:
  - tenant ownership для policy set, rule и action channel,
  - deterministic order после reorder (без hidden tie-break drift),
  - inactive rule не участвует в `list_active_resolution_rules`.

### 2) Host transport (`apps/server`)

- расширить REST controller `apps/server/src/controllers/channel.rs` для update/reorder/disable policy flows;
- оставить текущую cache invalidation contract (`invalidate_tenant_channel_cache`) для всех новых write-paths;
- при добавлении новых request payload удерживать shared validation semantics
  (host normalization, locale normalization, surface whitelist).

### 3) Runtime facts и middleware integration

- довести `RequestFacts` в `middleware/channel.rs` до реального runtime:
  - прокидывать `locale` из resolved request locale,
  - прокидывать `oauth_app_id` из auth context (`client_id`);
- при необходимости скорректировать middleware ordering в
  `apps/server/src/services/app_router.rs`, чтобы channel resolver видел нужные extension-данные;
- добавить targeted middleware tests на policy predicates `LocaleEquals` и `OAuthAppEquals`
  в реальном request pipeline, а не только на unit-level resolver.

### 4) Admin package (`rustok-channel/admin`)

- закрыть native-first parity для policy operations в `admin/src/transport/`
  (`#[server]` path + REST fallback, как у channel/target/module flows);
- расширить `PolicyWorkbench` / `PolicySetCard` (`admin/src/ui/leptos/`) до полного operator flow:
  - rule active toggle,
  - rule reorder (up/down или explicit priority move),
  - rule edit без удаления/пересоздания;
- при появлении отдельного selection state для policy-set/rule держать URL-owned contract
  через `rustok-api` route keys (без package-local state contract).

### 5) Proof points в доменных модулях

- расширять channel-aware proof points (`pages` / `blog` / `commerce`) только вместе
  с explicit tests и локальной документацией;
- для новых channel-aware чтений использовать уже резолвленный host channel context,
  не создавая второй канал выбора в module-local logic.

## Точки интеграции

| Слой | Компонент | Текущая роль | Планируемое изменение |
|---|---|---|---|
| Domain | `crates/rustok-channel/src/services/channel_service.rs` | create/activate/delete policy lifecycle | update/reorder/disable lifecycle + invariants |
| Domain | `crates/rustok-channel/src/resolution.rs` | execution pipeline и trace | подтвердить deterministic policy order после reorder |
| Host REST | `apps/server/src/controllers/channel.rs` | thin channel bootstrap/write API | новые policy update/reorder endpoints |
| Host middleware | `apps/server/src/middleware/channel.rs` | request -> `RequestFacts` -> `ChannelContext` | locale/oauth facts parity с runtime extensions |
| Host composition | `apps/server/src/services/app_router.rs` | middleware chaining | при необходимости корректировка порядка middleware |
| Admin transport | `crates/rustok-channel/admin/src/transport/` | facade + explicit native server-function adapter + REST fallback adapter после FFA split | добавить быстрый boundary verifier для отсутствия raw transport/API calls в UI |
| Admin UI | `crates/rustok-channel/admin/src/ui/leptos/` | явный каталог Leptos render adapter после FFA split | держать full operator flow за core/transport boundaries |
| Shared UI routing | `crates/rustok-api/src/route_selection.rs` | channel query keys (`channel_id/target_id/module_slug/oauth_app_id`) + policy edit keys (`policy_set_id/policy_rule_id`) | поддерживать URL-owned selection contract и dependency cleanup (`policy_set_id -> policy_rule_id`) |

## Этапы

### 1. Contract stability

- [x] зафиксировать финальную resolution-модель `explicit selectors -> built-in target slice -> typed policies -> explicit default -> unresolved`;
- [x] удерживать domain-owned resolver внутри `rustok-channel`;
- [x] удерживать sync между runtime contract, admin UI и server middleware tests.

### 2. Policy lifecycle parity

- [x] довести policy trace в admin bootstrap/runtime diagnostics;
- [x] добавить базовые operator flows для policy-set activation и policy-rule authoring/removal;
- [x] добавить policy rule update/reorder/disable lifecycle на уровне `ChannelService`, REST transport и admin UI controls;
- [x] добавить targeted tests на deterministic rule order и inactive-rule exclusion;
- [x] решить, остаётся ли built-in host slice отдельным fast-path после полного policy rollout.

### 3. Admin operator UX parity

- [x] довести `rustok-channel-admin` до operator flow для policy rules (reorder/disable);
- [x] добавить полноценный rule edit flow (изменение predicates/action без delete+recreate);
- [x] выровнять native-first `#[server]` transport для policy operations с существующими channel CRUD flows;
- [x] при добавлении policy edit-selection state закрепить URL query contract через shared `AdminQueryKey`.

### 4. Runtime integration rollout

- [x] подключить real request locale и OAuth app id в `RequestFacts`;
- [x] закрепить middleware ordering и source-level runtime facts/policy parity тестами в `apps/server`;
- [x] принять решение по built-in host slice (`fast-path` vs policy-only mode): оставить built-in host fast-path между explicit selectors и typed policies, зафиксировать docs/source guardrail `verify:channel:resolution-contract`.

### 5. Semantic expansion

- [ ] возвращаться к richer target/connector taxonomy только при реальном runtime pressure;
- [x] закрепить текущие channel-aware proof points (`rustok-pages`, `rustok-blog`, `rustok-commerce`, `rustok-forum`) no-compile verifier-ом `npm run verify:channel:proof-points` вместе с локальной документацией и test markers.
- [ ] расширять новые channel-aware proof points в доменных модулях только вместе с локальной документацией и tests.

## Проверка

- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- targeted server middleware tests для resolution order, explicit selectors, policy predicates и default semantics
- `npm run verify:channel:resolution-contract`
- `npm run verify:channel:proof-points`
- targeted channel service tests для policy lifecycle (`create/update/reorder/disable/delete`)

## Правила обновления

1. При изменении resolution/policy contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata и UI wiring синхронизировать `rustok-module.toml`.
4. При изменении route-selection contract синхронизировать `rustok-api` (`AdminQueryKey`) и UI docs.


## Quality backlog

- [x] Актуализировать source-level proof-point coverage по текущим channel-aware сценариям pages/blog/commerce/forum через `npm run verify:channel:proof-points`.
- [x] Проверить полноту и актуальность `README.md` и локальных docs для текущих proof-point guardrails.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля: `npm run verify:channel:fba` теперь проверяет static matrix и no-compile executable runtime fallback smoke без компиляции.
- [ ] Собрать full Rust runtime fallback evidence для повышения FBA выше `in_progress`.

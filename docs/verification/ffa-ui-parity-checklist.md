# FFA UI Migration: parity checklist (Phase A baseline)

Этот документ фиксирует обязательный baseline checklist для задач миграции по
плану `docs/research/dioxus-ffa-ui-migration-plan.md`.

## Назначение

Checklist используется как evidence для phase-gates `A -> B`, `B -> C`, `D -> E`
и для контроля того, что dual-path контракт (`native #[server]` + GraphQL fallback)
не деградирует во время FFA-декомпозиции.

## Scope

- module-owned UI пакеты `crates/rustok-*/admin` и `crates/rustok-*/storefront`;
- host wiring в `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`;
- verify scripts в `scripts/verify/*` при изменении contract-правил.

## Обязательные проверки на каждую migration-задачу

### 1) Contract parity

- [ ] Native path (Leptos SSR/hydrate) работает для целевого сценария.
- [ ] GraphQL fallback работает для того же сценария.
- [ ] Headless host path (Next/mobile/external) не сломан.
- [ ] GraphQL/REST surface не удалён и не ослаблен.

### 2) FFA layering

Целевой структурный shape фиксируется одним из значений:

- `none` — кодовый FFA split ещё не начат;
- `docs_boundary` — синхронизирован boundary/docs track, но UI split ещё не начат;
- `core_only` — framework-agnostic `core.rs` или `core/` уже владеет view-model/request/policy фрагментом;
- `core_transport` — добавлен module-owned `transport/` facade/adapters;
- `core_transport_ui` — есть `core`, `transport` и явный `ui/leptos.rs` или `ui/leptos/` adapter.

`core.rs` разрешён для небольшого среза; при появлении нескольких поддоменов (`view_model`, `policy`, `error`, `ports`, `identifiers`) модуль должен переходить на `core/`. Аналогично `ui/leptos.rs` разрешён для одного render adapter file, а `ui/leptos/` используется при разрастании adapter слоя.

- [ ] UI слой не владеет transport/business логикой.
- [ ] UI adapter обращается к transport только через module-owned facade; request/command/state construction и business/policy остаются в core ports/helpers.
- [ ] Core слой не зависит от `leptos*`.
- [ ] Transport adapters разделены по ролям: native и GraphQL fallback либо явно зафиксирован temporary single-adapter state с next-step parity plan.
- [ ] Host-visible UI status/error contracts имеют stable machine-readable codes и documented locale keys.

### 3) i18n/tenant/request context

- [ ] Используется host-provided effective locale, без package-local fallback chains.
- [ ] `RequestMeta`/tenant scope не теряется между native и GraphQL path.
- [ ] Route/query contract не расходится между Leptos и headless hosts.

### 4) Tests & verification evidence

- [ ] Выполнен `cargo xtask module validate <slug>`.
- [ ] Выполнен `cargo xtask module test <slug>`.
- [ ] При изменении host/UI wiring выполнены:
  - [ ] `npm run verify:i18n:ui`
  - [ ] `npm run verify:i18n:contract`
  - [ ] `npm.cmd run verify:storefront:routes`
- [ ] Выполнен `npm run verify:ffa:ui:migration`.
- [ ] Для изменённых error/status контрактов приложен список stable codes и locale keys.
- [ ] В PR приложен фактический вывод проверок.

### 5) Documentation double-check

- [ ] Обновлены локальные docs затронутых модулей.
- [ ] Обновлены central docs в `docs/`.
- [ ] Обновлён `docs/index.md`, если добавлен/изменён doc-узел.
- [ ] Выполнен проход №1: код и формулировки совпадают.
- [ ] Выполнен проход №2: удалены/исправлены устаревшие transport-формулировки.

## Evidence template (вставка в PR)

```md
### FFA parity evidence
- Module: <slug>
- Task slice: <one-task-per-iteration description>
- Native path: PASS/FAIL
- GraphQL fallback: PASS/FAIL
- Headless path: PASS/FAIL
- Structural shape: none/docs_boundary/core_only/core_transport/core_transport_ui
- Contract guard (GraphQL/REST retained): PASS/FAIL
- Docs double-check: PASS/FAIL
- Error/status contract (if changed): `<code>` -> `<locale key>`

Commands:
- cargo xtask module validate <slug>
- cargo xtask module test <slug>
- npm run verify:i18n:ui
- npm run verify:i18n:contract
- npm.cmd run verify:storefront:routes
- npm run verify:ffa:ui:migration
```

## Текущие evidence notes

- 2026-06-13, `region`, slices #33-#34: admin open-detail success/error outcome mapping moved into Leptos-free `RegionAdminOpenDetailViewModel`, and save-success selected/form/refresh/route-update outcome mapping moved into Leptos-free `RegionAdminSaveSuccessViewModel`; `node scripts/verify/verify-region-admin-boundary.mjs` passed; native/GraphQL transport surfaces were not changed.
- 2026-06-13, `blog`, slices #78-#80: admin editor form-state mapping/reset defaults moved into Leptos-free `BlogPostEditorFormState`, admin table-row display/action state moved into Leptos-free `BlogPostAdminTableRowViewModel`, and archive/delete row action presentation completed inside the same core view-model; `node scripts/verify/verify-blog-admin-boundary.mjs` passed for both slices; long `cargo test -p rustok-blog-admin --lib` was stopped during slice #78 after dependency compilation started to avoid long compile; targeted `timeout 20s cargo test -p rustok-blog-admin --lib table_row_view_model_composes_row_policy_without_ui_runtime` reached the timeout during dependency compilation, so no long compile was allowed; native/GraphQL transport surfaces were not changed.
- 2026-06-13, `blog`, slices #81-#82: admin write-path issue banner presentation moved into Leptos-free `BlogPostAdminIssueBannerViewModel`, then publish/unpublish, archive and delete action command preparation moved into Leptos-free `BlogPostStatusCommand`, `BlogPostArchiveCommand` and `BlogPostDeleteCommand`; `node scripts/verify/verify-blog-admin-boundary.mjs` passed after extending the guardrail for the new command helpers; Cargo compilation was intentionally not run by request; native/GraphQL transport surfaces were not changed.

## Связанные документы

- `docs/research/dioxus-ffa-ui-migration-plan.md`
- `docs/UI/graphql-architecture.md`
- `docs/UI/storefront.md`

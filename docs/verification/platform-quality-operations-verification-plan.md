# План верификации платформы: качество, эксплуатация и release-readiness

- **Статус:** Актуализированный детальный чеклист
- **Контур:** Тесты, observability, документация, CI/CD, безопасность, качество кода, correctness

---

## Фаза 14: Тестовое покрытие

### 14.1 Workspace test strategy

- [ ] `cargo nextest run --workspace --all-targets --all-features` проходит или known-failures явно задокументированы.
- [ ] `cargo test --workspace --lib` проходит для library-level regression baseline.
- [ ] `cargo test --workspace --doc --all-features` проходит для doc-test baseline.
- [ ] Нет флейковых тестов, зависящих от порядка выполнения.

### 14.2 Server integration tests

**Путь:** `apps/server/tests/`

- [ ] `integration/content_flow_test.rs`
- [ ] `integration/event_flow_test.rs`
- [ ] `integration/order_flow_test.rs`
- [ ] `library_api_smoke.rs`
- [ ] `module_lifecycle.rs`
- [ ] `multi_tenant_isolation_test.rs`
- [ ] `tenant_cache_stampede_test.rs`

### 14.3 Crate integration / contract tests

- [ ] `rustok-core/tests/*`
- [ ] `rustok-events/tests/*`
- [ ] `rustok-content/tests/*`
- [ ] `rustok-commerce/tests/*`
- [ ] `rustok-blog/tests/*`
- [ ] `rustok-forum/tests/*`
- [ ] `rustok-pages/tests/*`
- [ ] `rustok-index/tests/*`
- [ ] `rustok-rbac/tests/*`
- [ ] `rustok-tenant/tests/*`
- [ ] `rustok-outbox/tests/*`
- [ ] `rustok-telemetry/tests/*`
- [ ] `rustok-iggy/tests/*`
- [ ] `rustok-iggy-connector/tests/*`
- [ ] `rustok-mcp/tests/*`
- [ ] `rustok-test-utils/tests/*`

### 14.4 Property, security и benchmark проверки

- [ ] Property-based tests по state-machine/validation сценариям остаются актуальными.
- [ ] Security regression tests соответствуют текущему auth/RBAC/input-validation contract.
- [ ] Bench suite в `benches/` по-прежнему собирается и соответствует текущим hot paths.

---

## Фаза 15: Observability и операционная готовность

### 15.1 Metrics, health, tracing

- [ ] `/metrics` отражает текущий набор Prometheus metrics.
- [ ] `/health`, `/health/live`, `/health/ready`, `/health/runtime`, `/health/modules` отражают текущий health contract.
- [ ] Tracing / OTEL wiring соответствует текущему server bootstrap.
- [ ] GraphQL observability extension и build progress subscription не расходятся с runtime.

### 15.2 Grafana / Prometheus / Compose

- [ ] Datasource config в `grafana/` соответствует текущему стеку.
- [ ] Dashboards не ссылаются на несуществующие datasource/uid.
- [ ] `docker-compose.yml`, `docker-compose.full-dev.yml`, `docker-compose.observability.yml` отражают текущий dev/runtime stack.

### 15.3 Runtime operational paths

- [ ] Outbox backlog / retries / failed events отражены как operational concern.
- [ ] MCP / OAuth / build pipeline operational endpoints отражены в runbooks и verification notes.
- [ ] Cache invalidation, Redis optionality и degraded-mode scenarios не расходятся с кодом.

---

## Фаза 16: Синхронизация документации с кодом

### 16.1 `docs/architecture/`

- [ ] `overview.md`
- [ ] `api.md`
- [ ] `database.md`
- [ ] `dataloader.md`
- [ ] `diagram.md`
- [ ] `event-flow-contract.md`
- [ ] `channels.md`
- [ ] `i18n.md`
- [ ] `matryoshka.md`
- [ ] `modules.md`
- [ ] `performance-baseline.md`
- [ ] `principles.md`
- [ ] `routing.md`

### 16.2 `docs/guides/`

- [ ] `quickstart.md`
- [ ] `observability-quickstart.md`
- [ ] `testing.md`
- [ ] `testing-integration.md`
- [ ] `testing-property.md`
- [ ] `testing-oauth2-rfc.md`
- [ ] `error-handling.md`
- [ ] `input-validation.md`
- [ ] `rate-limiting.md`
- [ ] `security-audit.md`
- [ ] `module-metrics.md`
- [ ] `metrics.md`
- [ ] `runtime-guardrails.md`
- [ ] `tracing.md`
- [ ] `scheduler.md`

### 16.3 `docs/modules/` и централизованные реестры

- [ ] `registry.md`
- [ ] `crates-registry.md`
- [ ] `manifest.md`
- [ ] `_index.md`
- [ ] module-system / UI package документы не расходятся с текущим кодом

### 16.4 Локальная документация crate-ов и приложений

- [ ] Shared library crates: `rustok-core`, `rustok-events`, `rustok-api`, `rustok-storage`, `rustok-test-utils`
- [ ] Core modules: `rustok-auth`, `rustok-cache`, `rustok-email`, `rustok-index`, `rustok-outbox`, `rustok-tenant`, `rustok-rbac`
- [ ] Optional modules: `rustok-content`, `rustok-commerce`, `rustok-blog`, `rustok-forum`, `rustok-pages`, `rustok-media`, `rustok-workflow`
- [ ] Capability / support crates: `rustok-telemetry`, `rustok-iggy`, `rustok-iggy-connector`, `alloy`, `alloy-scripting`, `rustok-mcp`, `flex`
- [ ] Apps: `apps/server`, `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`

### 16.5 Root и verification docs

- [ ] `README.md`
- [ ] `PLATFORM_INFO_RU.md`
- [ ] `RUSTOK_MANIFEST.md`
- [ ] `AGENTS.md`
- [ ] `CONTRIBUTING.md`
- [ ] `CHANGELOG.md`
- [ ] `docs/index.md`
- [ ] `docs/verification/README.md`
- [ ] master/detailed verification-планы согласованы между собой

### 16.6 ADR

- [ ] Все решения в `DECISIONS/` согласованы с текущим кодом и статусами.
- [ ] ADR по events / module bundles / auth lifecycle / Alloy / MCP / shared API layer остаются релевантными.

---

## Фаза 17: CI/CD и DevOps

### 17.1 GitHub workflows

- [ ] `.github/workflows/ci.yml` отражает текущие обязательные проверки.
- [ ] `.github/workflows/dependencies.yml` не дрейфует от dependency-management стратегии.
- [ ] `cargo-nextest` используется как основной Rust test runner в CI, а doc-tests остаются отдельной проверкой.
- [ ] `cargo-machete` подключён как manifest-hygiene сигнал и не конфликтует с `cargo udeps`.
- [ ] Verification docs не описывают шаги, которых больше нет в CI.

### 17.2 Verify scripts и operational automation

- [ ] `scripts/verify/README.md` соответствует текущему набору verification-планов.
- [ ] Скрипты в `scripts/verify/` отражают актуальные checks и не ссылаются на удалённые документы/команды.
- [ ] Build/release helper scripts согласованы с current manifest/build flow.

### 17.3 План внедрения tooling-пакета качества

- [x] Установить локально инструменты текущего этапа: `cargo-nextest` и `cargo-machete`.
- [x] Использовать `cargo-nextest` как основной Rust test runner в CI и в локальном базовом workflow.
- [x] Оставить `cargo test --workspace --doc --all-features` как отдельный doc-test baseline, а не пытаться скрыто заменить его.
- [x] Подключить `cargo-machete` как advisory manifest-hygiene check без замены `cargo udeps`.
- [ ] Следующий этап внедрения: `Semgrep`, `GraphQL Inspector`, `knip`.
- [ ] Следующий после него этап: `Playwright` + `axe-core`, `GraphQL Code Generator`.
- [ ] Отложенный этап: `testcontainers`, `cargo-mutants`, `Stryker`.
- [ ] Инструменты локальной диагностики и performance-анализа держать вне blocking CI gate: `tokio-console`, `cargo-flamegraph`.
- [ ] Точечные deep-check инструменты оставлять на поздний адресный rollout для критичных модулей: `loom`, `miri`, `cargo-fuzz`.
- [ ] Не заменять на этом этапе существующие `cargo audit`, `cargo deny`, `cargo udeps`, `cargo llvm-cov`, `scripts/verify/*` и `Makefile`; новые инструменты должны усиливать текущий baseline, а не дублировать его без доказанной пользы.

---

## Фаза 18: Безопасность

### 18.1 Auth и session security

- [ ] Rate limiting на auth endpoints соответствует текущему middleware.
- [ ] Session revocation, password reset, verification и invite flows соответствуют текущему коду.
- [ ] OAuth browser/session flow отражён корректно.

### 18.2 Authorization security

- [ ] RBAC coverage соответствует текущим REST/GraphQL/capability surfaces.
- [ ] Нет role-based bypass вместо typed permissions.
- [ ] Flex / MCP / scripts / workflows не обходят общий authorization model.

### 18.3 Tenant boundary security

- [ ] Tenant isolation в data access, cache и module lifecycle соответствует текущему коду.
- [ ] Нет public/tenant-crossing endpoints, не отражённых в документации.

### 18.4 Input, dependency и secret hygiene

- [ ] Validation flows соответствуют текущим DTO/services.
- [ ] Secret handling и env contract отражены честно.
- [ ] Dependency risk checks соответствуют текущему toolchain/CI practice.

---

## Фаза 19: Антипаттерны и качество кода

### 19.1 Authorization / module / event antipatterns

- [ ] Нет hardcoded role checks в server authorization path.
- [ ] Нет publish-after-commit в доменных write-path.
- [ ] Runtime modules публикуют permissions/dependencies через module contracts.
- [ ] Host apps не содержат долговременные bypass-реализации вместо library/module contracts.

### 19.2 API / DTO / service quality

- [ ] DTO validation и error mapping соответствуют current service layer.
- [ ] GraphQL/REST parity не задокументирована там, где её фактически нет.
- [ ] Capability crates не смешаны с platform modules в документации и коде.

### 19.3 Frontend / library quality

- [ ] Module-owned UI surfaces не дублируются бессистемно в host apps.
- [ ] Leptos/Next/shared packages документированы по фактическому public contract.

---

## Фаза 20: Правильность кода и correctness

### 20.1 Type / serialization / migration correctness

- [ ] Type model и permission vocabulary соответствуют текущему коду.
- [ ] Serialization contracts для REST/GraphQL/OAuth/MCP не расходятся с текущими DTOs.
- [ ] Миграции соответствуют текущим entity/service ожиданиям.

### 20.2 Concurrency / retry / runtime correctness

- [ ] Retry/backoff logic соответствует текущим async runtime patterns.
- [ ] Event consumers и schedulers не содержат очевидных busy-loop / lag-handling regressions.
- [ ] Workflow, outbox, cache invalidation и build progress flows не расходятся с runtime behavior.

### 20.3 Boundary correctness

- [ ] Module registry, manifest, host apps, shared libraries и capability layers согласованы как единая система.
- [ ] Alloy/MCP/workflow/flex boundaries задокументированы и реализованы без архитектурной путаницы.

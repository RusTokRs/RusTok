# План верификации платформы: качество и эксплуатационная готовность

- **Статус:** актуальный детальный чеклист
- **Контур:** quality-gates, observability, release-readiness, security/dependency hygiene, синхронизация документации с кодом
- **Companion-план:** [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md)

---

## Актуальный контракт качества и эксплуатации

Этот план нужен не для исторического журнала CI-инцидентов, а для подтверждения
того, что локальный и CI-путь проверки качества отражают текущий код платформы.

Проверка охватывает:

- локальные проверки качества, которые должны воспроизводиться без GitHub как источника истины
- observability и operational surfaces, которые должны соответствовать runtime contract
- release-readiness и security/dependency hygiene
- синхронизацию central docs, local docs и verification entrypoints

## Фаза 1. Локальный baseline качества

### 1.1 Rust workspace baseline

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] targeted `cargo test`, если менялся runtime contract, DTO, migration path или shared service layer

### 1.2 Module baseline

- [ ] `cargo xtask validate-manifest`
- [ ] targeted `cargo xtask module validate <slug>` для затронутых модулей
- [ ] targeted `cargo xtask module test <slug>` для затронутых модулей
- [ ] Support/capability crates не подменяют scoped module checks.

### 1.3 Frontend/i18n baseline

- [ ] `npm run verify:i18n:ui`
- [ ] `npm run verify:i18n:contract`
- [ ] `npm run verify:storefront:routes`, если менялись storefront routes, locale-prefixed paths или host/UI wiring

## Фаза 2. Observability и operational readiness

### 2.1 Runtime observability contract

- [ ] `/metrics`, `/health`, `/health/live`, `/health/ready`, `/health/runtime`, `/health/modules` соответствуют текущему host/runtime contract.
- [ ] Tracing, OTEL и логирование не расходятся с server bootstrap и operational docs.
- [ ] Build progress, background tasks, workflow/outbox/cache operational flows не теряют observability contract при изменениях runtime.

### 2.2 Local operational tooling

- [ ] `scripts/verify/*` и `scripts/architecture_dependency_guard.py` отражают текущий код и активные boundary rules.
- [ ] Для Windows локальный обязательный path остаётся выполнимым без Bash как hard prerequisite.
- [ ] Legacy shell checks, если они нужны, документированы как perimeter path, а не как единственный способ подтвердить contract.

### 2.3 Compose и local stack readiness

- [ ] `docker-compose*.yml`, `grafana/`, `prometheus/` и связанные runbooks не расходятся с текущей dev/runtime картиной.
- [ ] Observability stack описывает реальный локальный контур, а не исторический rollout.

## Фаза 3. Security и dependency hygiene

### 3.1 Security baseline

- [ ] Auth/session/RBAC verification notes совпадают с текущим server/runtime contract.
- [ ] Tenant isolation, input validation и secret handling не расходятся с central docs и local docs.
- [ ] Capability crates и automation paths не обходят общий authorization model.

### 3.2 Dependency and manifest hygiene

- [ ] `cargo deny`, `cargo audit`, `cargo machete` и аналогичные quality tools трактуются как сигналы качества, согласованные с текущим workflow.
- [ ] Manifest hygiene не дублирует scoped module contract и не конфликтует с `cargo xtask validate-manifest`.
- [ ] Support tooling не документируется как обязательный gate, если он не воспроизводим в текущем локальном baseline.

## Фаза 4. Documentation sync и release-readiness

### 4.1 Central docs

- [ ] `docs/index.md`, `docs/verification/README.md`, `docs/modules/*`, `docs/architecture/*`, `docs/UI/*` отражают текущий код и навигацию.
- [ ] Verification plans остаются checklist-слоем, а не архивом прошлых падений CI.
- [ ] Старые rollout/install/investigation notes либо обновлены, либо явно вытеснены актуальными live docs.

### 4.2 Local docs

- [ ] Изменённые `apps/*` и `crates/*` синхронизируют root `README.md`, `docs/README.md` и `docs/implementation-plan.md`.
- [ ] Public contracts в `README.md` остаются на английском, central docs в `docs/` остаются на русском.
- [ ] Документация описывает реальный source of truth, а не временные workaround-ы.

### 4.3 Release-readiness

- [ ] Локальный baseline качества воспроизводим до публикации изменений.
- [ ] Environment blockers фиксируются отдельно и не маскируют code/docs drift.
- [ ] Release/readiness заметки не живут только в CI; критичные ограничения отражены в локальных docs и runbooks.

## Точечные локальные проверки

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] `cargo xtask validate-manifest`
- [ ] targeted `cargo xtask module validate <slug>`
- [ ] targeted `cargo xtask module test <slug>`
- [ ] `npm run verify:i18n:ui`
- [ ] `npm run verify:i18n:contract`
- [ ] `npm run verify:storefront:routes`, если затронут storefront/runtime routing contract
- [ ] `powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1`, если менялся architecture/runtime boundary

## Open blockers

- [ ] Не превращать этот документ в список разовых CI-ошибок или GitHub-specific workaround-ов.
- [ ] Локальные prerequisites и environment blockers фиксировать кратко и отдельно от contract-layer.
- [ ] Любой новый quality gate сначала описывать как локально воспроизводимый workflow, а уже потом как CI integration.

## Связанные документы

- [Главный README по верификации](./README.md)
- [Foundation verification](./platform-foundation-verification-plan.md)
- [Performance baseline](../architecture/performance-baseline.md)
- [Runtime guardrails](../guides/runtime-guardrails.md)
- [Контракт `rustok-module.toml`](../modules/manifest.md)

# Historical input: deep research report (control plane и module lifecycle)

Этот файл сохранён как historical input после перепроверки.

Актуальный рабочий backlog и статусы реализации ведутся в документе
[План устранения недостатков control plane и module lifecycle](./control-plane-module-lifecycle-remediation-plan.md).

Ниже оставлен сжатый срез исходных проверок и направлений работ, чтобы не терять контекст. Полный
исходный текст research-отчёта был ранее сжат для рабочей эксплуатации.

> Перепроверено 2026-05-18: часть пунктов уже закрыта в текущем коде. Актуальный
> remediation plan ведётся в [плане устранения недостатков control plane и module lifecycle](./control-plane-module-lifecycle-remediation-plan.md).

## Подтверждено по текущему коду

- Runtime/admin paths редактировали локальный `modules.toml` при install/uninstall/upgrade,
  а production `apps/server/Dockerfile` не копирует этот файл в runtime image.
- Build enqueue делал `load -> mutate -> save -> request_build` и blind rollback без revision/CAS.
- `settings.default_enabled` расходился с `tenant_modules`, seed/installer defaults и admin hardcoded lists.
- Module enable/disable имел прямые bypass paths через `tenant_modules::toggle` /
  `TenantService::toggle_module` и не фиксировал lifecycle operation journal.
- `ModuleLifecycleService` вызывал hooks после записи состояния и при ошибке откатывал только флаг.
- Server migrator использовал lexical ordering с ad-hoc special-case вместо dependency-aware order.
- `m20260405_000001_expand_locale_storage_columns::down()` сужал locale columns обратно до `VARCHAR(5)`.
- CI не запускал `cargo xtask validate-manifest` / `cargo xtask module validate`, а coverage не был
  публикуемым threshold gate.
- `.github/dependabot.yml` ссылался на несуществующий `/apps/mcp`.

## Устарело после перепроверки

- Утверждение “нет repository-level license policy” устарело: `deny.toml` уже есть, а CI запускает
  `cargo-deny-action`. Требуется не новая policy, а сохранение/усиление CI-сигнала и SBOM/provenance.

## Исходные направления исправлений (historical)

1. Ввести DB-backed `platform_state` с `revision`, `manifest_json`, `manifest_hash` и
   `active_release_id`; `modules.toml` оставить dev/bootstrap input, но не runtime-write target.
2. Расширить `builds`/`releases` immutable `manifest_revision` и `manifest_snapshot`.
3. Перевести GraphQL и Leptos `#[server]` module install/uninstall/upgrade на revision-aware
   `platform_state`; stale writes должны получать conflict-style ошибку без blind rollback.
4. Ввести `EffectiveModulePolicyService`: `core + manifest.default_enabled + tenant overrides`.
5. Перевести module enable/disable на `ModuleLifecycleService` + `module_operations` journal.
6. Сделать locale widening rollback non-destructive.
7. Заменить migration special-case на dependency-aware ordering.
8. Добавить CI gates: manifest validation, module validation, coverage artifact + threshold,
   SBOM/provenance, и удалить stale Dependabot path.
9. API compatibility policy держать в `docs/architecture/api.md`; RLS включать staged pilot после
   появления tenant DB session context, без broad big-bang миграции.

## Ссылки

- [Карта документации](../index.md)
- [Контракт `modules.toml`](../modules/manifest.md)
- [API и surface-контракты](../architecture/api.md)
- [База данных](../architecture/database.md)
- [Инструмент workspace CLI `xtask`](../../xtask/README.md)
